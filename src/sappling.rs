use bevy::prelude::*;
use bevy_proto::backend::schematics::{ReflectSchematic, Schematic};

use crate::{
    assets::SpriteAnchor,
    item::{PlaceItemEvent, WorldObject},
    proto::proto_param::ProtoParam,
    GameState,
};

#[derive(Component, Reflect, FromReflect, Schematic, Default)]
#[reflect(Component, Schematic)]
pub struct Sappling(pub Timer);

#[derive(Component, Reflect, FromReflect, Schematic, Default)]
#[reflect(Component, Schematic)]
pub struct GrowsInto(WorldObject);

pub struct SapplingPlugin;

impl Plugin for SapplingPlugin {
    fn build(&self, app: &mut App) {
        app.add_system(tick_sappling_color.in_set(Update(GameState::Main)));
    }
}

pub fn tick_sappling_color(
    time: Res<Time>,
    mut query: Query<(&WorldObject, &mut Sappling, &GrowsInto, &GlobalTransform)>,
    mut events: EventWriter<PlaceItemEvent>,
    proto_param: ProtoParam,
) {
    for (obj, mut sappling_state, growth, tfxm) in query.iter_mut() {
        sappling_state.0.tick(time.delta());

        if sappling_state.0.finished() {
            //swap sappling to next stage, or a tree
            //TODO: make it pick between 2 tree types
            let anchor = proto_param
                .get_component::<SpriteAnchor, _>(*obj)
                .unwrap_or(&SpriteAnchor(Vec2::ZERO));
            events.send(PlaceItemEvent {
                pos: tfxm.translation().truncate() - anchor.0,
                obj: growth.0,
                placed_by_player: false,
                override_existing_obj: true,
            });
        }
    }
}
