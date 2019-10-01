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

use crate::gl_context::GlContext;
use crate::gfx::{Color, draw_pass, gen_box, Mesh, Program, UniformValue};
use crate::player::{PlayerState, PLAYER_RADIUS};

use rand::prelude::*;

use cgmath::{Matrix4, Point3, Rad, Vector3};

pub trait Environment {
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

pub struct Runway {
    length: f32,
    width: f32,
    floor_mesh: Mesh,
    wall_mesh: Mesh,
    scenery_mesh: Mesh,
    scenery_transforms: Vec<Matrix4<f32>>,
}

impl Runway {
    pub fn from_dimensions(gl: &GlContext, length: f32, width: f32) -> Option<Self> {
        let scenery_transforms = {
            let n = (length / 256.0) as u32;
            let x = (width + 1.414 * BOX_WIDTH) / 2.0 + WALL_THICKNESS + 32.0;
            let xs = [-x, x];
            let mut rng = rand::thread_rng();
            (0..n)
                .flat_map(|i| {
                    let y = (i as f32) * length / (n as f32) - length / 2.0;
                    xs.iter()
                        .map(move |x| {
                            let offset = Vector3::new(
                                rng.gen_range(-32.0, 32.0) + x,
                                rng.gen_range(-32.0, 32.0) + y,
                                rng.gen_range( 64.0, 96.0));
                            let angle = Rad(rng.gen_range(-0.5, 0.5));
                            Matrix4::from_translation(offset) * Matrix4::from_angle_z(angle)
                        })
                })
                .collect()
        };
        Some(Self{
            length, width,
            floor_mesh: gen_box(gl,
                Point3::new(-width/2.0, -length/2.0, -WALL_THICKNESS),
                Point3::new( width/2.0,  length/2.0,  0.0),
                128.0)?,
            wall_mesh: gen_box(gl,
                Point3::new(-WALL_THICKNESS / 2.0, -length/2.0,  0.0),
                Point3::new( WALL_THICKNESS / 2.0,  length/2.0, 128.0),
                64.0)?,
            scenery_mesh: gen_box(gl,
                Point3::new(-BOX_WIDTH / 2.0, -BOX_WIDTH / 2.0,   0.0),
                Point3::new( BOX_WIDTH / 2.0,  BOX_WIDTH / 2.0, 256.0),
                64.0)?,
            scenery_transforms,
        })
    }
}

impl Environment for Runway {
    fn atmosphere_color(&self) -> Color { Color::new(0.6, 0.8, 1.0, 0.0002) }
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
            let floor_uniforms = [("M", UniformValue::Matrix4(offset_matrix))];
            let wall0_uniforms = [
                ("M", UniformValue::Matrix4(Matrix4::from_translation(
                    Vector3::new(-(self.width + WALL_THICKNESS)/2.0, y, 0.0))))];
            let wall1_uniforms = [
                ("M", UniformValue::Matrix4(Matrix4::from_translation(
                    Vector3::new((self.width + WALL_THICKNESS)/2.0, y, 0.0))))];
            let mut objects = vec![
                (&floor_uniforms, self.floor_mesh.clone()),
                (&wall0_uniforms, self.wall_mesh.clone()),
                (&wall1_uniforms, self.wall_mesh.clone()),
            ];
            let scenery = self.scenery_transforms.iter()
                .map(|m| {
                    [("M", UniformValue::Matrix4(offset_matrix * m))]
                })
                .collect::<Vec<_>>();
            objects.extend(scenery.iter()
                .map(|uniforms| {
                    (uniforms, self.scenery_mesh.clone())
                }));
            let fog_color = self.atmosphere_color();
            draw_pass(gl, program, &[
                ("V", UniformValue::Matrix4(*view_matrix)),
                ("P", UniformValue::Matrix4(*projection_matrix)),
                ("fog_color", UniformValue::Color(fog_color)),
            ], objects);
        }
    }
}