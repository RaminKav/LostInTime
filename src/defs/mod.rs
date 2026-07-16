pub mod generated;
pub mod lookup;
pub mod parse;
pub mod registry;
pub mod spawn;
pub mod types;

use bevy::prelude::*;

pub use registry::GameDefs;

pub struct DefsPlugin;

impl Plugin for DefsPlugin {
    fn build(&self, app: &mut App) {
        let mut defs = GameDefs::default();
        generated::register_all(&mut defs);
        app.insert_resource(defs)
            .add_plugin(spawn::DefsSpawnPlugin);
    }
}
