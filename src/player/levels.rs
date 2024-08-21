use bevy::{prelude::*, render::view::RenderLayers};
use bevy_proto::prelude::{ReflectSchematic, Schematic};
use serde::{Deserialize, Serialize};

use crate::{
    animations::AnimationTimer,
    colors::YELLOW,
    ui::{damage_numbers::spawn_floating_text_with_shadow, UIState},
    DEBUG, GAME_HEIGHT,
};

use super::{stats::SkillPoints, SkillChoiceQueue};

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

pub const BASE_LEVEL_EXP_REQ: f32 = 320.;
impl PlayerLevel {
    pub fn new(level: u8) -> Self {
        PlayerLevel {
            level,
            next_level: level + 1,
            xp: 0,
            next_level_xp: f32::floor(BASE_LEVEL_EXP_REQ * (1. + (0.25 * (level as f32 - 1.))))
                as u32,
        }
    }

    pub fn add_xp(&mut self, xp: u32) {
        self.xp += xp;
        if self.xp >= self.next_level_xp {
            self.level += 1;
            self.xp = self.xp - self.next_level_xp;
            self.next_level_xp =
                f32::floor(BASE_LEVEL_EXP_REQ * (1. + (0.25 * (self.level as f32 - 1.)))) as u32;
        }
        if *DEBUG {
            debug!(
                "EXP: {:?} LEVEL: {:?} NEXT: {:?}",
                self.xp, self.level, self.next_level_xp
            );
        }
    }
}

pub fn handle_level_up(
    mut player: Query<(&mut PlayerLevel, &mut SkillPoints, &GlobalTransform), Changed<PlayerLevel>>,
    mut skills_queue: ResMut<SkillChoiceQueue>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    for (mut player_level, mut sp, player_t) in player.iter_mut() {
        if player_level.level == player_level.next_level {
            player_level.next_level += 1;

            sp.count += 1;
            let mut rng = rand::thread_rng();
            skills_queue.add_new_skills_after_levelup(&mut rng);
            spawn_floating_text_with_shadow(
                &mut commands,
                &asset_server,
                player_t.translation() + Vec3::new(0., 15., 0.),
                YELLOW,
                "LEVEL UP!".to_string(),
            );
        }
    }
}
pub fn spawn_particles_when_leveling(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlases: ResMut<Assets<TextureAtlas>>,
    existing_particles: Query<Entity, With<LevelUpParticles>>,
    ui_state: Res<State<UIState>>,
    skill_queue: Res<SkillChoiceQueue>,
) {
    if ui_state.0 != UIState::Closed {
        return;
    }
    if !skill_queue.queue.is_empty() && existing_particles.iter().next().is_none() {
        let texture_handle = asset_server.load("textures/effects/levelup.png");
        let texture_atlas =
            TextureAtlas::from_grid(texture_handle, Vec2::new(27.0, 25.0), 4, 1, None, None);
        let texture_atlas_handle = texture_atlases.add(texture_atlas);
        let particles = commands
            .spawn((
                SpriteSheetBundle {
                    texture_atlas: texture_atlas_handle,
                    transform: Transform::from_translation(Vec3::new(
                        9.5,
                        -GAME_HEIGHT / 2. + 40.5,
                        5.,
                    )),
                    ..default()
                },
                AnimationTimer(Timer::from_seconds(0.2, TimerMode::Repeating)),
                LevelUpParticles,
                RenderLayers::from_layers(&[3]),
                Name::new("Level Particles"),
            ))
            .id();
        commands
            .spawn(SpriteBundle {
                texture: asset_server.load("textures/BKey.png"),
                transform: Transform::from_translation(Vec3::new(0.5, 15.5, 1.)),
                sprite: Sprite {
                    custom_size: Some(Vec2::new(10., 10.)),
                    ..Default::default()
                },
                visibility: Visibility::Inherited,
                ..Default::default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .set_parent(particles);
    } else if skill_queue.queue.is_empty() && existing_particles.iter().next().is_some() {
        commands.entity(existing_particles.single()).despawn();
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
