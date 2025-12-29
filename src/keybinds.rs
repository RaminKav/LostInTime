use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::BufReader;

use crate::datafiles;

mod keycode_serde {
    use bevy::prelude::KeyCode;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(key: &KeyCode, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{:?}", key))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<KeyCode, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        // Try to parse the string back to KeyCode
        // This is a simplified version - in production you'd want a complete mapping
        match s.as_str() {
            "Space" => Ok(KeyCode::Space),
            "LShift" => Ok(KeyCode::LShift),
            "RShift" => Ok(KeyCode::RShift),
            "Q" => Ok(KeyCode::Q),
            "W" => Ok(KeyCode::W),
            "E" => Ok(KeyCode::E),
            "R" => Ok(KeyCode::R),
            "T" => Ok(KeyCode::T),
            "Y" => Ok(KeyCode::Y),
            "U" => Ok(KeyCode::U),
            "I" => Ok(KeyCode::I),
            "O" => Ok(KeyCode::O),
            "P" => Ok(KeyCode::P),
            "A" => Ok(KeyCode::A),
            "S" => Ok(KeyCode::S),
            "D" => Ok(KeyCode::D),
            "F" => Ok(KeyCode::F),
            "G" => Ok(KeyCode::G),
            "H" => Ok(KeyCode::H),
            "J" => Ok(KeyCode::J),
            "K" => Ok(KeyCode::K),
            "L" => Ok(KeyCode::L),
            "Z" => Ok(KeyCode::Z),
            "X" => Ok(KeyCode::X),
            "C" => Ok(KeyCode::C),
            "V" => Ok(KeyCode::V),
            "B" => Ok(KeyCode::B),
            "N" => Ok(KeyCode::N),
            "M" => Ok(KeyCode::M),
            "LControl" => Ok(KeyCode::LControl),
            "RControl" => Ok(KeyCode::RControl),
            "LAlt" => Ok(KeyCode::LAlt),
            "RAlt" => Ok(KeyCode::RAlt),
            "Tab" => Ok(KeyCode::Tab),
            "Key1" => Ok(KeyCode::Key1),
            "Key2" => Ok(KeyCode::Key2),
            "Key3" => Ok(KeyCode::Key3),
            "Key4" => Ok(KeyCode::Key4),
            "Key5" => Ok(KeyCode::Key5),
            "Key6" => Ok(KeyCode::Key6),
            "Key7" => Ok(KeyCode::Key7),
            "Key8" => Ok(KeyCode::Key8),
            "Key9" => Ok(KeyCode::Key9),
            "Key0" => Ok(KeyCode::Key0),
            _ => Ok(KeyCode::Space), // Default fallback
        }
    }
}

#[derive(Resource, Debug, Clone, Serialize, Deserialize)]
pub struct KeyBindings {
    #[serde(with = "keycode_serde")]
    pub active_skill_slot_0: KeyCode,
    #[serde(with = "keycode_serde")]
    pub active_skill_slot_1: KeyCode,
    #[serde(with = "keycode_serde")]
    pub active_skill_slot_2: KeyCode,
    #[serde(with = "keycode_serde", default = "default_slot_3_key")]
    pub active_skill_slot_3: KeyCode,
    #[serde(with = "keycode_serde")]
    pub inventory: KeyCode,
    #[serde(with = "keycode_serde")]
    pub minimap: KeyCode,
}

fn default_slot_3_key() -> KeyCode {
    KeyCode::R
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self {
            active_skill_slot_0: KeyCode::Space,
            active_skill_slot_1: KeyCode::LShift,
            active_skill_slot_2: KeyCode::Q,
            active_skill_slot_3: KeyCode::R,
            inventory: KeyCode::E,
            minimap: KeyCode::M,
        }
    }
}

impl KeyBindings {
    pub fn get_inventory_key(&self) -> KeyCode {
        self.inventory
    }
    pub fn get_minimap_key(&self) -> KeyCode {
        self.minimap
    }
    pub fn get_active_skill_key(&self, slot: usize) -> KeyCode {
        match slot {
            0 => self.active_skill_slot_0,
            1 => self.active_skill_slot_1,
            2 => self.active_skill_slot_2,
            3 => self.active_skill_slot_3,
            _ => KeyCode::Space,
        }
    }

    pub fn set_active_skill_key(&mut self, slot: usize, key: KeyCode) {
        match slot {
            0 => self.active_skill_slot_0 = key,
            1 => self.active_skill_slot_1 = key,
            2 => self.active_skill_slot_2 = key,
            3 => self.active_skill_slot_3 = key,
            _ => {}
        }
    }

    pub fn set_inventory_key(&mut self, key: KeyCode) {
        self.inventory = key;
    }

    pub fn set_minimap_key(&mut self, key: KeyCode) {
        self.minimap = key;
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
pub fn get_key_display_name(key: KeyCode) -> String {
    match key {
        // Modifier keys - simplify left/right variants
        KeyCode::LShift | KeyCode::RShift => "Shift".to_string(),
        KeyCode::LControl | KeyCode::RControl => "Ctrl".to_string(),
        KeyCode::LAlt | KeyCode::RAlt => "Alt".to_string(),

        // Number keys - remove "Key" prefix
        KeyCode::Key1 => "1".to_string(),
        KeyCode::Key2 => "2".to_string(),
        KeyCode::Key3 => "3".to_string(),
        KeyCode::Key4 => "4".to_string(),
        KeyCode::Key5 => "5".to_string(),
        KeyCode::Key6 => "6".to_string(),
        KeyCode::Key7 => "7".to_string(),
        KeyCode::Key8 => "8".to_string(),
        KeyCode::Key9 => "9".to_string(),
        KeyCode::Key0 => "0".to_string(),

        // Special keys - shorten or rename
        KeyCode::Return => "Enter".to_string(),
        KeyCode::Back => "Bksp".to_string(),
        KeyCode::Capital => "Caps".to_string(),
        KeyCode::Escape => "Esc".to_string(),

        // Default: use Debug format but capitalize first letter
        _ => {
            let debug_str = format!("{:?}", key);
            debug_str
        }
    }
}
