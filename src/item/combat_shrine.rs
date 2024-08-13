use bevy::prelude::*;
use bevy_aseprite::{anim::AsepriteAnimation, aseprite, AsepriteBundle};
use bevy_proto::prelude::ProtoCommands;
use rand::seq::IteratorRandom;

use crate::{
    assets::{Graphics, SpriteAnchor},
    custom_commands::CommandsExt,
    item::object_actions::ObjectAction,
    proto::proto_param::ProtoParam,
    world::world_helpers::world_pos_to_tile_pos,
    GameParam,
};

use super::WorldObject;

#[derive(Component)]
pub struct CombatShrineMob {
    pub parent_shrine: Entity,
}

#[derive(Component)]
pub struct CombatShrine {
    pub num_mobs_left: usize,
}

pub struct CombatShrineMobDeathEvent(pub Entity);

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
) {
    for event in shrine_mob_event.iter() {
        if let Ok((e, t, mut shrine, mut anim)) = shrines.get_mut(event.0) {
            shrine.num_mobs_left -= 1;
            let drop_list = [
                WorldObject::WoodSword,
                WorldObject::WoodSword,
                WorldObject::WoodSword,
                WorldObject::Sword,
                WorldObject::Sword,
                WorldObject::Dagger,
                WorldObject::WoodBow,
                WorldObject::Claw,
                // WorldObject::MiracleSeed,
                WorldObject::FireStaff,
                WorldObject::BasicStaff,
                WorldObject::MagicWhip,
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
                    t.translation().truncate() + Vec2::new(0., -18.), // offset so it doesn't spawn on the shrine
                    1,
                    Some(1),
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
