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

use crate::collision::{Box2D, mat_drop_z};
use crate::gl_context::GlContext;
use crate::gfx::{
    build_vbo,
    Color,
    Constant,
    ConstantValue,
    draw_pass,
    gen_box,
    InstanceData,
    Mesh,
    Program,
    VertexAttrib,
    VERTEX_ATTRIB_DEFAULT,
};
use crate::player::{PlayerState, PLAYER_RADIUS};

use cgmath::prelude::*;
use rand::prelude::*;

use cgmath::{Matrix4, Point3, Rad, Vector3};
use web_sys::WebGlRenderingContext;

pub trait Map {
    fn atmosphere_color(&self) -> Color;
    fn interact(&mut self, player: &mut PlayerState);
    fn draw(&self,
        gl: &GlContext,
        program: &Program,
        view_matrix: &Matrix4<f32>,
        projection_matrix: &Matrix4<f32>);
}

const WALL_THICKNESS: f32 = 8.0;
const BOX_WIDTH: f32 = 128.0;

enum InstanceTransforms {
    Instanced(InstanceData),
    Fallback(Vec<Matrix4<f32>>),
}

pub struct Runway {
    length: f32,
    width: f32,
    floor_mesh: Mesh,
    wall_mesh: Mesh,
    scenery_mesh: Mesh,
    scenery_transforms: InstanceTransforms,
}

impl Runway {
    pub fn new(gl: &GlContext) -> Self {
        const LENGTH: f32 = 16384.0;
        const WIDTH : f32 = 2048.0;
        let scenery_transforms = {
            let density: f32 = if gl.webgl2().is_some() { 0.02 } else { 0.0025 };
            let n = (LENGTH * density) as usize;
            let x0 = (WIDTH + 1.414 * BOX_WIDTH) / 2.0 + WALL_THICKNESS + 32.0;
            let mut rng = rand::thread_rng();
            let mut positions = Vec::<(Vector3<f32>, f32)>::with_capacity(n);
            while positions.len() < n {
                for &sign in &[1.0, -1.0] {
                    let scale: f32 = rng.gen_range(1.0, 4.0)
                                   * rng.gen_range(1.0, 4.0);
                    let offset = Vector3::new(
                        sign * scale * x0,
                        rng.gen_range(-LENGTH, LENGTH) / 2.0,
                        rng.gen_range( 64.0, 96.0));
                    let scale = BOX_WIDTH * scale * rng.gen_range(1.0, 2.0);
                    let collides = positions.iter().find(|(other_offset, other_scale)| {
                        other_offset.xy().distance(offset.xy()) <= 1.414 * (scale + other_scale) / 2.0
                    }).is_some();
                    if !collides {
                        positions.push((offset, scale));
                    }
                }
            }
            // sort nearest first to reduce overdraw:
            positions.sort_by(|(lhs, _), (rhs, _)| {
                lhs.x.abs().partial_cmp(&rhs.x.abs()).unwrap_or(std::cmp::Ordering::Equal)
            });
            let mut data = Vec::<Matrix4<f32>>::with_capacity(n as usize);
            data.extend(positions.iter().map(|&(offset, scale)| {
                let angle = Rad(rng.gen_range(Rad::<f32>::zero().0, Rad::<f32>::full_turn().0));
                Matrix4::from_translation(offset) *
                Matrix4::from_angle_z(angle) *
                Matrix4::from_scale(scale)
            }));
            if gl.webgl2().is_some() {
                let instance = InstanceData{
                    buffer: build_vbo(gl, data.as_slice()).unwrap(),
                    attributes: &[
                        VertexAttrib {
                            ident: "M_instance",
                            size: 16,
                            type_: WebGlRenderingContext::FLOAT,
                            stride: 64,
                            divisor: 1,
                            ..VERTEX_ATTRIB_DEFAULT
                        },
                    ],
                    count: data.len() as i32,
                };
                InstanceTransforms::Instanced(instance)
            } else {
                InstanceTransforms::Fallback(data)
            }
        };
        Self{
            length: LENGTH, width: WIDTH,
            floor_mesh: gen_box(gl,
                Point3::new(-WIDTH/2.0, -LENGTH/2.0, -WALL_THICKNESS),
                Point3::new( WIDTH/2.0,  LENGTH/2.0,  0.0),
                128.0).unwrap(),
            wall_mesh: gen_box(gl,
                Point3::new(-WALL_THICKNESS / 2.0, -LENGTH/2.0,  0.0),
                Point3::new( WALL_THICKNESS / 2.0,  LENGTH/2.0, 128.0),
                64.0).unwrap(),
            scenery_mesh: gen_box(gl,
                Point3::new(-0.5, -0.5, 0.0),
                Point3::new( 0.5,  0.5, 2.0),
                0.5).unwrap(),
            scenery_transforms,
        }
    }
}

impl Map for Runway {
    fn atmosphere_color(&self) -> Color { Color::new(0.6, 0.8, 1.0, 0.0001) }
    fn interact(&mut self, player: &mut PlayerState) {
        if player.pos.x - PLAYER_RADIUS < -self.width / 2.0 {
            player.pos.x = -self.width / 2.0 + PLAYER_RADIUS;
            if player.vel.x < 0.0 {
                player.vel.x = 0.0;
            }
        }
        if player.pos.x + PLAYER_RADIUS > self.width / 2.0 {
            player.pos.x = self.width / 2.0 - PLAYER_RADIUS;
            if player.vel.x > 0.0 {
                player.vel.x = 0.0;
            }
        }
        if player.pos.y < -self.length / 2.0 {
            player.pos.y += self.length;
        }
        if player.pos.y > self.length / 2.0 {
            player.pos.y -= self.length;
        }
    }

    fn draw(&self,
        gl: &GlContext,
        program: &Program,
        view_matrix: &Matrix4<f32>,
        projection_matrix: &Matrix4<f32>)
    {
        for &y in &[0.0, -self.length, self.length] {
            let offset_matrix = Matrix4::from_translation(Vector3::new(0.0, y, 0.0));

            let draw_objects = |objects: Vec<(&[(&str, Constant)], Mesh, Option<&InstanceData>)>| {
                let fog_color = self.atmosphere_color();
                draw_pass(gl, program, &[
                    ("V"        , Constant::Uniform(ConstantValue::Matrix4(*view_matrix))),
                    ("P"        , Constant::Uniform(ConstantValue::Matrix4(*projection_matrix))),
                    ("fog_color", Constant::Uniform(ConstantValue::Color(fog_color))),
                    ("M_group"  , Constant::Uniform(ConstantValue::Matrix4(offset_matrix))),
                ], objects);
            };

            let floor_constants = [
                ("M_instance", Constant::VertexAttrib(ConstantValue::Matrix4(Matrix4::identity()))),
            ];
            let wall0_constants = [
                ("M_instance", Constant::VertexAttrib(ConstantValue::Matrix4(
                    Matrix4::from_translation(Vector3::unit_x() * -(self.width + WALL_THICKNESS)/2.0)))),
            ];
            let wall1_constants = [
                ("M_instance", Constant::VertexAttrib(ConstantValue::Matrix4(
                    Matrix4::from_translation(Vector3::unit_x() * (self.width + WALL_THICKNESS)/2.0)))),
            ];
            let mut objects: Vec<(&[_], _, _)> = vec![
                (&floor_constants, self.floor_mesh.clone(), None),
                (&wall0_constants, self.wall_mesh.clone(), None),
                (&wall1_constants, self.wall_mesh.clone(), None),
            ];
            match &self.scenery_transforms {
                InstanceTransforms::Instanced(instance_data) => {
                    objects.push((&[], self.scenery_mesh.clone(), Some(instance_data)));
                    draw_objects(objects);
                }
                InstanceTransforms::Fallback(transforms) => {
                    let scenery = transforms.iter()
                        .map(|m| {
                            [("M_instance", Constant::VertexAttrib(ConstantValue::Matrix4(*m)))]
                        })
                        .collect::<Vec<_>>();
                    objects.extend(scenery.iter()
                        .map(|constants| -> (&[_], _, _) {
                            (constants, self.scenery_mesh.clone(), None)
                        }));
                    draw_objects(objects);
                }
            }
        }
    }
}

pub struct Freestyle {
    size: f32,
    floor_mesh: Mesh,
    scenery_mesh: Mesh,
    scenery_collision: Vec<Box2D>,
    scenery_transforms: InstanceTransforms,
}

impl Freestyle {
    pub fn new(gl: &GlContext) -> Self {
        const SIZE: f32 = 8192.0;
        const DENSITY: f32 = 0.0015;
        let n = (SIZE * SIZE * DENSITY * DENSITY) as usize;
        let mut rng = rand::thread_rng();
        let mut positions = Vec::<(Vector3<f32>, f32)>::with_capacity(n);

        while positions.len() < n {
            let offset = Vector3::new(
                rng.gen_range(-0.5, 0.5) * SIZE,
                rng.gen_range(-0.5, 0.5) * SIZE,
                0.0);
            let scale = BOX_WIDTH * rng.gen_range(1.5, 3.0);
            let collides = offset.xy().magnitude() < (1.414 * scale + PLAYER_RADIUS) ||
                positions.iter().find(|(other_offset, other_scale)| {
                    other_offset.xy().distance(offset.xy()) <= (scale + other_scale) / 2.0
                }).is_some();
            if !collides {
                positions.push((offset, scale));
            }
        }
        let mut transforms = Vec::<Matrix4<f32>>::with_capacity(n as usize);
        transforms.extend(positions.iter().map(|&(offset, scale)| {
            let angle = Rad(rng.gen_range(Rad::<f32>::zero().0, Rad::<f32>::full_turn().0));
            Matrix4::from_translation(offset) *
            Matrix4::from_angle_z(angle) *
            Matrix4::from_scale(scale)
        }));
        let scenery_collision = transforms.iter()
            .map(|transform| Box2D::from_size_and_transform(1.0, mat_drop_z(*transform)))
            .collect();
        let scenery_transforms = if gl.webgl2().is_some() {
            let instance = InstanceData{
                buffer: build_vbo(gl, transforms.as_slice()).unwrap(),
                attributes: &[
                    VertexAttrib {
                        ident: "M_instance",
                        size: 16,
                        type_: WebGlRenderingContext::FLOAT,
                        stride: 64,
                        divisor: 1,
                        ..VERTEX_ATTRIB_DEFAULT
                    },
                ],
                count: transforms.len() as i32,
            };
            InstanceTransforms::Instanced(instance)
        } else {
            InstanceTransforms::Fallback(transforms)
        };
        Self{
            size: SIZE,
            floor_mesh: gen_box(gl,
                Point3::new(-SIZE/2.0, -SIZE/2.0, -WALL_THICKNESS),
                Point3::new( SIZE/2.0,  SIZE/2.0,  0.0),
                128.0).unwrap(),
            scenery_mesh: gen_box(gl,
                Point3::new(-0.5, -0.5, 0.0),
                Point3::new( 0.5,  0.5, 2.0),
                0.5).unwrap(),
            scenery_collision,
            scenery_transforms,
        }
    }
}

impl Map for Freestyle {
    fn atmosphere_color(&self) -> Color { Color::new(0.6, 0.8, 1.0, 0.0002) }
    fn interact(&mut self, player: &mut PlayerState) {
        for box2d in self.scenery_collision.iter() {
            if let Some(offset) = box2d.collide_circle(player.pos.xy(), PLAYER_RADIUS) {
                if offset.magnitude2() > 0.000_001 {
                    let dir = offset.normalize().extend(0.0);
                    player.vel -= dir * dir.dot(player.vel).min(0.0);
                    player.pos += offset.extend(0.0);
                }
            }
        }

        if player.pos.x < -self.size / 2.0 {
            player.pos.x += self.size;
        }
        if player.pos.x > self.size / 2.0 {
            player.pos.x -= self.size;
        }
        if player.pos.y < -self.size / 2.0 {
            player.pos.y += self.size;
        }
        if player.pos.y > self.size / 2.0 {
            player.pos.y -= self.size;
        }
    }

    fn draw(&self,
        gl: &GlContext,
        program: &Program,
        view_matrix: &Matrix4<f32>,
        projection_matrix: &Matrix4<f32>)
    {
        for &origin in &[
            Vector3::new(       0.0,        0.0, 0.0),
            Vector3::new(-self.size, -self.size, 0.0),
            Vector3::new(       0.0, -self.size, 0.0),
            Vector3::new( self.size, -self.size, 0.0),
            Vector3::new(-self.size,        0.0, 0.0),
            Vector3::new( self.size,        0.0, 0.0),
            Vector3::new(-self.size,  self.size, 0.0),
            Vector3::new(       0.0,  self.size, 0.0),
            Vector3::new( self.size,  self.size, 0.0),
        ] {
            let offset_matrix = Matrix4::from_translation(origin);

            let draw_objects = |objects: Vec<(&[(&str, Constant)], Mesh, Option<&InstanceData>)>| {
                let fog_color = self.atmosphere_color();
                draw_pass(gl, program, &[
                    ("V"        , Constant::Uniform(ConstantValue::Matrix4(*view_matrix))),
                    ("P"        , Constant::Uniform(ConstantValue::Matrix4(*projection_matrix))),
                    ("fog_color", Constant::Uniform(ConstantValue::Color(fog_color))),
                    ("M_group"  , Constant::Uniform(ConstantValue::Matrix4(offset_matrix))),
                ], objects);
            };

            let floor_constants = [
                ("M_instance", Constant::VertexAttrib(ConstantValue::Matrix4(Matrix4::identity()))),
            ];
            let mut objects: Vec<(&[_], _, _)> = vec![
                (&floor_constants, self.floor_mesh.clone(), None),
            ];
            match &self.scenery_transforms {
                InstanceTransforms::Instanced(instance_data) => {
                    objects.push((&[], self.scenery_mesh.clone(), Some(instance_data)));
                    draw_objects(objects);
                }
                InstanceTransforms::Fallback(transforms) => {
                    let scenery = transforms.iter()
                        .map(|m| {
                            [("M_instance", Constant::VertexAttrib(ConstantValue::Matrix4(*m)))]
                        })
                        .collect::<Vec<_>>();
                    objects.extend(scenery.iter()
                        .map(|constants| -> (&[_], _, _) {
                            (constants, self.scenery_mesh.clone(), None)
                        }));
                    draw_objects(objects);
                }
            }
        }
    }
}