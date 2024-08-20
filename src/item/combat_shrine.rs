use bevy::prelude::*;
use bevy_aseprite::{anim::AsepriteAnimation, aseprite, AsepriteBundle};
use bevy_proto::prelude::ProtoCommands;
use rand::{seq::IteratorRandom, Rng};

use crate::{
    assets::{Graphics, SpriteAnchor},
    custom_commands::CommandsExt,
    enemy::{spawn_helpers::can_spawn_mob_here, CombatAlignment, EliteMob, Mob},
    item::{object_actions::ObjectAction, LootTable},
    player::levels::PlayerLevel,
    proto::proto_param::ProtoParam,
    world::{world_helpers::world_pos_to_tile_pos, TILE_SIZE},
    GameParam,
};

use super::{Loot, WorldObject};

#[derive(Component)]
pub struct CombatShrineMob {
    pub parent_shrine: Entity,
}

#[derive(Component)]
pub struct CombatShrine {
    pub num_mobs_left: usize,
}

pub struct CombatShrineMobDeathEvent(pub Entity);
pub fn handle_combat_shrine_activate_animation(
    mut shrines: Query<(
        Entity,
        &GlobalTransform,
        &mut CombatShrine,
        &mut AsepriteAnimation,
    )>,
    mut proto_param: ProtoParam,
    mut commands: Commands,
    game: GameParam,
) {
    for (e, t, mut shrine, mut anim) in shrines.iter_mut() {
        if anim.current_frame() == 55 {
            *anim = AsepriteAnimation::from(CombatShrineAnim::tags::DONE);
            let mut num_to_spawn = shrine.num_mobs_left.clone();
            let possible_spawns = [Mob::FurDevil, Mob::Bushling, Mob::StingFly, Mob::SpikeSlime];
            let mut fallback_count = 0;
            let mut rng = rand::thread_rng();
            while num_to_spawn > 0 {
                let offset = Vec2::new(rng.gen_range(-3. ..=3.), rng.gen_range(-3. ..=3.))
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
                        //last mob is elite
                        if num_to_spawn == 0 {
                            commands.entity(mob).insert(EliteMob);
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
                            .insert(CombatShrineMob { parent_shrine: e });
                    }
                }
            }
        }
    }
}
pub fn handle_shrine_rewards(
    mut shrine_mob_event: EventReader<CombatShrineMobDeathEvent>,
    mut shrines: Query<(
        Entity,
        &GlobalTransform,
        &mut CombatShrine,
        &mut AsepriteAnimation,
    )>,
    mut proto_commands: ProtoCommands,
    proto: ProtoParam,
    mut commands: Commands,
    mut game: GameParam,
    level: Query<&PlayerLevel>,
) {
    for event in shrine_mob_event.iter() {
        if let Ok((e, t, mut shrine, mut anim)) = shrines.get_mut(event.0) {
            shrine.num_mobs_left -= 1;
            let drop_list = [
                WorldObject::WoodSword,
                WorldObject::WoodSword,
                WorldObject::Sword,
                WorldObject::Dagger,
                WorldObject::WoodBow,
                WorldObject::Claw,
                // WorldObject::MiracleSeed,
                WorldObject::FireStaff,
                WorldObject::BasicStaff,
                WorldObject::MagicWhip,
                WorldObject::LeatherPants,
                WorldObject::LeatherShoes,
                WorldObject::LeatherTunic,
                WorldObject::LeatherPants,
                WorldObject::LeatherShoes,
                WorldObject::LeatherTunic,
                WorldObject::Ring,
                WorldObject::Pendant,
            ];
            if shrine.num_mobs_left == 0 {
                // give rewards
                proto_commands.spawn_item_from_proto(
                    drop_list
                        .iter()
                        .choose(&mut rand::thread_rng())
                        .unwrap()
                        .clone(),
                    &proto,
                    t.translation().truncate() + Vec2::new(0., -26.), // offset so it doesn't spawn on the shrine
                    1,
                    Some(level.single().level),
                );
                commands
                    .entity(e)
                    .insert(WorldObject::CombatShrineDone)
                    .remove::<ObjectAction>();
                *anim = AsepriteAnimation::from(CombatShrineAnim::tags::DONE);
                let anchor = proto
                    .get_component::<SpriteAnchor, _>(WorldObject::CombatShrine)
                    .unwrap_or(&SpriteAnchor(Vec2::ZERO));
                game.add_object_to_chunk_cache(
                    world_pos_to_tile_pos(t.translation().truncate() - anchor.0),
                    WorldObject::CombatShrineDone,
                );
            }
        }
    }
}

aseprite!(pub CombatShrineAnim, "textures/combat_shrine/combat_shrine.ase");

pub fn add_shrine_visuals_on_spawn(
    mut commands: Commands,
    new_shrines: Query<(Entity, &WorldObject, &Transform), Added<WorldObject>>,
    graphics: Res<Graphics>,
) {
    for (e, obj, t) in new_shrines.iter() {
        if obj == &WorldObject::CombatShrine {
            commands
                .entity(e)
                .insert(AsepriteBundle {
                    transform: *t,
                    animation: AsepriteAnimation::from(CombatShrineAnim::tags::IDLE),
                    aseprite: graphics.combat_shrine_anim.as_ref().unwrap().clone(),
                    ..default()
                })
                .insert(Name::new("COMBAT"));
        } else if obj == &WorldObject::CombatShrineDone {
            commands
                .entity(e)
                .insert(AsepriteBundle {
                    transform: *t,
                    animation: AsepriteAnimation::from(CombatShrineAnim::tags::DONE),
                    aseprite: graphics.combat_shrine_anim.as_ref().unwrap().clone(),
                    ..default()
                })
                .insert(Name::new("COMBAT_DONE"));
        }
    }
}
