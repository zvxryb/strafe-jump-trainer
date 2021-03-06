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
use crate::{log, error};

use cgmath::prelude::*;
use rand::prelude::*;

use cgmath::{Matrix4, Point2, Point3, Vector2, Vector3};
use js_sys::Uint8Array;
use wasm_bindgen::JsCast;
use web_sys::{
    WebGlBuffer,
    WebGlProgram,
    WebGlRenderingContext,
    WebGl2RenderingContext,
    WebGlShader,
    WebGlUniformLocation,
};

use std::mem::{self, MaybeUninit};
use std::ptr;

#[derive(Copy, Clone)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self{r, g, b, a}
    }
    fn linear_to_srgb(x: f32) -> f32 {
        if x < 0.003_130_8 {
            12.92 * x
        } else {
            1.055 * x.powf(1.0/2.4) - 0.055
        }
    }
    pub fn to_srgb(&self) -> Self {
        Self{
            r: Self::linear_to_srgb(self.r),
            g: Self::linear_to_srgb(self.g),
            b: Self::linear_to_srgb(self.b),
            a: self.a,
        }
    }
}

#[allow(clippy::needless_lifetimes)]
fn get_byte_view<'a, T>(data: &'a [T]) -> &'a [u8]
where
    T: 'static + Sized + Copy + Send + Sync
{
    let start = data.as_ptr();
    let size  = data.len() * std::mem::size_of::<T>();
    unsafe { std::slice::from_raw_parts(start as *const u8, size) }
}

fn build_shader(gl: &GlContext, type_: u32, source: &str)
    -> Option<WebGlShader>
{
    let shader = gl.create_shader(type_)?;
    gl.shader_source(&shader, source);
    gl.compile_shader(&shader);
    let status = gl.get_shader_parameter(&shader,
        WebGlRenderingContext::COMPILE_STATUS);

    if let Some(true) = status.as_bool() {
        Some(shader)
    } else {
        error("failed to compile shader!");
        if let Some(log_) = gl.get_shader_info_log(&shader) {
            log(log_.as_str());
        }
        None
    }
}

fn link_program(gl: &GlContext, program: &WebGlProgram) -> Result<(), ()> {
    gl.link_program(&program);
    let status = gl.get_program_parameter(&program,
        WebGlRenderingContext::LINK_STATUS);

    if let Some(true) = status.as_bool() {
        Ok(())
    } else {
        error("failed to link program!");
        if let Some(log_) = gl.get_program_info_log(&program) {
            log(log_.as_str());
        }
        Err(())
    }
}

fn build_program(gl: &GlContext, source_vs: &str, source_fs: &str)
    -> Option<WebGlProgram>
{
    let vs = build_shader(gl, WebGlRenderingContext::VERTEX_SHADER  , source_vs)?;
    let fs = build_shader(gl, WebGlRenderingContext::FRAGMENT_SHADER, source_fs)?;
    let program = gl.create_program()?;
    gl.attach_shader(&program, &vs);
    gl.attach_shader(&program, &fs);
    link_program(gl, &program).ok()?;
    Some(program)
}

pub fn build_vbo<T>(gl: &GlContext, data: &[T]) -> Option<WebGlBuffer>
where
    T: 'static + Sized + Copy + Send + Sync
{
    let vbo = gl.create_buffer()?;
    gl.bind_buffer(WebGlRenderingContext::ARRAY_BUFFER, Some(&vbo));
    unsafe {
        let view = Uint8Array::view(get_byte_view(data));
        gl.buffer_data_with_array_buffer_view(
            WebGlRenderingContext::ARRAY_BUFFER, &view,
            WebGlRenderingContext::STATIC_DRAW);
    }
    gl.bind_buffer(WebGlRenderingContext::ARRAY_BUFFER, None);
    Some(vbo)
}

struct ProgramData {
    size: i32,
    type_: u32,
    name: String,
}

impl ProgramData {
    fn type_info(&self) -> (u32, i32, i32, i32) {
        match self.type_ {
            WebGlRenderingContext::FLOAT      => (WebGlRenderingContext::FLOAT, 4, 1, self.size),
            WebGlRenderingContext::FLOAT_VEC2 => (WebGlRenderingContext::FLOAT, 4, 2, self.size),
            WebGlRenderingContext::FLOAT_VEC3 => (WebGlRenderingContext::FLOAT, 4, 3, self.size),
            WebGlRenderingContext::FLOAT_VEC4 => (WebGlRenderingContext::FLOAT, 4, 4, self.size),
            WebGlRenderingContext::FLOAT_MAT4 => (WebGlRenderingContext::FLOAT, 4, 4, 4 * self.size),
            _ => panic!("unrecognized type")
        }
    }
}

#[derive(Clone)]
pub enum ConstantValue {
    Color(Color),
    Float(f32),
    Vector2(Vector2<f32>),
    Vector3(Vector3<f32>),
    Matrix4(Matrix4<f32>),
}

#[derive(Clone)]
pub enum Constant {
    Uniform(ConstantValue),
    VertexAttrib(ConstantValue),
}

pub struct Program {
    program: WebGlProgram,
    attributes: Vec<(ProgramData, u32)>,
    uniforms: Vec<(ProgramData, WebGlUniformLocation)>,
}

impl Program {
    pub fn wrap(gl: &GlContext, program: WebGlProgram) -> Self {
        let attrib_count = gl.get_program_parameter(&program,
            WebGlRenderingContext::ACTIVE_ATTRIBUTES).as_f64().unwrap() as u32;
        let attributes = (0..attrib_count)
            .map(|index| {
                let attrib = gl.get_active_attrib(&program, index).unwrap();
                let location = Some(gl.get_attrib_location(&program, attrib.name().as_str()))
                    .filter(|&idx| idx >= 0)
                    .map(|idx| idx as u32)
                    .unwrap();
                (ProgramData{
                    size : attrib.size (),
                    type_: attrib.type_(),
                    name : attrib.name (),
                }, location)
            })
            .collect::<Vec<_>>();
        let uniform_count = gl.get_program_parameter(&program,
            WebGlRenderingContext::ACTIVE_UNIFORMS).as_f64().unwrap() as u32;
        let uniforms = (0..uniform_count)
            .map(|index| {
                let uniform = gl.get_active_uniform(&program, index).unwrap();
                let location = gl.get_uniform_location(&program, uniform.name().as_str()).unwrap();
                (ProgramData{
                    size : uniform.size (),
                    type_: uniform.type_(),
                    name : uniform.name (),
                }, location)
            })
            .collect::<Vec<_>>();
        Self{program, attributes, uniforms}
    }

    pub fn from_source(gl: &GlContext, source_vs: &str, source_fs: &str) -> Option<Self> {
        Some(Self::wrap(gl, build_program(gl, source_vs, source_fs)?))
    }

    pub fn use_program(&self, gl: &GlContext) {
        gl.use_program(Some(&self.program));
    }

    fn assign_vertex_attribs<'a, VertexAttribs>(&self, gl: &GlContext, vertex_attribs: VertexAttribs)
    where
        VertexAttribs: IntoIterator<Item = &'a VertexAttrib>
    {
        for vertex_attrib in vertex_attribs {
            if let Some((program_attrib, location)) = self.attributes.iter()
                .find(|(attrib, _)| attrib.name == vertex_attrib.ident)
            {
                let (type_, type_bytes, col_size, cols) = program_attrib.type_info();
                if vertex_attrib.type_ == type_
                && vertex_attrib.size  == col_size * cols {
                    for i in 0..cols {
                        gl.enable_vertex_attrib_array(*location + i as u32);
                    }
                    for i in 0..cols {
                        gl.vertex_attrib_pointer_with_i32(*location + i as u32,
                            col_size,
                            type_,
                            vertex_attrib.normalized,
                            vertex_attrib.stride,
                            vertex_attrib.offset + i * col_size * type_bytes);
                    }
                    if let Some(gl) = gl.webgl2() {
                        for i in 0..cols {
                            gl.vertex_attrib_divisor(*location + i as u32, vertex_attrib.divisor);
                        }
                    } else {
                        assert_eq!(vertex_attrib.divisor, 0);
                    }
                } else {
                    error(format!("vertex data for {} does not match type/size expected by program
                        expected: {:04x}, {}
                        actual  : {:04x}, {}",
                        program_attrib.name, type_, col_size * cols,
                        vertex_attrib.type_, vertex_attrib.size).as_str());
                }
            }
        }
    }

    fn clear_vertex_attribs(&self, gl: &GlContext) {
        for (attrib, location) in &self.attributes {
            let (_, _, _, n) = attrib.type_info();
            for i in 0..n {
                gl.disable_vertex_attrib_array(*location + i as u32);
            }
        }
    }

    pub fn set_attribute(&self, gl: &GlContext, name: &str, value: &ConstantValue) {
        if let Some((attrib, location)) = self.attributes.iter()
            .find(|(attrib, _)| attrib.name == name)
        {
            match value {
                ConstantValue::Color(value) => {
                    assert_eq!(attrib.type_, WebGlRenderingContext::FLOAT_VEC4);
                    assert_eq!(attrib.size, 1);
                    gl.vertex_attrib4f(*location,
                        value.r,
                        value.g,
                        value.b,
                        value.a,
                    );
                }
                ConstantValue::Float(value) => {
                    assert_eq!(attrib.type_, WebGlRenderingContext::FLOAT);
                    assert_eq!(attrib.size, 1);
                    gl.vertex_attrib1f(*location, *value);
                }
                ConstantValue::Vector2(value) => {
                    assert_eq!(attrib.type_, WebGlRenderingContext::FLOAT_VEC2);
                    assert_eq!(attrib.size, 1);
                    gl.vertex_attrib2f(*location, value.x, value.y);
                }
                ConstantValue::Vector3(value) => {
                    assert_eq!(attrib.type_, WebGlRenderingContext::FLOAT_VEC3);
                    assert_eq!(attrib.size, 1);
                    gl.vertex_attrib3f(*location, value.x, value.y, value.z);
                }
                ConstantValue::Matrix4(value) => {
                    assert_eq!(attrib.type_, WebGlRenderingContext::FLOAT_MAT4);
                    assert_eq!(attrib.size, 1);
                    gl.vertex_attrib4f(*location    , value[0].x, value[0].y, value[0].z, value[0].w);
                    gl.vertex_attrib4f(*location + 1, value[1].x, value[1].y, value[1].z, value[1].w);
                    gl.vertex_attrib4f(*location + 2, value[2].x, value[2].y, value[2].z, value[2].w);
                    gl.vertex_attrib4f(*location + 3, value[3].x, value[3].y, value[3].z, value[3].w);
                }
            }
        } else {
            panic!("failed to locate attrib for {}", name);
        }
    }

    pub fn set_uniform(&self, gl: &GlContext, name: &str, value: &ConstantValue) {
        if let Some((uniform, location)) = self.uniforms.iter()
            .find(|(uniform, _)| uniform.name == name)
        {
            match value {
                ConstantValue::Color(value) => {
                    assert_eq!(uniform.type_, WebGlRenderingContext::FLOAT_VEC4);
                    assert_eq!(uniform.size, 1);
                    gl.uniform4f(Some(location),
                        value.r,
                        value.g,
                        value.b,
                        value.a,
                    );
                }
                ConstantValue::Float(value) => {
                    assert_eq!(uniform.type_, WebGlRenderingContext::FLOAT);
                    assert_eq!(uniform.size, 1);
                    gl.uniform1f(Some(location), *value);
                }
                ConstantValue::Vector2(value) => {
                    assert_eq!(uniform.type_, WebGlRenderingContext::FLOAT_VEC2);
                    assert_eq!(uniform.size, 1);
                    gl.uniform2f(Some(location), value.x, value.y);
                }
                ConstantValue::Vector3(value) => {
                    assert_eq!(uniform.type_, WebGlRenderingContext::FLOAT_VEC3);
                    assert_eq!(uniform.size, 1);
                    gl.uniform3f(Some(location), value.x, value.y, value.z);
                }
                ConstantValue::Matrix4(value) => {
                    assert_eq!(uniform.type_, WebGlRenderingContext::FLOAT_MAT4);
                    assert_eq!(uniform.size, 1);
                    gl.uniform_matrix4fv_with_f32_array(Some(location), false,
                        AsRef::<[f32; 16]>::as_ref(value));
                }
            }
        } else {
            panic!("failed to locate uniform for {}", name);
        }
    }

    pub fn set_constant(&self, gl: &GlContext, name: &str, value: &Constant) {
        match value {
            Constant::Uniform     (value) => { self.set_uniform  (gl, name, &value) }
            Constant::VertexAttrib(value) => { self.set_attribute(gl, name, &value) }
        }
    }
}

#[derive(Clone)]
pub struct VertexAttrib {
    pub ident: &'static str,
    pub size: i32,
    pub type_: u32,
    pub normalized: bool,
    pub stride: i32,
    pub offset: i32,
    pub divisor: u32,
}

pub const VERTEX_ATTRIB_DEFAULT: VertexAttrib = VertexAttrib{
    ident: "",
    size: 0,
    type_: WebGlRenderingContext::FLOAT,
    normalized: false,
    stride: 0,
    offset: 0,
    divisor: 0,
};

pub trait VertexLayout: 'static + Sized + Copy + Send + Sync {
    fn attribs() -> &'static [VertexAttrib];
}

#[derive(Clone)]
pub struct Mesh {
    vertices:   WebGlBuffer,
    attributes: &'static [VertexAttrib],
    draw_mode:  u32,
    count:      i32,
}

impl Mesh {
    pub fn from_vertices<V: VertexLayout>(gl: &GlContext, draw_mode: u32, data: &[V])
        -> Option<Self>
    {
        let vertices = build_vbo(gl, data)?;

        Some(Self{
            vertices,
            attributes: V::attribs(),
            draw_mode,
            count: data.len() as i32,
        })
    }
}

pub struct InstanceData {
    pub buffer:     WebGlBuffer,
    pub attributes: &'static [VertexAttrib],
    pub count:      i32,
}

pub fn draw_pass<'a, Constants, Meshes, MeshConstants>(
    gl: &GlContext,
    program: &Program,
    constants: Constants,
    meshes: Meshes,
)
where
    Constants: IntoIterator<Item=&'a (&'a str, Constant)>,
    Meshes: IntoIterator<Item=(MeshConstants, Mesh, Option<&'a InstanceData>)>,
    MeshConstants: IntoIterator<Item=&'a (&'a str, Constant)>,
{
    program.use_program(gl);

    for constant in constants.into_iter() {
        let (name, value) = constant;
        program.set_constant(gl, name, &value);
    }

    for mesh in meshes.into_iter() {
        let (constants, mesh, instances) = mesh;

        for constant in constants.into_iter() {
            let (name, value) = constant;
            program.set_constant(gl, name, &value);
        }

        gl.bind_buffer(
            WebGlRenderingContext::ARRAY_BUFFER,
            Some(&mesh.vertices));

        program.assign_vertex_attribs(gl, mesh.attributes);

        gl.bind_buffer(
            WebGlRenderingContext::ARRAY_BUFFER,
            None);

        if let Some(instances) = instances.as_ref() {
            let gl = gl.webgl2()
                .expect("instanced rendering requires WebGL 2.0");

            gl.bind_buffer(
                WebGlRenderingContext::ARRAY_BUFFER,
                Some(&instances.buffer));

            program.assign_vertex_attribs(gl, instances.attributes);

            gl.bind_buffer(
                WebGlRenderingContext::ARRAY_BUFFER,
                None);

            gl.draw_arrays_instanced(mesh.draw_mode, 0, mesh.count, instances.count);
        } else {
            gl.draw_arrays(mesh.draw_mode, 0, mesh.count);
        }

        program.clear_vertex_attribs(gl);
    }
}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct MeshVertex {
    pos: Point3<f32>,
    norm: Vector3<f32>,
    uv: Point2<f32>,
}

impl MeshVertex {
    pub fn new(pos: Point3<f32>, norm: Vector3<f32>, uv: Point2<f32>) -> MeshVertex {
        MeshVertex{pos, norm, uv}
    }

    #[allow(clippy::many_single_char_names, clippy::too_many_arguments)]
    pub fn from_scalars(x: f32, y: f32, z: f32, nx: f32, ny: f32, nz: f32, u: f32, v: f32) -> MeshVertex {
        MeshVertex{
            pos: Point3::new(x, y, z),
            norm: Vector3::new(nx, ny, nz),
            uv: Point2::new(u, v),
        }
    }
}

impl Default for MeshVertex {
    fn default() -> MeshVertex {
        MeshVertex{
            pos: Point3::new(0.0, 0.0, 0.0),
            norm: Vector3::new(0.0, 0.0, 1.0),
            uv: Point2::new(0.0, 0.0),
        }
    }
}

impl VertexLayout for MeshVertex {
    fn attribs() -> &'static [VertexAttrib] {
        const ATTRIBS: [VertexAttrib; 3] = [
            VertexAttrib {
                ident: "pos",
                size: 3,
                type_: WebGlRenderingContext::FLOAT,
                stride: 32,
                offset: 0,
                ..VERTEX_ATTRIB_DEFAULT
            },
            VertexAttrib {
                ident: "norm",
                size: 3,
                type_: WebGlRenderingContext::FLOAT,
                stride: 32,
                offset: 12,
                ..VERTEX_ATTRIB_DEFAULT
            },
            VertexAttrib {
                ident: "uv",
                size: 2,
                type_: WebGlRenderingContext::FLOAT,
                stride: 32,
                offset: 24,
                ..VERTEX_ATTRIB_DEFAULT
            }
        ];
        &ATTRIBS
    }
}

pub fn gen_box(gl: &GlContext, min: Point3<f32>, max: Point3<f32>, uv_scale: f32) -> Option<Mesh> {
    fn face_uv(min: Point3<f32>, max: Point3<f32>, uv_scale: f32, front: bool, flip_z: bool) -> Vec<MeshVertex> {
        let mut vs = Vec::new();
        let z = if front != flip_z { max.z } else { min.z };
        let nz = if front { 1.0 } else { -1.0 };
        let dx = max.x - min.x;
        let dy = max.y - min.y;
        let duv = Vector2::new(dx, dy) / uv_scale;
        let uv0 = Point2::new(0.0, 0.0);
        let uv1 = uv0 + duv;
        if front {
            vs.push(MeshVertex::from_scalars(min.x, min.y, z, 0.0, 0.0, nz, uv0.x, uv0.y));
            vs.push(MeshVertex::from_scalars(max.x, min.y, z, 0.0, 0.0, nz, uv1.x, uv0.y));
            vs.push(MeshVertex::from_scalars(max.x, max.y, z, 0.0, 0.0, nz, uv1.x, uv1.y));
            vs.push(MeshVertex::from_scalars(min.x, min.y, z, 0.0, 0.0, nz, uv0.x, uv0.y));
            vs.push(MeshVertex::from_scalars(max.x, max.y, z, 0.0, 0.0, nz, uv1.x, uv1.y));
            vs.push(MeshVertex::from_scalars(min.x, max.y, z, 0.0, 0.0, nz, uv0.x, uv1.y));
        } else {
            vs.push(MeshVertex::from_scalars(min.x, min.y, z, 0.0, 0.0, nz, uv0.x, uv0.y));
            vs.push(MeshVertex::from_scalars(max.x, max.y, z, 0.0, 0.0, nz, uv1.x, uv1.y));
            vs.push(MeshVertex::from_scalars(max.x, min.y, z, 0.0, 0.0, nz, uv1.x, uv0.y));
            vs.push(MeshVertex::from_scalars(min.x, min.y, z, 0.0, 0.0, nz, uv0.x, uv0.y));
            vs.push(MeshVertex::from_scalars(min.x, max.y, z, 0.0, 0.0, nz, uv0.x, uv1.y));
            vs.push(MeshVertex::from_scalars(max.x, max.y, z, 0.0, 0.0, nz, uv1.x, uv1.y));
        }
        vs
    }

    Mesh::from_vertices(gl, WebGlRenderingContext::TRIANGLES, [
        face_uv(min.xyz(), max.xyz(), uv_scale, false, false).into_iter().map(|v| MeshVertex::new(v.pos.xyz(), v.norm.xyz(), v.uv)).collect::<Vec<MeshVertex>>(),
        face_uv(min.xyz(), max.xyz(), uv_scale, true , false).into_iter().map(|v| MeshVertex::new(v.pos.xyz(), v.norm.xyz(), v.uv)).collect::<Vec<MeshVertex>>(),
        face_uv(min.xzy(), max.xzy(), uv_scale, false, true ).into_iter().map(|v| MeshVertex::new(v.pos.xzy(), v.norm.xzy(), v.uv)).collect::<Vec<MeshVertex>>(),
        face_uv(min.xzy(), max.xzy(), uv_scale, true , true ).into_iter().map(|v| MeshVertex::new(v.pos.xzy(), v.norm.xzy(), v.uv)).collect::<Vec<MeshVertex>>(),
        face_uv(min.zyx(), max.zyx(), uv_scale, false, true ).into_iter().map(|v| MeshVertex::new(v.pos.zyx(), v.norm.zyx(), v.uv)).collect::<Vec<MeshVertex>>(),
        face_uv(min.zyx(), max.zyx(), uv_scale, true , true ).into_iter().map(|v| MeshVertex::new(v.pos.zyx(), v.norm.zyx(), v.uv)).collect::<Vec<MeshVertex>>(),
    ].iter()
        .flatten()
        .cloned()
        .collect::<Vec<_>>()
        .as_slice())
}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct HudVertex {
    pos: Point2<f32>,
}

impl HudVertex {
    pub fn from_scalars(x: f32, y: f32) -> HudVertex {
        HudVertex{
            pos: Point2::new(x, y),
        }
    }
}

impl Default for HudVertex {
    fn default() -> HudVertex {
        HudVertex{
            pos: Point2::new(0.0, 0.0),
        }
    }
}

impl VertexLayout for HudVertex {
    fn attribs() -> &'static [VertexAttrib] {
        const ATTRIBS: [VertexAttrib; 1] = [
            VertexAttrib {
                ident: "pos",
                size: 2,
                type_: WebGlRenderingContext::FLOAT,
                stride: 8,
                ..VERTEX_ATTRIB_DEFAULT
            },
        ];
        &ATTRIBS
    }
}

pub fn gen_hud_quad(gl: &GlContext, min: Point2<f32>, max: Point2<f32>) -> Option<Mesh> {
    let vs = [
        HudVertex::from_scalars(min.x, min.y),
        HudVertex::from_scalars(max.x, min.y),
        HudVertex::from_scalars(max.x, max.y),
        HudVertex::from_scalars(min.x, min.y),
        HudVertex::from_scalars(max.x, max.y),
        HudVertex::from_scalars(min.x, max.y),
    ];

    Mesh::from_vertices(gl, WebGlRenderingContext::TRIANGLES, &vs)
}

const WARP_EFFECT_FRAMES: usize = 2;
const WARP_UPS_MIN: f32 = 2000.0;
const WARP_UPS_MAX: f32 = 5000.0;

const WARP_PHYS_VS_SRC: &str = "#version 100

attribute vec3 in_pos_0;
attribute vec3 in_pos;

varying vec3 out_pos;

uniform vec3 motion;
uniform float radius;

void main() {
    out_pos = in_pos;
    out_pos += motion;
    if (length(out_pos) > radius) {
        out_pos = 0.95 * in_pos_0;
        vec3 n = -normalize(motion);
        float d = dot(out_pos, n);
        if (d < 0.0) {
            out_pos -= 2.0 *  n * d;
        }
    }
}
";

const WARP_PHYS_FS_SRC: &str = "#version 100

void main() {
    discard;
}
";

const WARP_DRAW_VS_SRC: &str = "#version 100

attribute vec3 pos;
attribute float u;

uniform vec3 trail;
uniform mat4 V;
uniform mat4 P;

void main() {
    vec3 eye = mat3(V) * (pos + trail * (u - 0.5));
    vec4 clip = P * vec4(eye, 1.0);

    gl_Position = clip;
}
";

const WARP_DRAW_FS_SRC: &str = "#version 100

precision highp float;

void main() {
    gl_FragColor = vec4(1.0);
}
";

pub struct WarpEffect {
    capacity: u32,
    radius: f32,
    trail_length: f32,
    particles_init: WebGlBuffer,
    particles: [WebGlBuffer; WARP_EFFECT_FRAMES],
    frame: usize,
    line: Mesh,
    phys_program: Program,
    draw_program: Program,
}

impl WarpEffect {
    pub fn new(gl: &WebGl2RenderingContext, capacity: u32, radius: f32, trail_length: f32) -> Self {
        let particles_init = gl.create_buffer().unwrap();

        let mut particles: [MaybeUninit<WebGlBuffer>; WARP_EFFECT_FRAMES] = unsafe {
            MaybeUninit::zeroed().assume_init()
        };

        for dst in &mut particles {
            // existing buffers won't Drop if this panics; this is an unrecoverable failure, anyway
            let src = gl.create_buffer().unwrap();
            unsafe { ptr::write(dst.as_mut_ptr(), src) };
        };

        let particles = unsafe { mem::transmute::<_, [WebGlBuffer; WARP_EFFECT_FRAMES]>(particles) };

        let data = (0..capacity)
            .map(|_| {
                let mut rng = rand::thread_rng();
                loop {
                    let p = 2.0 * Point3::new(
                        rng.gen::<f32>() - 0.5,
                        rng.gen::<f32>() - 0.5,
                        rng.gen::<f32>() - 0.5);
                    if p.to_vec().magnitude2() <= 1.0 {
                        break p
                    }
                }
            })
            .map(|p| p * radius)
            .collect::<Vec<_>>();

        gl.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, Some(&particles_init));
        unsafe {
            let view = Uint8Array::view(get_byte_view(data.as_slice()));
            gl.buffer_data_with_array_buffer_view(
                WebGl2RenderingContext::ARRAY_BUFFER, &view,
                WebGl2RenderingContext::DYNAMIC_COPY);
        }

        gl.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, Some(&particles[0]));
        unsafe {
            let view = Uint8Array::view(get_byte_view(data.as_slice()));
            gl.buffer_data_with_array_buffer_view(
                WebGl2RenderingContext::ARRAY_BUFFER, &view,
                WebGl2RenderingContext::DYNAMIC_COPY);
        }

        for vbo in &particles[1..] {
            gl.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, Some(&vbo));
            gl.buffer_data_with_i32(WebGl2RenderingContext::ARRAY_BUFFER,
                (capacity as usize * mem::size_of::<Point3<f32>>()) as i32,
                WebGl2RenderingContext::DYNAMIC_COPY);
        }

        gl.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, None);

        const LINE_ATTRIBS: [VertexAttrib; 1] = [
            VertexAttrib {
                ident: "u",
                size: 1,
                type_: WebGlRenderingContext::FLOAT,
                stride: 4,
                ..VERTEX_ATTRIB_DEFAULT
            }];

        let data = [
            0f32,
            1f32,
        ];

        let line = Mesh{
            vertices: build_vbo(gl, &data).unwrap(),
            attributes: &LINE_ATTRIBS,
            draw_mode: WebGlRenderingContext::LINES,
            count: 2,
        };

        let phys_program = {
            let transform_feedback_varyings = js_sys::Array::new();
            transform_feedback_varyings.push(Into::<js_sys::JsString>::into("out_pos").as_ref());
            let vs = build_shader(gl, WebGlRenderingContext::VERTEX_SHADER  , WARP_PHYS_VS_SRC).unwrap();
            let fs = build_shader(gl, WebGlRenderingContext::FRAGMENT_SHADER, WARP_PHYS_FS_SRC).unwrap();
            let program = gl.create_program().unwrap();
            gl.attach_shader(&program, &vs);
            gl.transform_feedback_varyings(&program,
                &transform_feedback_varyings.dyn_into::<wasm_bindgen::JsValue>().unwrap(),
                WebGl2RenderingContext::INTERLEAVED_ATTRIBS);
            gl.attach_shader(&program, &fs);
            link_program(gl, &program).ok().unwrap();
            Program::wrap(gl, program)
        };
        let draw_program = Program::from_source(gl, WARP_DRAW_VS_SRC, WARP_DRAW_FS_SRC).unwrap();

        WarpEffect {
            capacity,
            radius,
            trail_length,
            particles_init,
            particles,
            frame: 0,
            line,
            phys_program,
            draw_program,
        }
    }

    pub fn draw(&mut self,
        gl: &WebGl2RenderingContext,
        view_matrix: &Matrix4<f32>,
        projection_matrix: &Matrix4<f32>,
        vel: Vector3<f32>, dt: f32)
    {
        let n = {
            let u = ((vel.magnitude() - WARP_UPS_MIN) / (WARP_UPS_MAX - WARP_UPS_MIN)).min(1.0).max(0.0);
            let n = (self.capacity as f32 * u * u) as i32;
            if n < 1 {
                return;
            }
            std::cmp::min(n, self.capacity as i32)
        };

        let i0 = self.frame;
        let i1 = (i0 + 1) % WARP_EFFECT_FRAMES;
        self.frame = i1;

        self.phys_program.use_program(gl);

        self.phys_program.set_uniform(gl, "motion", &ConstantValue::Vector3(-vel * dt));
        self.phys_program.set_uniform(gl, "radius", &ConstantValue::Float(self.radius));

        gl.bind_buffer(
            WebGlRenderingContext::ARRAY_BUFFER,
            Some(&self.particles_init));

        self.phys_program.assign_vertex_attribs(gl, 
            &[
                VertexAttrib {
                    ident: "in_pos_0",
                    size: 3,
                    type_: WebGlRenderingContext::FLOAT,
                    stride: 12,
                    ..VERTEX_ATTRIB_DEFAULT
                }
            ]
        );

        gl.bind_buffer(
            WebGlRenderingContext::ARRAY_BUFFER,
            Some(&self.particles[i0]));

        self.phys_program.assign_vertex_attribs(gl, 
            &[
                VertexAttrib {
                    ident: "in_pos",
                    size: 3,
                    type_: WebGlRenderingContext::FLOAT,
                    stride: 12,
                    ..VERTEX_ATTRIB_DEFAULT
                }
            ]
        );

        gl.bind_buffer(
            WebGlRenderingContext::ARRAY_BUFFER,
            None);

        gl.bind_buffer_base(
            WebGl2RenderingContext::TRANSFORM_FEEDBACK_BUFFER,
            0, Some(&self.particles[i1]));

        gl.enable(WebGl2RenderingContext::RASTERIZER_DISCARD);
        gl.begin_transform_feedback(WebGl2RenderingContext::POINTS);
        gl.draw_arrays(WebGl2RenderingContext::POINTS, 0, n);
        gl.end_transform_feedback();
        gl.disable(WebGl2RenderingContext::RASTERIZER_DISCARD);

        gl.bind_buffer_base(
            WebGl2RenderingContext::TRANSFORM_FEEDBACK_BUFFER,
            0, None);

        self.phys_program.clear_vertex_attribs(gl);

        gl.enable(WebGl2RenderingContext::BLEND);
        gl.blend_func(
                WebGlRenderingContext::ONE_MINUS_DST_COLOR,
                WebGlRenderingContext::ZERO);

        let instances = InstanceData{
                buffer: self.particles[i1].clone(),
                attributes: &[
                    VertexAttrib {
                        ident: "pos",
                        size: 3,
                        type_: WebGlRenderingContext::FLOAT,
                        stride: 12,
                        divisor: 1,
                        ..VERTEX_ATTRIB_DEFAULT
                    }],
                count: n,
            };

        draw_pass(gl, &self.draw_program, &[
                ("trail", Constant::Uniform(ConstantValue::Vector3(vel * self.trail_length))),
                ("V"    , Constant::Uniform(ConstantValue::Matrix4(*view_matrix))),
                ("P"    , Constant::Uniform(ConstantValue::Matrix4(*projection_matrix))),
            ], vec![
                (&[], self.line.clone(), Some(&instances)),
            ]);

        gl.disable(WebGl2RenderingContext::BLEND);
    }
}