use bevy::prelude::*;

use crate::{
    item::object_actions::ObjectAction,
    ui::{key_input_guide::InteractionGuideTrigger, minimap::UpdateMiniMapEvent},
    world::TileMapPosition,
    GameParam,
};

use super::WorldObject;

/// Per-shrine state; only present on an un-consumed microwave shrine and
/// removed on use. Stored `SparseSet` because lots of shrines get spawned
/// per run and each cycles through the state component when interacted.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct MicrowaveShrineState {
    pub is_used: bool,
    pub tile_pos: TileMapPosition,
}

pub fn handle_microwave_shrine_completion(
    shrines: Query<(Entity, &MicrowaveShrineState)>,
    mut commands: Commands,
    mut game: GameParam,
    mut minimap_event: EventWriter<UpdateMiniMapEvent>,
) {
    for (e, shrine) in shrines.iter() {
        if shrine.is_used {
            commands
                .entity(e)
                .insert(WorldObject::MicrowaveShrineDone)
                .remove::<ObjectAction>()
                .remove::<InteractionGuideTrigger>()
                .remove::<MicrowaveShrineState>();

            game.add_object_to_chunk_cache(shrine.tile_pos, WorldObject::MicrowaveShrineDone);

            minimap_event.send(UpdateMiniMapEvent {
                pos: Some(shrine.tile_pos),
                new_tile: Some(WorldObject::MicrowaveShrineDone),
            });
        }
    }
}

/// When the microwave shrine UI is closed (e.g. Esc), clear the active ref.
/// Do not set is_used here — the shrine only goes to Done after a successful swap.
pub fn handle_microwave_shrine_esc(
    active: Option<Res<crate::ui::microwave_shrine_ui::MicrowaveShrineActive>>,
    mut commands: Commands,
    curr_ui_state: Res<State<crate::ui::UIState>>,
) {
    if curr_ui_state.0 != crate::ui::UIState::MicrowaveShrine {
        if active.is_some() {
            commands.remove_resource::<crate::ui::microwave_shrine_ui::MicrowaveShrineActive>();
        }
    }
}
