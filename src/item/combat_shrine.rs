use bevy::prelude::*;
use bevy_proto::prelude::ProtoCommands;
use rand::seq::IteratorRandom;

use crate::{
    custom_commands::CommandsExt, item::object_actions::ObjectAction,
    proto::proto_param::ProtoParam,
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
    mut shrines: Query<(Entity, &GlobalTransform, &mut CombatShrine)>,
    mut proto_commands: ProtoCommands,
    proto: ProtoParam,
    mut commands: Commands,
) {
    for event in shrine_mob_event.iter() {
        if let Ok((e, t, mut shrine)) = shrines.get_mut(event.0) {
            shrine.num_mobs_left -= 1;
            let drop_list = [
                WorldObject::WoodSword,
                WorldObject::Sword,
                WorldObject::Dagger,
                WorldObject::WoodBow,
                WorldObject::Claw,
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
            }
        }
    }
}
