use bevy::{prelude::*, utils::HashMap};
use bevy_aseprite::aseprite;
use bevy_proto::prelude::ProtoCommands;
use rand::{seq::IteratorRandom, Rng};

use crate::{
    custom_commands::CommandsExt,
    enemy::{CombatAlignment, EliteMob, FollowSpeed, Mob, PendingTint},
    item::object_actions::ObjectAction,
    item::LootTable,
    proto::proto_param::ProtoParam,
    ui::minimap::UpdateMiniMapEvent,
    world::{TileMapPosition, TILE_SIZE},
    GameParam,
};

use super::{Loot, WorldObject};

// Kept for dungeon shrine activate/done animation tags.
aseprite!(pub CombatShrineAnim, "textures/combat_shrine/combat_shrine.ase");

#[derive(Component)]
pub struct CombatShrineMob {
    pub parent_shrine: Entity,
    pub shrine_tile_pos: TileMapPosition,
}

#[derive(Component)]
pub struct CombatShrine {
    pub num_mobs_left: usize,
    pub tile_pos: TileMapPosition,
}

/// Tracks live combat-shrine mob counts by tile so completion still works after the shrine
/// entity is despawned with its chunk (mobs are not chunk children and can outlive it).
#[derive(Resource, Default)]
pub struct CombatShrineMobCounts {
    pub remaining: HashMap<TileMapPosition, usize>,
}

pub struct CombatShrineMobDeathEvent {
    pub shrine: Entity,
    pub tile_pos: TileMapPosition,
}

/// Spawn combat-shrine mobs as soon as the shrine is activated.
pub fn handle_combat_shrine_activate_animation(
    mut shrines: Query<(Entity, &GlobalTransform, &mut CombatShrine), Added<CombatShrine>>,
    mut proto_param: ProtoParam,
    mut commands: Commands,
    mut mob_counts: ResMut<CombatShrineMobCounts>,
) {
    for (e, t, mut shrine) in shrines.iter_mut() {
        let target = shrine.num_mobs_left;
        let possible_spawns = [Mob::Bushling, Mob::StingFly, Mob::SpikeSlime];
        let mut rng = rand::thread_rng();
        let mut spawned = 0usize;
        let mut attempts = 0usize;
        let max_attempts = target.saturating_mul(12).max(12);

        while spawned < target && attempts < max_attempts {
            attempts += 1;
            let offset = Vec2::new(rng.gen_range(-3. ..=3.), rng.gen_range(-3. ..=3.))
                * Vec2::splat(TILE_SIZE.x);
            let spawn_pos = t.translation().truncate() + offset;
            let choice_mob = rng.gen_range(0..possible_spawns.len());
            if let Some(mob) = proto_param.proto_commands.spawn_from_proto(
                possible_spawns[choice_mob].clone(),
                &proto_param.prototypes,
                spawn_pos,
            ) {
                spawned += 1;
                commands.entity(mob).insert(EliteMob);
                proto_param
                    .proto_commands
                    .commands()
                    .entity(mob)
                    .insert(CombatAlignment::Hostile)
                    .insert(LootTable {
                        drops: vec![
                            Loot {
                                item: WorldObject::Coin,
                                min: 1,
                                max: 1,
                                rate: 0.2,
                            },
                            Loot {
                                item: WorldObject::TimeFragment,
                                min: 1,
                                max: 1,
                                rate: 0.02,
                            },
                        ],
                    })
                    .insert(CombatShrineMob {
                        parent_shrine: e,
                        shrine_tile_pos: shrine.tile_pos,
                    });
            }
        }
        shrine.num_mobs_left = spawned;
        mob_counts.remaining.insert(shrine.tile_pos, spawned);
    }
}

/// Combat shrine mobs are always elite, move 25% faster, and use the red endless-mode tint.
pub fn enhance_combat_shrine_mobs(
    mut mobs: Query<
        (Entity, &mut FollowSpeed, Option<&mut TextureAtlasSprite>),
        Added<CombatShrineMob>,
    >,
    mut commands: Commands,
) {
    const COMBAT_SHRINE_SPEED_MULTIPLIER: f32 = 1.25;
    const COMBAT_SHRINE_TINT: Color = Color::rgba(1.0, 0.5, 0.5, 1.0);

    for (entity, mut follow_speed, maybe_sprite) in mobs.iter_mut() {
        follow_speed.0 *= COMBAT_SHRINE_SPEED_MULTIPLIER;
        if let Some(mut sprite) = maybe_sprite {
            sprite.color = COMBAT_SHRINE_TINT;
        } else {
            commands
                .entity(entity)
                .insert(PendingTint(COMBAT_SHRINE_TINT));
        }
    }
}

fn complete_combat_shrine(
    tile_pos: TileMapPosition,
    shrine_entity: Option<Entity>,
    shrines: &mut Query<(Entity, &GlobalTransform, &CombatShrine)>,
    proto_commands: &mut ProtoCommands,
    proto: &ProtoParam,
    commands: &mut Commands,
    game: &mut GameParam,
    minimap_event: &mut EventWriter<UpdateMiniMapEvent>,
) {
    let drop_list = [
        WorldObject::ChestBlock,
        WorldObject::HeirloomChest,
        WorldObject::Coin,
    ];
    let mut rng = rand::thread_rng();
    let picked_drop = *drop_list.iter().choose(&mut rng).unwrap();
    let count = match picked_drop {
        WorldObject::Coin => rng.gen_range(34..53),
        _ => 1,
    };

    let reward_pos = if let Some(shrine_e) = shrine_entity {
        if let Ok((_, t, _)) = shrines.get(shrine_e) {
            t.translation().truncate() + Vec2::new(0., -26.)
        } else {
            crate::world::world_helpers::tile_pos_to_world_pos(tile_pos, false)
                + Vec2::new(0., -26.)
        }
    } else {
        crate::world::world_helpers::tile_pos_to_world_pos(tile_pos, false) + Vec2::new(0., -26.)
    };

    proto_commands.spawn_item_from_proto(
        picked_drop,
        proto,
        reward_pos,
        count,
        Some(game.get_player_level()),
    );

    if let Some(shrine_e) = shrine_entity {
        if shrines.get(shrine_e).is_ok() {
            commands
                .entity(shrine_e)
                .insert(WorldObject::CombatShrineDone)
                .remove::<ObjectAction>()
                .remove::<CombatShrine>();
        }
    }

    game.add_object_to_chunk_cache(tile_pos, WorldObject::CombatShrineDone);
    minimap_event.send(UpdateMiniMapEvent {
        pos: Some(tile_pos),
        new_tile: Some(WorldObject::CombatShrineDone),
    });
}

pub fn handle_shrine_rewards(
    mut shrine_mob_event: EventReader<CombatShrineMobDeathEvent>,
    mut shrines: Query<(Entity, &GlobalTransform, &CombatShrine)>,
    mut proto_commands: ProtoCommands,
    proto: ProtoParam,
    mut commands: Commands,
    mut game: GameParam,
    mut minimap_event: EventWriter<UpdateMiniMapEvent>,
    mut mob_counts: ResMut<CombatShrineMobCounts>,
) {
    for event in shrine_mob_event.iter() {
        let Some(remaining) = mob_counts.remaining.get_mut(&event.tile_pos) else {
            continue;
        };
        *remaining = remaining.saturating_sub(1);
        if *remaining > 0 {
            continue;
        }
        mob_counts.remaining.remove(&event.tile_pos);

        let shrine_entity = shrines.get(event.shrine).ok().map(|(e, _, _)| e);
        complete_combat_shrine(
            event.tile_pos,
            shrine_entity,
            &mut shrines,
            &mut proto_commands,
            &proto,
            &mut commands,
            &mut game,
            &mut minimap_event,
        );
    }
}
