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

use crate::input::{KeyState, KEYS_DEFAULT};
use crate::player::PlayerState;

use cgmath::prelude::*;

use cgmath::{Deg, Rad, Vector2};

const CJ_START_X: f32 = -160.0;
const CJ_ANGLE: Deg<f32> = Deg(150.0);

const MAX_TURN_RATE: Deg<f32> = Deg(250.0);
const START_DELAY_S: f32 = 1.0;

enum StrafeBotState {
    Setup(f32),
    Takeoff(Deg<f32>),
    Flight(bool, bool),
}

#[derive(Clone, Default, Eq, PartialEq)]
pub struct StrafeConfig {
    keys_cw: Option<KeyState>,
    keys_ccw: Option<KeyState>,
}

impl StrafeConfig {
    const KEYS_A: KeyState = KeyState {
        key_a: true,
        ..KEYS_DEFAULT
    };

    const KEYS_D: KeyState = KeyState {
        key_d: true,
        ..KEYS_DEFAULT
    };

    const KEYS_SA: KeyState = KeyState {
        key_s: true,
        key_a: true,
        ..KEYS_DEFAULT
    };

    const KEYS_SD: KeyState = KeyState {
        key_s: true,
        key_d: true,
        ..KEYS_DEFAULT
    };

    const KEYS_WA: KeyState = KeyState {
        key_w: true,
        key_a: true,
        ..KEYS_DEFAULT
    };

    const KEYS_WD: KeyState = KeyState {
        key_w: true,
        key_d: true,
        ..KEYS_DEFAULT
    };

    pub const PLAYER_KEYS: Self = Self{
        keys_cw : None,
        keys_ccw: None,
    };

    pub const STANDARD: Self = Self{
        keys_cw : Some(Self::KEYS_WD),
        keys_ccw: Some(Self::KEYS_WA),
    };

    pub const REVERSE: Self = Self{
        keys_cw : Some(Self::KEYS_SA),
        keys_ccw: Some(Self::KEYS_SD),
    };

    pub const HALF_BEAT_LEFT: Self = Self{
        keys_cw : Some(Self::KEYS_D),
        keys_ccw: Some(Self::KEYS_WA),
    };

    pub const HALF_BEAT_RIGHT: Self = Self{
        keys_cw : Some(Self::KEYS_WD),
        keys_ccw: Some(Self::KEYS_A),
    };

    pub const HIGH_SPEED: Self = Self{
        keys_cw : Some(Self::KEYS_D),
        keys_ccw: Some(Self::KEYS_A),
    };

    pub const SIDEWAYS_LEFT: Self = Self{
        keys_cw : Some(Self::KEYS_WA),
        keys_ccw: Some(Self::KEYS_SA),
    };

    pub const SIDEWAYS_RIGHT: Self = Self{
        keys_cw : Some(Self::KEYS_SD),
        keys_ccw: Some(Self::KEYS_WD),
    };
}

pub struct StrafeBot {
    state: StrafeBotState,
    pub config: StrafeConfig,
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
    pub fn new(config: StrafeConfig) -> Self {
        Self{
            state: StrafeBotState::Setup(0.0),
            config,
        }
    }

    pub fn is_setting_up(&self) -> bool {
        if let StrafeBotState::Setup(..) = self.state {
            true
        } else {
            false
        }
    }

    fn strafe_turning(dt: f32,
        move_dir: Vector2<f32>,
        wish_dir: Vector2<f32>,
        warp_factor: f32,
        turn_rate: Rad<f32>,
        is_clockwise: bool,
    )
        -> Rad<f32>
    {
        if warp_factor > 1.0 {
            if wish_dir.magnitude2() > 0.5 {
                let move_angle = Vector2::unit_y().angle(move_dir);
                let wish_angle = Vector2::unit_y().angle(wish_dir);
                let optimal_angle = Rad((1.0 / warp_factor).acos());
                let mut turn_angle = optimal_angle + turn_rate * dt;
                if is_clockwise {
                    turn_angle = -turn_angle;
                }
                move_angle + turn_angle - wish_angle
            } else {
                Rad::zero()
            }
        } else {
            Rad::zero()
        }
    }

    pub fn sim(&mut self, dt: f32,
        player: &PlayerState,
        keys: KeyState,
        speed_limit: f32,
        add_yaw: Rad<f32>,
        add_pitch: Rad<f32>,
    )
        -> (KeyState, Rad<f32>, Rad<f32>)
    {
        let speed = player.vel.xy().magnitude();
        let yaw   = player.dir.0 + add_yaw;
        let pitch = player.dir.1 + add_pitch;
        let max_turn: Rad<f32> = (MAX_TURN_RATE * dt).into();
        let (out_keys, turn_yaw) = loop { match &mut self.state {
            StrafeBotState::Setup(duration) => {
                let target_angle = -CJ_ANGLE;
                let move_x = CJ_START_X - player.pos.x;
                if move_x.abs() < 10.0 && speed < 10.0 {
                    *duration += dt;
                    if *duration > START_DELAY_S {
                        self.state = StrafeBotState::Takeoff(Deg::zero());
                        continue;
                    } else {
                        break (KeyState::default(), Rad::zero());
                    }
                } else {
                    *duration = 0.0;
                }
                let move_angle: Rad<f32> = if move_x > 0.0 {
                    Rad::zero()
                } else {
                    Rad::turn_div_2()
                };
                let (ny, nx) = (move_angle - yaw).sin_cos();
                const MOVE_THRESHOLD: f32 = 0.383;
                let out_keys = KeyState{
                    key_w: ny >  MOVE_THRESHOLD,
                    key_a: nx < -MOVE_THRESHOLD,
                    key_s: ny < -MOVE_THRESHOLD,
                    key_d: nx >  MOVE_THRESHOLD,
                    ..Default::default()
                };
                break (out_keys, Into::<Rad<_>>::into(target_angle) - yaw);
            },
            StrafeBotState::Takeoff(turned) => {
                if *turned >= CJ_ANGLE || speed > 410.0 {
                    self.state = StrafeBotState::Flight(false, false);
                    continue;
                }
                let cj_started = speed > 0.99 * speed_limit;
                let out_keys = KeyState{
                    key_w: true,
                    key_a: cj_started,
                    ..Default::default()
                };
                let turn_angle = Self::strafe_turning(dt,
                    player.vel.xy(),
                    player.wish_dir(out_keys, add_yaw, add_pitch).xy(),
                    speed / speed_limit,
                    Rad(10.0),
                    false);
                *turned += clamp_angle(turn_angle, max_turn).into();
                break (out_keys, turn_angle);
            }
            StrafeBotState::Flight(jumped, is_clockwise) => {
                if speed < 1.1 * speed_limit {
                    self.state = StrafeBotState::Setup(0.0);
                    continue;
                }
                let is_grounded = player.is_grounded();
                if is_grounded {
                    if !*jumped {
                        *jumped = true;
                        if player.pos.x < -512.0 {
                            *is_clockwise = true;
                        } else if player.pos.x > 512.0 {
                            *is_clockwise = false;
                        } else if player.vel.x < -80.0 {
                            *is_clockwise = true;
                        } else if player.vel.x > 80.0 {
                            *is_clockwise = false;
                        }
                    }
                } else {
                    *jumped = false;
                }
                let out_keys = KeyState{
                    space: is_grounded,
                    ..Default::default()
                } | (if *is_clockwise {
                    self.config.keys_cw
                } else {
                    self.config.keys_ccw
                }).unwrap_or(keys);
                let turn_angle = Self::strafe_turning(dt,
                    player.vel.xy(),
                    player.wish_dir(out_keys, add_yaw, add_pitch).xy(),
                    speed / speed_limit,
                    Rad(2.0),
                    *is_clockwise);
                break (out_keys, turn_angle);
            }
        }};
        let turn_pitch = Into::<Rad<_>>::into(Deg(90.0)) - pitch;
        (out_keys,
            clamp_angle(turn_yaw  , max_turn),
            clamp_angle(turn_pitch, max_turn))
    }
}