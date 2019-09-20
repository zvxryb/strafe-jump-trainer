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

use cgmath::prelude::*;

use cgmath::{
    Deg,
    Matrix3,
    Matrix4,
    Point2,
    Point3,
    Rad,
    Vector2,
    Vector3,
};

use crate::input::KeyState;

pub const PLAYER_EYELEVEL: f32 = 64.0;
pub const PLAYER_RADIUS: f32 = 16.0;
pub const JUMP_GROUND_DIST: f32 = 0.25;

#[derive(Copy, Clone)]
pub struct Friction {
    pub stall_speed: f32,
    pub friction: f32,
}

impl Friction {
    fn sim(&self, vel: &mut Vector3<f32>, dt: f32) {
        let speed0 = vel.xy().magnitude();
        if speed0 > 0.0001 {
            let speed1 = (speed0 - speed0.max(self.stall_speed) * self.friction * dt).max(0.0);
            vel.x *= speed1 / speed0;
            vel.y *= speed1 / speed0;
        }
    }
}

#[derive(Copy, Clone)]
pub struct Movement {
    pub max_speed: f32,
    pub accel: f32,
}

impl Movement {
    fn sim(&self, vel: &mut Vector3<f32>, dt: f32, wish_dir: Vector2<f32>) {
        let add_speed = (self.max_speed - vel.xy().dot(wish_dir)).max(0.0);
        let dv = wish_dir.extend(0.0) * (self.accel * dt).min(add_speed);
        *vel += dv;
    }
}

#[derive(Clone)]
pub struct Kinematics {
    pub gravity: f32,
    pub jump_impulse: f32,
    pub friction: Friction,
    pub move_ground: Movement,
    pub move_air: Movement,
    pub move_air_turning: Option<Movement>,
}

impl Kinematics {
    pub fn effective_movement(&self, is_grounded: bool, is_turning: bool) -> Movement {
        if is_grounded {
            self.move_ground
        } else {
            if is_turning {
                if let Some(move_air_turning) = self.move_air_turning {
                    return move_air_turning
                }
            }
            self.move_air
        }
    }

    pub fn sim(&self,
        vel: &mut Vector3<f32>,
        dt: f32,
        wish_dir: Vector2<f32>,
        mut is_grounded: bool,
        is_jumping: bool,
        is_turning: bool)
    {
        if is_grounded && is_jumping {
            vel.z += self.jump_impulse;
            is_grounded = false;
        }

        if is_grounded {
            self.friction.sim(vel, dt);
        }

        self.effective_movement(is_grounded, is_turning).sim(vel, dt, wish_dir);

        vel.z -= self.gravity * dt;
    }
}

pub const MOVE_VQ3_LIKE: Kinematics = Kinematics{
    gravity: 800.0,
    jump_impulse: 270.0,
    friction: Friction{
        stall_speed: 100.0,
        friction: 6.0,
    },
    move_ground: Movement{
        max_speed: 320.0,
        accel: 10.0 * 320.0,
    },
    move_air: Movement{
        max_speed: 320.0,
        accel: 1.0 * 320.0,
    },
    move_air_turning: None,
};

pub const MOVE_QW_LIKE: Kinematics = Kinematics{
    gravity: 800.0,
    jump_impulse: 270.0,
    friction: Friction{
        stall_speed: 100.0,
        friction: 6.0,
    },
    move_ground: Movement{
        max_speed: 320.0,
        accel: 10.0 * 320.0,
    },
    move_air: Movement{
        max_speed: 30.0,
        accel: 10.0 * 320.0,
    },
    move_air_turning: None,
};

pub const MOVE_HYBRID: Kinematics = Kinematics{
    gravity: 800.0,
    jump_impulse: 270.0,
    friction: Friction{
        stall_speed: 100.0,
        friction: 6.0,
    },
    move_ground: Movement{
        max_speed: 320.0,
        accel: 10.0 * 320.0,
    },
    move_air: Movement{
        max_speed: 320.0,
        accel: 1.0 * 320.0,
    },
    move_air_turning: Some(Movement{
        max_speed: 35.0,
        accel: 2100.0,
    }),
};

pub struct PlayerState {
    pub pos: Point3<f32>,
    pub vel: Vector3<f32>,
    pub dir: (Rad<f32>, Rad<f32>),
}

impl Default for PlayerState {
    fn default() -> Self {
        Self{
            pos: Point3::new(0.0, 0.0, 0.0),
            vel: Vector3::new(0.0, 0.0, 0.0),
            dir: (Rad(0.0), Deg(90.0).into()),
        }
    }
}

fn rotation_matrix_2dof(yaw: Rad<f32>, pitch: Rad<f32>) -> Matrix3<f32> {
    let (s0, c0) = yaw  .sin_cos();
    let (s1, c1) = pitch.sin_cos();
    Matrix3::new(
            c0,     s0, 0.0,
        -s0*c1,  c0*c1,  s1,
         s0*s1, -c0*s1,  c1)
}

impl PlayerState {
    fn rotation_matrix(&self, add_yaw: Rad<f32>, add_pitch: Rad<f32>) -> Matrix3<f32> {
        let yaw   = (self.dir.0 + add_yaw).normalize();
        let mut pitch = self.dir.1 + add_pitch;
        if pitch < Rad::zero      () { pitch = Rad::zero      (); }
        if pitch > Rad::turn_div_2() { pitch = Rad::turn_div_2(); }
        rotation_matrix_2dof(yaw, pitch)
    }

    pub fn view_matrix(&self, dt: f32, add_yaw: Rad<f32>, add_pitch: Rad<f32>) -> Matrix4<f32> {
        let view_rot = self.rotation_matrix(add_yaw, add_pitch).transpose();
        let offset = view_rot * -(self.pos + self.vel * dt + Vector3::unit_z() * PLAYER_EYELEVEL).to_vec();
        Matrix4::from_cols(
            view_rot.x.extend(0.0),
            view_rot.y.extend(0.0),
            view_rot.z.extend(0.0),
            offset    .extend(1.0))
    }

    pub fn add_rotation(&mut self, yaw: Rad<f32>, pitch: Rad<f32>) {
        self.dir.0 = (self.dir.0 + yaw).normalize();
        self.dir.1 = self.dir.1 + pitch;
        if self.dir.1 < Rad::zero      () { self.dir.1 = Rad::zero      (); }
        if self.dir.1 > Rad::turn_div_2() { self.dir.1 = Rad::turn_div_2(); }
    }

    pub fn wish_dir(&self, key_state: &KeyState, add_yaw: Rad<f32>, add_pitch: Rad<f32>) -> Vector2<f32> {
        let rotation = self.rotation_matrix(add_yaw, add_pitch);
        let up      = Vector3::<f32>::unit_z();
        let right   = rotation.x;
        let forward = up.cross(right);

        let mut wish_dir = Vector3::<f32>::zero();
        if key_state.key_w { wish_dir += forward; }
        if key_state.key_a { wish_dir -= right; }
        if key_state.key_s { wish_dir -= forward; }
        if key_state.key_d { wish_dir += right; }
        let norm = wish_dir.magnitude();
        (if norm < 0.0001 { wish_dir } else { wish_dir / norm }).xy()
    }

    pub fn is_grounded(&self) -> bool {
        self.pos.z < JUMP_GROUND_DIST && self.vel.z < 0.001
    }

    pub fn sim_kinematics(&mut self,
        kinematics: &Kinematics,
        dt: f32,
        wish_dir: Vector2<f32>,
        is_jumping: bool,
        is_turning: bool)
    {
        let is_grounded = self.is_grounded();

        kinematics.sim(&mut self.vel, dt, wish_dir, is_grounded, is_jumping, is_turning);

        self.pos += self.vel * dt;

        if self.pos.z < 0.0 {
            self.pos.z = 0.0;
            if self.vel.z < 0.0 {
                self.vel.z = 0.0;
            }
        }
    }
}