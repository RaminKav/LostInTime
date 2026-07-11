use bevy::prelude::*;

use crate::{
    combat::pickup_radius::{pull_all_eligible_ground_items_to_player, BeingPulledToPlayer},
    inventory::{Inventory, ItemStack},
    item::{object_actions::ObjectAction, ItemDrop},
    pets::state::Pet,
    player::Player,
    proto::proto_param::ProtoParam,
    ui::{
        key_input_guide::InteractionGuideTrigger, minimap::UpdateMiniMapEvent,
    },
    world::TileMapPosition,
    GameParam,
};

use super::WorldObject;

#[derive(Component)]
pub struct GambleShrine {
    pub success: bool,
    pub tile_pos: TileMapPosition,
}

pub struct GambleShrineEvent {
    pub entity: Entity,
    pub success: bool,
}

pub fn handle_gamble_shrine_rewards(
    shrines: Query<(Entity, &GambleShrine), Added<GambleShrine>>,
    proto: ProtoParam,
    mut commands: Commands,
    mut game: GameParam,
    mut minimap_event: EventWriter<UpdateMiniMapEvent>,
    item_drop_query: Query<(Entity, &ItemStack), (With<ItemDrop>, Without<BeingPulledToPlayer>)>,
    inv: Query<&Inventory, With<Player>>,
    pets: Query<(), With<Pet>>,
) {
    for (e, shrine) in shrines.iter() {
        if shrine.success {
            commands.entity(e).remove::<GambleShrine>();

            pull_all_eligible_ground_items_to_player(
                &mut commands,
                &item_drop_query,
                &inv,
                &pets,
                &proto,
            );

            commands
                .entity(e)
                .insert(WorldObject::GambleShrineDone)
                .remove::<ObjectAction>();
            game.add_object_to_chunk_cache(shrine.tile_pos, WorldObject::GambleShrineDone);

            minimap_event.send(UpdateMiniMapEvent {
                pos: Some(shrine.tile_pos),
                new_tile: Some(WorldObject::GambleShrineDone),
            });
        } else {
            let obj_action = proto
                .get_component::<ObjectAction, _>(WorldObject::GambleShrine)
                .expect("Gamble shrine missing ObjectAction");
            commands
                .entity(e)
                .remove::<GambleShrine>()
                .insert(InteractionGuideTrigger {
                    text: Some("Interact".to_string()),
                    activation_distance: 32.,
                    icon_stack: Some(ItemStack::crate_icon_stack(WorldObject::Coin)),
                })
                .insert(obj_action.clone());
        }
    }
}
