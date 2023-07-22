use std::fs::File;
use std::io::Write;

use bevy::{
    ecs::system::SystemState,
    math::{Affine3A, Mat3A, Vec3A},
    prelude::*,
    tasks::IoTaskPool,
};
use bevy_inspector_egui::quick::ResourceInspectorPlugin;
use strum_macros::{Display, IntoStaticStr};
mod schematic_spawner;
use crate::{
    inventory::ItemStack,
    item::{handle_placing_world_object, Foliage, PlaceItemEvent, Wall, WorldObject},
    player::Player,
    world::world_helpers::world_pos_to_tile_pos,
    GameParam, GameState,
};

use self::schematic_spawner::{
    attempt_to_spawn_schematic_in_chunk, give_chunks_schematic_spawners,
};
#[derive(Component, Debug, Clone, Reflect, Default, IntoStaticStr, Display)]
pub enum SchematicType {
    #[default]
    House,
}
#[derive(Component)]
pub struct SchematicBuilderObject;

#[derive(Resource, Default, Debug, Reflect, Clone)]
pub struct SchematicToggle {
    enabled: bool,
}
pub struct SchematicPlugin;
impl Plugin for SchematicPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SchematicToggle::default())
            .register_type::<SchematicToggle>()
            .add_plugin(ResourceInspectorPlugin::<SchematicToggle>::default())
            .add_systems(
                (
                    handle_new_scene_entities_parent_chunk.before(handle_placing_world_object),
                    save_schematic_scene,
                    load_schematic,
                    clear_schematic_entities,
                    mark_new_world_obj_as_schematic,
                    attempt_to_spawn_schematic_in_chunk,
                    give_chunks_schematic_spawners,
                )
                    .in_set(OnUpdate(GameState::Main)),
            );
    }
}
fn mark_new_world_obj_as_schematic(
    mut commands: Commands,
    query: Query<Entity, (Added<Wall>, Without<ItemStack>)>,
    toggle: Res<SchematicToggle>,
) {
    if toggle.enabled {
        for e in query.iter() {
            if let Some(mut entity_cmds) = commands.get_entity(e) {
                entity_cmds.insert(SchematicBuilderObject);
            }
        }
    }
}
fn clear_schematic_entities(
    mut commands: Commands,
    query: Query<Entity, With<SchematicBuilderObject>>,
    key_input: Res<Input<KeyCode>>,
) {
    if key_input.just_pressed(KeyCode::C) {
        for e in query.iter() {
            if let Some(entity_cmds) = commands.get_entity(e) {
                entity_cmds.despawn_recursive();
            }
        }
    }
}
fn save_schematic_scene(world: &mut World) {
    let mut state: SystemState<(
        Query<Entity, With<SchematicBuilderObject>>,
        Res<Input<KeyCode>>,
    )> = SystemState::new(world);
    let (query, key_input) = state.get(world);
    if key_input.just_pressed(KeyCode::J) {
        println!("Saving schematic scene...");
        let type_registry = AppTypeRegistry::default();
        {
            let mut writer = type_registry.write();
            writer.register::<WorldObject>();
            writer.register::<Wall>();
            writer.register::<Foliage>();
            writer.register::<Transform>();
            writer.register::<GlobalTransform>();
            writer.register::<Vec3>();
            writer.register::<Quat>();
            writer.register::<Affine3A>();
            writer.register::<Mat3A>();
            writer.register::<Vec3A>();
        }
        let mut builder =
            DynamicSceneBuilder::from_world_with_type_registry(&world, type_registry.clone());
        for e in query.iter() {
            builder.extract_entity(e);
        }
        let scene = builder.build();

        let serialized_scene = scene.serialize_ron(&type_registry).unwrap();

        IoTaskPool::get()
            .spawn(async move {
                // Write the scene RON data to file
                File::create(format!("assets/scenes/house.scn.ron"))
                    .and_then(|mut file| file.write(serialized_scene.as_bytes()))
                    .expect("Error while writing scene to file");
            })
            .detach();
    }
}

fn load_schematic(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    key_input: Res<Input<KeyCode>>,
    game: GameParam,
) {
    if key_input.just_pressed(KeyCode::M) {
        println!(
            "Loading schematic scene... {}",
            game.game.player_state.position
        );
        commands
            .spawn(DynamicSceneBundle {
                scene: asset_server.load("scenes/test.scn.ron"),
                ..default()
            })
            .insert(Name::new("Schematic"));
    }
}

pub fn handle_new_scene_entities_parent_chunk(
    game: GameParam,
    new_scenes: Query<
        (Entity, &Children, &Transform),
        (With<Handle<DynamicScene>>, Added<Children>),
    >,
    obj_data: Query<(&WorldObject, &GlobalTransform), (With<WorldObject>, Without<Player>)>,
    mut commands: Commands,
    mut place_item_event: EventWriter<PlaceItemEvent>,
) {
    for (e, children, scene_txfm) in new_scenes.iter() {
        let mut x_offset: f32 = 1_000_000_000.;
        let mut y_offset: f32 = 1_000_000_000.;
        for child in children.iter() {
            if let Ok((_, txfm)) = obj_data.get(*child) {
                if txfm.translation().x < x_offset {
                    x_offset = txfm.translation().x;
                }
                if txfm.translation().y < y_offset {
                    y_offset = txfm.translation().y;
                }
            }
        }
        for child in children.iter() {
            if let Ok((obj, txfm)) = obj_data.get(*child) {
                let pos = scene_txfm.translation.truncate()
                    + (txfm.translation().truncate() - Vec2::new(x_offset, y_offset));

                let mut is_valid_to_spawn = false;
                if let Some(tile_data) = game.get_tile_data(world_pos_to_tile_pos(pos)) {
                    let tile_type = tile_data.block_type;

                    let filter = game
                        .world_generation_params
                        .obj_allowed_tiles_map
                        .get(obj)
                        .unwrap()
                        .clone();
                    for allowed_tile in filter.iter() {
                        if tile_type.contains(allowed_tile) {
                            is_valid_to_spawn = true;
                        }
                    }
                } else {
                    is_valid_to_spawn = true;
                }
                if is_valid_to_spawn {
                    place_item_event.send(PlaceItemEvent {
                        obj: *obj,
                        pos,
                        is_from_player_item: false,
                    });
                } else {
                    println!("did not spawn, Invalid tile type for object: {:?}", obj);
                }
            }
        }
        commands.entity(e).despawn_recursive();
    }
}
