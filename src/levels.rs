use bevy::prelude::*;
use bevy_proto::prelude::{ReflectSchematic, Schematic};

#[derive(Component, Debug)]
pub struct PlayerLevel {
    pub level: u8,
    pub xp: u32,
    pub next_level_xp: u32,
}
#[derive(Component, Reflect, FromReflect, Schematic, Default)]
#[reflect(Component, Schematic)]
pub struct ExperienceReward(pub u32);

pub const LEVEL_REQ_XP: [u32; 10] = [100, 200, 400, 800, 1600, 3200, 6400, 12800, 25600, 25600];
impl PlayerLevel {
    pub fn new(level: u8) -> Self {
        PlayerLevel {
            level,
            xp: 0,
            next_level_xp: LEVEL_REQ_XP[if level >= LEVEL_REQ_XP.len() as u8 {
                LEVEL_REQ_XP.len() - 1
            } else {
                level as usize
            }],
        }
    }

    pub fn add_xp(&mut self, xp: u32) {
        self.xp += xp;
        if self.xp >= self.next_level_xp {
            self.level += 1;
            self.xp = self.xp - self.next_level_xp;
            self.next_level_xp = LEVEL_REQ_XP[self.level as usize];
        }
    }
}
