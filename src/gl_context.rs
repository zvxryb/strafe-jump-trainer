/*
 * Copyright 2019 Michael Lodato <zvxryb@gmail.com>
 * 
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to
 * deal in the Software without restriction, including without limitation the
 * rights to use, copy, modify, merge, publish, distribute, sublicense, and/or
 * sell copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 * 
 * The above copyright notice and this permission notice shall be included in
 * all copies or substantial portions of the Software.
 * 
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
 * FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS
 * IN THE SOFTWARE.
*/

use web_sys::{
    WebGlActiveInfo,
    WebGlBuffer,
    WebGlProgram,
    WebGlRenderingContext,
    WebGl2RenderingContext,
    WebGlShader,
    WebGlUniformLocation,
};

pub trait VersionedContext {
    fn webgl1(&self) -> Option<&WebGlRenderingContext>;
    fn webgl2(&self) -> Option<&WebGl2RenderingContext>;
}

macro_rules! impl_webgl_trait {
    ($trait_name:ident implementors {$($implementor:ident;)*} methods {$(fn $method:ident($($arg_name:ident: $arg_type:ty),*) -> $result_type:ty;)*}) => {
        impl_webgl_trait!{trait $trait_name {$(fn $method($($arg_name: $arg_type),*) -> $result_type;)*}}
        impl_webgl_trait!{impl $trait_name implementors {$($implementor;)*} methods {$(fn $method($($arg_name: $arg_type),*) -> $result_type;)*}}
    };
    (trait $trait_name:ident {$(fn $method:ident($($arg_name:ident: $arg_type:ty),*) -> $result_type:ty;)*}) => {
        pub trait $trait_name: VersionedContext {
            $(
                fn $method(&self, $($arg_name: $arg_type),*) -> $result_type;
            )*
        }
    };
    (impl $trait_name:ident implementors {$impl_head:ident; $($impl_tail:ident;)*} methods {$(fn $method:ident($($arg_name:ident: $arg_type:ty),*) -> $result_type:ty;)*}) => {
        impl $trait_name for $impl_head {
            $(
                fn $method(&self, $($arg_name: $arg_type),*) -> $result_type {
                    Self::$method(self, $($arg_name),*)
                }
            )*
        }
        impl_webgl_trait!{impl $trait_name implementors {$($impl_tail;)*} methods {$(fn $method($($arg_name: $arg_type),*) -> $result_type;)*}}
    };
    (impl $trait_name:ident implementors {} methods {$(fn $method:ident($($arg_name:ident: $arg_type:ty),*) -> $result_type:ty;)*}) => {};
}

impl_webgl_trait!{
    GlContext
    implementors {
        WebGlRenderingContext;
        WebGl2RenderingContext;
    }
    methods {
        fn attach_shader(program: &WebGlProgram, shader: &WebGlShader) -> ();
        fn bind_buffer(target: u32, buffer: Option<&WebGlBuffer>) -> ();
        fn blend_func(sfactor: u32, dfactor: u32) -> ();
        fn buffer_data_with_array_buffer_view(target: u32, src_data: &js_sys::Object, usage: u32) -> ();
        fn buffer_data_with_i32(target: u32, size: i32, usage: u32) -> ();
        fn clear(mask: u32) -> ();
        fn clear_color(red: f32, green: f32, blue: f32, alpha: f32) -> ();
        fn compile_shader(shader: &WebGlShader) -> ();
        fn create_buffer() -> Option<WebGlBuffer>;
        fn create_program() -> Option<WebGlProgram>;
        fn create_shader(type_: u32) -> Option<WebGlShader>;
        fn depth_func(func: u32) -> ();
        fn disable(cap: u32) -> ();
        fn disable_vertex_attrib_array(index: u32) -> ();
        fn draw_arrays(mode: u32, first: i32, count: i32) -> ();
        fn enable(cap: u32) -> ();
        fn enable_vertex_attrib_array(index: u32) -> ();
        fn get_active_attrib(program: &WebGlProgram, index: u32) -> Option<WebGlActiveInfo>;
        fn get_active_uniform(program: &WebGlProgram, index: u32) -> Option<WebGlActiveInfo>;
        fn get_attrib_location(program: &WebGlProgram, name: &str) -> i32;
        fn get_extension(name: &str) -> Result<Option<js_sys::Object>, wasm_bindgen::JsValue>;
        fn get_program_info_log(program: &WebGlProgram) -> Option<String>;
        fn get_program_parameter(program: &WebGlProgram, pname: u32) -> wasm_bindgen::JsValue;
        fn get_shader_info_log(shader: &WebGlShader) -> Option<String>;
        fn get_shader_parameter(shader: &WebGlShader, pname: u32) -> wasm_bindgen::JsValue;
        fn get_uniform_location(program: &WebGlProgram, name: &str) -> Option<WebGlUniformLocation>;
        fn link_program(program: &WebGlProgram) -> ();
        fn shader_source(shader: &WebGlShader, source: &str) -> ();
        fn uniform1f(location: Option<&WebGlUniformLocation>, x: f32) -> ();
        fn uniform2f(location: Option<&WebGlUniformLocation>, x: f32, y: f32) -> ();
        fn uniform3f(location: Option<&WebGlUniformLocation>, x: f32, y: f32, z: f32) -> ();
        fn uniform4f(location: Option<&WebGlUniformLocation>, x: f32, y: f32, z: f32, w: f32) -> ();
        fn uniform_matrix4fv_with_f32_array(location: Option<&WebGlUniformLocation>, transpose: bool, data: &[f32]) -> ();
        fn use_program(program: Option<&WebGlProgram>) -> ();
        fn viewport(x: i32, y: i32, width: i32, height: i32) -> ();
        fn vertex_attrib_pointer_with_i32(index: u32, size: i32, type_: u32, normalized: bool, stride: i32, offset: i32) -> ();
    }
}

impl VersionedContext for WebGlRenderingContext {
    fn webgl1(&self) -> Option<&WebGlRenderingContext> { Some(&self) }
    fn webgl2(&self) -> Option<&WebGl2RenderingContext> { None }
}

impl VersionedContext for WebGl2RenderingContext {
    fn webgl1(&self) -> Option<&WebGlRenderingContext> { None }
    fn webgl2(&self) -> Option<&WebGl2RenderingContext> { Some(&self) }
}

pub enum AnyGlContext {
    Gl1(WebGlRenderingContext),
    Gl2(WebGl2RenderingContext),
}

impl AnyGlContext {
    pub fn gl(&self) -> &GlContext {
        match self {
            AnyGlContext::Gl1(gl) => gl,
            AnyGlContext::Gl2(gl) => gl,
        }
    }
}

impl VersionedContext for AnyGlContext {
    fn webgl1(&self) -> Option<&WebGlRenderingContext> {
        if let AnyGlContext::Gl1(gl) = &self {
            Some(gl)
        } else {
            None
        }
    }

    fn webgl2(&self) -> Option<&WebGl2RenderingContext> {
        if let AnyGlContext::Gl2(gl) = &self {
            Some(gl)
        } else {
            None
        }
    }
}