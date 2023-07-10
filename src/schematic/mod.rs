use std::fs::File;
use std::io::Write;

use bevy::{
    ecs::system::SystemState,
    math::{Affine3A, Mat3A, Vec3A},
    prelude::*,
    scene::SceneInstance,
    tasks::IoTaskPool,
};
use bevy_proto::prelude::{ProtoCommands, Prototypes};

use crate::{
    inventory::ItemStack,
    item::{Foliage, Placeable, Wall, WorldObject},
    player::Player,
    proto::proto_param::ProtoParam,
    ui::minimap::UpdateMiniMapEvent,
    GameParam,
};

#[derive(Component)]
pub struct SchematicObject;

pub struct SchematicPlugin;
impl Plugin for SchematicPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems((
            save_schematic_scene,
            load_schematic,
            handle_new_scene_entities_parent_chunk,
            mark_new_world_obj_as_schematic,
        ));
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
    let (query, key_input) = state.get(world);
    if key_input.just_pressed(KeyCode::J) {
        println!("Saving schematic scene...");
        let type_registry = AppTypeRegistry::default();
        {
            let mut writer = type_registry.write();
            writer.register::<WorldObject>();
            writer.register::<Wall>();
            writer.register::<Foliage>();
            writer.register::<Placeable>();
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
                File::create(format!("assets/scenes/test.scn.ron"))
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

fn handle_new_scene_entities_parent_chunk(
    game: GameParam,
    new_scenes: Query<(Entity, &Children), Added<SceneInstance>>,
    obj_data: Query<(&WorldObject, &GlobalTransform), (With<WorldObject>, Without<Player>)>,
    mut proto_commands: ProtoCommands,
    mut commands: Commands,
    prototypes: Prototypes,
    mut proto_params: ProtoParam,
    mut minimap_event: EventWriter<UpdateMiniMapEvent>,
) {
    for (e, children) in new_scenes.iter() {
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
        println!("{:?} {:?}", x_offset, y_offset);
        for child in children.iter() {
            if let Ok((obj, txfm)) = obj_data.get(*child) {
                let pos = txfm.translation().truncate() - Vec2::new(x_offset, y_offset)
                    + game.game.player_state.position.truncate();
                obj.spawn_and_save_block(
                    &mut proto_commands,
                    &prototypes,
                    pos,
                    &mut minimap_event,
                    &mut proto_params,
                    &game,
                    &mut commands,
                );
            }
        }
        commands.entity(e).despawn_recursive();
    }
}
