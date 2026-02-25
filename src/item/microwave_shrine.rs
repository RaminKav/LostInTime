use bevy::prelude::*;
use bevy_aseprite::{anim::AsepriteAnimation, aseprite, AsepriteBundle};

use crate::{
    assets::Graphics,
    item::object_actions::ObjectAction,
    ui::{key_input_guide::InteractionGuideTrigger, minimap::UpdateMiniMapEvent},
    world::TileMapPosition,
    GameParam,
};

use super::WorldObject;

#[derive(Component)]
pub struct MicrowaveShrineState {
    pub is_used: bool,
    pub tile_pos: TileMapPosition,
}
aseprite!(pub MicrowaveShrineAnim, "textures/gamble_shrine/gamble_shrine_purple.ase");

pub fn add_microwave_shrine_visuals_on_spawn(
    mut commands: Commands,
    new_shrines: Query<
        (Entity, &WorldObject, &Transform),
        Or<(Added<WorldObject>, Changed<WorldObject>)>,
    >,
    graphics: Res<Graphics>,
) {
    for (e, obj, t) in new_shrines.iter() {
        if obj == &WorldObject::MicrowaveShrine {
            commands
                .entity(e)
                .insert(AsepriteBundle {
                    transform: *t,
                    animation: AsepriteAnimation::from(MicrowaveShrineAnim::tags::IDLE),
                    aseprite: graphics.microwave_anim.as_ref().unwrap().clone(), // reuse gamble sprite for now
                    ..default()
                })
                .insert(Name::new("MICROWAVE_SHRINE"));
        } else if obj == &WorldObject::MicrowaveShrineDone {
            commands
                .entity(e)
                .insert(AsepriteBundle {
                    transform: *t,
                    animation: AsepriteAnimation::from(MicrowaveShrineAnim::tags::DONE),
                    aseprite: graphics.microwave_anim.as_ref().unwrap().clone(),
                    ..default()
                })
                .insert(Name::new("MICROWAVE_SHRINE_DONE"));
        }
    }
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
