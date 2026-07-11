use bevy::prelude::*;

use crate::{
    item::object_actions::ObjectAction,
    player::skills::HeirloomChoiceQueue,
    ui::{key_input_guide::InteractionGuideTrigger, minimap::UpdateMiniMapEvent},
    world::TileMapPosition,
    GameParam,
};

use super::WorldObject;

/// Per-shrine state; only present on an un-consumed heirloom shrine and
/// removed on use. `SparseSet` for the same reason as the other shrine state
/// components.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct HeirloomShrineState {
    pub is_used: bool,
    pub tile_pos: TileMapPosition,
}

pub fn handle_heirloom_shrine_completion(
    shrines: Query<(Entity, &HeirloomShrineState)>,
    mut commands: Commands,
    mut game: GameParam,
    mut minimap_event: EventWriter<UpdateMiniMapEvent>,
) {
    for (e, shrine) in shrines.iter() {
        if shrine.is_used {
            commands
                .entity(e)
                .insert(WorldObject::HeirloomShrineDone)
                .remove::<ObjectAction>()
                .remove::<InteractionGuideTrigger>()
                .remove::<HeirloomShrineState>();

            game.add_object_to_chunk_cache(shrine.tile_pos, WorldObject::HeirloomShrineDone);

            minimap_event.send(UpdateMiniMapEvent {
                pos: Some(shrine.tile_pos),
                new_tile: Some(WorldObject::HeirloomShrineDone),
            });
        }
    }
}

/// Populate the HeirloomChoiceQueue when the shrine UI opens
pub fn handle_heirloom_shrine_ui_setup(
    mut skills_queue: ResMut<HeirloomChoiceQueue>,
    shrine_query: Query<&HeirloomShrineState>,
    player_atts: Query<
        (
            &crate::attributes::LootRateBonus,
            &crate::player::levels::PlayerLevel,
        ),
        With<crate::player::Player>,
    >,
) {
    let shrine_just_activated = shrine_query.iter().any(|shrine| !shrine.is_used);

    if shrine_just_activated {
        let mut rng = rand::thread_rng();
        let (loot_bonus, player_level) = player_atts
            .get_single()
            .map(|a| (a.0 .0, a.1.level))
            .unwrap_or((0, 1));
        skills_queue.add_new_skills_after_levelup(&mut rng, loot_bonus, player_level);
    }
}
