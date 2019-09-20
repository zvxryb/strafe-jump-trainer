/*
 * Copyright 2019 Michael Lodato <zvxryb@gmail.com>
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

use crate::input::KeyState;

use cgmath::prelude::*;

use cgmath::{Basis2, Deg, Rad, Vector2};

const CJ_START_X: f32 = -160.0;
const CJ_ANGLE: Deg<f32> = Deg(150.0);

const MAX_TURN_RATE: Deg<f32> = Deg(1000.0);

enum StrafeBotState {
    Idle,
    Takeoff(Deg<f32>),
    Flight(bool, bool),
}

pub struct StrafeConfig {
    keys_cw: KeyState,
    keys_ccw: KeyState,
}

impl StrafeConfig {
    fn keys_wa() -> KeyState {
        KeyState{
            key_w: true,
            key_a: true,
            ..Default::default()
        }
    }

    fn keys_wd() -> KeyState {
        KeyState{
            key_w: true,
            key_d: true,
            ..Default::default()
        }
    }

    pub fn full_beat() -> Self {
        Self{
            keys_cw : Self::keys_wd(),
            keys_ccw: Self::keys_wa(),
        }
    }
}

pub struct StrafeBot {
    state: StrafeBotState,
    config: StrafeConfig,
}

fn clamp_angle<T: Angle>(x: T, max: T) -> T {
    let x = x.normalize_signed();
    if x < -max {
        -max
    } else if x > max {
        max
    } else {
        x
    }
}

impl StrafeBot {
    pub fn new() -> Self {
        Self{
            state: StrafeBotState::Idle,
            config: StrafeConfig::full_beat(),
        }
    }

    pub fn take_off(&mut self) {
        self.state = StrafeBotState::Takeoff(Deg(0.0));
    }

    fn strafe_turning(dt: f32,
        move_angle: Rad<f32>,
        input_angle: Rad<f32>,
        warp_factor: f32,
        turn_rate: Rad<f32>,
        is_clockwise: bool,
    )
        -> Rad<f32>
    {
        if warp_factor > 1.0 {
            let optimal_angle = Rad((1.0 / warp_factor).acos());
            let mut turn_angle = optimal_angle + turn_rate * dt;
            if is_clockwise {
                turn_angle = -turn_angle;
            }
            move_angle + turn_angle - input_angle
        } else {
            Rad::zero()
        }
    }

    pub fn sim(&mut self, dt: f32,
        vel: Vector2<f32>,
        speed_limit: f32,
        x: f32,
        theta: Rad<f32>,
        phi: Rad<f32>,
        is_grounded: bool,
    )
        -> (KeyState, Rad<f32>, Rad<f32>)
    {
        let max_turn: Rad<f32> = (MAX_TURN_RATE * dt).into();
        let (keys, turn_theta) = loop { match &mut self.state {
            StrafeBotState::Idle => {
                let target_angle = -CJ_ANGLE;
                let move_x = CJ_START_X - x;
                let should_move = move_x.abs() > 10.0;
                let move_angle: Rad<f32> = if move_x > 0.0 {
                    Rad::zero()
                } else {
                    Rad::turn_div_2()
                };
                let (ny, nx) = (move_angle - theta).sin_cos();
                const MOVE_THRESHOLD: f32 = 0.383;
                let keys = KeyState{
                    key_w: should_move && ny >  MOVE_THRESHOLD,
                    key_a: should_move && nx < -MOVE_THRESHOLD,
                    key_s: should_move && ny < -MOVE_THRESHOLD,
                    key_d: should_move && nx >  MOVE_THRESHOLD,
                    ..Default::default()
                };
                break (keys, Into::<Rad<_>>::into(target_angle) - theta);
            },
            StrafeBotState::Takeoff(turned) => {
                if *turned >= CJ_ANGLE {
                    self.state = StrafeBotState::Flight(false, false);
                    continue;
                }
                let speed = vel.magnitude();
                let cj_started = speed > 0.99 * speed_limit;
                let keys = KeyState{
                    key_w: true,
                    key_a: cj_started,
                    ..Default::default()
                };
                let input_angle: Rad<f32> = (if cj_started {
                    Deg(45.0).into()
                } else {
                    Rad::zero()
                }) + theta;
                let turn_angle = Self::strafe_turning(dt,
                    Vector2::unit_y().angle(vel),
                    input_angle,
                    speed / speed_limit,
                    Rad(10.0),
                    false);
                *turned += clamp_angle(turn_angle, max_turn).into();
                break (keys, turn_angle);
            }
            StrafeBotState::Flight(jumped, is_clockwise) => {
                let speed = vel.magnitude();
                if speed < speed_limit {
                    self.state = StrafeBotState::Idle;
                    continue;
                }
                if is_grounded {
                    if !*jumped {
                        *jumped = true;
                        *is_clockwise = vel.x < 0.0;
                        if x < -512.0 {
                            *is_clockwise = true;
                        } else if x > 512.0 {
                            *is_clockwise = false;
                        }
                    }
                } else {
                    *jumped = false;
                }
                let keys = KeyState{
                    space: is_grounded,
                    ..Default::default()
                } | (if *is_clockwise {
                    self.config.keys_cw
                } else {
                    self.config.keys_ccw
                });
                let input_angle: Rad<f32> = Into::<Rad<_>>::into(
                    if *is_clockwise {
                        Deg(-45.0)
                    } else {
                        Deg(45.0)
                    }) + theta;
                let turn_angle = Self::strafe_turning(dt,
                    Vector2::unit_y().angle(vel),
                    input_angle,
                    speed / speed_limit,
                    Rad(2.0),
                    *is_clockwise);
                break (keys, turn_angle);
            }
        }};
        let turn_phi = Into::<Rad<_>>::into(Deg(90.0)) - phi;
        (keys,
            clamp_angle(turn_theta, max_turn),
            clamp_angle(turn_phi  , max_turn))
    }
}