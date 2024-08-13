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
pub struct GambleShrine {
    pub success: bool,
}

pub struct GambleShrineEvent {
    pub entity: Entity,
    pub success: bool,
}

pub fn handle_gamble_shrine_rewards(
    mut shrines: Query<(
        Entity,
        &GlobalTransform,
        &GambleShrine,
        &mut AsepriteAnimation,
    )>,
    mut proto_commands: ProtoCommands,
    proto: ProtoParam,
    mut commands: Commands,
    mut game: GameParam,
) {
    for (e, t, mut shrine, mut anim) in shrines.iter_mut() {
        if shrine.success {
            if anim.current_frame() == 50 {
                *anim = AsepriteAnimation::from(GambleShrineAnim::tags::DONE);
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
                    .insert(WorldObject::GambleShrineDone)
                    .remove::<ObjectAction>();
                let anchor = proto
                    .get_component::<SpriteAnchor, _>(WorldObject::GambleShrine)
                    .unwrap_or(&SpriteAnchor(Vec2::ZERO));
                game.add_object_to_chunk_cache(
                    world_pos_to_tile_pos(t.translation().truncate() - anchor.0),
                    WorldObject::GambleShrineDone,
                );
            }
        } else {
            if anim.current_frame() == 86 {
                *anim = AsepriteAnimation::from(GambleShrineAnim::tags::DONE);
            }
        }
    }
}

aseprite!(pub GambleShrineAnim, "textures/gamble_shrine/GambleShrine.ase");

pub fn add_gamble_visuals_on_spawn(
    mut commands: Commands,
    new_shrines: Query<(Entity, &WorldObject, &Transform), Added<WorldObject>>,
    graphics: Res<Graphics>,
) {
    for (e, obj, t) in new_shrines.iter() {
        if obj == &WorldObject::GambleShrine {
            commands
                .entity(e)
                .insert(AsepriteBundle {
                    transform: *t,
                    animation: AsepriteAnimation::from(GambleShrineAnim::tags::IDLE),
                    aseprite: graphics.gamble_shrine_anim.as_ref().unwrap().clone(),
                    ..default()
                })
                .insert(Name::new("GAMBLE"));
        } else if obj == &WorldObject::GambleShrineDone {
            commands
                .entity(e)
                .insert(AsepriteBundle {
                    transform: *t,
                    animation: AsepriteAnimation::from(GambleShrineAnim::tags::DONE),
                    aseprite: graphics.gamble_shrine_anim.as_ref().unwrap().clone(),
                    ..default()
                })
                .insert(Name::new("GAMBLE_DONE"));
        }
    }
}
