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

use std::ops;

#[derive(Copy, Clone, Default)]
pub struct KeyState {
    pub key_w: bool,
    pub key_a: bool,
    pub key_s: bool,
    pub key_d: bool,
    pub key_f: bool,
    pub space: bool,
}

impl KeyState {
    pub fn is_side_strafe(self) -> bool {
         (self.key_a || self.key_d) &&
        !(self.key_w || self.key_s)
    }

    pub fn rising_edge(self, previous: KeyState) -> KeyState {
        self & !previous
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