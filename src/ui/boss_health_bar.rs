use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::{
    attributes::{CurrentHealth, MaxHealth},
    colors::{BLACK, RED, YELLOW},
    enemy::Mob,
    ui::{game_fonts as gf, ui_helpers::Z_DEPTH_BOSS_HEALTH_BAR},
    GAME_HEIGHT,
};

#[derive(Component)]
pub struct BossHealthBar;

#[derive(Component)]
pub struct BossHealthBarFrame;

#[derive(Component)]
pub struct BossNameText;

const BOSS_BAR_WIDTH: f32 = 120.0;
const BOSS_BAR_HEIGHT: f32 = 6.0;
const BOSS_BAR_Y_OFFSET: f32 = GAME_HEIGHT / 2.0 - 60.0; // Top center half of screen
const BOSS_BAR_SPACING: f32 = 22.0; // Vertical spacing between multiple boss bars

/// Spawns the boss health bar UI when a boss spawns
pub fn spawn_boss_health_bar(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    bosses: Query<(Entity, &Mob, &MaxHealth), (Added<Mob>, With<MaxHealth>)>,
    boss_health: Query<&CurrentHealth, With<Mob>>,
    existing_bars: Query<
        &BossEntity,
        Or<(
            With<BossHealthBar>,
            With<BossHealthBarFrame>,
            With<BossNameText>,
        )>,
    >,
) {
    for (boss_entity, mob, max_health) in bosses.iter() {
        if !mob.is_boss() {
            continue;
        }

        // Count how many boss health bars already exist to determine offset
        let existing_boss_count = existing_bars
            .iter()
            .map(|be| be.0)
            .collect::<std::collections::HashSet<_>>()
            .len();

        // Calculate vertical offset based on number of existing boss bars
        let y_offset = BOSS_BAR_Y_OFFSET - (existing_boss_count as f32 * BOSS_BAR_SPACING);

        let boss_name = format!("{}", mob.get_boss_name().unwrap_or("BOSS"));

        // Get initial health to set correct bar size
        let current_health = boss_health
            .get(boss_entity)
            .map(|ch| ch.0)
            .unwrap_or(max_health.0);
        let health_percent = (current_health as f32 / max_health.0 as f32)
            .max(0.0)
            .min(1.0);

        // Spawn boss name text
        let _name_text = commands
            .spawn((
                gf::DISPLAY
                    .text(&asset_server, boss_name.clone(), BLACK)
                    .anchor(Anchor::CENTER)
                    .with_transform(Transform {
                        translation: Vec3::new(0., y_offset + 12.0, Z_DEPTH_BOSS_HEALTH_BAR + 1.),
                        scale: gf::DISPLAY.transform_scale(),
                        ..Default::default()
                    }),
                BossNameText,
                BossEntity(boss_entity),
                RenderLayers::from_layers(&[3]),
            ))
            .id();

        // Spawn health bar frame (background) - centered
        let _bar_frame = commands
            .spawn((
                (
                    Sprite {
                        color: YELLOW,
                        custom_size: Some(Vec2::new(BOSS_BAR_WIDTH, BOSS_BAR_HEIGHT)),
                        ..default()
                    },
                    Transform {
                        translation: Vec3::new(0., y_offset, Z_DEPTH_BOSS_HEALTH_BAR),
                        ..Default::default()
                    },
                    Visibility::Visible,
                ),
                Anchor::CENTER,
                BossHealthBarFrame,
                BossEntity(boss_entity),
                RenderLayers::from_layers(&[3]),
            ))
            .id();

        // Spawn health bar fill with initial health - starts from left edge, scales to right
        let _bar_fill = commands
            .spawn((
                (
                    Sprite {
                        color: RED,
                        custom_size: Some(Vec2::new(BOSS_BAR_WIDTH, BOSS_BAR_HEIGHT)),
                        ..default()
                    },
                    Transform {
                        translation: Vec3::new(
                            -BOSS_BAR_WIDTH / 2.0,
                            y_offset,
                            Z_DEPTH_BOSS_HEALTH_BAR + 1.,
                        ),
                        scale: Vec3::new(health_percent, 1.0, 1.0),
                        ..Default::default()
                    },
                    Visibility::Visible,
                ),
                Anchor::CENTER_LEFT,
                BossHealthBar,
                BossEntity(boss_entity),
                RenderLayers::from_layers(&[3]),
            ))
            .id();
    }
}

#[derive(Component)]
pub(crate) struct BossEntity(pub Entity);

/// Updates the boss health bar based on current health
pub fn update_boss_health_bar(
    mut health_bars: Query<(&mut Transform, &BossEntity), With<BossHealthBar>>,
    bosses: Query<(&CurrentHealth, &MaxHealth), With<Mob>>,
) {
    for (mut bar_transform, boss_entity) in health_bars.iter_mut() {
        if let Ok((current_health, max_health)) = bosses.get(boss_entity.0) {
            let health_percent = (current_health.0 as f32 / max_health.0 as f32)
                .max(0.0)
                .min(1.0);

            bar_transform.scale.x = health_percent;
            // Keep the fill anchored at the left edge of the frame
            bar_transform.translation.x = -BOSS_BAR_WIDTH / 2.0;
        }
    }
}

/// Cleanup system that removes boss health bar UI when boss despawns or dies
pub fn cleanup_boss_health_bar_on_despawn(
    mut commands: Commands,
    boss_ui_elements: Query<
        (Entity, &BossEntity),
        Or<(
            With<BossHealthBar>,
            With<BossHealthBarFrame>,
            With<BossNameText>,
        )>,
    >,
    bosses: Query<Entity, With<Mob>>,
    boss_health: Query<&CurrentHealth, With<Mob>>,
) {
    let existing_boss_entities: std::collections::HashSet<Entity> = bosses.iter().collect();

    for (ui_entity, boss_entity) in boss_ui_elements.iter() {
        // Check if boss entity still exists
        if !existing_boss_entities.contains(&boss_entity.0) {
            commands.entity(ui_entity).despawn();
            continue;
        }

        // Check if boss is dead
        if let Ok(current_health) = boss_health.get(boss_entity.0) {
            if current_health.0 <= 0 {
                commands.entity(ui_entity).despawn();
            }
        }
    }
}
