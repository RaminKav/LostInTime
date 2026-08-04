use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::File;
use std::io::BufReader;

use crate::datafiles;

/// Rewrite pre-Bevy-0.19 `KeyCode` names in persisted JSON (`"E"` → `"KeyE"`, `"Key1"` → `"Digit1"`).
///
/// Without this, `GameData` deserialization fails, the name-entry popup opens every boot, and
/// main-menu clicks are discarded while hover still works.
pub fn migrate_legacy_keybinding_json(value: &mut Value) {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(name)) = map.get_mut("KeyBinding") {
                *name = migrate_legacy_keycode_name(name).into_owned();
            }
            for child in map.values_mut() {
                migrate_legacy_keybinding_json(child);
            }
        }
        Value::Array(items) => {
            for child in items {
                migrate_legacy_keybinding_json(child);
            }
        }
        _ => {}
    }
}

fn migrate_legacy_keycode_name(name: &str) -> std::borrow::Cow<'_, str> {
    match name {
        "Key0" => "Digit0".into(),
        "Key1" => "Digit1".into(),
        "Key2" => "Digit2".into(),
        "Key3" => "Digit3".into(),
        "Key4" => "Digit4".into(),
        "Key5" => "Digit5".into(),
        "Key6" => "Digit6".into(),
        "Key7" => "Digit7".into(),
        "Key8" => "Digit8".into(),
        "Key9" => "Digit9".into(),
        s if s.len() == 1 && s.as_bytes()[0].is_ascii_alphabetic() => {
            format!("Key{}", s.to_ascii_uppercase()).into()
        }
        other => other.into(),
    }
}

fn default_hotbar_slot_0() -> InputBinding {
    InputBinding::KeyBinding(KeyCode::Digit1)
}
fn default_hotbar_slot_1() -> InputBinding {
    InputBinding::KeyBinding(KeyCode::Digit2)
}
fn default_hotbar_slot_2() -> InputBinding {
    InputBinding::KeyBinding(KeyCode::Digit3)
}
fn default_hotbar_slot_3() -> InputBinding {
    InputBinding::KeyBinding(KeyCode::Digit4)
}
fn default_interact() -> InputBinding {
    InputBinding::KeyBinding(KeyCode::KeyF)
}
fn default_attack_auto_target() -> InputBinding {
    InputBinding::KeyBinding(KeyCode::KeyE)
}
fn default_shop_mark() -> InputBinding {
    InputBinding::KeyBinding(KeyCode::KeyX)
}

/// Hidden fourth skill slot (no HUD / options row while `VISIBLE_CLASS_SKILL_COUNT` is 3).
fn default_active_skill_slot_3() -> InputBinding {
    InputBinding::KeyBinding(KeyCode::F10)
}

/// Collapse left/right modifier variants so bindings compare and persist consistently.
pub fn normalize_key_binding(key: KeyCode) -> KeyCode {
    match key {
        KeyCode::ShiftRight => KeyCode::ShiftLeft,
        KeyCode::ControlRight => KeyCode::ControlLeft,
        KeyCode::AltRight => KeyCode::AltLeft,
        other => other,
    }
}

/// Whether `keys` registered a press for the bound key this frame.
/// Left/right Shift, Ctrl, and Alt are treated as interchangeable.
pub fn key_binding_just_pressed(bound: KeyCode, keys: &ButtonInput<KeyCode>) -> bool {
    match normalize_key_binding(bound) {
        KeyCode::ShiftLeft => {
            keys.just_pressed(KeyCode::ShiftLeft) || keys.just_pressed(KeyCode::ShiftRight)
        }
        KeyCode::ControlLeft => {
            keys.just_pressed(KeyCode::ControlLeft) || keys.just_pressed(KeyCode::ControlRight)
        }
        KeyCode::AltLeft => {
            keys.just_pressed(KeyCode::AltLeft) || keys.just_pressed(KeyCode::AltRight)
        }
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
    keys: &ButtonInput<KeyCode>,
    mouse: &ButtonInput<MouseButton>,
) -> bool {
    match binding {
        InputBinding::KeyBinding(key) => key_binding_just_pressed(key, keys),
        InputBinding::MouseBinding(button) => mouse.just_pressed(button),
    }
}

/// Whether `keys` currently has the bound key held down (not just this frame). Left/right
/// Shift, Ctrl, and Alt are treated as interchangeable, matching `key_binding_just_pressed`.
pub fn key_binding_pressed(bound: KeyCode, keys: &ButtonInput<KeyCode>) -> bool {
    match normalize_key_binding(bound) {
        KeyCode::ShiftLeft => keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight),
        KeyCode::ControlLeft => {
            keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight)
        }
        KeyCode::AltLeft => keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight),
        key => keys.pressed(key),
    }
}

/// Held-down (`.pressed()`) counterpart to `check_binding_input`'s `.just_pressed()` check.
/// Used by the Mouseless Mode hold-to-aim flow to detect when a ground-targeted skill's
/// button is released (see `InputMappings::check_skill_input_held`).
fn check_binding_input_held(
    binding: InputBinding,
    keys: &ButtonInput<KeyCode>,
    mouse: &ButtonInput<MouseButton>,
) -> bool {
    match binding {
        InputBinding::KeyBinding(key) => key_binding_pressed(key, keys),
        InputBinding::MouseBinding(button) => mouse.pressed(button),
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
    /// Marks / unmarks a merchant shop item to track while the essence shop UI is open.
    #[serde(default = "default_shop_mark")]
    pub shop_mark: InputBinding,
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
            active_skill_slot_4: InputBinding::KeyBinding(KeyCode::KeyE), // Bonus slot
            inventory: InputBinding::KeyBinding(KeyCode::Tab),
            minimap: InputBinding::KeyBinding(KeyCode::KeyC),
            interact: InputBinding::KeyBinding(KeyCode::KeyF),
            hotbar_slot_0: InputBinding::KeyBinding(KeyCode::Digit1),
            hotbar_slot_1: InputBinding::KeyBinding(KeyCode::Digit2),
            hotbar_slot_2: InputBinding::KeyBinding(KeyCode::Digit3),
            hotbar_slot_3: InputBinding::KeyBinding(KeyCode::Digit4),
            attack_auto_target: default_attack_auto_target(),
            shop_mark: default_shop_mark(),
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
        keys: &ButtonInput<KeyCode>,
        mouse: &ButtonInput<MouseButton>,
    ) -> bool {
        check_binding_input(self.get_active_skill_key(slot), keys, mouse)
    }

    /// Held-down counterpart to `check_skill_input`, used while charging a ground-targeted
    /// skill's hold-to-aim reticle in Mouseless Mode to detect when the button is released.
    pub fn check_skill_input_held(
        &self,
        slot: usize,
        keys: &ButtonInput<KeyCode>,
        mouse: &ButtonInput<MouseButton>,
    ) -> bool {
        check_binding_input_held(self.get_active_skill_key(slot), keys, mouse)
    }
    pub fn check_inv_input(
        &self,
        keys: &ButtonInput<KeyCode>,
        mouse: &ButtonInput<MouseButton>,
    ) -> bool {
        check_binding_input(self.get_inventory_key(), keys, mouse)
    }
    pub fn check_map_input(
        &self,
        keys: &ButtonInput<KeyCode>,
        mouse: &ButtonInput<MouseButton>,
    ) -> bool {
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

    pub fn check_interact_input(
        &self,
        keys: &ButtonInput<KeyCode>,
        mouse: &ButtonInput<MouseButton>,
    ) -> bool {
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
        keys: &ButtonInput<KeyCode>,
        mouse: &ButtonInput<MouseButton>,
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
        keys: &ButtonInput<KeyCode>,
        mouse: &ButtonInput<MouseButton>,
    ) -> bool {
        check_binding_input(self.get_attack_auto_target_key(), keys, mouse)
    }

    pub fn get_shop_mark_key(&self) -> InputBinding {
        self.shop_mark
    }

    pub fn set_shop_mark_key(&mut self, key: InputBinding) {
        self.shop_mark = key;
    }

    pub fn check_shop_mark_input(
        &self,
        keys: &ButtonInput<KeyCode>,
        mouse: &ButtonInput<MouseButton>,
    ) -> bool {
        check_binding_input(self.get_shop_mark_key(), keys, mouse)
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
        InputBinding::KeyBinding(KeyCode::ShiftLeft)
        | InputBinding::KeyBinding(KeyCode::ShiftRight) => "Shift".to_string(),
        InputBinding::KeyBinding(KeyCode::ControlLeft)
        | InputBinding::KeyBinding(KeyCode::ControlRight) => "Ctrl".to_string(),
        InputBinding::KeyBinding(KeyCode::AltLeft)
        | InputBinding::KeyBinding(KeyCode::AltRight) => "Alt".to_string(),

        // Number keys - remove "Key" prefix
        InputBinding::KeyBinding(KeyCode::Digit1) => "1".to_string(),
        InputBinding::KeyBinding(KeyCode::Digit2) => "2".to_string(),
        InputBinding::KeyBinding(KeyCode::Digit3) => "3".to_string(),
        InputBinding::KeyBinding(KeyCode::Digit4) => "4".to_string(),
        InputBinding::KeyBinding(KeyCode::Digit5) => "5".to_string(),
        InputBinding::KeyBinding(KeyCode::Digit6) => "6".to_string(),
        InputBinding::KeyBinding(KeyCode::Digit7) => "7".to_string(),
        InputBinding::KeyBinding(KeyCode::Digit8) => "8".to_string(),
        InputBinding::KeyBinding(KeyCode::Digit9) => "9".to_string(),
        InputBinding::KeyBinding(KeyCode::Digit0) => "0".to_string(),
        // Special keys - shorten or rename
        InputBinding::KeyBinding(KeyCode::Enter) => "Enter".to_string(),
        InputBinding::KeyBinding(KeyCode::Backspace) => "Bksp".to_string(),
        InputBinding::KeyBinding(KeyCode::CapsLock) => "Caps".to_string(),
        InputBinding::KeyBinding(KeyCode::Escape) => "Esc".to_string(),
        InputBinding::KeyBinding(KeyCode::Space) => "[_]".to_string(),
        InputBinding::MouseBinding(MouseButton::Left) => "LMB".to_string(),
        InputBinding::MouseBinding(MouseButton::Right) => "RMB".to_string(),
        InputBinding::MouseBinding(MouseButton::Middle) => "MMB".to_string(),
        InputBinding::MouseBinding(MouseButton::Other(i)) => format!("MB{}", i),

        // Default: Debug variant name. Bevy 0.19 letter keys are `KeyA`..`KeyZ`
        // (0.10 was `A`..`Z`); strip that prefix so HUD badges show "F" not "KeyF".
        // Digits/modifiers/specials are mapped above — Bevy has no display-name API.
        _ => match key {
            InputBinding::KeyBinding(key_input) => {
                let name = format!("{:?}", key_input);
                if let Some(rest) = name.strip_prefix("Key") {
                    if rest.len() == 1 && rest.as_bytes()[0].is_ascii_alphabetic() {
                        return rest.to_string();
                    }
                }
                name
            }
            InputBinding::MouseBinding(mouse_input) => format!("{:?}", mouse_input),
        },
    }
}
