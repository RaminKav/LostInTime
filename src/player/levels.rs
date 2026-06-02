use bevy::prelude::*;
use bevy_proto::prelude::{ReflectSchematic, Schematic};
use serde::{Deserialize, Serialize};

use crate::{
    chaos::ChaosTracker,
    colors::YELLOW,
    player::skills::{Heirloom, PlayerSkills},
    ui::{damage_numbers::spawn_floating_text_with_shadow, game_fonts::FLOATING_TEXT, UIState},
    DEBUG,
};

use super::{stats::SkillPoints, HeirloomChoiceQueue};

#[derive(Component, Clone, Default, Debug, Serialize, Deserialize)]
pub struct PlayerLevel {
    pub level: u8,
    pub next_level: u8,
    pub xp: u32,
    pub next_level_xp: u32,
}
#[derive(Component, Reflect, FromReflect, Schematic, Default)]
#[reflect(Component, Schematic)]
pub struct ExperienceReward(pub u32);

#[derive(Component)]
pub struct LevelUpParticles;

pub const BASE_LEVEL_EXP_REQ: f32 = 150.;
impl PlayerLevel {
    pub fn new(level: u8) -> Self {
        PlayerLevel {
            level,
            next_level: level + 1,
            xp: 0,
            next_level_xp: f32::floor(BASE_LEVEL_EXP_REQ * f32::powf(level as f32, 0.9)) as u32,
        }
    }

    pub fn add_xp(
        &mut self,
        xp: u32,
        skills: &PlayerSkills,
        chaos_tracker: &mut ChaosTracker,
    ) -> bool {
        let mut did_level_up = false;
        self.xp += (xp as f32 * (1. + skills.get_count(Heirloom::XPGain) as f32 * 0.07)) as u32;

        if self.xp >= self.next_level_xp {
            self.level += 1;
            self.xp -= self.next_level_xp;
            self.next_level_xp =
                f32::floor(BASE_LEVEL_EXP_REQ * f32::powf(self.level as f32, 0.9)) as u32;
            did_level_up = true;
        }
        if *DEBUG {
            debug!(
                "EXP: {:?} LEVEL: {:?} NEXT: {:?}",
                self.xp, self.level, self.next_level_xp
            );
        }
        if did_level_up {
            chaos_tracker.add_chaos(0.1);
        }
        did_level_up
    }
}

pub fn handle_level_up(
    mut player: Query<(&mut PlayerLevel, &mut SkillPoints, &GlobalTransform), Changed<PlayerLevel>>,
    mut skills_queue: ResMut<HeirloomChoiceQueue>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut next_inv_state: ResMut<NextState<UIState>>,
    player_atts: Query<&crate::attributes::LootRateBonus, With<crate::player::Player>>,
) {
    for (mut player_level, mut sp, player_t) in player.iter_mut() {
        if player_level.level == player_level.next_level {
            player_level.next_level += 1;

            sp.count += 1;

            if player_level.level <= 50 {
                let mut rng = rand::thread_rng();
                let loot_bonus = player_atts.get_single().map(|a| a.0).unwrap_or(0);
                skills_queue.add_new_skills_after_levelup(&mut rng, loot_bonus, player_level.level);
                next_inv_state.set(UIState::Skills);
            }

            spawn_floating_text_with_shadow(
                &mut commands,
                &asset_server,
                player_t.translation() + Vec3::new(0., 15., 0.),
                YELLOW,
                "LEVEL UP!".to_string(),
                FLOATING_TEXT,
            );
        }
    }
}

pub fn hide_particles_when_inv_open(
    mut commands: Commands,
    particles: Query<Entity, With<LevelUpParticles>>,
    ui_state: Res<State<UIState>>,
) {
    if ui_state.0 != UIState::Closed {
        for p in particles.iter() {
            commands.entity(p).despawn_recursive();
        }
    }
}
