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

use crate::input::KeyCode;

use wasm_bindgen::JsCast;
use web_sys::{
    Document,
    Element,
    HtmlButtonElement,
    HtmlCanvasElement,
    HtmlDivElement,
    HtmlElement,
    HtmlInputElement,
    HtmlSelectElement,
    Window,
};

#[derive(Clone)]
pub struct UI {
    pub window: Window,
    pub document: Document,
    pub root_node: HtmlDivElement,
    pub canvas: HtmlCanvasElement,
    pub dialog: HtmlElement,
    pub keys: Element,
    pub key_forward: Element,
    pub key_back: Element,
    pub key_left: Element,
    pub key_right: Element,
    pub key_jump: Element,
    pub framerate: HtmlElement,
    pub speed_ups: HtmlElement,
    pub speed_mph: HtmlElement,
    pub speed_kph: HtmlElement,
    pub menu: HtmlDivElement,
    pub menu_continue: HtmlButtonElement,
    pub menu_tutorial: HtmlButtonElement,
    pub menu_practice: HtmlButtonElement,
    pub mouse_input: HtmlInputElement,
    pub mouse_display: Element,
    pub bind_forward : HtmlButtonElement,
    pub bind_left    : HtmlButtonElement,
    pub bind_back    : HtmlButtonElement,
    pub bind_right   : HtmlButtonElement,
    pub bind_jump    : HtmlButtonElement,
    pub bind_interact: HtmlButtonElement,
    pub practice_options: HtmlElement,
    pub map_runway: HtmlButtonElement,
    pub map_freestyle: HtmlButtonElement,
    pub move_vq3_like: HtmlButtonElement,
    pub move_qw_like: HtmlButtonElement,
    pub move_hybrid: HtmlButtonElement,
    pub move_gravity: HtmlInputElement,
    pub move_jump_impulse: HtmlInputElement,
    pub move_stall_speed: HtmlInputElement,
    pub move_friction: HtmlInputElement,
    pub move_ground_speed: HtmlInputElement,
    pub move_ground_accel: HtmlInputElement,
    pub move_air_speed: HtmlInputElement,
    pub move_air_accel: HtmlInputElement,
    pub move_turn_enabled: HtmlInputElement,
    pub move_turn_speed: HtmlInputElement,
    pub move_turn_accel: HtmlInputElement,
    pub menu_bot: HtmlElement,
    pub bot_mode: HtmlSelectElement,
    pub bot_hop: HtmlInputElement,
    pub bot_move: HtmlInputElement,
    pub bot_turn: HtmlInputElement,
}

impl UI {
    pub fn keybind_button(&self, key: KeyCode) -> &HtmlButtonElement {
        match key {
            KeyCode::KeyW  => &self.bind_forward,
            KeyCode::KeyA  => &self.bind_left,
            KeyCode::KeyS  => &self.bind_back,
            KeyCode::KeyD  => &self.bind_right,
            KeyCode::KeyF  => &self.bind_interact,
            KeyCode::Space => &self.bind_jump,
        }
    }
}

pub fn get_ui() -> UI {
    let window = web_sys::window()
        .expect("failed to get window");

    let document = window.document()
        .expect("failed to get document");

    fn get_as<T: JsCast>(document: &Document, id: &str) -> T {
        document.get_element_by_id(id)
            .unwrap_or_else(|| panic!("failed to get {}", id))
            .dyn_into::<T>()
            .unwrap_or_else(|_| panic!("failed to cast {}", id))
    }

    UI {
        window,
        document: document.clone(),
        root_node        : get_as::<HtmlDivElement   >(&document, "strafe_root"),
        canvas           : get_as::<HtmlCanvasElement>(&document, "strafe_canvas"),
        dialog           : get_as::<HtmlElement      >(&document, "strafe_dialog"),
        keys             : get_as::<Element          >(&document, "strafe_keys"),
        key_forward      : get_as::<Element          >(&document, "strafe_key_forward"),
        key_back         : get_as::<Element          >(&document, "strafe_key_back"),
        key_left         : get_as::<Element          >(&document, "strafe_key_left"),
        key_right        : get_as::<Element          >(&document, "strafe_key_right"),
        key_jump         : get_as::<Element          >(&document, "strafe_key_jump"),
        framerate        : get_as::<HtmlElement      >(&document, "strafe_framerate"),
        speed_ups        : get_as::<HtmlElement      >(&document, "strafe_speed_ups"),
        speed_mph        : get_as::<HtmlElement      >(&document, "strafe_speed_mph"),
        speed_kph        : get_as::<HtmlElement      >(&document, "strafe_speed_kph"),
        menu             : get_as::<HtmlDivElement   >(&document, "strafe_menu"),
        menu_continue    : get_as::<HtmlButtonElement>(&document, "strafe_menu_continue"),
        menu_tutorial    : get_as::<HtmlButtonElement>(&document, "strafe_menu_tutorial"),
        menu_practice    : get_as::<HtmlButtonElement>(&document, "strafe_menu_practice"),
        mouse_input      : get_as::<HtmlInputElement >(&document, "strafe_mouse_input"),
        mouse_display    : get_as::<Element          >(&document, "strafe_mouse_display"),
        bind_forward     : get_as::<HtmlButtonElement>(&document, "strafe_bind_forward"),
        bind_left        : get_as::<HtmlButtonElement>(&document, "strafe_bind_left"),
        bind_back        : get_as::<HtmlButtonElement>(&document, "strafe_bind_back"),
        bind_right       : get_as::<HtmlButtonElement>(&document, "strafe_bind_right"),
        bind_jump        : get_as::<HtmlButtonElement>(&document, "strafe_bind_jump"),
        bind_interact    : get_as::<HtmlButtonElement>(&document, "strafe_bind_interact"),
        practice_options : get_as::<HtmlElement      >(&document, "strafe_practice_options"),
        map_runway       : get_as::<HtmlButtonElement>(&document, "strafe_map_runway"),
        map_freestyle    : get_as::<HtmlButtonElement>(&document, "strafe_map_freestyle"),
        move_vq3_like    : get_as::<HtmlButtonElement>(&document, "strafe_move_vq3-like"),
        move_qw_like     : get_as::<HtmlButtonElement>(&document, "strafe_move_qw-like"),
        move_hybrid      : get_as::<HtmlButtonElement>(&document, "strafe_move_hybrid"),
        move_gravity     : get_as::<HtmlInputElement >(&document, "strafe_move_gravity"),
        move_jump_impulse: get_as::<HtmlInputElement >(&document, "strafe_move_jump_impulse"),
        move_stall_speed : get_as::<HtmlInputElement >(&document, "strafe_move_stall_speed"),
        move_friction    : get_as::<HtmlInputElement >(&document, "strafe_move_friction"),
        move_ground_speed: get_as::<HtmlInputElement >(&document, "strafe_move_ground_speed"),
        move_ground_accel: get_as::<HtmlInputElement >(&document, "strafe_move_ground_accel"),
        move_air_speed   : get_as::<HtmlInputElement >(&document, "strafe_move_air_speed"),
        move_air_accel   : get_as::<HtmlInputElement >(&document, "strafe_move_air_accel"),
        move_turn_enabled: get_as::<HtmlInputElement >(&document, "strafe_move_turn_enabled"),
        move_turn_speed  : get_as::<HtmlInputElement >(&document, "strafe_move_turn_speed"),
        move_turn_accel  : get_as::<HtmlInputElement >(&document, "strafe_move_turn_accel"),
        menu_bot         : get_as::<HtmlElement      >(&document, "strafe_menu_bot"),
        bot_mode         : get_as::<HtmlSelectElement>(&document, "strafe_bot_mode"),
        bot_hop          : get_as::<HtmlInputElement >(&document, "strafe_bot_hop"),
        bot_move         : get_as::<HtmlInputElement >(&document, "strafe_bot_move"),
        bot_turn         : get_as::<HtmlInputElement >(&document, "strafe_bot_turn"),
    }
}