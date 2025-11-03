use bevy::prelude::*;
use bevy_proto::backend::schematics::ReflectSchematic;
use bevy_proto::prelude::Schematic;
use bevy_rapier2d::prelude::Collider;
use serde::{Deserialize, Serialize};
use strum_macros::EnumIter;

use crate::{
    enemy::Mob,
    inventory::{Inventory, ItemStack},
    item::projectile::{Projectile, RangedAttack, RangedAttackEvent},
    player::Player,
    proto::proto_param::ProtoParam,
    ui::InventoryState,
    world::y_sort::YSort,
    FairyPetSprite, SlimePetSprite,
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
}

/// Marker component for pet spawners (pets that can be collected by the player)
#[derive(Component, Debug, Clone)]
pub struct PetSpawner {
    pub pet_type: Pet,
}
impl Pet {
    pub fn get_idle_anim(&self) -> &str {
        match self {
            Pet::Slime => SlimePetSprite::tags::IDLE,
            Pet::Fairy => FairyPetSprite::tags::IDLE,
        }
    }
    pub fn get_walk_anim(&self) -> &str {
        match self {
            Pet::Slime => SlimePetSprite::tags::WALK,
            Pet::Fairy => FairyPetSprite::tags::WALK,
        }
    }
    pub fn get_aseprite_path(&self) -> &str {
        match self {
            Pet::Slime => SlimePetSprite::PATH,
            Pet::Fairy => FairyPetSprite::PATH,
        }
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
    pub hot_bar_slot: usize,
    pub is_following_player: bool,
}
#[derive(Clone)]
pub struct UpdatePetWeaponEvent;

impl PetState {
    pub fn change_wepon(&mut self, new_weapon: Option<ItemStack>, proto: &ProtoParam) {
        self.weapon_slot = new_weapon.clone();

        match new_weapon {
            Some(item) => {
                self.attack_cooldown =
                    Timer::from_seconds(item.attributes.attack_cooldown, TimerMode::Repeating);
                self.projectile = proto
                    .get_component::<RangedAttack, _>(item.obj_type)
                    .expect("Weapon with no RangedAttack Projectile!")
                    .0
                    .clone();
                self.min_target_distance = item
                    .obj_type
                    .is_ranged_weapon()
                    .then(|| 100.0)
                    .unwrap_or(25.0);
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
    if !keys.just_pressed(KeyCode::Z) {
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
            hot_bar_slot: 5,
            is_following_player: false,
        };

        commands.entity(pet_entity).insert(pet_state);

        // Add pet-specific ability timer components
        match pet {
            crate::pets::state::Pet::Slime => {
                commands
                    .entity(pet_entity)
                    .insert(crate::pets::pet_abilities::SlimeShieldTimer(
                        Timer::from_seconds(30.0, TimerMode::Repeating),
                    ));
            }
            crate::pets::state::Pet::Fairy => {
                commands
                    .entity(pet_entity)
                    .insert(crate::pets::pet_abilities::FairyHealTimer(
                        Timer::from_seconds(35.0, TimerMode::Repeating),
                    ));
            }
        }

        events.send(UpdatePetWeaponEvent);
    }
}

pub fn update_pet_weapon_on_inv_change(
    mut pets: Query<&mut PetState, With<Pet>>,
    player_inventory: Query<&Inventory>,
    proto: ProtoParam,
    inv_state: Res<InventoryState>,
    mut events: EventReader<UpdatePetWeaponEvent>,
) {
    for _ in events.iter() {
        if let Ok(inventory) = player_inventory.get_single() {
            for mut pet_state in pets.iter_mut() {
                if inv_state.active_hotbar_slot == pet_state.hot_bar_slot {
                    pet_state.change_wepon(None, &proto);
                    continue;
                }
                // Check if player has a weapon in their hot bar slot
                if let Some(item_stack) = &inventory.items.items[pet_state.hot_bar_slot] {
                    let weapon_obj = item_stack.get_obj();
                    if weapon_obj.is_weapon() {
                        pet_state.change_wepon(Some(item_stack.item_stack.clone()), &proto);
                    } else {
                        // Player has no weapon in hot bar slot
                        pet_state.change_wepon(None, &proto);
                    }
                } else {
                    pet_state.change_wepon(None, &proto);
                }
            }
        }
    }
}
