use bevy::input::gamepad::GamepadButton;
use bevy::prelude::*;
use leafwing_input_manager::prelude::{GamepadStick, InputMap};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::BufReader;

use crate::datafiles;
use crate::gamepad_input::GamepadAction;
use crate::keybinds::{get_key_display_name, InputMappings};

/// Identifies a rebindable action for display (keyboard vs controller label selection).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BindingLabel {
    ActiveSkill(usize),
    Hotbar(usize),
    Inventory,
    Minimap,
    Interact,
    AttackAutoTarget,
}

/// Returns the label shown on HUD badges / interact guides for the active input device.
pub fn format_binding_label(
    label: BindingLabel,
    keybinds: &InputMappings,
    gamepad_mappings: &GamepadMappings,
    gamepad_connected: bool,
) -> String {
    if gamepad_connected {
        let button = match label {
            BindingLabel::ActiveSkill(slot) => gamepad_mappings.get_active_skill_button(slot),
            BindingLabel::Hotbar(slot) => gamepad_mappings.get_hotbar_button(slot),
            BindingLabel::Inventory => gamepad_mappings.get_inventory_button(),
            BindingLabel::Minimap => gamepad_mappings.get_minimap_button(),
            BindingLabel::Interact => gamepad_mappings.get_interact_button(),
            BindingLabel::AttackAutoTarget => gamepad_mappings.get_attack_auto_target_button(),
        };
        get_gamepad_display_name(button)
    } else {
        let key = match label {
            BindingLabel::ActiveSkill(slot) => keybinds.get_active_skill_key(slot),
            BindingLabel::Hotbar(slot) => keybinds.get_hotbar_key(slot),
            BindingLabel::Inventory => keybinds.get_inventory_key(),
            BindingLabel::Minimap => keybinds.get_minimap_key(),
            BindingLabel::Interact => keybinds.get_interact_key(),
            BindingLabel::AttackAutoTarget => keybinds.get_attack_auto_target_key(),
        };
        get_key_display_name(key)
    }
}

/// Whether on-screen keybind labels should refresh this frame.
pub fn binding_labels_dirty(
    keybinds_changed: bool,
    gamepad_mappings_changed: bool,
    connected: bool,
    last_gamepad_connected: &mut Option<bool>,
) -> bool {
    let device_changed = match *last_gamepad_connected {
        None => true,
        Some(prev) => prev != connected,
    };
    *last_gamepad_connected = Some(connected);
    keybinds_changed || gamepad_mappings_changed || device_changed
}

/// Pause/options hint on the HUD corner icon (Start on controller, Esc on keyboard).
pub fn format_pause_options_label(gamepad_connected: bool) -> String {
    if gamepad_connected {
        "Start".to_string()
    } else {
        "Esc".to_string()
    }
}

/// Persisted gamepad button identity (mirrors Bevy `GamepadButton` face/trigger/d-pad values).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum GamepadBindingButton {
    South,
    East,
    North,
    West,
    LeftTrigger,
    RightTrigger,
    LeftTrigger2,
    RightTrigger2,
    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight,
    Start,
    Select,
}

impl GamepadBindingButton {
    pub fn to_button_type(self) -> GamepadButton {
        match self {
            Self::South => GamepadButton::South,
            Self::East => GamepadButton::East,
            Self::North => GamepadButton::North,
            Self::West => GamepadButton::West,
            Self::LeftTrigger => GamepadButton::LeftTrigger,
            Self::RightTrigger => GamepadButton::RightTrigger,
            Self::LeftTrigger2 => GamepadButton::LeftTrigger2,
            Self::RightTrigger2 => GamepadButton::RightTrigger2,
            Self::DPadUp => GamepadButton::DPadUp,
            Self::DPadDown => GamepadButton::DPadDown,
            Self::DPadLeft => GamepadButton::DPadLeft,
            Self::DPadRight => GamepadButton::DPadRight,
            Self::Start => GamepadButton::Start,
            Self::Select => GamepadButton::Select,
        }
    }

    /// Returns `None` for stick axes or other non-button inputs.
    pub fn from_button_type(button: GamepadButton) -> Option<Self> {
        Some(match button {
            GamepadButton::South => Self::South,
            GamepadButton::East => Self::East,
            GamepadButton::North => Self::North,
            GamepadButton::West => Self::West,
            GamepadButton::LeftTrigger => Self::LeftTrigger,
            GamepadButton::RightTrigger => Self::RightTrigger,
            GamepadButton::LeftTrigger2 => Self::LeftTrigger2,
            GamepadButton::RightTrigger2 => Self::RightTrigger2,
            GamepadButton::DPadUp => Self::DPadUp,
            GamepadButton::DPadDown => Self::DPadDown,
            GamepadButton::DPadLeft => Self::DPadLeft,
            GamepadButton::DPadRight => Self::DPadRight,
            GamepadButton::Start => Self::Start,
            GamepadButton::Select => Self::Select,
            _ => return None,
        })
    }
}

fn default_active_skill_slot_3() -> GamepadBindingButton {
    GamepadBindingButton::Select
}

#[derive(Resource, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct GamepadMappings {
    pub active_skill_slot_0: GamepadBindingButton,
    pub active_skill_slot_1: GamepadBindingButton,
    pub active_skill_slot_2: GamepadBindingButton,
    #[serde(default = "default_active_skill_slot_3")]
    pub active_skill_slot_3: GamepadBindingButton,
    pub active_skill_slot_4: GamepadBindingButton,
    pub inventory: GamepadBindingButton,
    pub minimap: GamepadBindingButton,
    pub interact: GamepadBindingButton,
    pub hotbar_slot_0: GamepadBindingButton,
    pub hotbar_slot_1: GamepadBindingButton,
    pub hotbar_slot_2: GamepadBindingButton,
    pub hotbar_slot_3: GamepadBindingButton,
    pub attack_auto_target: GamepadBindingButton,
}

impl Default for GamepadMappings {
    fn default() -> Self {
        Self {
            active_skill_slot_0: GamepadBindingButton::South,
            active_skill_slot_1: GamepadBindingButton::RightTrigger2,
            active_skill_slot_2: GamepadBindingButton::LeftTrigger2,
            active_skill_slot_3: default_active_skill_slot_3(),
            active_skill_slot_4: GamepadBindingButton::West,
            inventory: GamepadBindingButton::LeftTrigger,
            minimap: GamepadBindingButton::RightTrigger,
            interact: GamepadBindingButton::North,
            hotbar_slot_0: GamepadBindingButton::DPadUp,
            hotbar_slot_1: GamepadBindingButton::DPadRight,
            hotbar_slot_2: GamepadBindingButton::DPadDown,
            hotbar_slot_3: GamepadBindingButton::DPadLeft,
            attack_auto_target: GamepadBindingButton::West,
        }
    }
}

impl GamepadMappings {
    pub fn get_active_skill_button(&self, slot: usize) -> GamepadBindingButton {
        match slot {
            0 => self.active_skill_slot_0,
            1 => self.active_skill_slot_1,
            2 => self.active_skill_slot_2,
            3 => self.active_skill_slot_3,
            4 => self.active_skill_slot_4,
            _ => self.active_skill_slot_0,
        }
    }

    pub fn set_active_skill_button(&mut self, slot: usize, button: GamepadBindingButton) {
        match slot {
            0 => self.active_skill_slot_0 = button,
            1 => self.active_skill_slot_1 = button,
            2 => self.active_skill_slot_2 = button,
            3 => self.active_skill_slot_3 = button,
            4 => self.active_skill_slot_4 = button,
            _ => {}
        }
    }

    pub fn clear_active_skill_button_from_other_slots(
        &mut self,
        button: GamepadBindingButton,
        except_slot: usize,
    ) {
        for slot in 0..=4 {
            if slot == except_slot {
                continue;
            }
            if self.get_active_skill_button(slot) == button {
                self.set_active_skill_button(slot, Self::default().get_active_skill_button(slot));
            }
        }
    }

    pub fn get_hotbar_button(&self, slot: usize) -> GamepadBindingButton {
        match slot {
            0 => self.hotbar_slot_0,
            1 => self.hotbar_slot_1,
            2 => self.hotbar_slot_2,
            3 => self.hotbar_slot_3,
            _ => self.hotbar_slot_0,
        }
    }

    pub fn set_hotbar_button(&mut self, slot: usize, button: GamepadBindingButton) {
        match slot {
            0 => self.hotbar_slot_0 = button,
            1 => self.hotbar_slot_1 = button,
            2 => self.hotbar_slot_2 = button,
            3 => self.hotbar_slot_3 = button,
            _ => {}
        }
    }

    pub fn get_inventory_button(&self) -> GamepadBindingButton {
        self.inventory
    }

    pub fn set_inventory_button(&mut self, button: GamepadBindingButton) {
        self.inventory = button;
    }

    pub fn get_minimap_button(&self) -> GamepadBindingButton {
        self.minimap
    }

    pub fn set_minimap_button(&mut self, button: GamepadBindingButton) {
        self.minimap = button;
    }

    pub fn get_interact_button(&self) -> GamepadBindingButton {
        self.interact
    }

    pub fn set_interact_button(&mut self, button: GamepadBindingButton) {
        self.interact = button;
    }

    pub fn get_attack_auto_target_button(&self) -> GamepadBindingButton {
        self.attack_auto_target
    }

    pub fn set_attack_auto_target_button(&mut self, button: GamepadBindingButton) {
        self.attack_auto_target = button;
    }

    /// Builds the gameplay `InputMap`, preserving fixed stick bindings and mirroring the
    /// default layout where basic attack shares skill slot 1's trigger.
    pub fn to_input_map(&self) -> InputMap<GamepadAction> {
        let mut map = InputMap::default();
        map.insert_dual_axis(GamepadAction::Move, GamepadStick::LEFT);
        map.insert_dual_axis(GamepadAction::Aim, GamepadStick::RIGHT);

        let skill0 = self.active_skill_slot_0.to_button_type();
        let skill1 = self.active_skill_slot_1.to_button_type();
        let skill2 = self.active_skill_slot_2.to_button_type();

        map.insert(GamepadAction::Attack, skill1);
        map.insert(GamepadAction::Skill0, skill0);
        map.insert(GamepadAction::Skill1, skill1);
        map.insert(GamepadAction::Skill2, skill2);
        map.insert(
            GamepadAction::ToggleInventory,
            self.inventory.to_button_type(),
        );
        map.insert(GamepadAction::ToggleMap, self.minimap.to_button_type());
        map.insert(GamepadAction::Interact, self.interact.to_button_type());
        map.insert(
            GamepadAction::AutoTarget,
            self.attack_auto_target.to_button_type(),
        );
        map.insert(GamepadAction::Hotbar0, self.hotbar_slot_0.to_button_type());
        map.insert(GamepadAction::Hotbar1, self.hotbar_slot_1.to_button_type());
        map.insert(GamepadAction::Hotbar2, self.hotbar_slot_2.to_button_type());
        map.insert(GamepadAction::Hotbar3, self.hotbar_slot_3.to_button_type());
        map
    }

    pub fn load() -> Self {
        let path = datafiles::game_data();
        if let Ok(file) = File::open(&path) {
            let reader = BufReader::new(file);
            if let Ok(game_data) = crate::client::GameData::try_from_json_reader(reader) {
                return game_data.gamepad_bindings.unwrap_or_default();
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

        game_data.gamepad_bindings = Some(*self);

        if let Ok(file) = File::create(&path) {
            let _ = serde_json::to_writer_pretty(file, &game_data);
        }
    }
}

/// User-facing label for a bound gamepad button (Xbox-style names; raw `Debug` as fallback).
pub fn get_gamepad_display_name(button: GamepadBindingButton) -> String {
    match button {
        GamepadBindingButton::South => "A".to_string(),
        GamepadBindingButton::East => "B".to_string(),
        GamepadBindingButton::North => "Y".to_string(),
        GamepadBindingButton::West => "X".to_string(),
        GamepadBindingButton::LeftTrigger => "LB".to_string(),
        GamepadBindingButton::RightTrigger => "RB".to_string(),
        GamepadBindingButton::LeftTrigger2 => "LT".to_string(),
        GamepadBindingButton::RightTrigger2 => "RT".to_string(),
        GamepadBindingButton::DPadUp => "D-U".to_string(),
        GamepadBindingButton::DPadRight => "D-R".to_string(),
        GamepadBindingButton::DPadDown => "D-D".to_string(),
        GamepadBindingButton::DPadLeft => "D-L".to_string(),
        GamepadBindingButton::Start => "Start".to_string(),
        GamepadBindingButton::Select => "Select".to_string(),
    }
}

/// Connected gamepads, which Bevy models as entities carrying a [`Gamepad`] component.
pub type ConnectedGamepads<'w, 's> = Query<'w, 's, (), With<Gamepad>>;

/// True when at least one gamepad is connected — controller bindings take priority in options UI.
pub fn gamepad_connected(gamepads: &ConnectedGamepads<'_, '_>) -> bool {
    !gamepads.is_empty()
}
