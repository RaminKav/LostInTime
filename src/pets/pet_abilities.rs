use bevy::prelude::*;
use rand::Rng;

use crate::attributes::CurrentHealth;
use crate::combat::damage_tracker::PetAbilityStats;
use crate::custom_commands::CommandsExt;
use crate::ecs_helpers::SafeHierarchyExt;
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

/// Marker added to the player when we temporarily grant a 1-point shield from
/// the Slime pet. Cycles on/off as shields generate and break — stored
/// `SparseSet` to keep the player in one archetype.
#[derive(Component, Debug)]
#[component(storage = "SparseSet")]
pub struct SlimeTempShield;

/// Marker on the slime shield sprite entity; tracked alongside
/// [`SlimeTempShield`] and paired with it one-to-one. `SparseSet` for the
/// same reason as [`SlimeTempShield`].
#[derive(Component, Debug)]
#[component(storage = "SparseSet")]
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
        if timer.0.is_finished() {
            if let Ok((player_e, temp_shield_opt)) = player_query.single_mut() {
                // Grant a temporary 1-point shield only if player currently has none
                if temp_shield_opt.is_none() {
                    pet_stats.shields_generated += 1;
                    commands.entity(player_e).insert(SlimeTempShield);

                    commands
                        .spawn((
                            Sprite {
                                image: asset_server.load("textures/effects/SlimeShield.png"),
                                custom_size: Some(Vec2::new(34., 34.)),
                                ..default()
                            },
                            Transform {
                                translation: Vec3::new(0., 0., 1.),
                                scale: Vec3::new(1., 1., 1.),
                                ..Default::default()
                            },
                        ))
                        .insert(SlimeTempShieldSprite)
                        .safe_set_parent(player_e);
                }
            }
        }
    }
}

/// Fairy pet ability: periodically heals the player for a small amount.
pub fn fairy_heal_ability(
    time: Res<Time>,
    mut fairy_pets: Query<&mut FairyHealTimer, With<Pet>>,
    mut heal_events: MessageWriter<ModifyHealthEvent>,
    player_health: Query<&CurrentHealth, With<Player>>,
    mut pet_stats: ResMut<PetAbilityStats>,
) {
    const HEAL_AMOUNT: i32 = 15;
    // Tick all fairy pet timers and heal when ready
    for mut timer in fairy_pets.iter_mut() {
        timer.0.tick(time.delta());

        // When timer finishes, heal the player
        if timer.0.is_finished() {
            if player_health.single().is_ok() {
                pet_stats.healing += HEAL_AMOUNT as i64;
                heal_events.write(ModifyHealthEvent(HEAL_AMOUNT));
            }
        }
    }
}

/// Porkipine pet ability: damages the player for 1 HP every 1.5s when above 30% health.
pub fn porkipine_damage_ability(
    time: Res<Time>,
    mut porkipine_pets: Query<&mut PorkipineDamageTimer, With<Pet>>,
    mut damage_events: MessageWriter<ModifyHealthEvent>,
    health_percent: Res<crate::PlayerHealthPercent>,
    mut pet_stats: ResMut<PetAbilityStats>,
) {
    for mut timer in porkipine_pets.iter_mut() {
        timer.0.tick(time.delta());

        if timer.0.is_finished() && health_percent.percent > 0.30 {
            pet_stats.self_damage += 1;
            damage_events.write(ModifyHealthEvent(-1));
        }
    }
}

/// GoldenPig pet ability: every 15s drops 1–5 coins near the player, 5% chance to drop 25 coins instead.
pub fn golden_pig_coin_ability(
    time: Res<Time>,
    mut golden_pig_pets: Query<(&GlobalTransform, &mut GoldenPigCoinTimer), With<Pet>>,
    mut commands: Commands,
    proto: ProtoParam,
    mut pet_stats: ResMut<PetAbilityStats>,
) {
    for (pet_txfm, mut timer) in golden_pig_pets.iter_mut() {
        timer.0.tick(time.delta());

        if timer.0.is_finished() {
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

                commands.spawn_item_from_proto(
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
