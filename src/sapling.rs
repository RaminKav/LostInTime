use bevy::prelude::*;

use crate::{
    assets::SpriteAnchor,
    item::{PlaceItemEvent, WorldObject},
    proto::proto_param::ProtoParam,
    GameState,
};

#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct Sapling(pub Timer);

#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component)]
pub struct GrowsInto(pub WorldObject);

pub struct SaplingPlugin;

impl Plugin for SaplingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, tick_sapling_color.run_if(in_state(GameState::Main)));
    }
}

pub fn tick_sapling_color(
    time: Res<Time>,
    mut query: Query<(&WorldObject, &mut Sapling, &GrowsInto, &GlobalTransform)>,
    mut events: MessageWriter<PlaceItemEvent>,
    proto_param: ProtoParam,
) {
    for (obj, mut sapling_state, growth, tfxm) in query.iter_mut() {
        sapling_state.0.tick(time.delta());

        if sapling_state.0.is_finished() {
            //swap sapling to next stage, or a tree
            //TODO: make it pick between 2 tree types
            let anchor = proto_param
                .get_component::<SpriteAnchor, _>(*obj)
                .unwrap_or(&SpriteAnchor(Vec2::ZERO));
            events.write(PlaceItemEvent {
                pos: tfxm.translation().truncate() - anchor.0,
                obj: growth.0,
                placed_by_player: false,
                override_existing_obj: true,
            });
        }
    }
}
