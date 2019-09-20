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

extern crate cgmath;
extern crate console_error_panic_hook;
extern crate js_sys;
extern crate rand;
extern crate wasm_bindgen;
extern crate web_sys;

use cgmath::prelude::*;
use wasm_bindgen::prelude::*;

use cgmath::{
    Deg,
    Matrix4,
    PerspectiveFov,
    Point2,
    Rad,
    Vector2,
};
use wasm_bindgen::JsCast;
use web_sys::{
    Element,
    KeyboardEvent,
    MouseEvent,
    WebGlRenderingContext,
    WebGl2RenderingContext,
};

#[wasm_bindgen]
extern {
    #[wasm_bindgen(js_namespace = console)]
    fn log(_: &str);
    #[wasm_bindgen(js_namespace = console)]
    fn warn(_: &str);
    #[wasm_bindgen(js_namespace = console)]
    fn error(_: &str);
}

mod env;
mod gl_context;
mod gfx;
mod input;
mod player;
mod ai;
mod ui;

use ai::StrafeBot;
use env::{Environment, Runway};
use gl_context::AnyGlContext;
use gfx::{
    draw_pass,
    gen_hud_quad,
    Mesh,
    Program,
    UniformValue,
    WarpEffect,
};
use input::KeyState;
use player::{
    Kinematics,
    Movement,
    MOVE_HYBRID,
    MOVE_QW_LIKE,
    MOVE_VQ3_LIKE,
    PlayerState,
    PLAYER_RADIUS,
};
use ui::{get_ui, UI};

use std::cell::RefCell;
use std::rc::Rc;


const UNITS_PER_MILE: f32 = 12.0 * 5280.0;
const UNITS_PER_KM: f32 = 39370.1;
const MPH_PER_UPS: f32 = 3600.0 / UNITS_PER_MILE;
const KPH_PER_UPS: f32 = 3600.0 / UNITS_PER_KM;

enum TutorialStage {
    Intro,
    Mouse,
    Keyboard,
    Hopping,
    Moving,
    Turning
}

fn show(element: &Element) {
    element.class_list().remove_1("strafe_hidden")
        .expect("failed to add strafe_hidden css class");
}

fn hide(element: &Element) {
    element.class_list().add_1("strafe_hidden")
        .expect("failed to add strafe_hidden css class");
}

const MAIN_VS_SRC: &str = "#version 100

attribute vec3 pos;
attribute vec3 norm;
attribute vec2 uv;

varying vec3 f_eye;
varying vec3 f_norm;
varying vec2 f_uv;

uniform mat4 M;
uniform mat4 V;
uniform mat4 P;

void main() {
    vec4 world = M * vec4(pos, 1.0);
    vec4 eye = V * world;
    vec4 clip = P * eye;

    f_eye = eye.xyz/eye.w;
    f_norm = norm;
    f_uv = uv;
    gl_Position = clip;
}
";

const MAIN_FS_SRC: &str = "#version 100

precision highp float;

varying vec3 f_eye;
varying vec3 f_norm;
varying vec2 f_uv;

uniform mat4 M;
uniform vec4 fog_color;

vec3 to_srgb(vec3 x) {
    return mix(12.92 * x, 1.055 * pow(x, vec3(1.0/2.4)) - 0.055, step(0.0031308, x));
}

void main() {
    vec3 norm = normalize(mat3(M) * f_norm);

    vec3 color;
    vec2 uv = mod(floor(f_uv), 2.0);
    if (uv.s != uv.t) {
        color = vec3(0.03, 0.03, 0.03);
    } else {
        color = vec3(0.01, 0.01, 0.01);
    }

    vec3 light = normalize(vec3(1.0, 2.0, 3.0));
    color *= 0.5 * dot(norm, light) + 0.5;

    vec4 fog = fog_color;
    fog.a = 1.0 - exp(-fog.a * length(f_eye));

    // at least fog will be gamma-correct...
    color = to_srgb(mix(color, fog.rgb, fog.a));

    gl_FragColor = vec4(color, 1.0);
}
";

const HUD_VS_SRC: &str = "#version 100

uniform float fov;
uniform vec2 wish_dir;

attribute vec2 pos;

varying vec2 target_dir;
varying float x_axis;

void main() {
    gl_Position = vec4(pos, 0.0, 1.0);
    float theta = -pos.x * fov / 2.0;
    float r_cos = cos(theta);
    float r_sin = sin(theta);
    mat2 R = mat2(
         r_cos, r_sin,
        -r_sin, r_cos);
    target_dir = R * wish_dir;
    x_axis = pos.x;
}
";

const HUD_FS_SRC: &str = "#version 100

precision highp float;

uniform vec2 move_dir;
uniform float warp_factor;

varying vec2 target_dir;
varying float x_axis;

void main() {
    vec2 dir = normalize(target_dir);
    float accel = 0.0;
    if (warp_factor * dot(dir, move_dir) < 0.999) {
        accel = dot(dir, move_dir);
    }
    vec4 target_color = vec4(0.0);
    vec4 cursor_color = vec4(0.0);
    if (accel <= 0.0) {
        target_color = vec4(1.0, 0.0, 0.0, -accel);
    } else {
        target_color = vec4(0.0, 1.0, 0.0, accel);
        cursor_color.rgb = vec3(1.0);
    }
    target_color.a *= smoothstep(1.0, 1.1, warp_factor);
    cursor_color.a = smoothstep(0.998, 0.999, 1.0 - abs(x_axis));

    gl_FragColor = mix(target_color, vec4(cursor_color.rgb, 1.0), cursor_color.a);
}
";

struct Application {
    ui: UI,
    gl: AnyGlContext,
    stage: Option<TutorialStage>,
    perspective: PerspectiveFov::<f32>,
    player_state: PlayerState,
    kinematics: Kinematics,
    strafe_bot: Option<StrafeBot>,
    menu_shown: bool,
    have_pointer: bool,
    input_rotation: (Rad<f32>, Rad<f32>),
    mouse_scale: Rad<f32>,
    key_state: KeyState,
    key_history: KeyState,
    last_frame_us: u32,
    tick_remainder_s: f32,
    framerate: f32,
    environment: Box<Environment>,
    warp_effect: Option<WarpEffect>,
    main_program: Program,
    hud_program: Program,
    hud_mesh: Mesh,
}

impl Application {
    fn from_ui(ui: UI) -> Self {
        let gl = ui.canvas.get_context("webgl2")
            .ok()
            .and_then(|gl| gl)
            .map(|gl| {
                AnyGlContext::Gl2(gl.dyn_into::<WebGl2RenderingContext>().unwrap())
            })
            .or_else(||
                ui.canvas.get_context("webgl")
                    .ok()
                    .and_then(|gl| gl)
                    .map(|gl| {
                        AnyGlContext::Gl1(gl.dyn_into::<WebGlRenderingContext>().unwrap())
                    }))
            .expect("failed to get webgl context");

        let warp_effect = if let AnyGlContext::Gl2(gl) = &gl {
            Some(WarpEffect::new(gl, 25000, 1000.0, 1.0/120.0))
        } else {
            None
        };

        let environment = Box::new(Runway::from_dimensions(gl.gl(), 16384.0, 2048.0)
            .expect("failed to create environment"));

        let main_program = Program::from_source(gl.gl(), MAIN_VS_SRC, MAIN_FS_SRC)
            .expect("failed to build main shader program");

        let hud_program = Program::from_source(gl.gl(), HUD_VS_SRC, HUD_FS_SRC)
            .expect("failed to build HUD shader program");

        let hud_mesh = gen_hud_quad(gl.gl(),
            Point2::new(-1.0, -0.0125),
            Point2::new( 1.0,  0.0125))
            .expect("failed to build box VBO");

        let mut app = Application{
            ui, gl,
            stage: None,
            perspective: PerspectiveFov::<f32>{
                fovy: Deg(80.0).into(),
                aspect: 1.0,
                near: 0.1 * PLAYER_RADIUS,
                far: 100000.0,
            },
            player_state: PlayerState::default(),
            kinematics: MOVE_VQ3_LIKE,
            strafe_bot: None,
            menu_shown: true,
            have_pointer: false,
            input_rotation: (Rad::zero(), Rad::zero()),
            mouse_scale: Rad(0.001),
            key_state: KeyState::default(),
            key_history: KeyState::default(),
            last_frame_us: 0,
            tick_remainder_s: 0.0,
            framerate: 0.0,
            environment,
            warp_effect,
            main_program,
            hud_program,
            hud_mesh,
        };

        app.update_movement_display();

        app
    }

    fn set_stage(&mut self, stage: Option<TutorialStage>) {
        self.stage = stage;
        match self.stage {
            Some(_) => {
                show(self.ui.menu_continue.dyn_ref::<Element>().unwrap());
                hide(self.ui.menu_tutorial.dyn_ref::<Element>().unwrap());
                show(self.ui.menu_practice.dyn_ref::<Element>().unwrap());
            },
            None => {
                show(self.ui.menu_continue.dyn_ref::<Element>().unwrap());
                show(self.ui.menu_tutorial.dyn_ref::<Element>().unwrap());
                hide(self.ui.menu_practice.dyn_ref::<Element>().unwrap());
            },
        }
    }

    fn show_menu(&mut self) {
        show(&self.ui.menu);
        self.menu_shown = true;
    }

    fn hide_menu(&mut self) {
        hide(&self.ui.menu);
        self.menu_shown = false;
    }

    fn update_movement_display(&mut self) {
        self.ui.move_gravity     .set_value_as_number(self.kinematics.gravity               as f64);
        self.ui.move_jump_impulse.set_value_as_number(self.kinematics.jump_impulse          as f64);
        self.ui.move_stall_speed .set_value_as_number(self.kinematics.friction.stall_speed  as f64);
        self.ui.move_friction    .set_value_as_number(self.kinematics.friction.friction     as f64);
        self.ui.move_ground_speed.set_value_as_number(self.kinematics.move_ground.max_speed as f64);
        self.ui.move_ground_accel.set_value_as_number(self.kinematics.move_ground.accel     as f64);
        self.ui.move_air_speed   .set_value_as_number(self.kinematics.move_air.max_speed    as f64);
        self.ui.move_air_accel   .set_value_as_number(self.kinematics.move_air.accel        as f64);
        if let Some(move_air_turning) = self.kinematics.move_air_turning {
            self.ui.move_turn_enabled.set_checked(true);
            self.ui.move_turn_speed  .set_disabled(false);
            self.ui.move_turn_accel  .set_disabled(false);
            self.ui.move_turn_speed  .set_value_as_number(move_air_turning.max_speed as f64);
            self.ui.move_turn_accel  .set_value_as_number(move_air_turning.accel     as f64);
        } else {
            self.ui.move_turn_enabled.set_checked(false);
            self.ui.move_turn_speed  .set_disabled(true);
            self.ui.move_turn_accel  .set_disabled(true);
            self.ui.move_turn_speed  .set_value("");
            self.ui.move_turn_accel  .set_value("");
        }
    }

    fn validate_movement(&mut self) {
        fn validate(value: &mut f32, default: f32) {
            if !value.is_finite() {
                *value = default;
            }
        }
        validate(&mut self.kinematics.gravity              , MOVE_VQ3_LIKE.gravity              );
        validate(&mut self.kinematics.jump_impulse         , MOVE_VQ3_LIKE.jump_impulse         );
        validate(&mut self.kinematics.friction.stall_speed , MOVE_VQ3_LIKE.friction.stall_speed );
        validate(&mut self.kinematics.friction.friction    , MOVE_VQ3_LIKE.friction.friction    );
        validate(&mut self.kinematics.move_ground.max_speed, MOVE_VQ3_LIKE.move_ground.max_speed);
        validate(&mut self.kinematics.move_ground.accel    , MOVE_VQ3_LIKE.move_ground.accel    );
        validate(&mut self.kinematics.move_air.max_speed   , MOVE_VQ3_LIKE.move_air.max_speed   );
        validate(&mut self.kinematics.move_air.accel       , MOVE_VQ3_LIKE.move_air.accel       );
        if let Some(move_air_turning) = &mut self.kinematics.move_air_turning {
            validate(&mut move_air_turning.max_speed, self.kinematics.move_air.max_speed);
            validate(&mut move_air_turning.accel    , self.kinematics.move_air.accel    );
        }
    }

    fn update_movement_input(&mut self) {
        self.kinematics.gravity               = self.ui.move_gravity     .value_as_number() as f32;
        self.kinematics.jump_impulse          = self.ui.move_jump_impulse.value_as_number() as f32;
        self.kinematics.friction.stall_speed  = self.ui.move_stall_speed .value_as_number() as f32;
        self.kinematics.friction.friction     = self.ui.move_friction    .value_as_number() as f32;
        self.kinematics.move_ground.max_speed = self.ui.move_ground_speed.value_as_number() as f32;
        self.kinematics.move_ground.accel     = self.ui.move_ground_accel.value_as_number() as f32;
        self.kinematics.move_air.max_speed    = self.ui.move_air_speed   .value_as_number() as f32;
        self.kinematics.move_air.accel        = self.ui.move_air_accel   .value_as_number() as f32;
        if self.ui.move_turn_enabled.checked() {
            self.ui.move_turn_speed.set_disabled(false);
            self.ui.move_turn_accel.set_disabled(false);
            self.kinematics.move_air_turning = Some(Movement{
                max_speed: self.ui.move_turn_speed.value_as_number() as f32,
                accel    : self.ui.move_turn_accel.value_as_number() as f32,
            });
        } else {
            self.ui.move_turn_speed.set_disabled(true);
            self.ui.move_turn_accel.set_disabled(true);
            self.kinematics.move_air_turning = None;
        }
        self.validate_movement();
        self.update_movement_display();
    }

    fn setup_events(app: Rc<RefCell<Self>>) {
        {
            let w = app.borrow().ui.canvas.client_width ();
            let h = app.borrow().ui.canvas.client_height();
            app.borrow_mut().perspective.aspect = (w as f32)/(h as f32);
        }

        let resize_cb = {
            let app = app.clone();
            let resize = move || {
                let (w, h) = {
                    let canvas = &app.borrow().ui.canvas;
                    let w = canvas.client_width ();
                    let h = canvas.client_height();
                    canvas.set_width (w as u32);
                    canvas.set_height(h as u32);
                    (w, h)
                };
                app.borrow().gl.gl().viewport(0, 0, w, h);
                app.borrow_mut().perspective.aspect = (w as f32)/(h as f32);
            };
            resize();
            Closure::wrap(Box::new(resize) as Box<dyn FnMut()>)
        };

        let fullscreen_cb = {
            let app = app.clone();
            Closure::wrap(Box::new(move || {
                let document = app.borrow().ui.document.clone();
                let root_node = app.borrow().ui.root_node.clone().dyn_into::<Element>().unwrap();
                let is_fullscreen = document.fullscreen_element() == Some(root_node.clone());
                if !is_fullscreen {
                    app.borrow_mut().show_menu();
                    document.exit_pointer_lock();
                } else {
                    let _ = root_node.request_pointer_lock();
                }
            }) as Box<dyn FnMut()>)
        };

        let pointer_lock_cb = {
            let app = app.clone();
            Closure::wrap(Box::new(move || {
                let document = app.borrow().ui.document.clone();
                let root_node = app.borrow().ui.root_node.clone().dyn_into::<Element>().unwrap();
                app.borrow_mut().have_pointer = document.pointer_lock_element() == Some(root_node.clone());
            }) as Box<dyn FnMut()>)
        };

        let mouse_move_cb = {
            let app = app.clone();
            Closure::wrap(Box::new(move |event: MouseEvent| {
                let have_pointer = app.borrow().have_pointer;
                let menu_shown = app.borrow().menu_shown;
                if have_pointer && !menu_shown {
                    let scale = app.borrow().mouse_scale;
                    app.borrow_mut().input_rotation.0 -= scale * (event.movement_x() as f32);
                    app.borrow_mut().input_rotation.1 -= scale * (event.movement_y() as f32);
                }
            }) as Box<dyn FnMut(_)>)
        };

        let key_down_cb = {
            let app = app.clone();
            Closure::wrap(Box::new(move |event: KeyboardEvent| {
                let key_state = &mut app.borrow_mut().key_state;
                match event.code().as_str() {
                    "KeyW" => key_state.key_w = true,
                    "KeyA" => key_state.key_a = true,
                    "KeyS" => key_state.key_s = true,
                    "KeyD" => key_state.key_d = true,
                    "KeyF" => key_state.key_f = true,
                    "Space" => key_state.space = true,
                    _ => {},
                }
            }) as Box<dyn FnMut(_)>)
        };

        let key_up_cb = {
            let app = app.clone();
            Closure::wrap(Box::new(move |event: KeyboardEvent| {
                let key_state = &mut app.borrow_mut().key_state;
                match event.code().as_str() {
                    "KeyW" => key_state.key_w = false,
                    "KeyA" => key_state.key_a = false,
                    "KeyS" => key_state.key_s = false,
                    "KeyD" => key_state.key_d = false,
                    "KeyF" => key_state.key_f = false,
                    "Space" => key_state.space = false,
                    _ => {},
                }
            }) as Box<dyn FnMut(_)>)
        };

        app.borrow().ui.window.add_event_listener_with_callback("resize",
            resize_cb.as_ref().dyn_ref().unwrap())
            .expect("failed to add resize event listener");

        app.borrow().ui.document.add_event_listener_with_callback("fullscreenchange",
            fullscreen_cb.as_ref().dyn_ref().unwrap())
            .expect("failed to add fullscreenchange event listener");

        app.borrow().ui.document.add_event_listener_with_callback("pointerlockchange",
            pointer_lock_cb.as_ref().dyn_ref().unwrap())
            .expect("failed to add pointerlockchange event listener");

        app.borrow().ui.document.add_event_listener_with_callback("mousemove",
            mouse_move_cb.as_ref().dyn_ref().unwrap())
            .expect("failed to add mousemove event listener");

        app.borrow().ui.document.add_event_listener_with_callback("keydown",
            key_down_cb.as_ref().dyn_ref().unwrap())
            .expect("failed to add keydown event listener");

        app.borrow().ui.document.add_event_listener_with_callback("keyup",
            key_up_cb.as_ref().dyn_ref().unwrap())
            .expect("failed to add keyup event listener");

        let continue_cb = {
            let app = app.clone();
            Closure::wrap(Box::new(move || {
                let root_node = app.borrow().ui.root_node.clone();
                app.borrow_mut().hide_menu();
                let _ = root_node.request_fullscreen();
                let _ = root_node.request_pointer_lock();
            }) as Box<dyn FnMut()>)
        };

        app.borrow().ui.menu_continue.add_event_listener_with_callback("click",
            continue_cb.as_ref().dyn_ref().unwrap())
            .expect("failed to add menu_continue click listener");

        let tutorial_cb = {
            let app = app.clone();
            Closure::wrap(Box::new(move || {
                let root_node = app.borrow().ui.root_node.clone();
                app.borrow_mut().set_stage(Some(TutorialStage::Intro));
                app.borrow_mut().hide_menu();
                let _ = root_node.request_fullscreen();
                let _ = root_node.request_pointer_lock();
            }) as Box<dyn FnMut()>)
        };

        app.borrow().ui.menu_tutorial.add_event_listener_with_callback("click",
            tutorial_cb.as_ref().dyn_ref().unwrap())
            .expect("failed to add menu_tutorial click listener");

        let practice_cb = {
            let app = app.clone();
            Closure::wrap(Box::new(move || {
                let root_node = app.borrow().ui.root_node.clone();
                app.borrow_mut().set_stage(None);
                app.borrow_mut().hide_menu();
                let _ = root_node.request_fullscreen();
                let _ = root_node.request_pointer_lock();
            }) as Box<dyn FnMut()>)
        };

        app.borrow().ui.menu_practice.add_event_listener_with_callback("click",
            practice_cb.as_ref().dyn_ref().unwrap())
            .expect("failed to add menu_practice click listener");

        let gen_move_preset_cb = |kinematics: Kinematics| {
            let app = app.clone();
            Closure::wrap(Box::new(move || {
                app.borrow_mut().kinematics = kinematics.clone();
                app.borrow_mut().update_movement_display();
            }) as Box<dyn FnMut()>)
        };

        let move_vq3_like_cb = gen_move_preset_cb(MOVE_VQ3_LIKE);
        let move_qw_like_cb = gen_move_preset_cb(MOVE_QW_LIKE);
        let move_hybrid_cb = gen_move_preset_cb(MOVE_HYBRID);

        app.borrow().ui.move_vq3_like.add_event_listener_with_callback("click",
            move_vq3_like_cb.as_ref().dyn_ref().unwrap())
            .expect("failed to add move_vq3_like click listener");

        app.borrow().ui.move_qw_like.add_event_listener_with_callback("click",
            move_qw_like_cb.as_ref().dyn_ref().unwrap())
            .expect("failed to add move_qw_like click listener");

        app.borrow().ui.move_hybrid.add_event_listener_with_callback("click",
            move_hybrid_cb.as_ref().dyn_ref().unwrap())
            .expect("failed to add move_hybrid click listener");

        let update_movement_cb = {
            let app = app.clone();
            Closure::wrap(Box::new(move || {
                app.borrow_mut().update_movement_input();
            }) as Box<dyn FnMut()>)
        };

        [
            &app.borrow().ui.move_gravity     ,
            &app.borrow().ui.move_jump_impulse,
            &app.borrow().ui.move_stall_speed ,
            &app.borrow().ui.move_friction    ,
            &app.borrow().ui.move_ground_speed,
            &app.borrow().ui.move_ground_accel,
            &app.borrow().ui.move_air_speed   ,
            &app.borrow().ui.move_air_accel   ,
            &app.borrow().ui.move_turn_enabled,
            &app.borrow().ui.move_turn_speed  ,
            &app.borrow().ui.move_turn_accel  ,
        ].iter().for_each(|element| {
            element.add_event_listener_with_callback("change",
                update_movement_cb.as_ref().dyn_ref().unwrap())
                .expect("failed to add movement value change listener");
        });

        // stop tracking these so they stay around for the lifetime of the app
        resize_cb.forget();
        fullscreen_cb.forget();
        pointer_lock_cb.forget();
        mouse_move_cb.forget();
        key_down_cb.forget();
        key_up_cb.forget();
        continue_cb.forget();
        tutorial_cb.forget();
        practice_cb.forget();
        move_vq3_like_cb.forget();
        move_qw_like_cb.forget();
        move_hybrid_cb.forget();
        update_movement_cb.forget();
    }

    fn tick_sim(&mut self, dt: f32) {
        let u = dt / self.tick_remainder_s;
        let yaw   = self.input_rotation.0 * u;
        let pitch = self.input_rotation.1 * u;
        self.input_rotation.0 -= yaw;
        self.input_rotation.1 -= pitch;
        self.player_state.add_rotation(yaw, pitch);

        let is_jumping = self.key_state.space;
        let is_turning = self.key_state.is_side_strafe();

        let wish_dir = self.player_state.wish_dir(&self.key_state, Rad::zero(), Rad::zero());
        self.player_state.sim_kinematics(&self.kinematics, dt, wish_dir, is_jumping, is_turning);

        self.environment.interact(&mut self.player_state);

        self.tick_remainder_s -= dt;
    }

    fn draw_frame(&mut self) {
        let keys_pressed = self.key_state.rising_edge(self.key_history);
        self.key_history = self.key_state;

        {
            let c = self.environment.atmosphere_color().to_srgb();
            self.gl.gl().clear_color(c.r, c.g, c.b, 1.0);
        }
        self.gl.gl().clear(WebGlRenderingContext::COLOR_BUFFER_BIT | WebGlRenderingContext::DEPTH_BUFFER_BIT);

        const MAX_FRAME_DURATION_S: f32 = 0.2;
        const TICK_DURATION_S: f32 = 0.01;

        let current_frame_us = (1_000.0 * self.ui.window.performance().unwrap().now()) as u32;
        let frame_duration_s = (current_frame_us - self.last_frame_us) as f32 / 1_000_000.0;
        self.last_frame_us = current_frame_us;
        self.tick_remainder_s += frame_duration_s;

        if self.tick_remainder_s > MAX_FRAME_DURATION_S {
            warn("dropped below min framerate, slowing down");
            self.tick_remainder_s = MAX_FRAME_DURATION_S;
        }

        while self.tick_remainder_s > TICK_DURATION_S {
            self.tick_sim(TICK_DURATION_S);
        }

        let view_matrix = self.player_state.view_matrix(
            self.tick_remainder_s,
            self.input_rotation.0,
            self.input_rotation.1);
        let projection_matrix: Matrix4<f32> = self.perspective.into();

        {
            self.gl.gl().enable(WebGlRenderingContext::DEPTH_TEST);
            self.gl.gl().depth_func(WebGlRenderingContext::LESS);

            self.environment.draw(self.gl.gl(),
                &self.main_program,
                &view_matrix,
                &projection_matrix);

            if let Some(warp_effect) = &mut self.warp_effect {
                if let AnyGlContext::Gl2(gl) = &self.gl {
                    warp_effect.draw(gl, &view_matrix, &projection_matrix, self.player_state.vel, frame_duration_s);
                } else { panic!() }
            }

            self.gl.gl().disable(WebGlRenderingContext::DEPTH_TEST);
        }

        {
            let fovx = Rad::atan(self.perspective.aspect * (self.perspective.fovy / 2.0).tan()) * 2.0;
            let wish_dir = self.player_state.wish_dir(
                &self.key_state,
                self.input_rotation.0,
                self.input_rotation.1).xy();
            let velocity_xy = self.player_state.vel.xy();
            let speed = velocity_xy.magnitude();
            let move_dir = if speed > 0.0001 { velocity_xy / speed } else { Vector2::zero() };
            let is_jumping = self.key_state.space;
            let is_grounded = self.player_state.is_grounded() && !is_jumping;
            let is_turning = self.key_state.is_side_strafe();
            let max_speed = self.kinematics.effective_movement(is_grounded, is_turning).max_speed;
            let warp_factor = speed / max_speed;

            self.gl.gl().enable(WebGlRenderingContext::BLEND);
            self.gl.gl().blend_func(
                WebGlRenderingContext::SRC_ALPHA,
                WebGlRenderingContext::ONE_MINUS_SRC_ALPHA);

            draw_pass(self.gl.gl(), &self.hud_program, &[
                ("fov"         , UniformValue::Float  (fovx.0     )),
                ("wish_dir"    , UniformValue::Vector2(wish_dir   )),
                ("move_dir"    , UniformValue::Vector2(move_dir   )),
                ("warp_factor" , UniformValue::Float  (warp_factor)),
            ], vec![
                (&[], self.hud_mesh.clone()),
            ]);

            self.gl.gl().disable(WebGlRenderingContext::BLEND);

            if let Some(strafe_bot) = &mut self.strafe_bot {
                if keys_pressed.key_f {
                    strafe_bot.take_off();
                }
                let pos_x = self.player_state.pos.x + self.player_state.vel.x * self.tick_remainder_s;
                let (mut theta, mut phi) = self.player_state.dir;
                theta += self.input_rotation.0;
                phi   += self.input_rotation.1;
                let (keys, theta, phi) = strafe_bot.sim(frame_duration_s,
                    velocity_xy, max_speed, pos_x, theta, phi, self.player_state.is_grounded());
                self.key_state.key_w = keys.key_w;
                self.key_state.key_a = keys.key_a;
                self.key_state.key_s = keys.key_s;
                self.key_state.key_d = keys.key_d;
                self.key_state.space = keys.space;
                self.input_rotation.0 += theta;
                self.input_rotation.1 += phi;
            } else {
                if keys_pressed.key_f {
                    self.strafe_bot = Some(StrafeBot::new());
                }
            }
        }

        {
            let speed_ups = self.player_state.vel.xy().magnitude();
            let speed_mph = speed_ups * MPH_PER_UPS;
            let speed_kph = speed_ups * KPH_PER_UPS;

            self.ui.speed_ups.dyn_ref::<web_sys::Node>().unwrap()
                .set_text_content(Some(format!("{:.1}UPS", speed_ups).as_str()));
            self.ui.speed_mph.dyn_ref::<web_sys::Node>().unwrap()
                .set_text_content(Some(format!("{:.1}MPH", speed_mph).as_str()));
            self.ui.speed_kph.dyn_ref::<web_sys::Node>().unwrap()
                .set_text_content(Some(format!("{:.1}KPH", speed_kph).as_str()));
        }

        if frame_duration_s > 0.000001 {
            let framerate = 1.0 / frame_duration_s;

            const DECAY: f32 = 0.05;

            self.framerate = (1.0 - DECAY) * self.framerate + framerate * DECAY;

            self.ui.framerate.dyn_ref::<web_sys::Node>().unwrap()
                .set_text_content(Some(format!("{:.0}Hz", self.framerate).as_str()));
        }
    }
}

#[wasm_bindgen]
pub fn strafe_main() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));

    let app = Rc::new(RefCell::new(Application::from_ui(get_ui())));
    Application::setup_events(app.clone());

    let animation_cb: Rc<RefCell<Option<Closure<_>>>> = Rc::new(RefCell::new(None));

    *animation_cb.borrow_mut() = Some({
        let app = app.clone();
        let animation_cb = animation_cb.clone();
        Closure::wrap(Box::new(move || {
            app.borrow_mut().draw_frame();
            app.borrow().ui.window.request_animation_frame(
                animation_cb
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .as_ref()
                    .dyn_ref()
                    .unwrap());
        }) as Box<dyn FnMut()>)
    });

    app.borrow().ui.window.request_animation_frame(
        animation_cb
            .borrow()
            .as_ref()
            .unwrap()
            .as_ref()
            .dyn_ref()
            .unwrap());
}