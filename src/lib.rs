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
    Storage,
    WebGlRenderingContext,
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

mod collision;
mod env;
mod gl_context;
mod gfx;
mod input;
mod player;
mod ai;
mod ui;

use ai::{StrafeBot, StrafeConfig};
use env::{Map, Freestyle, Runway};
use gl_context::{AnyGlContext, GlVersionRequirement};
use gfx::{
    draw_pass,
    gen_hud_quad,
    Mesh,
    Program,
    Constant,
    ConstantValue,
    WarpEffect,
};
use input::{
    Button,
    KeyBinds,
    KeyCode,
    KeyState,
    MouseSettings,
};
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

#[derive(Copy, Clone)]
enum TimedStage {
    Waiting(f32),
    Done,
}

#[derive(Copy, Clone)]
enum SpeedStage {
    MaxSpeed(f32),
    Done,
}

#[derive(Copy, Clone)]
enum TutorialStage {
    Intro  (TimedStage),
    Observe(TimedStage),
    Hopping(SpeedStage),
    Moving (SpeedStage),
    Turning(SpeedStage),
}

impl TutorialStage {
    fn next(&mut self) -> Option<Self> {
        match self {
            TutorialStage::Intro  (..) => Some(TutorialStage::Observe(TimedStage::Waiting (0.0))),
            TutorialStage::Observe(..) => Some(TutorialStage::Hopping(SpeedStage::MaxSpeed(0.0))),
            TutorialStage::Hopping(..) => Some(TutorialStage::Moving (SpeedStage::MaxSpeed(0.0))),
            TutorialStage::Moving (..) => Some(TutorialStage::Turning(SpeedStage::MaxSpeed(0.0))),
            TutorialStage::Turning(..) => None,
        }
    }
}

#[derive(PartialEq, Copy, Clone)]
enum MapOption {
    Runway,
    Freestyle,
}

fn show(element: &Element) {
    element.class_list().remove_1("strafe_hidden")
        .expect("failed to add strafe_hidden css class");
}

fn hide(element: &Element) {
    element.class_list().add_1("strafe_hidden")
        .expect("failed to add strafe_hidden css class");
}

fn set_highlight(element: &Element, highlight: bool) {
    let classes = element.class_list();
    if highlight {
        classes.add_1("strafe_highlight")
    } else {
        classes.remove_1("strafe_highlight")
    }.expect("failed to add/remove strafe_highlight css class");
}

const MAIN_VS_SRC: &str = "#version 100

attribute vec3 pos;
attribute vec3 norm;
attribute vec2 uv;
attribute mat4 M_instance;

varying vec3 f_eye;
varying vec3 f_norm;
varying vec2 f_uv;

uniform mat4 M_group;
uniform mat4 V;
uniform mat4 P;

void main() {
    mat4 M = M_group * M_instance;
    vec4 world = M * vec4(pos, 1.0);
    vec4 eye = V * world;
    vec4 clip = P * eye;

    f_eye = eye.xyz/eye.w;
    f_norm = mat3(M) * norm;
    f_uv = uv;
    gl_Position = clip;
}
";

const MAIN_FS_SRC: &str = "#version 100

precision highp float;

varying vec3 f_eye;
varying vec3 f_norm;
varying vec2 f_uv;

uniform vec4 fog_color;

vec3 to_srgb(vec3 x) {
    return mix(12.92 * x, 1.055 * pow(x, vec3(1.0/2.4)) - 0.055, step(0.0031308, x));
}

void main() {
    vec3 norm = normalize(f_norm);

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
    fog.a *= length(f_eye);
    fog.a *= fog.a;
    fog.a  = 1.0 - exp(-fog.a);

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
    storage: Option<Storage>,
    stage: Option<TutorialStage>,
    perspective: PerspectiveFov::<f32>,
    player_state: PlayerState,
    kinematics: Kinematics,
    strafe_bot: Option<StrafeBot>,
    auto_hop : bool,
    auto_move: bool,
    auto_turn: bool,
    menu_shown: bool,
    have_pointer: bool,
    input_rotation: (Rad<f32>, Rad<f32>),
    mouse_settings:  MouseSettings,
    key_binds:       KeyBinds,
    key_selected:    Option<KeyCode>,
    key_state:       KeyState,
    key_history:     KeyState,
    input_key_state: KeyState,
    bot_key_state:   KeyState,
    bot_key_history: KeyState,
    last_frame_us: u32,
    tick_remainder_s: f32,
    framerate: f32,
    map_option: MapOption,
    map: Box<Map>,
    warp_effect: Option<WarpEffect>,
    main_program: Program,
    hud_program: Program,
    hud_mesh: Mesh,
}

impl Application {
    fn from_ui(ui: UI) -> Self {
        let storage = ui.window.local_storage()
            .ok()
            .and_then(|storage| storage);

        let key_binds = storage.as_ref()
            .map(|storage| { KeyBinds::load(storage, "key_binds").ok() })
            .and_then(|key_binds| key_binds)
            .unwrap_or_default();

        let mouse_settings = storage.as_ref()
            .map(|storage| { MouseSettings::load(storage, "mouse_settings").ok() })
            .and_then(|mouse_settings| mouse_settings)
            .unwrap_or_default();

        ui.mouse_flip_x.set_checked(mouse_settings.flip_x);
        ui.mouse_flip_y.set_checked(mouse_settings.flip_y);

        let gl = AnyGlContext::from_canvas(&ui.canvas,
            GlVersionRequirement::Any)
            .expect("failed to get WebGL context");

        match &gl {
            AnyGlContext::Gl1(_) => {
                warn("running in WebGL 1.0 fallback mode; this may be slow");
            }
            AnyGlContext::Gl2(_) => {
                log("successfully obtained WebGL 2.0 context");
            }
        }

        let warp_effect = if let AnyGlContext::Gl2(gl) = &gl {
            Some(WarpEffect::new(gl, 25000, 1000.0, 1.0/120.0))
        } else {
            None
        };

        let map = Box::new(Runway::new(gl.gl()));

        let main_program = Program::from_source(gl.gl(), MAIN_VS_SRC, MAIN_FS_SRC)
            .expect("failed to build main shader program");

        let hud_program = Program::from_source(gl.gl(), HUD_VS_SRC, HUD_FS_SRC)
            .expect("failed to build HUD shader program");

        let hud_mesh = gen_hud_quad(gl.gl(),
            Point2::new(-1.0, -0.0125),
            Point2::new( 1.0,  0.0125))
            .expect("failed to build box VBO");

        let mut app = Application{
            ui, gl, storage,
            stage: None,
            perspective: PerspectiveFov::<f32>{
                fovy: Deg(80.0).into(),
                aspect: 1.0,
                near: 0.1 * PLAYER_RADIUS,
                far: 100_000.0,
            },
            player_state: PlayerState::default(),
            kinematics: MOVE_VQ3_LIKE,
            strafe_bot: Some(StrafeBot::new(StrafeConfig::STANDARD)),
            auto_hop : true,
            auto_move: true,
            auto_turn: true,
            menu_shown: true,
            have_pointer: false,
            input_rotation: (Rad::zero(), Rad::zero()),
            mouse_settings,
            key_binds,
            key_selected:    None,
            key_state:       KeyState::default(),
            key_history:     KeyState::default(),
            input_key_state: KeyState::default(),
            bot_key_state:   KeyState::default(),
            bot_key_history: KeyState::default(),
            last_frame_us: 0,
            tick_remainder_s: 0.0,
            framerate: 0.0,
            map_option: MapOption::Runway,
            map,
            warp_effect,
            main_program,
            hud_program,
            hud_mesh,
        };

        app.update_mouse_sensitivity();
        app.update_key_binds();
        app.update_movement_display();
        app.update_bot_display();

        app
    }

    fn set_stage(&mut self, stage: Option<TutorialStage>) {
        self.stage = stage;
        self.player_state.reset();
        let dialog = &mut self.ui.dialog.dyn_ref::<web_sys::Node>().unwrap();
        match self.stage {
            Some(stage) => {
                match stage {
                    TutorialStage::Intro(..) => {
                        dialog.set_text_content(Some("\
                            Welcome to Strafe Jump Trainer!\n\n\
                            Strafe jumping is an advanced movement technique involving skillful, \
                            coordinated motions to accelerate beyond typical in-game speed limits.  \
                            Many first-person game engines share similar lineage and support some \
                            variation of these techniques.  When the engine checks the players' \
                            intended direction of motion against the current velocity, it creates an \
                            effective \"dead zone\" \u{2014} a region where the player is not allowed \
                            to continue acceleration.  If the player maintains a movement direction \
                            which is just outside this region, acceleration is allowed to continue.\n\n\
                            This interactive tutorial will guide you through the different mechanics \
                            required for successful strafe jumping."));
                        self.strafe_bot = None;
                        self.update_bot_display();
                        hide(&self.ui.keys);
                    }
                    TutorialStage::Observe(..) => {
                        dialog.set_text_content(Some("\
                            Meet Strafe Bot.\n\n\
                            As Strafe Bot strafes, observe how he:\n\
                            1. Begins with a rapid turn before his first jump to gain ground speed\n\
                            2. Repeatedly jumps to maintain speed\n\
                            3. Alternates between left-forward and right-forward motion keys\n\
                            4. Keeps his cursor within the green part of the strafe HUD\n\n\
                            When done correctly, the cursor lights up to indicate acceleration"));
                        self.strafe_bot = Some(StrafeBot::new(StrafeConfig::STANDARD));
                        self.auto_hop  = true;
                        self.auto_move = true;
                        self.auto_turn = true;
                        self.update_bot_display();
                        show(&self.ui.keys);
                    }
                    TutorialStage::Hopping(..) => {
                        dialog.set_text_content(Some("\
                            We'll begin with hopping practice.  Strafe bot will continue to handle basic \
                            motion, but you will have to press SPACE as indicated on the HUD.\n\n\
                            The first motion is referred to as a \"circle-jump\", which is a quick turn \
                            while grounded to gain maximum ground acceleration before jumping.  This works \
                            because ground acceleration is generally higher than air acceleration, but is \
                            limited to just over 400UPS by friction.  Once maximum ground speed is reached \
                            this speed is maintained by repeatedly hopping.\n\n\
                            Reach 1000 UPS to continue."));
                        self.strafe_bot = Some(StrafeBot::new(StrafeConfig::STANDARD));
                        self.auto_hop  = false;
                        self.auto_move = true;
                        self.auto_turn = true;
                        self.update_bot_display();
                        show(&self.ui.keys);
                    }
                    TutorialStage::Moving(..) => {
                        dialog.set_text_content(Some("\
                            Next, let's practice movement keys.  This time you'll have control over \
                            W/A/S/D, exclusively.  Simply follow along with the HUD indicators.\n\n\
                            While a player can strafe using any movement key or pair of movement keys \
                            at any time, the choice of key(s) determines which direction a player faces \
                            for a chosen direction of motion.  It is possible to strafe backwards, sideways, \
                            etc.  W/A and W/D are the most common strafe keys enabling a player to travel in \
                            a mostly-forward direction.\n\n\
                            Reach 1000 UPS to continue."));
                        self.strafe_bot = Some(StrafeBot::new(StrafeConfig::STANDARD));
                        self.auto_hop  = true;
                        self.auto_move = false;
                        self.auto_turn = true;
                        self.update_bot_display();
                        show(&self.ui.keys);
                    }
                    TutorialStage::Turning(..) => {
                        dialog.set_text_content(Some("\
                            Finally, mouse motion.  This is arguably the most important element of \
                            strafe jumping, as this angle, in combination with your movement keys, determines \
                            your intended direction of travel, which must be precisely controlled to gain speed.\n\n\
                            Begin with a quick turn to the left while grounded (i.e. perform a circle jump motion), \
                            then, once airborne, use gradual motion to keep your cursor in the green area of the \
                            HUD, while Strafe Bot handles movement.  Keep the cursor lit up for as much time as \
                            possible.\n\n\
                            Reach 1000 UPS to complete tutorial."));
                        self.strafe_bot = Some(StrafeBot::new(StrafeConfig::STANDARD));
                        self.auto_hop  = true;
                        self.auto_move = true;
                        self.auto_turn = false;
                        self.update_bot_display();
                        show(&self.ui.keys);
                    }
                };
                self.set_map(MapOption::Runway);
                show(self.ui.menu_continue   .dyn_ref::<Element>().unwrap());
                hide(self.ui.menu_tutorial   .dyn_ref::<Element>().unwrap());
                show(self.ui.menu_practice   .dyn_ref::<Element>().unwrap());
                hide(self.ui.practice_options.dyn_ref::<Element>().unwrap());
                hide(self.ui.menu_bot        .dyn_ref::<Element>().unwrap());
            },
            None => {
                dialog.set_text_content(None);
                self.strafe_bot = None;
                self.auto_hop  = false;
                self.auto_move = false;
                self.auto_turn = false;
                self.update_bot_display();
                show(self.ui.menu_continue   .dyn_ref::<Element>().unwrap());
                show(self.ui.menu_tutorial   .dyn_ref::<Element>().unwrap());
                hide(self.ui.menu_practice   .dyn_ref::<Element>().unwrap());
                show(self.ui.practice_options.dyn_ref::<Element>().unwrap());
                show(self.ui.menu_bot        .dyn_ref::<Element>().unwrap());
                hide(&self.ui.keys);
            },
        };
    }

    fn update_key_bind_text(&self, target: KeyCode) {
        let is_selected = match self.key_selected {
            Some(selected) => target == selected,
            None => false,
        };
        let text = if is_selected {
             "Press any button".to_string()
        } else {
             format!("{}", self.key_binds.button(target))
        };
        self.ui.keybind_button(target)
            .dyn_ref::<web_sys::Node>().unwrap()
            .set_text_content(
                Some(text.as_str()));
    }

    fn update_key_binds(&self) {
        [
            KeyCode::KeyW,
            KeyCode::KeyA,
            KeyCode::KeyS,
            KeyCode::KeyD,
            KeyCode::KeyF,
            KeyCode::Space,
        ].iter().for_each(|&target| {
            self.update_key_bind_text(target)
        });
    }

    fn input_button(&mut self, button: Button, pressed: bool) {
        if let Some(target) = self.key_selected {
            if !pressed {
                self.key_binds.rebind(target, button);
                self.key_selected = None;
                self.update_key_binds();
                if let Some(storage) = &self.storage {
                    if self.key_binds.save(storage, "key_binds").is_err() {
                        error("failed to save key binds");
                    }
                } else {
                    warn("cannot save key binds; no local_storage");
                }
            }
        } else {
            self.input_key_state.set_mapped(&self.key_binds, button, pressed);
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

    fn save_mouse_settings(&self) {
        if let Some(storage) = &self.storage {
            if self.mouse_settings.save(storage, "mouse_settings").is_err() {
                error("failed to save mouse settings");
            }
        } else {
            warn("cannot save mouse settings; no local_storage");
        }
    }

    fn update_mouse_sensitivity(&mut self) {
        let sense = self.mouse_settings.scale;
        self.ui.mouse_input.set_value_as_number(f64::from(sense.0.log2()));
        self.ui.mouse_display.dyn_ref::<web_sys::Node>().unwrap().set_text_content(
            Some(format!("{:.0} counts/rotation", Rad::<f32>::full_turn() / sense).as_str()));
    }

    fn set_map(&mut self, map: MapOption) {
        if self.map_option == map { return; }
        self.map_option = map;
        self.map = match map {
            MapOption::Runway    => Box::new(Runway   ::new(self.gl.gl())),
            MapOption::Freestyle => Box::new(Freestyle::new(self.gl.gl())),
        };
        if map == MapOption::Runway && self.stage.is_none() {
            show(self.ui.menu_bot.dyn_ref::<Element>().unwrap());
        } else {
            hide(self.ui.menu_bot.dyn_ref::<Element>().unwrap());
        }
    }

    fn update_movement_display(&mut self) {
        self.ui.move_gravity     .set_value_as_number(f64::from(self.kinematics.gravity              ));
        self.ui.move_jump_impulse.set_value_as_number(f64::from(self.kinematics.jump_impulse         ));
        self.ui.move_stall_speed .set_value_as_number(f64::from(self.kinematics.friction.stall_speed ));
        self.ui.move_friction    .set_value_as_number(f64::from(self.kinematics.friction.friction    ));
        self.ui.move_ground_speed.set_value_as_number(f64::from(self.kinematics.move_ground.max_speed));
        self.ui.move_ground_accel.set_value_as_number(f64::from(self.kinematics.move_ground.accel    ));
        self.ui.move_air_speed   .set_value_as_number(f64::from(self.kinematics.move_air.max_speed   ));
        self.ui.move_air_accel   .set_value_as_number(f64::from(self.kinematics.move_air.accel       ));
        if let Some(move_air_turning) = self.kinematics.move_air_turning {
            self.ui.move_turn_enabled.set_checked(true);
            self.ui.move_turn_speed  .set_disabled(false);
            self.ui.move_turn_accel  .set_disabled(false);
            self.ui.move_turn_speed  .set_value_as_number(f64::from(move_air_turning.max_speed));
            self.ui.move_turn_accel  .set_value_as_number(f64::from(move_air_turning.accel    ));
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

    fn update_bot_display(&mut self) {
        self.ui.bot_mode.set_value(match self.strafe_bot {
            Some(StrafeBot{config: StrafeConfig::STANDARD       , ..}) => "standard",
            Some(StrafeBot{config: StrafeConfig::REVERSE        , ..}) => "reverse",
            Some(StrafeBot{config: StrafeConfig::HALF_BEAT_LEFT , ..}) => "half-beat-left",
            Some(StrafeBot{config: StrafeConfig::HALF_BEAT_RIGHT, ..}) => "half-beat-right",
            Some(_) => "unspecified",
            None => "disabled",
        });
        if self.strafe_bot.is_some() {
            self.ui.bot_hop .set_checked(self.auto_hop);
            self.ui.bot_move.set_checked(self.auto_move);
            self.ui.bot_turn.set_checked(self.auto_turn);
        } else {
            self.ui.bot_hop .set_checked(false);
            self.ui.bot_move.set_checked(false);
            self.ui.bot_turn.set_checked(false);
            self.ui.bot_hop .set_disabled(true);
            self.ui.bot_move.set_disabled(true);
            self.ui.bot_turn.set_disabled(true);
        }
    }

    fn update_bot_input(&mut self) {
        fn update_config(bot: &mut Option<StrafeBot>, config: StrafeConfig) {
            if let Some(bot) = bot {
                bot.config = config;
            } else {
                *bot = Some(StrafeBot::new(config));
            }
        };
        match self.ui.bot_mode.value().as_str() {
            "standard"       => update_config(&mut self.strafe_bot, StrafeConfig::STANDARD),
            "reverse"        => update_config(&mut self.strafe_bot, StrafeConfig::REVERSE),
            "half-beat-left" => update_config(&mut self.strafe_bot, StrafeConfig::HALF_BEAT_LEFT),
            "half-beat-right"=> update_config(&mut self.strafe_bot, StrafeConfig::HALF_BEAT_RIGHT),
            "disabled"       => { self.strafe_bot = None },
            _ => {},
        }
        if self.strafe_bot.is_some() {
            self.auto_hop  = self.ui.bot_hop .checked();
            self.auto_move = self.ui.bot_move.checked();
            self.auto_turn = self.ui.bot_turn.checked();
            self.ui.bot_hop .set_disabled(false);
            self.ui.bot_move.set_disabled(false);
            self.ui.bot_turn.set_disabled(false);
            show(&self.ui.keys);
        } else {
            self.auto_hop  = false;
            self.auto_move = false;
            self.auto_turn = false;
            self.ui.bot_hop .set_disabled(true);
            self.ui.bot_move.set_disabled(true);
            self.ui.bot_turn.set_disabled(true);
            hide(&self.ui.keys);
        }
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
                    root_node.request_pointer_lock();
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
                let override_turning = app.borrow().override_turning();
                if have_pointer && !menu_shown && !override_turning {
                    let settings = app.borrow().mouse_settings;
                    let flip_x = if settings.flip_x { -1.0 } else { 1.0 };
                    let flip_y = if settings.flip_y { -1.0 } else { 1.0 };
                    app.borrow_mut().input_rotation.0 -= settings.scale * (event.movement_x() as f32) * flip_x;
                    app.borrow_mut().input_rotation.1 -= settings.scale * (event.movement_y() as f32) * flip_y;
                }
            }) as Box<dyn FnMut(_)>)
        };

        let key_down_cb = {
            let app = app.clone();
            Closure::wrap(Box::new(move |event: KeyboardEvent| {
                app.borrow_mut().input_button(Button::Key(event.code()), true);
            }) as Box<dyn FnMut(_)>)
        };

        let key_up_cb = {
            let app = app.clone();
            Closure::wrap(Box::new(move |event: KeyboardEvent| {
                app.borrow_mut().input_button(Button::Key(event.code()), false);
            }) as Box<dyn FnMut(_)>)
        };

        let mouse_down_cb = {
            let app = app.clone();
            Closure::wrap(Box::new(move |event: MouseEvent| {
                app.borrow_mut().input_button(Button::Mouse(event.button()), true);
            }) as Box<dyn FnMut(_)>)
        };

        let mouse_up_cb = {
            let app = app.clone();
            Closure::wrap(Box::new(move |event: MouseEvent| {
                app.borrow_mut().input_button(Button::Mouse(event.button()), false);
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

        app.borrow().ui.document.add_event_listener_with_callback("mousedown",
            mouse_down_cb.as_ref().dyn_ref().unwrap())
            .expect("failed to add mousedown event listener");

        app.borrow().ui.document.add_event_listener_with_callback("mouseup",
            mouse_up_cb.as_ref().dyn_ref().unwrap())
            .expect("failed to add mouseup event listener");

        let continue_cb = {
            let app = app.clone();
            Closure::wrap(Box::new(move || {
                let root_node = app.borrow().ui.root_node.clone();
                app.borrow_mut().hide_menu();
                let _ = root_node.request_fullscreen();
                root_node.request_pointer_lock();
            }) as Box<dyn FnMut()>)
        };

        app.borrow().ui.menu_continue.add_event_listener_with_callback("click",
            continue_cb.as_ref().dyn_ref().unwrap())
            .expect("failed to add menu_continue click listener");

        let tutorial_cb = {
            let app = app.clone();
            Closure::wrap(Box::new(move || {
                let root_node = app.borrow().ui.root_node.clone();
                app.borrow_mut().set_stage(Some(TutorialStage::Intro(TimedStage::Waiting(0.0))));
                app.borrow_mut().hide_menu();
                let _ = root_node.request_fullscreen();
                root_node.request_pointer_lock();
            }) as Box<dyn FnMut()>)
        };

        app.borrow().ui.menu_tutorial.add_event_listener_with_callback("click",
            tutorial_cb.as_ref().dyn_ref().unwrap())
            .expect("failed to add menu_tutorial click listener");

        let practice_cb = {
            let app = app.clone();
            Closure::wrap(Box::new(move || {
                app.borrow_mut().set_stage(None);
            }) as Box<dyn FnMut()>)
        };

        app.borrow().ui.menu_practice.add_event_listener_with_callback("click",
            practice_cb.as_ref().dyn_ref().unwrap())
            .expect("failed to add menu_practice click listener");

        let mouse_sense_cb = {
            let app = app.clone();
            Closure::wrap(Box::new(move || {
                let log2_sense = app.borrow().ui.mouse_input.value_as_number() as f32;
                app.borrow_mut().mouse_settings.scale = Rad::<f32>(log2_sense.exp2());
                app.borrow_mut().update_mouse_sensitivity();
                app.borrow().save_mouse_settings();
            }) as Box<dyn FnMut()>)
        };

        app.borrow().ui.mouse_input.add_event_listener_with_callback("input",
            mouse_sense_cb.as_ref().dyn_ref().unwrap())
            .expect("failed to add mouse_input input listener");

        let mouse_flip_x_cb = {
            let app = app.clone();
            Closure::wrap(Box::new(move || {
                let flip = app.borrow().ui.mouse_flip_x.checked();
                app.borrow_mut().mouse_settings.flip_x = flip;
                app.borrow().save_mouse_settings();
            }) as Box<dyn FnMut()>)
        };

        app.borrow().ui.mouse_flip_x.add_event_listener_with_callback("change",
            mouse_flip_x_cb.as_ref().dyn_ref().unwrap())
            .expect("failed to add mouse_flip_x change listener");

        let mouse_flip_y_cb = {
            let app = app.clone();
            Closure::wrap(Box::new(move || {
                let flip = app.borrow().ui.mouse_flip_y.checked();
                app.borrow_mut().mouse_settings.flip_y = flip;
                app.borrow().save_mouse_settings();
            }) as Box<dyn FnMut()>)
        };

        app.borrow().ui.mouse_flip_y.add_event_listener_with_callback("change",
            mouse_flip_y_cb.as_ref().dyn_ref().unwrap())
            .expect("failed to add mouse_flip_y change listener");

        [
            KeyCode::KeyW,
            KeyCode::KeyA,
            KeyCode::KeyS,
            KeyCode::KeyD,
            KeyCode::KeyF,
            KeyCode::Space,
        ].iter().for_each(|&target| {
            let callback = {
                let app = app.clone();
                Closure::wrap(Box::new(move || {
                    app.borrow().ui.keybind_button(target)
                        .dyn_ref::<web_sys::Node>().unwrap()
                        .set_text_content(Some("Press any button"));
                    app.borrow_mut().key_selected = Some(target);
                }) as Box<dyn FnMut()>)
            };
            app.borrow().ui.keybind_button(target)
                .add_event_listener_with_callback("click",
                    callback.as_ref().dyn_ref().unwrap())
                .expect("failed to add keybind click listener");
            callback.forget();
        });

        let gen_map_cb = |map: MapOption| {
            let app = app.clone();
            Closure::wrap(Box::new(move || {
                app.borrow_mut().set_map(map);
            }) as Box<dyn FnMut()>)
        };

        let map_runway_cb = gen_map_cb(MapOption::Runway);
        let map_freestyle_cb = gen_map_cb(MapOption::Freestyle);

        app.borrow().ui.map_runway.add_event_listener_with_callback("click",
            map_runway_cb.as_ref().dyn_ref().unwrap())
            .expect("failed to add map_runway click listener");

        app.borrow().ui.map_freestyle.add_event_listener_with_callback("click",
            map_freestyle_cb.as_ref().dyn_ref().unwrap())
            .expect("failed to add map_freestyle click listener");

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

        let update_bot_cb = {
            let app = app.clone();
            Closure::wrap(Box::new(move || {
                app.borrow_mut().update_bot_input();
            }) as Box<dyn FnMut()>)
        };

        [
            &app.borrow().ui.menu_bot,
            &app.borrow().ui.bot_hop,
            &app.borrow().ui.bot_move,
            &app.borrow().ui.bot_turn,
        ].iter().for_each(|element| {
            element.add_event_listener_with_callback("change",
                update_bot_cb.as_ref().dyn_ref().unwrap())
                .expect("failed to add movement value change listener");
        });

        // stop tracking these so they stay around for the lifetime of the app
        resize_cb.forget();
        fullscreen_cb.forget();
        pointer_lock_cb.forget();
        mouse_move_cb.forget();
        key_down_cb.forget();
        key_up_cb.forget();
        mouse_down_cb.forget();
        mouse_up_cb.forget();
        continue_cb.forget();
        tutorial_cb.forget();
        practice_cb.forget();
        mouse_sense_cb.forget();
        mouse_flip_x_cb.forget();
        mouse_flip_y_cb.forget();
        map_runway_cb.forget();
        map_freestyle_cb.forget();
        move_vq3_like_cb.forget();
        move_qw_like_cb.forget();
        move_hybrid_cb.forget();
        update_movement_cb.forget();
        update_bot_cb.forget();
    }

    fn override_hopping(&self) -> bool { self.strafe_bot.as_ref().map_or(false, |bot| self.auto_hop  || bot.is_setting_up()) }
    fn override_moving (&self) -> bool { self.strafe_bot.as_ref().map_or(false, |bot| self.auto_move || bot.is_setting_up()) }
    fn override_turning(&self) -> bool { self.strafe_bot.as_ref().map_or(false, |bot| self.auto_turn || bot.is_setting_up()) }

    fn tick_sim(&mut self, dt: f32) {
        let u = dt / self.tick_remainder_s;
        let yaw   = self.input_rotation.0 * u;
        let pitch = self.input_rotation.1 * u;
        self.input_rotation.0 -= yaw;
        self.input_rotation.1 -= pitch;
        self.player_state.add_rotation(yaw, pitch);

        let is_jumping = self.key_state.space;
        let is_turning = self.key_state.is_side_strafe();

        let wish_dir = self.player_state.wish_dir(self.key_state, Rad::zero(), Rad::zero());
        self.player_state.sim_kinematics(&self.kinematics, dt, wish_dir, is_jumping, is_turning);

        self.map.interact(&mut self.player_state);

        self.tick_remainder_s -= dt;
    }

    fn update_tutorial(&mut self, dt: f32, ground_speed: f32, action_pressed: bool) {
        let next_stage = if let Some(stage) = &mut self.stage {
            let (is_ready, was_ready) = match stage {
                TutorialStage::Intro  (status) |
                TutorialStage::Observe(status) =>
                {
                    let (is_ready, was_ready) = if let TimedStage::Waiting(time) = status {
                        *time += dt;
                        (*time > 5.0, false)
                    } else {
                        (true, true)
                    };

                    if is_ready && !was_ready {
                        *status = TimedStage::Done;
                    }

                    (is_ready, was_ready)
                }
                TutorialStage::Hopping(status) |
                TutorialStage::Moving (status) |
                TutorialStage::Turning(status) =>
                {
                    let (is_ready, was_ready) = if let SpeedStage::MaxSpeed(speed) = status {
                        *speed = speed.max(ground_speed);
                        (*speed > 1000.0, false)
                    } else {
                        (true, true)
                    };

                    if is_ready && !was_ready {
                        *status = SpeedStage::Done;
                    }

                    (is_ready, was_ready)
                }
            };

            if is_ready && !was_ready {
                let dialog = &mut self.ui.dialog.dyn_ref::<web_sys::Node>().unwrap();
                let mut text = dialog.text_content().unwrap_or_default();
                let prompt = if let TutorialStage::Turning(..) = stage {
                    "\n\nPress \"F\" to conclude tutorial."
                } else {
                    "\n\nPress \"F\" to proceed."
                };
                text.push_str(prompt);
                dialog.set_text_content(Some(text.as_str()));
            }

            if was_ready && action_pressed {
                Some(stage.next())
            } else {
                None
            }
        } else {
            None
        };

        if let Some(next_stage) = next_stage {
            self.set_stage(next_stage);
        }
    }

    fn update_keys(&mut self) -> KeyState {
        self.key_state = self.input_key_state;

        if self.override_moving() {
            self.key_state.key_w = self.bot_key_state.key_w;
            self.key_state.key_a = self.bot_key_state.key_a;
            self.key_state.key_s = self.bot_key_state.key_s;
            self.key_state.key_d = self.bot_key_state.key_d;
        }

        if self.override_hopping() {
            self.key_state.space = self.bot_key_state.space;
        }

        let keys_pressed = self.key_state.pressed(self.key_history);
        self.key_history = self.key_state;
        keys_pressed
    }

    fn draw_frame(&mut self) {
        let keys_pressed = self.update_keys();

        {
            let c = self.map.atmosphere_color().to_srgb();
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

            self.gl.gl().enable(WebGlRenderingContext::CULL_FACE);
            self.gl.gl().cull_face(WebGlRenderingContext::BACK);

            self.map.draw(self.gl.gl(),
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

        let is_jumping = self.key_state.space;
        let is_grounded = self.player_state.is_grounded() && !is_jumping;
        let is_turning = self.key_state.is_side_strafe();
        let max_speed = self.kinematics.effective_movement(is_grounded, is_turning).max_speed;

        {
            let fovx = Rad::atan(self.perspective.aspect * (self.perspective.fovy / 2.0).tan()) * 2.0;
            let wish_dir = self.player_state.wish_dir(
                self.key_state,
                self.input_rotation.0,
                self.input_rotation.1).xy();
            let velocity_xy = self.player_state.vel.xy();
            let speed = velocity_xy.magnitude();
            let move_dir = if speed > 0.0001 { velocity_xy / speed } else { Vector2::zero() };
            let warp_factor = speed / max_speed;

            self.gl.gl().enable(WebGlRenderingContext::BLEND);
            self.gl.gl().blend_func(
                WebGlRenderingContext::SRC_ALPHA,
                WebGlRenderingContext::ONE_MINUS_SRC_ALPHA);

            draw_pass(self.gl.gl(), &self.hud_program, &[
                ("fov"         , Constant::Uniform(ConstantValue::Float  (fovx.0     ))),
                ("wish_dir"    , Constant::Uniform(ConstantValue::Vector2(wish_dir   ))),
                ("move_dir"    , Constant::Uniform(ConstantValue::Vector2(move_dir   ))),
                ("warp_factor" , Constant::Uniform(ConstantValue::Float  (warp_factor))),
            ], vec![
                (&[], self.hud_mesh.clone(), None),
            ]);

            self.gl.gl().disable(WebGlRenderingContext::BLEND);
        }

        if let Some(strafe_bot) = &mut self.strafe_bot {
            let (keys, theta, phi) = strafe_bot.sim(frame_duration_s,
                &self.player_state, self.key_state, max_speed, self.input_rotation.0, self.input_rotation.1);
            self.bot_key_history = self.bot_key_state;
            self.bot_key_state   = keys;

            let pressed  = self.bot_key_state.pressed (self.bot_key_history);
            let released = self.bot_key_state.released(self.bot_key_history);

            if pressed.key_w { set_highlight(&self.ui.key_forward, true); }
            if pressed.key_a { set_highlight(&self.ui.key_left   , true); }
            if pressed.key_s { set_highlight(&self.ui.key_back   , true); }
            if pressed.key_d { set_highlight(&self.ui.key_right  , true); }
            if pressed.space { set_highlight(&self.ui.key_jump   , true); }

            if released.key_w { set_highlight(&self.ui.key_forward, false); }
            if released.key_a { set_highlight(&self.ui.key_left   , false); }
            if released.key_s { set_highlight(&self.ui.key_back   , false); }
            if released.key_d { set_highlight(&self.ui.key_right  , false); }
            if released.space { set_highlight(&self.ui.key_jump   , false); }

            if self.override_turning() {
                self.input_rotation.0 += theta;
                self.input_rotation.1 += phi;
            }
        }

        {
            let ground_speed = self.player_state.vel.xy().magnitude();
            self.update_tutorial(frame_duration_s, ground_speed, keys_pressed.key_f);
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

        if frame_duration_s > 0.000_001 {
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
                    .unwrap())
                .unwrap_or_else(|_| panic!("failed to request animation frame"));
        }) as Box<dyn FnMut()>)
    });

    app.borrow().ui.window.request_animation_frame(
        animation_cb
            .borrow()
            .as_ref()
            .unwrap()
            .as_ref()
            .dyn_ref()
            .unwrap())
        .unwrap_or_else(|_| panic!("failed to request animation frame"));
}