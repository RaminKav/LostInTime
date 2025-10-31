use bevy::prelude::*;

use crate::attributes::CurrentHealth;
use crate::player::Player;

use crate::attributes::modifiers::ModifyHealthEvent;

use super::state::Pet;

/// Timer component for Slime pet's shield ability
#[derive(Component, Debug)]
pub struct SlimeShieldTimer(pub Timer);

/// Timer component for Fairy pet's heal ability
#[derive(Component, Debug)]
pub struct FairyHealTimer(pub Timer);

/// Marker added to the player when we temporarily grant a 1-point shield from the Slime pet.
#[derive(Component, Debug)]
pub struct SlimeTempShield;
#[derive(Component, Debug)]
pub struct SlimeTempShieldSprite;

/// Slime pet ability: periodically grants the player a temporary 1-point shield if they have none.
/// The shield blocks one instance of damage and then is removed (restoring previous max shield).
pub fn slime_shield_ability(
    time: Res<Time>,
    mut slime_pets: Query<&mut SlimeShieldTimer, With<Pet>>,
    mut player_query: Query<(Entity, Option<&SlimeTempShield>), With<Player>>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    // Tick all slime pet timers and grant shield when ready
    for mut timer in slime_pets.iter_mut() {
        timer.0.tick(time.delta());

        // When timer finishes, grant shield if needed
        if timer.0.finished() {
            if let Ok((player_e, temp_shield_opt)) = player_query.get_single_mut() {
                // Grant a temporary 1-point shield only if player currently has none
                if temp_shield_opt.is_none() {
                    commands.entity(player_e).insert(SlimeTempShield);

                    commands
                        .spawn(SpriteBundle {
                            texture: asset_server.load("textures/effects/SlimeShield.png"),
                            sprite: Sprite {
                                custom_size: Some(Vec2::new(34., 34.)),
                                ..Default::default()
                            },
                            transform: Transform {
                                translation: Vec3::new(0., 0., 1.),
                                scale: Vec3::new(1., 1., 1.),
                                ..Default::default()
                            },
                            ..Default::default()
                        })
                        .insert(SlimeTempShieldSprite)
                        .set_parent(player_e);
                }
            }
        }
    }
}

/// Fairy pet ability: periodically heals the player for a small amount.
pub fn fairy_heal_ability(
    time: Res<Time>,
    mut fairy_pets: Query<&mut FairyHealTimer, With<Pet>>,
    mut heal_events: EventWriter<ModifyHealthEvent>,
    player_health: Query<&CurrentHealth, With<Player>>,
) {
    // Tick all fairy pet timers and heal when ready
    for mut timer in fairy_pets.iter_mut() {
        timer.0.tick(time.delta());

        // When timer finishes, heal the player
        if timer.0.finished() {
            if player_health.get_single().is_ok() {
                heal_events.send(ModifyHealthEvent(15));
            }
        }
    }
}
