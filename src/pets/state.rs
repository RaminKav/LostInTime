use bevy::prelude::*;
use bevy_proto::backend::schematics::ReflectSchematic;
use bevy_proto::prelude::Schematic;
use bevy_rapier2d::prelude::Collider;
use serde::{Deserialize, Serialize};
use strum_macros::EnumIter;

use crate::{
    blessings::{Blessing, OwnedBlessings},
    enemy::Mob,
    inventory::{Inventory, ItemStack},
    item::projectile::{Projectile, RangedAttack, RangedAttackEvent},
    player::Player,
    proto::proto_param::ProtoParam,
    world::y_sort::YSort,
    FairyPetSprite, SlimePetSprite, DEBUG,
};

#[derive(
    Component,
    Reflect,
    FromReflect,
    Schematic,
    Default,
    EnumIter,
    Clone,
    Debug,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
)]
#[reflect(Component, Schematic)]
pub enum Pet {
    #[default]
    Slime,
    Fairy,
    Porkipine,
    GoldenPig,
    /// Passive: +1% projectile size per level (cape). Active: 1% chance to double
    /// weapon/skill projectile spawn scale while this pet is in your party.
    Goliath,
}

/// Marker component for pet spawners (pets that can be collected by the player)
#[derive(Component, Debug, Clone)]
pub struct PetSpawner {
    pub pet_type: Pet,
}
impl Pet {
    pub fn get_idle_anim(&self) -> &str {
        match self {
            Pet::Slime | Pet::Porkipine | Pet::GoldenPig | Pet::Goliath => {
                SlimePetSprite::tags::IDLE
            }
            Pet::Fairy => FairyPetSprite::tags::IDLE,
        }
    }
    pub fn get_walk_anim(&self) -> &str {
        match self {
            Pet::Slime | Pet::Porkipine | Pet::GoldenPig | Pet::Goliath => {
                SlimePetSprite::tags::WALK
            }
            Pet::Fairy => FairyPetSprite::tags::WALK,
        }
    }
    pub fn get_aseprite_path(&self) -> &str {
        match self {
            Pet::Slime | Pet::Porkipine | Pet::GoldenPig | Pet::Goliath => SlimePetSprite::PATH,
            Pet::Fairy => FairyPetSprite::PATH,
        }
    }

    /// Compute pet passive stats based on level (similar to SkillClass::compute_cape_stats)
    pub fn compute_pet_stats(&self, level: i32) -> crate::attributes::ItemAttributes {
        use crate::attributes::{AttributeQuality, AttributeValue, ItemAttributes};

        let mut stats = ItemAttributes::default();
        let quality = if level > 10 {
            AttributeQuality::High
        } else if level >= 5 {
            AttributeQuality::Average
        } else {
            AttributeQuality::Low
        };

        match self {
            Pet::Slime => {
                // +1 Defence per level
                stats.defence = AttributeValue::new(level * 1, quality, 1.);
            }
            Pet::Fairy => {
                // +1 Health Regen per level
                stats.health_regen = AttributeValue::new(level * 1, quality, 1.);
            }
            Pet::Porkipine => {
                // +1% Lifesteal per level
                stats.lifesteal =
                    AttributeValue::new((level as f32 * 0.5).round() as i32, quality, 1.);
            }
            Pet::GoldenPig => {
                // +1.5% Pickup Range per level
                stats.pickup_range =
                    AttributeValue::new((level as f32 * 2.).round() as i32, quality, 1.5);
            }
            Pet::Goliath => {
                // +1% projectile size per level (same attribute as size on gear)
                stats.size = AttributeValue::new(level * 1, quality, 1.);
            }
        }

        stats
    }
}

#[derive(Component, Reflect, FromReflect, Schematic, Default)]
#[reflect(Component, Schematic)]
pub struct PetState {
    pub max_distance_from_player: f32,
    pub max_target_distance: f32,
    pub min_target_distance: f32,
    pub min_distance_from_player: f32,
    pub follow_speed: f32,
    pub weapon_slot: Option<ItemStack>,
    pub current_target: Option<Entity>,
    pub attack_cooldown: Timer,
    pub projectile: Projectile,
    pub is_following_player: bool,
}
#[derive(Clone)]
pub struct UpdatePetWeaponEvent;

impl PetState {
    pub fn change_wepon(
        &mut self,
        new_weapon: Option<ItemStack>,
        proto: &ProtoParam,
        attack_speed_buff: f32,
    ) {
        self.weapon_slot = new_weapon.clone();

        match new_weapon {
            Some(item) => {
                let obj = item.obj_type;
                let pet_att_speed_nerf = if obj.is_melee_weapon() {
                    1.25
                } else if obj.is_magic_weapon() {
                    2.
                } else {
                    1.55
                };
                self.attack_cooldown = Timer::from_seconds(
                    item.attributes.attack_cooldown * pet_att_speed_nerf * attack_speed_buff,
                    TimerMode::Repeating,
                );
                self.projectile = proto
                    .get_component::<RangedAttack, _>(obj)
                    .expect("Weapon with no RangedAttack Projectile!")
                    .0
                    .clone();
                self.min_target_distance = obj.is_ranged_weapon().then(|| 85.0).unwrap_or(25.0);
            }
            None => {
                self.attack_cooldown = Timer::from_seconds(10., TimerMode::Repeating);
                self.projectile = Projectile::None;
            }
        };
    }
}

pub fn handle_inv_change_pet_wep_update(
    inv_updates: Query<&Inventory, Changed<Inventory>>,
    mut events: EventWriter<UpdatePetWeaponEvent>,
) {
    if inv_updates.is_empty() {
        return;
    }
    events.send(UpdatePetWeaponEvent);
}

pub fn find_new_target(
    mobs: Query<(Entity, &Transform, &Mob), With<Mob>>,
    mut pets: Query<(&Transform, &mut PetState), With<Pet>>,
    player_txfm: Query<&GlobalTransform, With<Player>>,
) {
    for (pet_transform, mut pet_state) in pets.iter_mut() {
        let distance_from_player = pet_transform
            .translation
            .distance(player_txfm.single().translation());
        if distance_from_player > pet_state.max_distance_from_player {
            pet_state.current_target = None;
            continue;
        }
        let mut closest_mob: Option<(Entity, f32, &Mob)> = None;
        for (mob_entity, mob_transform, mob) in mobs.iter() {
            let distance = pet_transform
                .translation
                .distance(mob_transform.translation);

            if distance <= pet_state.max_target_distance {
                match closest_mob {
                    Some((_, closest_distance, _)) => {
                        if distance < closest_distance {
                            closest_mob = Some((mob_entity, distance, mob));
                        }
                    }
                    None => {
                        closest_mob = Some((mob_entity, distance, mob));
                    }
                }
            }
        }
        pet_state.current_target = closest_mob.map(|(e, _, _)| e);
    }
}

pub fn use_weapon(
    mut pets: Query<
        (Entity, &Transform, &mut PetState),
        (With<Pet>, Without<Player>, Without<Mob>),
    >,
    mobs: Query<(&Transform, Entity), (With<Mob>, Without<Pet>, Without<Player>)>,
    mut ranged_attack_event: EventWriter<RangedAttackEvent>,
    time: Res<Time>,
) {
    for (pet_e, pet_transform, mut pet_state) in pets.iter_mut() {
        pet_state.attack_cooldown.tick(time.delta());

        let Some(weapon_slot) = pet_state.weapon_slot.as_ref() else {
            continue;
        };

        let Some(target_entity) = pet_state.current_target else {
            continue;
        };
        let Ok((target_transform, _)) = mobs.get(target_entity) else {
            continue;
        };

        let delta = target_transform.translation - pet_transform.translation;
        let dir = delta.normalize_or_zero().truncate();
        if pet_state.attack_cooldown.finished() {
            ranged_attack_event.send(RangedAttackEvent {
                projectile: pet_state.projectile.clone(),
                direction: dir,
                from_enemy: false,
                from_entity: Some(pet_e),
                is_followup_proj: false,
                mana_cost: None,
                mana_cost_heirloom: None,
                dmg_override: Some(weapon_slot.attributes.attack.value),
                pos_override: if pet_state.projectile.is_anchored_to_player_pos() {
                    Some(Vec2::ZERO)
                } else {
                    None
                },
                spawn_delay: 0.1,
            });
            pet_state.attack_cooldown.reset();
        }
    }
}

pub fn test_spawn_pet(mut commands: Commands, _proto: ProtoParam, keys: Res<Input<KeyCode>>) {
    if !keys.just_pressed(KeyCode::Z) || !*DEBUG {
        return;
    }
    commands.spawn((
        Pet::Slime,
        YSort(0.001),
        Collider::capsule(Vec2::new(0., -6.), Vec2::new(0., -6.), 5.0),
        Transform::from_xyz(0.0, 0.0, 1.0),
        Name::new("Test Pet"),
    ));
}

pub fn configure_pet_on_spawn(
    mut commands: Commands,
    new_pets: Query<(Entity, &Pet), Added<Pet>>,
    mut events: EventWriter<UpdatePetWeaponEvent>,
) {
    for (pet_entity, pet) in new_pets.iter() {
        let pet_state = PetState {
            max_distance_from_player: 16. * 9.,
            max_target_distance: 16. * 9.,
            min_target_distance: 30.0,
            min_distance_from_player: 30.0,
            follow_speed: 50.0,
            weapon_slot: None,
            current_target: None,
            attack_cooldown: Timer::from_seconds(10.0, TimerMode::Repeating),
            projectile: Projectile::None,
            is_following_player: false,
        };

        commands.entity(pet_entity).insert(pet_state);

        // Add pet-specific ability timer components
        match pet {
            crate::pets::state::Pet::Slime => {
                commands
                    .entity(pet_entity)
                    .insert(crate::pets::pet_abilities::SlimeShieldTimer(
                        Timer::from_seconds(45.0, TimerMode::Repeating),
                    ));
            }
            crate::pets::state::Pet::Fairy => {
                commands
                    .entity(pet_entity)
                    .insert(crate::pets::pet_abilities::FairyHealTimer(
                        Timer::from_seconds(35.0, TimerMode::Repeating),
                    ));
            }
            crate::pets::state::Pet::Porkipine => {
                commands.entity(pet_entity).insert(
                    crate::pets::pet_abilities::PorkipineDamageTimer(Timer::from_seconds(
                        1.5,
                        TimerMode::Repeating,
                    )),
                );
            }
            crate::pets::state::Pet::GoldenPig => {
                commands
                    .entity(pet_entity)
                    .insert(crate::pets::pet_abilities::GoldenPigCoinTimer(
                        Timer::from_seconds(15.0, TimerMode::Repeating),
                    ));
            }
            crate::pets::state::Pet::Goliath => {}
        }

        events.send(UpdatePetWeaponEvent);
    }
}

/// Refreshes every pet's weapon from the single-slot `Inventory::pet_items` container.
///
/// Previously this read from `items[pet_state.hot_bar_slot]` (last hotbar slots were
/// reserved for pet weapons), but the hotbar is no longer used for that purpose — the
/// dedicated Pet slot in the equipment panel is now the sole source of truth.
///
/// All pets currently share the same Pet slot; if per-pet weapon slots are needed later,
/// extend `Inventory::pet_items` to `with_size(N)` and index by pet id.
pub fn update_pet_weapon_on_inv_change(
    mut pets: Query<&mut PetState, With<Pet>>,
    player_inventory: Query<&Inventory>,
    proto: ProtoParam,
    mut events: EventReader<UpdatePetWeaponEvent>,
    blessings: Query<&OwnedBlessings>,
) {
    for _ in events.iter() {
        if let Ok(inventory) = player_inventory.get_single() {
            let blessings = blessings.single();
            let attack_speed_buff = if blessings.has_blessing(Blessing::PetAttackSpeed) {
                0.75
            } else {
                1.0
            };
            let pet_weapon = inventory
                .pet_items
                .items
                .get(0)
                .cloned()
                .flatten()
                .filter(|stack| stack.get_obj().is_weapon())
                .map(|stack| stack.item_stack.clone());
            for mut pet_state in pets.iter_mut() {
                pet_state.change_wepon(pet_weapon.clone(), &proto, attack_speed_buff);
            }
        }
    }
}
