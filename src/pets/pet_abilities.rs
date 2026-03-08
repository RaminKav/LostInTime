use bevy::prelude::*;
use bevy_proto::prelude::ProtoCommands;
use rand::Rng;

use crate::attributes::CurrentHealth;
use crate::combat::damage_tracker::PetAbilityStats;
use crate::custom_commands::CommandsExt;
use crate::item::WorldObject;
use crate::player::Player;
use crate::proto::proto_param::ProtoParam;

use crate::attributes::modifiers::ModifyHealthEvent;

use super::state::Pet;

/// Timer component for Slime pet's shield ability
#[derive(Component, Debug)]
pub struct SlimeShieldTimer(pub Timer);

/// Timer component for Fairy pet's heal ability
#[derive(Component, Debug)]
pub struct FairyHealTimer(pub Timer);

/// Timer component for Porkipine pet's self-damage (if above 30% health)
#[derive(Component, Debug)]
pub struct PorkipineDamageTimer(pub Timer);

/// Timer component for GoldenPig pet's coin drop ability
#[derive(Component, Debug)]
pub struct GoldenPigCoinTimer(pub Timer);

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
    mut pet_stats: ResMut<PetAbilityStats>,
) {
    // Tick all slime pet timers and grant shield when ready
    for mut timer in slime_pets.iter_mut() {
        timer.0.tick(time.delta());

        // When timer finishes, grant shield if needed
        if timer.0.finished() {
            if let Ok((player_e, temp_shield_opt)) = player_query.get_single_mut() {
                // Grant a temporary 1-point shield only if player currently has none
                if temp_shield_opt.is_none() {
                    pet_stats.shields_generated += 1;
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
    mut pet_stats: ResMut<PetAbilityStats>,
) {
    const HEAL_AMOUNT: i32 = 15;
    // Tick all fairy pet timers and heal when ready
    for mut timer in fairy_pets.iter_mut() {
        timer.0.tick(time.delta());

        // When timer finishes, heal the player
        if timer.0.finished() {
            if player_health.get_single().is_ok() {
                pet_stats.healing += HEAL_AMOUNT as i64;
                heal_events.send(ModifyHealthEvent(HEAL_AMOUNT));
            }
        }
    }
}

/// Porkipine pet ability: damages the player for 1 HP every 1.5s when above 30% health.
pub fn porkipine_damage_ability(
    time: Res<Time>,
    mut porkipine_pets: Query<&mut PorkipineDamageTimer, With<Pet>>,
    mut damage_events: EventWriter<ModifyHealthEvent>,
    health_percent: Res<crate::PlayerHealthPercent>,
    mut pet_stats: ResMut<PetAbilityStats>,
) {
    for mut timer in porkipine_pets.iter_mut() {
        timer.0.tick(time.delta());

        if timer.0.finished() && health_percent.percent > 0.30 {
            pet_stats.self_damage += 1;
            damage_events.send(ModifyHealthEvent(-1));
        }
    }
}

/// GoldenPig pet ability: every 15s drops 1–5 coins near the player, 5% chance to drop 25 coins instead.
pub fn golden_pig_coin_ability(
    time: Res<Time>,
    mut golden_pig_pets: Query<(&GlobalTransform, &mut GoldenPigCoinTimer), With<Pet>>,
    mut proto_commands: ProtoCommands,
    proto: ProtoParam,
    mut pet_stats: ResMut<PetAbilityStats>,
) {
    for (pet_txfm, mut timer) in golden_pig_pets.iter_mut() {
        timer.0.tick(time.delta());

        if timer.0.finished() {
            let mut rng = rand::thread_rng();
            let count = if rng.gen_ratio(5, 100) {
                25
            } else {
                rng.gen_range(1..=5)
            };

            pet_stats.coins += count;

            let d = 32.0;
            for _ in 0..count {
                let drop_offset = Vec2::new(rng.gen_range(-d..d), rng.gen_range(-d..d));

                proto_commands.spawn_item_from_proto(
                    WorldObject::Coin,
                    &proto,
                    pet_txfm.translation().truncate() + drop_offset,
                    1,
                    None,
                );
            }
        }
    }
}
