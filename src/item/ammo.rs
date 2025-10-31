use bevy::prelude::*;
use std::collections::HashMap;

#[derive(Component, Debug, Clone)]
pub struct Ammo {
    pub max: u32,
    pub current: u32,
    pub reload: Timer,
    pub reloading: bool,
}

impl Ammo {
    pub fn new(max: u32, reload_seconds: f32) -> Self {
        Self {
            max,
            current: 0,
            reload: Timer::from_seconds(reload_seconds, TimerMode::Once),
            reloading: false,
        }
    }

    pub fn start_reload(&mut self) {
        if !self.reloading {
            self.reloading = true;
            self.reload.reset();
        }
    }

    pub fn can_fire(&self) -> bool {
        self.current > 0 && !self.reloading
    }

    /// Consume one ammo and auto-start reload if depleted
    pub fn use_ammo_and_maybe_reload(&mut self) -> bool {
        if self.current > 0 {
            self.current -= 1;
            if self.current == 0 {
                self.start_reload();
            }
            true
        } else {
            false
        }
    }
}

pub fn tick_reload(time: Res<Time>, mut q: Query<&mut Ammo>) {
    for mut ammo in q.iter_mut() {
        if ammo.reloading {
            ammo.reload.tick(time.delta());
            if ammo.reload.finished() {
                ammo.current = ammo.max;
                ammo.reloading = false;
            }
        }
    }
}

/// Stores ammo state per weapon type so swaps can restore state
#[derive(Resource, Default, Debug)]
pub struct AmmoMemory(pub HashMap<Entity, u32>); // key: UI item entity in active hotbar slot
