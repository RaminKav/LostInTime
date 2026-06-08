use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::BufReader;

use crate::datafiles;

fn default_hotbar_slot_0() -> InputBinding {
    InputBinding::KeyBinding(KeyCode::Key1)
}
fn default_hotbar_slot_1() -> InputBinding {
    InputBinding::KeyBinding(KeyCode::Key2)
}
fn default_hotbar_slot_2() -> InputBinding {
    InputBinding::KeyBinding(KeyCode::Key3)
}
fn default_hotbar_slot_3() -> InputBinding {
    InputBinding::KeyBinding(KeyCode::Key4)
}
fn default_interact() -> InputBinding {
    InputBinding::KeyBinding(KeyCode::F)
}
fn default_attack_auto_target() -> InputBinding {
    InputBinding::KeyBinding(KeyCode::T)
}

/// Hidden fourth skill slot (no HUD / options row while `VISIBLE_CLASS_SKILL_COUNT` is 3).
fn default_active_skill_slot_3() -> InputBinding {
    InputBinding::KeyBinding(KeyCode::F10)
}

/// Collapse left/right modifier variants so bindings compare and persist consistently.
pub fn normalize_key_binding(key: KeyCode) -> KeyCode {
    match key {
        KeyCode::RShift => KeyCode::LShift,
        KeyCode::RControl => KeyCode::LControl,
        KeyCode::RAlt => KeyCode::LAlt,
        other => other,
    }
}

/// Whether `keys` registered a press for the bound key this frame.
/// Left/right Shift, Ctrl, and Alt are treated as interchangeable.
pub fn key_binding_just_pressed(bound: KeyCode, keys: &Input<KeyCode>) -> bool {
    match normalize_key_binding(bound) {
        KeyCode::LShift => {
            keys.just_pressed(KeyCode::LShift) || keys.just_pressed(KeyCode::RShift)
        }
        KeyCode::LControl => {
            keys.just_pressed(KeyCode::LControl) || keys.just_pressed(KeyCode::RControl)
        }
        KeyCode::LAlt => keys.just_pressed(KeyCode::LAlt) || keys.just_pressed(KeyCode::RAlt),
        key => keys.just_pressed(key),
    }
}

pub fn bindings_conflict(a: InputBinding, b: InputBinding) -> bool {
    match (a, b) {
        (InputBinding::KeyBinding(ka), InputBinding::KeyBinding(kb)) => {
            normalize_key_binding(ka) == normalize_key_binding(kb)
        }
        (InputBinding::MouseBinding(ma), InputBinding::MouseBinding(mb)) => ma == mb,
        _ => false,
    }
}

fn check_binding_input(
    binding: InputBinding,
    keys: &Input<KeyCode>,
    mouse: &Input<MouseButton>,
) -> bool {
    match binding {
        InputBinding::KeyBinding(key) => key_binding_just_pressed(key, keys),
        InputBinding::MouseBinding(button) => mouse.just_pressed(button),
    }
}

#[derive(Resource, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct InputMappings {
    pub active_skill_slot_0: InputBinding,
    pub active_skill_slot_1: InputBinding,
    pub active_skill_slot_2: InputBinding,
    #[serde(default = "default_active_skill_slot_3")]
    pub active_skill_slot_3: InputBinding,
    pub active_skill_slot_4: InputBinding, // Bonus slot from blessings
    pub inventory: InputBinding,
    pub minimap: InputBinding,
    #[serde(default = "default_interact")]
    pub interact: InputBinding,
    /// Keys that consume / use the item currently sitting in hotbar slot 0..=3.
    /// Driven by `handle_hotbar_consume_keys` — pressing runs the slot item's
    /// `ItemActions`, no "selection" is performed.
    #[serde(default = "default_hotbar_slot_0")]
    pub hotbar_slot_0: InputBinding,
    #[serde(default = "default_hotbar_slot_1")]
    pub hotbar_slot_1: InputBinding,
    #[serde(default = "default_hotbar_slot_2")]
    pub hotbar_slot_2: InputBinding,
    #[serde(default = "default_hotbar_slot_3")]
    pub hotbar_slot_3: InputBinding,
    #[serde(default = "default_attack_auto_target")]
    pub attack_auto_target: InputBinding,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]

pub enum InputBinding {
    KeyBinding(KeyCode),
    MouseBinding(MouseButton),
}

impl Default for InputMappings {
    fn default() -> Self {
        Self {
            active_skill_slot_0: InputBinding::KeyBinding(KeyCode::Space),
            active_skill_slot_1: InputBinding::MouseBinding(MouseButton::Left),
            active_skill_slot_2: InputBinding::MouseBinding(MouseButton::Right),
            active_skill_slot_3: default_active_skill_slot_3(),
            active_skill_slot_4: InputBinding::KeyBinding(KeyCode::E), // Bonus slot
            inventory: InputBinding::KeyBinding(KeyCode::Tab),
            minimap: InputBinding::KeyBinding(KeyCode::C),
            interact: InputBinding::KeyBinding(KeyCode::F),
            hotbar_slot_0: InputBinding::KeyBinding(KeyCode::Key1),
            hotbar_slot_1: InputBinding::KeyBinding(KeyCode::Key2),
            hotbar_slot_2: InputBinding::KeyBinding(KeyCode::Key3),
            hotbar_slot_3: InputBinding::KeyBinding(KeyCode::Key4),
            attack_auto_target: default_attack_auto_target(),
        }
    }
}

impl InputMappings {
    pub fn get_inventory_key(&self) -> InputBinding {
        self.inventory
    }
    pub fn get_minimap_key(&self) -> InputBinding {
        self.minimap
    }
    pub fn get_interact_key(&self) -> InputBinding {
        self.interact
    }
    pub fn get_active_skill_key(&self, slot: usize) -> InputBinding {
        match slot {
            0 => self.active_skill_slot_0,
            1 => self.active_skill_slot_1,
            2 => self.active_skill_slot_2,
            3 => self.active_skill_slot_3,
            4 => self.active_skill_slot_4,
            _ => InputBinding::KeyBinding(KeyCode::Space),
        }
    }
    pub fn check_skill_input(
        &self,
        slot: usize,
        keys: &Input<KeyCode>,
        mouse: &Input<MouseButton>,
    ) -> bool {
        check_binding_input(self.get_active_skill_key(slot), keys, mouse)
    }
    pub fn check_inv_input(&self, keys: &Input<KeyCode>, mouse: &Input<MouseButton>) -> bool {
        check_binding_input(self.get_inventory_key(), keys, mouse)
    }
    pub fn check_map_input(&self, keys: &Input<KeyCode>, mouse: &Input<MouseButton>) -> bool {
        check_binding_input(self.get_minimap_key(), keys, mouse)
    }

    pub fn set_active_skill_key(&mut self, slot: usize, key: InputBinding) {
        let key = match key {
            InputBinding::KeyBinding(k) => InputBinding::KeyBinding(normalize_key_binding(k)),
            other => other,
        };
        match slot {
            0 => self.active_skill_slot_0 = key,
            1 => self.active_skill_slot_1 = key,
            2 => self.active_skill_slot_2 = key,
            3 => self.active_skill_slot_3 = key,
            4 => self.active_skill_slot_4 = key,
            _ => {}
        }
    }

    /// Remove the same binding from other active-skill slots so a hidden/default slot
    /// cannot swallow input (e.g. slot 3 still on Shift while only slots 0–2 are used).
    pub fn clear_active_skill_binding_from_other_slots(
        &mut self,
        binding: InputBinding,
        except_slot: usize,
    ) {
        for slot in 0..=4 {
            if slot == except_slot {
                continue;
            }
            if bindings_conflict(self.get_active_skill_key(slot), binding) {
                let fallback = Self::default().get_active_skill_key(slot);
                self.set_active_skill_key(slot, fallback);
            }
        }
    }

    pub fn set_inventory_key(&mut self, key: InputBinding) {
        self.inventory = key;
    }

    pub fn set_minimap_key(&mut self, key: InputBinding) {
        self.minimap = key;
    }

    pub fn set_interact_key(&mut self, key: InputBinding) {
        self.interact = key;
    }

    pub fn check_interact_input(&self, keys: &Input<KeyCode>, mouse: &Input<MouseButton>) -> bool {
        check_binding_input(self.get_interact_key(), keys, mouse)
    }

    /// Returns the binding that consumes/uses the item in hotbar slot `slot` (0..=3).
    /// Any out-of-range slot falls back to the slot-0 binding.
    pub fn get_hotbar_key(&self, slot: usize) -> InputBinding {
        match slot {
            0 => self.hotbar_slot_0,
            1 => self.hotbar_slot_1,
            2 => self.hotbar_slot_2,
            3 => self.hotbar_slot_3,
            _ => self.hotbar_slot_0,
        }
    }

    pub fn set_hotbar_key(&mut self, slot: usize, key: InputBinding) {
        match slot {
            0 => self.hotbar_slot_0 = key,
            1 => self.hotbar_slot_1 = key,
            2 => self.hotbar_slot_2 = key,
            3 => self.hotbar_slot_3 = key,
            _ => {}
        }
    }

    pub fn check_hotbar_input(
        &self,
        slot: usize,
        keys: &Input<KeyCode>,
        mouse: &Input<MouseButton>,
    ) -> bool {
        check_binding_input(self.get_hotbar_key(slot), keys, mouse)
    }

    pub fn get_attack_auto_target_key(&self) -> InputBinding {
        self.attack_auto_target
    }

    pub fn set_attack_auto_target_key(&mut self, key: InputBinding) {
        self.attack_auto_target = key;
    }

    pub fn check_attack_auto_target_input(
        &self,
        keys: &Input<KeyCode>,
        mouse: &Input<MouseButton>,
    ) -> bool {
        check_binding_input(self.get_attack_auto_target_key(), keys, mouse)
    }

    pub fn load() -> Self {
        let path = datafiles::game_data();
        if let Ok(file) = File::open(&path) {
            let reader = BufReader::new(file);
            if let Ok(game_data) = crate::client::GameData::try_from_json_reader(reader) {
                return game_data.keybindings.unwrap_or_default();
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        let path = datafiles::game_data();
        let mut game_data = if let Ok(file) = File::open(&path) {
            let reader = BufReader::new(file);
            crate::client::GameData::try_from_json_reader(reader).unwrap_or_default()
        } else {
            crate::client::GameData::default()
        };

        game_data.keybindings = Some(self.clone());

        if let Ok(file) = File::create(&path) {
            let _ = serde_json::to_writer_pretty(file, &game_data);
        }
    }
}

/// Returns a user-friendly display string for a KeyCode
pub fn get_key_display_name(key: InputBinding) -> String {
    match key {
        // Modifier keys - simplify left/right variants
        InputBinding::KeyBinding(KeyCode::LShift) | InputBinding::KeyBinding(KeyCode::RShift) => {
            "Shift".to_string()
        }
        InputBinding::KeyBinding(KeyCode::LControl)
        | InputBinding::KeyBinding(KeyCode::RControl) => "Ctrl".to_string(),
        InputBinding::KeyBinding(KeyCode::LAlt) | InputBinding::KeyBinding(KeyCode::RAlt) => {
            "Alt".to_string()
        }

        // Number keys - remove "Key" prefix
        InputBinding::KeyBinding(KeyCode::Key1) => "1".to_string(),
        InputBinding::KeyBinding(KeyCode::Key2) => "2".to_string(),
        InputBinding::KeyBinding(KeyCode::Key3) => "3".to_string(),
        InputBinding::KeyBinding(KeyCode::Key4) => "4".to_string(),
        InputBinding::KeyBinding(KeyCode::Key5) => "5".to_string(),
        InputBinding::KeyBinding(KeyCode::Key6) => "6".to_string(),
        InputBinding::KeyBinding(KeyCode::Key7) => "7".to_string(),
        InputBinding::KeyBinding(KeyCode::Key8) => "8".to_string(),
        InputBinding::KeyBinding(KeyCode::Key9) => "9".to_string(),
        InputBinding::KeyBinding(KeyCode::Key0) => "0".to_string(),
        // Special keys - shorten or rename
        InputBinding::KeyBinding(KeyCode::Return) => "Enter".to_string(),
        InputBinding::KeyBinding(KeyCode::Back) => "Bksp".to_string(),
        InputBinding::KeyBinding(KeyCode::Capital) => "Caps".to_string(),
        InputBinding::KeyBinding(KeyCode::Escape) => "Esc".to_string(),
        InputBinding::KeyBinding(KeyCode::Space) => "[_]".to_string(),
        InputBinding::MouseBinding(MouseButton::Left) => "LMB".to_string(),
        InputBinding::MouseBinding(MouseButton::Right) => "RMB".to_string(),
        InputBinding::MouseBinding(MouseButton::Middle) => "MMB".to_string(),

        // Default: use Debug format but capitalize first letter
        _ => match key {
            InputBinding::KeyBinding(key_input) => format!("{:?}", key_input),
            InputBinding::MouseBinding(mouse_input) => format!("{:?}", mouse_input),
        },
    }
}
