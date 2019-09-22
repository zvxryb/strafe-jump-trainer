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

use wasm_bindgen::JsCast;
use web_sys::{
    Document,
    Element,
    HtmlButtonElement,
    HtmlCanvasElement,
    HtmlDivElement,
    HtmlElement,
    HtmlInputElement,
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
    pub menu_movement: HtmlElement,
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
}

pub fn get_ui() -> UI {
    let window = web_sys::window()
        .expect("failed to get window");

    let document = window.document()
        .expect("failed to get document");

    fn get_as<T: JsCast>(document: &Document, id: &str) -> T {
        document.get_element_by_id(id)
            .expect(format!("failed to get {}", id).as_str())
            .dyn_into::<T>()
            .expect(format!("failed to cast {}", id).as_str())
    }

    let root_node         = get_as::<HtmlDivElement   >(&document, "strafe_root");
    let canvas            = get_as::<HtmlCanvasElement>(&document, "strafe_canvas");
    let dialog            = get_as::<HtmlElement      >(&document, "strafe_dialog");
    let keys              = get_as::<Element          >(&document, "strafe_keys");
    let key_forward       = get_as::<Element          >(&document, "strafe_key_forward");
    let key_back          = get_as::<Element          >(&document, "strafe_key_back");
    let key_left          = get_as::<Element          >(&document, "strafe_key_left");
    let key_right         = get_as::<Element          >(&document, "strafe_key_right");
    let key_jump          = get_as::<Element          >(&document, "strafe_key_jump");
    let framerate         = get_as::<HtmlElement      >(&document, "strafe_framerate");
    let speed_ups         = get_as::<HtmlElement      >(&document, "strafe_speed_ups");
    let speed_mph         = get_as::<HtmlElement      >(&document, "strafe_speed_mph");
    let speed_kph         = get_as::<HtmlElement      >(&document, "strafe_speed_kph");
    let menu              = get_as::<HtmlDivElement   >(&document, "strafe_menu");
    let menu_continue     = get_as::<HtmlButtonElement>(&document, "strafe_menu_continue");
    let menu_tutorial     = get_as::<HtmlButtonElement>(&document, "strafe_menu_tutorial");
    let menu_practice     = get_as::<HtmlButtonElement>(&document, "strafe_menu_practice");
    let menu_movement     = get_as::<HtmlElement      >(&document, "strafe_menu_movement");
    let move_vq3_like     = get_as::<HtmlButtonElement>(&document, "strafe_move_vq3-like");
    let move_qw_like      = get_as::<HtmlButtonElement>(&document, "strafe_move_qw-like");
    let move_hybrid       = get_as::<HtmlButtonElement>(&document, "strafe_move_hybrid");
    let move_gravity      = get_as::<HtmlInputElement >(&document, "strafe_move_gravity");
    let move_jump_impulse = get_as::<HtmlInputElement >(&document, "strafe_move_jump_impulse");
    let move_stall_speed  = get_as::<HtmlInputElement >(&document, "strafe_move_stall_speed");
    let move_friction     = get_as::<HtmlInputElement >(&document, "strafe_move_friction");
    let move_ground_speed = get_as::<HtmlInputElement >(&document, "strafe_move_ground_speed");
    let move_ground_accel = get_as::<HtmlInputElement >(&document, "strafe_move_ground_accel");
    let move_air_speed    = get_as::<HtmlInputElement >(&document, "strafe_move_air_speed");
    let move_air_accel    = get_as::<HtmlInputElement >(&document, "strafe_move_air_accel");
    let move_turn_enabled = get_as::<HtmlInputElement >(&document, "strafe_move_turn_enabled");
    let move_turn_speed   = get_as::<HtmlInputElement >(&document, "strafe_move_turn_speed");
    let move_turn_accel   = get_as::<HtmlInputElement >(&document, "strafe_move_turn_accel");

    UI{
        window,
        document,
        root_node,
        canvas,
        dialog,
        keys,
        key_forward,
        key_back,
        key_left,
        key_right,
        key_jump,
        framerate,
        speed_ups,
        speed_mph,
        speed_kph,
        menu,
        menu_continue,
        menu_tutorial,
        menu_practice,
        menu_movement,
        move_vq3_like,
        move_qw_like,
        move_hybrid,
        move_gravity,
        move_jump_impulse,
        move_stall_speed,
        move_friction,
        move_ground_speed,
        move_ground_accel,
        move_air_speed,
        move_air_accel,
        move_turn_enabled,
        move_turn_speed,
        move_turn_accel,
    }
}