use super::SchematicType;
use crate::{
    world::{
        chunk::Chunk, world_helpers::tile_pos_to_world_pos, TileMapPositionData, CHUNK_SIZE,
        TILE_SIZE,
    },
    GameParam,
};
use bevy_ecs_tilemap::tiles::TilePos;
use rand::Rng;

use bevy::prelude::*;

#[derive(Component, Debug, Clone, Reflect, Default)]
pub struct SchematicSpawner {
    schematic: SchematicType,
}

pub fn attempt_to_spawn_schematic_in_chunk(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    game: GameParam,
    chunks: Query<(Entity, &Chunk, &SchematicSpawner)>,
) {
    for (e, chunk, schematic) in chunks.iter() {
        let mut rng = rand::thread_rng();
        let rng_x = rng.gen_range(0..CHUNK_SIZE);
        let rng_y = rng.gen_range(0..CHUNK_SIZE);
        let target_pos = tile_pos_to_world_pos(TileMapPositionData::new(
            chunk.chunk_pos,
            TilePos::new(rng_x, rng_y),
        ));
        let tile_pos = Vec2::new(4. * TILE_SIZE.x, 9. * TILE_SIZE.x);
        if let Some(chunk_e) = game.get_chunk_entity(chunk.chunk_pos) {
            println!("Spawning schematic at {:?} {:?}", chunk.chunk_pos, tile_pos);

            commands
                .spawn(DynamicSceneBundle {
                    scene: asset_server.load(format!("scenes/{}.scn.ron", schematic.schematic)),
                    transform: Transform::from_translation(target_pos.extend(0.)),
                    ..default()
                })
                .set_parent(*chunk_e)
                .insert(Name::new("Schematic"));
            commands.entity(e).remove::<SchematicSpawner>();
        }
    }
}

pub fn give_chunks_schematic_spawners(
    mut commands: Commands,
    new_chunks: Query<Entity, Added<Chunk>>,
) {
    for e in new_chunks.iter() {
        let mut rng = rand::thread_rng();
        if rng.gen_ratio(1, 4) == false {
            continue;
        }

        let schematic = match rng.gen_range(0..=1) {
            _ => SchematicType::House,
        };
        commands.entity(e).insert(SchematicSpawner { schematic });
    }
}
