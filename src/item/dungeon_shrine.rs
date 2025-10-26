use bevy::prelude::*;
use bevy_aseprite::{anim::AsepriteAnimation, aseprite, AsepriteBundle};
use bevy_proto::prelude::ProtoCommands;
use rand::{seq::IteratorRandom, Rng};
use strum::IntoEnumIterator;

use crate::{
    assets::{Graphics, SpriteAnchor},
    custom_commands::CommandsExt,
    enemy::{spawn_helpers::can_spawn_mob_here, CombatAlignment, EliteMob, Mob},
    item::{
        combat_shrine::CombatShrineAnim, object_actions::ObjectAction, LootTable, PlaceItemEvent,
    },
    proto::proto_param::ProtoParam,
    world::{world_helpers::world_pos_to_tile_pos, TILE_SIZE},
    GameParam,
};

use super::{Loot, WorldObject};

#[derive(Component)]
pub struct DungeonShrineMob {
    pub parent_shrine: Entity,
}

#[derive(Component)]
pub struct DungeonShrine {
    pub shrine_type: DungeonShrineType,
    pub num_mobs_left: usize,
    pub is_cleared: bool,
    pub is_activated: bool,
}

#[derive(Component, Clone, Debug)]
pub enum DungeonShrineType {
    Weapon,
    Armor,
    Accessory,
}

pub struct DungeonShrineMobDeathEvent(pub Entity);

aseprite!(pub WeaponShrineAnim, "textures/dungeon_shrines/dungeon_weapon_shrine.ase");
aseprite!(pub ArmorShrineAnim, "textures/dungeon_shrines/dungeon_armor_shrine.ase");
aseprite!(pub AccessoryShrineAnim, "textures/dungeon_shrines/dungeon_accessory_shrine.ase");
pub const NUM_DUNGEON_SHRINE_MOBS: usize = 15;

pub fn handle_dungeon_shrine_activation(
    mut shrines: Query<(
        Entity,
        &GlobalTransform,
        &mut DungeonShrine,
        &mut AsepriteAnimation,
    )>,
    mut proto_param: ProtoParam,
    mut commands: Commands,
    game: GameParam,
) {
    for (e, t, mut shrine, mut anim) in shrines.iter_mut() {
        if !shrine.is_activated && anim.current_frame() == 55 {
            shrine.is_activated = true;
            *anim = AsepriteAnimation::from(CombatShrineAnim::tags::DONE);

            // Spawn mobs when shrine is activated
            let mut num_to_spawn = NUM_DUNGEON_SHRINE_MOBS; // Total mobs to spawn
            let possible_spawns = [Mob::FurDevil, Mob::Bushling, Mob::StingFly, Mob::SpikeSlime];
            let mut fallback_count = 0;
            let mut rng = rand::thread_rng();
            let mut elite_count = 0;

            while num_to_spawn > 0 {
                let offset = Vec2::new(rng.gen_range(-16. ..=16.), rng.gen_range(-20. ..=2.))
                    * Vec2::splat(TILE_SIZE.x);
                let spawn_pos = t.translation().truncate() + offset;
                let choice_mob = rng.gen_range(0..possible_spawns.len());

                if can_spawn_mob_here(spawn_pos, &game, &proto_param, fallback_count >= 10) {
                    if let Some(mob) = proto_param.proto_commands.spawn_from_proto(
                        possible_spawns[choice_mob].clone(),
                        &proto_param.prototypes,
                        spawn_pos,
                    ) {
                        fallback_count = 0;
                        num_to_spawn -= 1;

                        // Make 5 of them elite
                        if elite_count < 7 {
                            commands.entity(mob).insert(EliteMob);
                            elite_count += 1;
                        }

                        proto_param
                            .proto_commands
                            .commands()
                            .entity(mob)
                            .insert(CombatAlignment::Hostile)
                            .insert(LootTable {
                                drops: vec![Loot {
                                    item: WorldObject::TimeFragment,
                                    min: 1,
                                    max: 1,
                                    rate: 0.2,
                                }],
                            })
                            .insert(DungeonShrineMob { parent_shrine: e });
                    }
                } else {
                    fallback_count += 1;
                }
            }
        }
    }
}

pub fn handle_dungeon_shrine_rewards(
    mut shrine_mob_event: EventReader<DungeonShrineMobDeathEvent>,
    mut shrines: Query<(
        Entity,
        &GlobalTransform,
        &mut DungeonShrine,
        &mut AsepriteAnimation,
    )>,
    mut proto_commands: ProtoCommands,
    proto: ProtoParam,
    mut commands: Commands,
    mut game: GameParam,
    mut place_item_event: EventWriter<PlaceItemEvent>,
    objs: Query<(Entity, &WorldObject)>,
) {
    let mut is_done = false;
    for event in shrine_mob_event.iter() {
        if let Ok((e, t, mut shrine, _anim)) = shrines.get_mut(event.0) {
            shrine.num_mobs_left -= 1;

            if shrine.num_mobs_left == 0 && !shrine.is_cleared {
                shrine.is_cleared = true;
                is_done = true;

                // Give shrine-specific reward
                let reward_item = match shrine.shrine_type {
                    DungeonShrineType::Weapon => get_weapon_reward(),
                    DungeonShrineType::Armor => get_armor_reward(),
                    DungeonShrineType::Accessory => get_accessory_reward(),
                };

                proto_commands.spawn_item_from_proto(
                    reward_item,
                    &proto,
                    t.translation().truncate() + Vec2::new(0., -44.),
                    1,
                    Some(game.get_player_level()),
                );

                // Mark shrine as done
                let done_object = match shrine.shrine_type {
                    DungeonShrineType::Weapon => WorldObject::WeaponShrineDone,
                    DungeonShrineType::Armor => WorldObject::ArmorShrineDone,
                    DungeonShrineType::Accessory => WorldObject::AccessoryShrineDone,
                };

                commands
                    .entity(e)
                    .insert(done_object)
                    .remove::<ObjectAction>();

                let anchor = proto
                    .get_component::<SpriteAnchor, _>(done_object)
                    .unwrap_or(&SpriteAnchor(Vec2::ZERO));
                game.add_object_to_chunk_cache(
                    world_pos_to_tile_pos(t.translation().truncate() - anchor.0),
                    done_object,
                );

                place_item_event.send(PlaceItemEvent {
                    obj: WorldObject::DungeonExit,
                    pos: t.translation().truncate() + Vec2::new(0., -36.),
                    placed_by_player: false,
                    override_existing_obj: false,
                });
            }
        }
    }
    if is_done {
        for (e, obj) in objs.iter() {
            if obj == &WorldObject::WeaponShrine
                || obj == &WorldObject::ArmorShrine
                || obj == &WorldObject::AccessoryShrine
            {
                let done_object = match obj {
                    &WorldObject::WeaponShrine => WorldObject::WeaponShrineDone,
                    &WorldObject::ArmorShrine => WorldObject::ArmorShrineDone,
                    &WorldObject::AccessoryShrine => WorldObject::AccessoryShrineDone,
                    _ => continue,
                };
                commands
                    .entity(e)
                    .insert(done_object)
                    .remove::<ObjectAction>();
            }
        }
    }
}

fn get_weapon_reward() -> WorldObject {
    let weapon_options = WorldObject::iter()
        .filter(|obj| obj.is_weapon())
        .collect::<Vec<WorldObject>>();
    *weapon_options
        .iter()
        .choose(&mut rand::thread_rng())
        .unwrap()
}

fn get_armor_reward() -> WorldObject {
    let armor_options = WorldObject::iter()
        .filter(|obj| obj.is_armor())
        .collect::<Vec<WorldObject>>();
    *armor_options
        .iter()
        .choose(&mut rand::thread_rng())
        .unwrap()
}

fn get_accessory_reward() -> WorldObject {
    let accessory_options = WorldObject::iter()
        .filter(|obj| obj.is_accessory())
        .collect::<Vec<WorldObject>>();
    *accessory_options
        .iter()
        .choose(&mut rand::thread_rng())
        .unwrap()
}

// Use the same animation as combat shrine for now

pub fn add_dungeon_shrine_visuals_on_spawn(
    mut commands: Commands,
    new_shrines: Query<
        (Entity, &WorldObject, &Transform),
        Or<(Added<WorldObject>, Changed<WorldObject>)>,
    >,
    graphics: Res<Graphics>,
) {
    for (e, obj, t) in new_shrines.iter() {
        match obj {
            WorldObject::WeaponShrine => {
                commands
                    .entity(e)
                    .insert(AsepriteBundle {
                        transform: *t,
                        animation: AsepriteAnimation::from(WeaponShrineAnim::tags::IDLE),
                        aseprite: graphics.weapon_shrine_anim.as_ref().unwrap().clone(),
                        ..default()
                    })
                    .insert(Name::new("WEAPON_SHRINE"));
            }
            WorldObject::ArmorShrine => {
                commands
                    .entity(e)
                    .insert(AsepriteBundle {
                        transform: *t,
                        animation: AsepriteAnimation::from(ArmorShrineAnim::tags::IDLE),
                        aseprite: graphics.armor_shrine_anim.as_ref().unwrap().clone(),
                        ..default()
                    })
                    .insert(Name::new("ARMOR_SHRINE"));
            }
            WorldObject::AccessoryShrine => {
                commands
                    .entity(e)
                    .insert(AsepriteBundle {
                        transform: *t,
                        animation: AsepriteAnimation::from(AccessoryShrineAnim::tags::IDLE),
                        aseprite: graphics.accessory_shrine_anim.as_ref().unwrap().clone(),
                        ..default()
                    })
                    .insert(Name::new("ACCESSORY_SHRINE"));
            }
            WorldObject::WeaponShrineDone
            | WorldObject::ArmorShrineDone
            | WorldObject::AccessoryShrineDone => {
                commands
                    .entity(e)
                    .insert(AsepriteAnimation::from(CombatShrineAnim::tags::DONE))
                    .insert(Name::new("DUNGEON_SHRINE_DONE"));
            }
            _ => {}
        }
    }
}
