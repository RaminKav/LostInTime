use bevy::{prelude::*, render::view::RenderLayers};
use bevy_proto::prelude::{ReflectSchematic, Schematic};

use crate::{animations::AnimationTimer, ui::InventoryState, world::y_sort::YSort};

use super::stats::SkillPoints;

#[derive(Component, Debug)]
pub struct PlayerLevel {
    pub level: u8,
    pub xp: u32,
    pub next_level_xp: u32,
}
#[derive(Component, Reflect, FromReflect, Schematic, Default)]
#[reflect(Component, Schematic)]
pub struct ExperienceReward(pub u32);

#[derive(Component)]
pub struct LevelUpParticles;

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

//TODO: move player xp after mob death system here, out of combat.rs handle_enemy_death
// pub fn player_xp_system(
//     mut player_query: Query<(&mut PlayerLevel, &mut ExperienceReward)>,
//     mut xp_query: Query<&mut ExperienceReward>,
// ) {
//     for (mut player_level, mut xp_reward) in player_query.iter_mut() {
//         player_level.add_xp(xp_reward.0);
//         xp_reward.0 = 0;
//     }
//     for mut xp_reward in xp_query.iter_mut() {
//         xp_reward.0 = 0;
//     }
// }
pub fn handle_level_up(
    mut player: Query<(&PlayerLevel, &mut SkillPoints), Changed<PlayerLevel>>,
    mut next_level: Local<u8>,
) {
    for (player_level, mut sp) in player.iter_mut() {
        if player_level.level == 1 {
            *next_level = 2;
            return;
        }
        if player_level.level == *next_level {
            sp.count += 1;
            *next_level += 1;
        }
    }
}
pub fn spawn_particles_when_leveling(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlases: ResMut<Assets<TextureAtlas>>,
    player_xp: Query<&SkillPoints>,
    existing_particles: Query<Entity, With<LevelUpParticles>>,
) {
    if player_xp.single().count > 0 && existing_particles.iter().next().is_none() {
        let texture_handle = asset_server.load("textures/effects/levelup.png");
        let texture_atlas =
            TextureAtlas::from_grid(texture_handle, Vec2::new(27.0, 25.0), 4, 1, None, None);
        let texture_atlas_handle = texture_atlases.add(texture_atlas);
        commands.spawn((
            SpriteSheetBundle {
                texture_atlas: texture_atlas_handle,
                transform: Transform::from_translation(Vec3::new(9.5, -52.5, 5.)),
                ..default()
            },
            AnimationTimer(Timer::from_seconds(0.2, TimerMode::Repeating)),
            YSort(0.),
            LevelUpParticles,
            RenderLayers::from_layers(&[3]),
            Name::new("Level Particles"),
        ));
    } else if player_xp.single().count == 0 && existing_particles.iter().next().is_some() {
        commands.entity(existing_particles.single()).despawn();
    }
}

pub fn hide_particles_when_inv_open(
    mut commands: Commands,
    particles: Query<Entity, With<LevelUpParticles>>,
    inv_state: Res<InventoryState>,
) {
    if inv_state.open {
        for p in particles.iter() {
            commands.entity(p).despawn();
        }
    }
}
