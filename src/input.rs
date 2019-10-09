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

use std::fmt;
use std::ops;

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum KeyCode {
    KeyW,
    KeyA,
    KeyS,
    KeyD,
    KeyF,
    Space,
}

#[derive(Copy, Clone, Default, Eq, PartialEq)]
pub struct KeyState {
    pub key_w: bool,
    pub key_a: bool,
    pub key_s: bool,
    pub key_d: bool,
    pub key_f: bool,
    pub space: bool,
}

pub const KEYS_DEFAULT: KeyState = KeyState{
    key_w: false,
    key_a: false,
    key_s: false,
    key_d: false,
    key_f: false,
    space: false,
};

impl KeyState {
    pub fn is_side_strafe(self) -> bool {
         (self.key_a || self.key_d) &&
        !(self.key_w || self.key_s)
    }

    pub fn pressed(self, previous: KeyState) -> KeyState {
        self & !previous
    }

    pub fn released(self, previous: KeyState) -> KeyState {
        !self & previous
    }

    pub fn set_mapped(&mut self, binds: &KeyBinds, button: Button, pressed: bool) {
        if binds.key_w == button { self.key_w = pressed; }
        if binds.key_a == button { self.key_a = pressed; }
        if binds.key_s == button { self.key_s = pressed; }
        if binds.key_d == button { self.key_d = pressed; }
        if binds.key_f == button { self.key_f = pressed; }
        if binds.space == button { self.space = pressed; }
    }
}

impl ops::Not for KeyState {
    type Output = KeyState;
    fn not(self) -> KeyState {
        KeyState{
            key_w: !self.key_w,
            key_a: !self.key_a,
            key_s: !self.key_s,
            key_d: !self.key_d,
            key_f: !self.key_f,
            space: !self.space,
        }
    }
}

impl ops::BitAnd for KeyState {
    type Output = KeyState;
    fn bitand(self, other: KeyState) -> KeyState {
        KeyState{
            key_w: self.key_w & other.key_w,
            key_a: self.key_a & other.key_a,
            key_s: self.key_s & other.key_s,
            key_d: self.key_d & other.key_d,
            key_f: self.key_f & other.key_f,
            space: self.space & other.space,
        }
    }
}

impl ops::BitOr for KeyState {
    type Output = KeyState;
    fn bitor(self, other: KeyState) -> KeyState {
        KeyState{
            key_w: self.key_w | other.key_w,
            key_a: self.key_a | other.key_a,
            key_s: self.key_s | other.key_s,
            key_d: self.key_d | other.key_d,
            key_f: self.key_f | other.key_f,
            space: self.space | other.space,
        }
    }
}

#[derive(PartialEq)]
pub enum Button {
    Key(String),
    Mouse(u64),
}

impl fmt::Display for Button {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Button::Key  (code ) => { write!(f, "{}", code) }
            Button::Mouse(index) => { write!(f, "Mouse{}", index) }
        }
    }
}

pub struct KeyBinds {
    pub key_w: Button,
    pub key_a: Button,
    pub key_s: Button,
    pub key_d: Button,
    pub key_f: Button,
    pub space: Button,
}

impl KeyBinds {
    pub fn button(&self, target: KeyCode) -> &Button {
        match target {
            KeyCode::KeyW  => &self.key_w,
            KeyCode::KeyA  => &self.key_a,
            KeyCode::KeyS  => &self.key_s,
            KeyCode::KeyD  => &self.key_d,
            KeyCode::KeyF  => &self.key_f,
            KeyCode::Space => &self.space,
        }
    }

    pub fn rebind(&mut self, target: KeyCode, button: Button) {
        let target = match target {
            KeyCode::KeyW  => &mut self.key_w,
            KeyCode::KeyA  => &mut self.key_a,
            KeyCode::KeyS  => &mut self.key_s,
            KeyCode::KeyD  => &mut self.key_d,
            KeyCode::KeyF  => &mut self.key_f,
            KeyCode::Space => &mut self.space,
        };
        *target = button;
    }
}

impl Default for KeyBinds {
    fn default() -> Self {
        Self{
            key_w: Button::Key("KeyW" .to_string()),
            key_a: Button::Key("KeyA" .to_string()),
            key_s: Button::Key("KeyS" .to_string()),
            key_d: Button::Key("KeyD" .to_string()),
            key_f: Button::Key("KeyF" .to_string()),
            space: Button::Key("Space".to_string()),
        }
    }
}