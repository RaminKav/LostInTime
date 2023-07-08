use std::fs::File;
use std::io::Write;

use bevy::{ecs::system::SystemState, prelude::*, tasks::IoTaskPool};

use crate::{
    inventory::ItemStack,
    item::{Wall, WorldObject},
};

#[derive(Component)]
pub struct SchematicObject;

pub struct SchematicPlugin;
impl Plugin for SchematicPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems((save_schematic_scene, mark_new_world_obj_as_schematic));
    }
}
fn mark_new_world_obj_as_schematic(
    mut commands: Commands,
    query: Query<Entity, (Added<Wall>, Without<ItemStack>)>,
) {
    for e in query.iter() {
        if let Some(mut entity_cmds) = commands.get_entity(e) {
            entity_cmds.insert(SchematicObject);
        }
    }
}
fn save_schematic_scene(world: &mut World) {
    let mut state: SystemState<(Query<Entity, With<SchematicObject>>, Res<Input<KeyCode>>)> =
        SystemState::new(world);
    let mut builder = DynamicSceneBuilder::from_world(&world);
    let (query, key_input) = state.get(world);
    if key_input.just_pressed(KeyCode::J) {
        println!("Saving schematic scene...");
        for e in query.iter() {
            builder.extract_entity(e);
        }
        let scene = builder.build();

        let type_registry = world.resource::<AppTypeRegistry>();
        let serialized_scene = scene.serialize_ron(type_registry).unwrap();

        info!("{}", serialized_scene);

        IoTaskPool::get()
            .spawn(async move {
                // Write the scene RON data to file
                File::create(format!("assets/scenes/test.scn.ron"))
                    .and_then(|mut file| file.write(serialized_scene.as_bytes()))
                    .expect("Error while writing scene to file");
            })
            .detach();
    }
}
