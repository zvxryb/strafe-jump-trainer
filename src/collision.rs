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

use cgmath::{Matrix3, Matrix4, Point2, Vector2, Vector3};

#[derive(Copy, Clone, Debug)]
pub struct Plane2D {
    pub norm: Vector2<f32>,
    pub dist: f32,
}

impl Plane2D {
    pub fn new(norm: Vector2<f32>, dist: f32) -> Self {
        Self{norm, dist}
    }

    pub fn normalize(self) -> Self {
        let magnitude = self.norm.magnitude();
        Self{
            norm: self.norm / magnitude,
            dist: self.dist / magnitude
        }
    }

    pub fn dist_to_point(self, point: Point2<f32>) -> f32 {
        self.norm.dot(point.to_vec()) + self.dist
    }

    pub fn dist_to_circle(self, center: Point2<f32>, radius: f32) -> f32 {
        self.dist_to_point(center) - radius
    }
}

pub struct Box2D([Plane2D; 4]);

impl Box2D {
    pub fn from_size_and_transform(size: f32, transform: Matrix3<f32>) -> Self {
        fn plane_from_vec3(v: Vector3<f32>) -> Plane2D {
            Plane2D::new(v.xy(), v.z)
        }

        let inverse_transpose = transform.invert().unwrap().transpose();
        Self([
            plane_from_vec3(inverse_transpose * Vector3::new(-1.0,  0.0, -size/2.0)).normalize(),
            plane_from_vec3(inverse_transpose * Vector3::new( 1.0,  0.0, -size/2.0)).normalize(),
            plane_from_vec3(inverse_transpose * Vector3::new( 0.0, -1.0, -size/2.0)).normalize(),
            plane_from_vec3(inverse_transpose * Vector3::new( 0.0,  1.0, -size/2.0)).normalize(),
        ])
    }

    pub fn collide_circle(&self, center: Point2<f32>, radius: f32) -> Option<Vector2<f32>> {
        let Self(planes) = &self;
        planes.iter().try_fold((Vector2::<f32>::zero(), std::f32::MAX),
            |(nearest_dir, nearest_scale), plane| {
                let dist = plane.dist_to_circle(center, radius);
                if dist < 0.0 {
                    if -dist < nearest_scale {
                        Some((plane.norm, -dist))
                    } else {
                        Some((nearest_dir, nearest_scale))
                    }
                } else {
                    None
                }
            }
        ).map(|(norm, scale)| norm * scale)
    }
}

pub fn mat_drop_z(transform: Matrix4<f32>) -> Matrix3<f32> {
    Matrix3::from_cols(
        transform[0].xyw(),
        transform[1].xyw(),
        transform[3].xyw())
}