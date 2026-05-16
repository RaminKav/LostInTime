use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::{
    player::{
        skills::{ActiveSkill, ClassSkillSlots, PlayerSkills},
        Player,
    },
    Game,
};

use super::player_hud::{skill_slot_cooldown_progress, SkillCooldownOverlay};

const BAR_SIZE: Vec2 = Vec2::new(10., 2.);
const BAR_Y_OFFSET: f32 = 14.;

#[derive(Component)]
pub struct PlayerMovementCooldownBar;

/// On the player entity after [`spawn_player_movement_cooldown_bar`] runs.
#[derive(Component)]
pub struct PlayerMovementCooldownBarSpawned;

#[derive(Component)]
pub struct PlayerMovementCooldownBarBg;

#[derive(Component)]
pub struct PlayerMovementCooldownBarFill;

pub fn spawn_player_movement_cooldown_bar(
    mut commands: Commands,
    players: Query<Entity, (With<Player>, Without<PlayerMovementCooldownBarSpawned>)>,
) {
    for player in players.iter() {
        let bar = commands
            .spawn((
                SpatialBundle {
                    transform: Transform::from_translation(Vec3::new(0., BAR_Y_OFFSET, 0.5)),
                    visibility: Visibility::Hidden,
                    ..default()
                },
                PlayerMovementCooldownBar,
                Name::new("Movement Cooldown Bar"),
            ))
            .with_children(|parent| {
                parent
                    .spawn(SpriteBundle {
                        sprite: Sprite {
                            color: Color::rgba(1., 1., 1., 0.25),
                            custom_size: Some(BAR_SIZE),
                            ..default()
                        },
                        transform: Transform::from_xyz(0., 0., 0.),
                        ..default()
                    })
                    .insert(PlayerMovementCooldownBarBg);

                parent
                    .spawn(SpriteBundle {
                        sprite: Sprite {
                            color: Color::WHITE,
                            custom_size: Some(Vec2::new(0., BAR_SIZE.y)),
                            anchor: Anchor::CenterLeft,
                            ..default()
                        },
                        transform: Transform::from_xyz(-BAR_SIZE.x / 2., 0., 0.1),
                        ..default()
                    })
                    .insert(PlayerMovementCooldownBarFill);
            })
            .id();

        commands
            .entity(player)
            .add_child(bar)
            .insert(PlayerMovementCooldownBarSpawned);
    }
}

pub fn update_player_movement_cooldown_bar(
    player_skills: Query<&PlayerSkills, With<Player>>,
    class_slots: Query<&ClassSkillSlots, With<Player>>,
    overlays: Query<&SkillCooldownOverlay>,
    game: Res<Game>,
    mut bar_roots: Query<&mut Visibility, With<PlayerMovementCooldownBar>>,
    mut fill_sprites: Query<&mut Sprite, With<PlayerMovementCooldownBarFill>>,
) {
    let Ok(skills) = player_skills.get_single() else {
        return;
    };
    let Ok(slots) = class_slots.get_single() else {
        return;
    };

    let Some(movement_slot) = skills.movement_skill_slot() else {
        for mut visibility in bar_roots.iter_mut() {
            *visibility = Visibility::Hidden;
        }
        return;
    };

    let roll_slot = skills.has_active_skill(ActiveSkill::Roll);
    let overlay = overlays.iter().find(|o| o.index == movement_slot);
    let progress = skill_slot_cooldown_progress(
        movement_slot,
        roll_slot,
        slots,
        &game.player_state.player_dash_cooldown,
        overlay,
    );

    let show = progress.is_some();
    for mut visibility in bar_roots.iter_mut() {
        *visibility = if show {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }

    if let Some(progress) = progress {
        for mut sprite in fill_sprites.iter_mut() {
            sprite.custom_size = Some(Vec2::new(BAR_SIZE.x * progress, BAR_SIZE.y));
        }
    }
}
