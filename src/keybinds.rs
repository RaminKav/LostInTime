use bevy::prelude::*;

pub fn get_active_skill_keybind(slot: usize) -> KeyCode {
    match slot {
        0 => KeyCode::Space,
        1 => KeyCode::LShift,
        i => unreachable!("invalid active skill slot {i:?}"),
    }
}
