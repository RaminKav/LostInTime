use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::BufReader;

use crate::datafiles;

fn default_quick_consume_slot_1() -> InputBinding {
    InputBinding::KeyBinding(KeyCode::Z)
}
fn default_quick_consume_slot_2() -> InputBinding {
    InputBinding::KeyBinding(KeyCode::X)
}

#[derive(Resource, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct InputMappings {
    pub active_skill_slot_0: InputBinding,
    pub active_skill_slot_1: InputBinding,
    pub active_skill_slot_2: InputBinding,
    pub active_skill_slot_3: InputBinding,
    pub active_skill_slot_4: InputBinding, // Bonus slot from blessings
    pub inventory: InputBinding,
    pub minimap: InputBinding,
    #[serde(default = "default_quick_consume_slot_1")]
    pub quick_consume_slot_1: InputBinding,
    #[serde(default = "default_quick_consume_slot_2")]
    pub quick_consume_slot_2: InputBinding,
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
            active_skill_slot_1: InputBinding::MouseBinding(MouseButton::Right),
            active_skill_slot_2: InputBinding::KeyBinding(KeyCode::LShift),
            active_skill_slot_3: InputBinding::KeyBinding(KeyCode::Q),
            active_skill_slot_4: InputBinding::KeyBinding(KeyCode::E), // Bonus slot
            inventory: InputBinding::KeyBinding(KeyCode::Tab),
            minimap: InputBinding::KeyBinding(KeyCode::M),
            quick_consume_slot_1: InputBinding::KeyBinding(KeyCode::Z),
            quick_consume_slot_2: InputBinding::KeyBinding(KeyCode::X),
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
        keys: &Res<Input<KeyCode>>,
        mouse: &Res<Input<MouseButton>>,
    ) -> bool {
        let input = self.get_active_skill_key(slot);
        match input {
            InputBinding::KeyBinding(key) => keys.just_pressed(key),
            InputBinding::MouseBinding(button) => mouse.just_pressed(button),
        }
    }
    pub fn check_inv_input(
        &self,
        keys: &Res<Input<KeyCode>>,
        mouse: &Res<Input<MouseButton>>,
    ) -> bool {
        let input = self.get_inventory_key();
        match input {
            InputBinding::KeyBinding(key) => keys.just_pressed(key),
            InputBinding::MouseBinding(button) => mouse.just_pressed(button),
        }
    }
    pub fn check_map_input(
        &self,
        keys: &Res<Input<KeyCode>>,
        mouse: &Res<Input<MouseButton>>,
    ) -> bool {
        let input = self.get_minimap_key();
        match input {
            InputBinding::KeyBinding(key) => keys.just_pressed(key),
            InputBinding::MouseBinding(button) => mouse.just_pressed(button),
        }
    }

    pub fn set_active_skill_key(&mut self, slot: usize, key: InputBinding) {
        match slot {
            0 => self.active_skill_slot_0 = key,
            1 => self.active_skill_slot_1 = key,
            2 => self.active_skill_slot_2 = key,
            3 => self.active_skill_slot_3 = key,
            4 => self.active_skill_slot_4 = key,
            _ => {}
        }
    }

    pub fn set_inventory_key(&mut self, key: InputBinding) {
        self.inventory = key;
    }

    pub fn set_minimap_key(&mut self, key: InputBinding) {
        self.minimap = key;
    }

    pub fn get_quick_consume_key(&self, slot: usize) -> InputBinding {
        match slot {
            1 => self.quick_consume_slot_1,
            2 => self.quick_consume_slot_2,
            _ => InputBinding::KeyBinding(KeyCode::Z),
        }
    }

    pub fn set_quick_consume_key(&mut self, slot: usize, key: InputBinding) {
        match slot {
            1 => self.quick_consume_slot_1 = key,
            2 => self.quick_consume_slot_2 = key,
            _ => {}
        }
    }

    pub fn check_quick_consume_input(
        &self,
        slot: usize,
        keys: &Res<Input<KeyCode>>,
        mouse: &Res<Input<MouseButton>>,
    ) -> bool {
        let input = self.get_quick_consume_key(slot);
        match input {
            InputBinding::KeyBinding(key) => keys.just_pressed(key),
            InputBinding::MouseBinding(button) => mouse.just_pressed(button),
        }
    }

    pub fn load() -> Self {
        let path = datafiles::game_data();
        if let Ok(file) = File::open(&path) {
            let reader = BufReader::new(file);
            if let Ok(game_data) = serde_json::from_reader::<_, crate::client::GameData>(reader) {
                return game_data.keybindings.unwrap_or_default();
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        let path = datafiles::game_data();
        let mut game_data = if let Ok(file) = File::open(&path) {
            let reader = BufReader::new(file);
            serde_json::from_reader::<_, crate::client::GameData>(reader).unwrap_or_default()
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
