use crate::assets::Graphics;
use crate::client::GameOverEvent;
use crate::item::WorldObject;
use crate::world::dimension::{ActiveDimension, SpawnDimension};
use crate::world::dungeon::Dungeon;
use crate::world::world_helpers::{camera_pos_to_chunk_pos, camera_pos_to_tile_pos};
use crate::world::{TileMapPosition, CHUNK_SIZE, ISLAND_SIZE};
use crate::{CustomFlush, GameParam, GameState, Player};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::render::view::RenderLayers;
use bevy::sprite::MaterialMesh2dBundle;
use bevy::utils::{HashMap, HashSet};
use bevy_ecs_tilemap::prelude::*;

use super::UIElement;
pub struct MinimapPlugin;

impl Plugin for MinimapPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(MinimapTileCache::default())
            .insert_resource(IslandMapOpen(false))
            .insert_resource(FogOfWarData::default())
            .add_event::<UpdateMiniMapEvent>()
            .add_system(toggle_island_map.run_if(in_state(GameState::Main)))
            .add_system(
                clear_cache_for_new_dimensions
                    .run_if(in_state(GameState::Main).or_else(in_state(GameState::Initializing))),
            )
            .add_system(
                update_minimap_cache
                    .run_if(in_state(GameState::Main).or_else(in_state(GameState::Initializing))),
            )
            .add_system(
                cache_explored_chunks
                    .after(CustomFlush)
                    .run_if(in_state(GameState::Main).or_else(in_state(GameState::Initializing))),
            )
            .add_system(
                update_fog_of_war
                    .run_if(in_state(GameState::Main).or_else(in_state(GameState::Initializing))),
            )
            .add_system(
                setup_island_map
                    .after(CustomFlush)
                    .run_if(in_state(GameState::Main).or_else(in_state(GameState::Initializing))),
            )
            .add_system(update_player_marker_on_map.run_if(in_state(GameState::Main)))
            .add_system(
                update_object_icons_on_map
                    .after(setup_island_map)
                    .run_if(in_state(GameState::Main)),
            )
            .add_system(
                update_fog_overlay_on_map
                    .after(setup_island_map)
                    .run_if(in_state(GameState::Main)),
            )
            .add_system(close_map_on_game_over.run_if(in_state(GameState::Main)));
    }
}

#[derive(Resource)]
pub struct IslandMapOpen(pub bool);

#[derive(Debug, Clone)]
pub struct UpdateMiniMapEvent {
    pub pos: Option<TileMapPosition>,
    pub new_tile: Option<WorldObject>,
}

#[derive(Component)]
pub struct IslandMap;

#[derive(Component)]
pub struct IslandMapPlayerMarker;

#[derive(Component)]
pub struct IslandMapFogOverlay;

#[derive(Component)]
pub struct IslandMapObjectIcon {
    pub tile_pos: TileMapPosition,
    pub object_type: WorldObject,
}

#[derive(Resource, Default)]
pub struct FogOfWarData {
    /// Set of tile positions that have been explored by the player
    pub explored_tiles: HashSet<TileMapPosition>,
}

#[derive(Resource, Default)]
pub struct MinimapTileCache {
    pub cache: HashMap<TileMapPosition, WorldObject>,
    // Fog of war: stores explored terrain data that persists when chunks despawn
    pub explored_terrain: HashMap<TileMapPosition, [WorldObject; 4]>,
}

fn toggle_island_map(
    key_input: Res<Input<KeyCode>>,
    mut map_open: ResMut<IslandMapOpen>,
    keybinds: Res<crate::keybinds::KeyBindings>,
) {
    if key_input.just_pressed(keybinds.get_minimap_key()) {
        map_open.0 = !map_open.0;
    }
}

fn update_minimap_cache(
    mut minimap_update: EventReader<UpdateMiniMapEvent>,
    mut cache: ResMut<MinimapTileCache>,
) {
    for event in minimap_update.iter() {
        let Some(pos) = event.pos else {
            //TODO: find another way to trigger change detection
            let _ = &mut cache.cache;
            continue;
        };
        if let Some(new_tile) = event.new_tile {
            if new_tile != WorldObject::None {
                cache.cache.insert(pos, new_tile);
            } else {
                cache.cache.remove(&pos);
            }
        } else {
            cache.cache.remove(&pos);
        }
    }
}

fn clear_cache_for_new_dimensions(
    mut commands: Commands,
    new_dim: Query<Entity, Added<SpawnDimension>>,
    mut cache: ResMut<MinimapTileCache>,
    mut fog_data: ResMut<FogOfWarData>,
    mut map_open: ResMut<IslandMapOpen>,
    map_query: Query<Entity, With<IslandMap>>,
    fog_query: Query<Entity, With<IslandMapFogOverlay>>,
    marker_query: Query<Entity, With<IslandMapPlayerMarker>>,
) {
    for _ in new_dim.iter() {
        // Clear both caches for new dimensions (eras)
        cache.cache = HashMap::new();
        cache.explored_terrain = HashMap::new();
        fog_data.explored_tiles.clear();
        map_open.0 = false;

        for map in map_query.iter() {
            commands.entity(map).despawn_recursive();
        }
        for fog in fog_query.iter() {
            commands.entity(fog).despawn_recursive();
        }
        for marker in marker_query.iter() {
            commands.entity(marker).despawn_recursive();
        }

        info!("Cleared map cache, fog of war, and map entities for new dimension");
    }
}

// Cache terrain from spawned chunks for the fog of war system (only terrain, not objects)
fn cache_explored_chunks(mut cache: ResMut<MinimapTileCache>, game: GameParam) {
    let num_chunks = ((ISLAND_SIZE / CHUNK_SIZE as f32) + 1.) as i32;

    // Check all possible chunks and cache their terrain data if they exist
    for chunk_y in -num_chunks..=num_chunks {
        for chunk_x in -num_chunks..=num_chunks {
            let chunk_pos = IVec2::new(chunk_x, chunk_y);

            // Only cache if chunk is currently spawned
            if game.get_chunk_entity(chunk_pos).is_some() {
                // Cache all tiles in this chunk (TERRAIN ONLY - objects handled separately)
                for tile_y in 0..CHUNK_SIZE {
                    for tile_x in 0..CHUNK_SIZE {
                        let tile_pos = TilePos {
                            x: tile_x,
                            y: tile_y,
                        };
                        let map_pos = TileMapPosition::new(chunk_pos, tile_pos);

                        // Only cache terrain if not already cached (avoid overwriting)
                        if !cache.explored_terrain.contains_key(&map_pos) {
                            if let Some(tile_data) = game.get_tile_data(map_pos) {
                                cache.explored_terrain.insert(map_pos, tile_data.block_type);
                            }
                        }
                    }
                }
            }
        }
    }
}
fn setup_island_map(
    mut commands: Commands,
    graphics: Res<Graphics>,
    mut assets: ResMut<Assets<Image>>,
    mut color_mat: ResMut<Assets<ColorMaterial>>,
    game: GameParam,
    minimap_cache: Res<MinimapTileCache>,
    old_map: Query<Entity, With<IslandMap>>,
    mut meshes: ResMut<Assets<Mesh>>,
    dungeon_check: Query<&Dungeon, With<ActiveDimension>>,
    map_open: Res<IslandMapOpen>,
    mut minimap_events: EventReader<UpdateMiniMapEvent>,
) {
    let should_show_map = map_open.0 && dungeon_check.is_empty();

    if !should_show_map {
        if map_open.is_changed() {
            for old_map in old_map.iter() {
                commands.entity(old_map).insert(Visibility::Hidden);
            }
        }
        return;
    }

    let map_exists = old_map.iter().next().is_some();
    let has_object_updates = minimap_events.iter().count() > 0;

    // Only rebuild if:
    // 1. Map doesn't exist yet, OR
    // 2. Object updates occurred (shrines completing, etc.)
    // Do NOT rebuild for terrain exploration or map toggle
    if map_exists && !has_object_updates {
        if map_open.is_changed() {
            for old_map in old_map.iter() {
                commands.entity(old_map).insert(Visibility::Inherited);
            }
        }
        return;
    }

    for old_map in old_map.iter() {
        commands.entity(old_map).despawn_recursive();
    }

    let num_chunks = ((ISLAND_SIZE / CHUNK_SIZE as f32) + 1.) as i32;
    let total_chunks = (num_chunks * 2 + 1) as u32;
    let tiles_per_chunk = CHUNK_SIZE;

    // Each tile is represented by 2x2 pixels for better visibility
    let pixels_per_tile = 2;
    let total_tiles = total_chunks * tiles_per_chunk;
    let total_pixels = total_tiles * pixels_per_tile;

    let size = Extent3d {
        width: total_pixels,
        height: total_pixels,
        depth_or_array_layers: 1,
    };

    let mut data = Vec::with_capacity((total_pixels * total_pixels * 4) as usize);

    for image_y in 0..total_pixels {
        for image_x in 0..total_pixels {
            let tile_x_in_island = image_x / pixels_per_tile;
            let tile_y_in_island = image_y / pixels_per_tile;

            let chunk_x = (tile_x_in_island as i32 / tiles_per_chunk as i32) - num_chunks;
            let chunk_y = num_chunks - (tile_y_in_island as i32 / tiles_per_chunk as i32);

            let tile_x_in_chunk = tile_x_in_island % tiles_per_chunk;
            let tile_y_in_chunk = (tiles_per_chunk - 1) - (tile_y_in_island % tiles_per_chunk);

            let chunk_pos = IVec2::new(chunk_x, chunk_y);
            let tile_pos = TilePos {
                x: tile_x_in_chunk,
                y: tile_y_in_chunk,
            };

            let pixel_x_in_tile = image_x % pixels_per_tile;
            let pixel_y_in_tile = image_y % pixels_per_tile;
            let quadrant = (pixel_y_in_tile * pixels_per_tile + pixel_x_in_tile) as usize;

            let map_pos = TileMapPosition::new(chunk_pos, tile_pos);

            if let Some(explored_tile) = minimap_cache.explored_terrain.get(&map_pos) {
                let mut drew_large_object = false;
                for dy in -2..=2 {
                    for dx in -2..=2 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        let check_tile_x = tile_x_in_chunk as i32 + dx;
                        let check_tile_y = tile_y_in_chunk as i32 + dy;
                        let check_chunk_x = chunk_x + (check_tile_x / tiles_per_chunk as i32);
                        let check_chunk_y = chunk_y + (check_tile_y / tiles_per_chunk as i32);
                        let check_tile_x = check_tile_x.rem_euclid(tiles_per_chunk as i32) as u32;
                        let check_tile_y = check_tile_y.rem_euclid(tiles_per_chunk as i32) as u32;

                        let check_pos = TileMapPosition::new(
                            IVec2::new(check_chunk_x, check_chunk_y),
                            TilePos {
                                x: check_tile_x,
                                y: check_tile_y,
                            },
                        );

                        if let Some(obj) = minimap_cache.cache.get(&check_pos) {
                            if matches!(obj, WorldObject::BossShrine) {
                                let c = obj.get_obj_color();
                                data.push((c.r() * 255.) as u8);
                                data.push((c.g() * 255.) as u8);
                                data.push((c.b() * 255.) as u8);
                                data.push(255);
                                drew_large_object = true;
                                break;
                            }
                        }
                    }
                    if drew_large_object {
                        break;
                    }
                }
                if drew_large_object {
                    continue;
                }

                let mut drew_medium_object = false;
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        let check_tile_x = tile_x_in_chunk as i32 + dx;
                        let check_tile_y = tile_y_in_chunk as i32 + dy;
                        let check_chunk_x = chunk_x + (check_tile_x / tiles_per_chunk as i32);
                        let check_chunk_y = chunk_y + (check_tile_y / tiles_per_chunk as i32);
                        let check_tile_x = check_tile_x.rem_euclid(tiles_per_chunk as i32) as u32;
                        let check_tile_y = check_tile_y.rem_euclid(tiles_per_chunk as i32) as u32;

                        let check_pos = TileMapPosition::new(
                            IVec2::new(check_chunk_x, check_chunk_y),
                            TilePos {
                                x: check_tile_x,
                                y: check_tile_y,
                            },
                        );

                        if let Some(obj) = minimap_cache.cache.get(&check_pos) {
                            if matches!(
                                obj,
                                WorldObject::CombatShrine
                                    | WorldObject::CombatShrineDone
                                    | WorldObject::GambleShrine
                                    | WorldObject::GambleShrineDone
                                    | WorldObject::ActiveSkillShrine
                                    | WorldObject::ActiveSkillShrineDone
                                    | WorldObject::WeaponShrine
                                    | WorldObject::WeaponShrineDone
                                    | WorldObject::ArmorShrine
                                    | WorldObject::ArmorShrineDone
                                    | WorldObject::AccessoryShrine
                                    | WorldObject::AccessoryShrineDone
                                    | WorldObject::HeirloomShrine
                                    | WorldObject::HeirloomShrineDone
                                    | WorldObject::BlacksmithMerchant
                                    | WorldObject::BlacksmithMerchantDone
                                    | WorldObject::DungeonEntrance
                                    | WorldObject::TimeGate
                            ) {
                                let c = obj.get_obj_color();
                                data.push((c.r() * 255.) as u8);
                                data.push((c.g() * 255.) as u8);
                                data.push((c.b() * 255.) as u8);
                                data.push(255);
                                drew_medium_object = true;
                                break;
                            }
                        }
                    }
                    if drew_medium_object {
                        break;
                    }
                }
                if drew_medium_object {
                    continue;
                }

                if let Some(cached_tile) = minimap_cache.cache.get(&map_pos) {
                    let c = cached_tile.get_obj_color();
                    data.push((c.r() * 255.) as u8);
                    data.push((c.g() * 255.) as u8);
                    data.push((c.b() * 255.) as u8);
                    data.push(255);
                    continue;
                }

                let c = explored_tile[quadrant].get_obj_color();
                data.push((c.r() * 255.) as u8);
                data.push((c.g() * 255.) as u8);
                data.push((c.b() * 255.) as u8);
                data.push(255);
            } else {
                data.push(40);
                data.push(40);
                data.push(40);
                data.push(255);
            }
        }
    }

    let image = Image::new(
        size,
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
    );
    let handle = assets.add(image);
    let mat = color_mat.add(ColorMaterial::from(handle));

    let map_display_size = f32::min(
        game.resolution.game_height * 0.85,
        game.resolution.game_width * 0.85,
    );

    let map_border = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::Minimap),
            sprite: Sprite {
                custom_size: Some(Vec2::new(map_display_size + 4., map_display_size + 4.)),
                ..Default::default()
            },
            transform: Transform::from_translation(Vec3::new(0., 0., 900.)),
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(IslandMap)
        .insert(Name::new("ISLAND_MAP_BORDER"))
        .id();

    let map = commands
        .spawn((
            MaterialMesh2dBundle {
                mesh: meshes
                    .add(
                        shape::Quad {
                            size: Vec2::new(map_display_size, map_display_size),
                            ..Default::default()
                        }
                        .into(),
                    )
                    .into(),
                transform: Transform::from_translation(Vec3::new(0., 0., 2.)),
                material: mat,
                ..default()
            },
            RenderLayers::from_layers(&[3]),
            Name::new("ISLAND_MAP_IMAGE"),
        ))
        .id();

    commands.entity(map_border).add_child(map);
}

/// System to dynamically update the player marker on the map
/// This runs every frame when the map is open, updating the player's position
fn update_player_marker_on_map(
    mut commands: Commands,
    map_open: Res<IslandMapOpen>,
    player_query: Query<&GlobalTransform, With<Player>>,
    marker_query: Query<Entity, With<IslandMapPlayerMarker>>,
    map_border_query: Query<Entity, With<IslandMap>>,
    game: GameParam,
) {
    if !map_open.0 {
        for marker in marker_query.iter() {
            commands.entity(marker).despawn_recursive();
        }
        return;
    }

    let Ok(player_transform) = player_query.get_single() else {
        return;
    };

    let player_world_pos = player_transform.translation().truncate();
    let player_chunk = camera_pos_to_chunk_pos(&player_world_pos);
    let player_tile = camera_pos_to_tile_pos(&player_world_pos);

    use crate::world::TILE_SIZE;
    let chunk_world_pos = Vec2::new(
        player_chunk.x as f32 * CHUNK_SIZE as f32 * TILE_SIZE.x,
        player_chunk.y as f32 * CHUNK_SIZE as f32 * TILE_SIZE.y,
    );
    let tile_world_pos = chunk_world_pos
        + Vec2::new(
            player_tile.x as f32 * TILE_SIZE.x,
            player_tile.y as f32 * TILE_SIZE.y,
        );
    let offset_in_tile = player_world_pos - tile_world_pos;
    let tile_fraction_x = offset_in_tile.x / TILE_SIZE.x;
    let tile_fraction_y = offset_in_tile.y / TILE_SIZE.y;

    let num_chunks = ((ISLAND_SIZE / CHUNK_SIZE as f32) + 1.) as i32; // Same as map building!
    let total_chunks = (num_chunks * 2 + 1) as u32;
    let tiles_per_chunk = CHUNK_SIZE;
    let pixels_per_tile = 2;
    let total_tiles = total_chunks * tiles_per_chunk;
    let total_pixels = total_tiles * pixels_per_tile;

    if player_chunk.x < -num_chunks
        || player_chunk.x > num_chunks
        || player_chunk.y < -num_chunks
        || player_chunk.y > num_chunks
    {
        return;
    }

    let tile_x_in_island =
        ((player_chunk.x + num_chunks) * tiles_per_chunk as i32 + player_tile.x as i32) as u32;

    let tile_y_in_island = ((num_chunks - player_chunk.y) * tiles_per_chunk as i32
        + ((tiles_per_chunk - 1) - player_tile.y) as i32) as u32;

    let image_x = tile_x_in_island * pixels_per_tile;
    let image_y = tile_y_in_island * pixels_per_tile;

    let map_display_size = f32::min(
        game.resolution.game_height * 0.85,
        game.resolution.game_width * 0.85,
    );

    let scale_factor = map_display_size / total_pixels as f32;

    let screen_x = (image_x as f32 + tile_fraction_x * pixels_per_tile as f32
        - total_pixels as f32 / 2.0)
        * scale_factor;
    let screen_y =
        (total_pixels as f32 / 2.0 - image_y as f32 - tile_fraction_y * pixels_per_tile as f32)
            * scale_factor;

    if let Some(marker_entity) = marker_query.iter().next() {
        commands
            .entity(marker_entity)
            .insert(Transform::from_translation(Vec3::new(
                screen_x, screen_y, 5.0,
            )))
            .insert(Sprite {
                color: Color::rgb(1.0, 1.0, 0.4),
                custom_size: Some(Vec2::new(6.0 * scale_factor, 6.0 * scale_factor)),
                ..Default::default()
            });
    } else {
        if let Some(map_border) = map_border_query.iter().next() {
            let marker = commands
                .spawn((
                    SpriteBundle {
                        sprite: Sprite {
                            color: Color::rgb(1.0, 1.0, 0.4), // Bright yellow
                            custom_size: Some(Vec2::new(6.0 * scale_factor, 6.0 * scale_factor)), // 3x3 tiles * 2 pixels/tile = 6 pixels, scaled
                            ..Default::default()
                        },
                        transform: Transform::from_translation(Vec3::new(screen_x, screen_y, 5.0)),
                        ..Default::default()
                    },
                    RenderLayers::from_layers(&[3]),
                    IslandMapPlayerMarker,
                    Name::new("ISLAND_MAP_PLAYER_MARKER"),
                ))
                .id();

            commands.entity(map_border).add_child(marker);
        }
    }
}

/// System to spawn and update special icons for important objects on the map
/// Icons: Boss Shrine, Dungeon, Portal, and other Shrines
fn update_object_icons_on_map(
    mut commands: Commands,
    map_open: Res<IslandMapOpen>,
    minimap_cache: Res<MinimapTileCache>,
    graphics: Res<Graphics>,
    icon_query: Query<(Entity, &IslandMapObjectIcon)>,
    map_border_query: Query<Entity, With<IslandMap>>,
    mut minimap_events: EventReader<UpdateMiniMapEvent>,
    game: GameParam,
) {
    if !map_open.0 {
        for (icon_entity, _) in icon_query.iter() {
            if let Some(entity_commands) = commands.get_entity(icon_entity) {
                entity_commands.despawn_recursive();
            }
        }
        return;
    }

    let has_object_updates = minimap_events.iter().count() > 0;
    if has_object_updates {
        return;
    }

    if map_border_query.is_empty() {
        return;
    }

    let num_chunks = ((ISLAND_SIZE / CHUNK_SIZE as f32) + 1.) as i32;
    let total_chunks = (num_chunks * 2 + 1) as u32;
    let tiles_per_chunk = CHUNK_SIZE;
    let pixels_per_tile = 2;
    let total_tiles = total_chunks * tiles_per_chunk;
    let total_pixels = total_tiles * pixels_per_tile;

    let map_display_size = f32::min(
        game.resolution.game_height * 0.85,
        game.resolution.game_width * 0.85,
    );
    let scale_factor = map_display_size / total_pixels as f32;

    fn get_icon_for_object(obj: &WorldObject) -> Option<UIElement> {
        match obj {
            WorldObject::BossShrine => Some(UIElement::MinimapSkullIcon),
            WorldObject::TimePortal => Some(UIElement::MinimapPortalIcon),
            WorldObject::DungeonEntrance => Some(UIElement::MinimapDungeonIcon),
            WorldObject::TimeGate => Some(UIElement::MinimapPortalIcon),
            WorldObject::CombatShrine
            | WorldObject::HeirloomShrine
            | WorldObject::GambleShrine
            | WorldObject::ActiveSkillShrine
            | WorldObject::WeaponShrine
            | WorldObject::ArmorShrine
            | WorldObject::AccessoryShrine
            | WorldObject::BlacksmithMerchant => Some(UIElement::MinimapStarIcon),
            WorldObject::CombatShrineDone
            | WorldObject::HeirloomShrineDone
            | WorldObject::GambleShrineDone
            | WorldObject::ActiveSkillShrineDone
            | WorldObject::WeaponShrineDone
            | WorldObject::ArmorShrineDone
            | WorldObject::AccessoryShrineDone
            | WorldObject::BlacksmithMerchantDone => None,
            _ => None,
        }
    }

    let mut desired_icons: HashMap<TileMapPosition, WorldObject> = HashMap::new();
    desired_icons.insert(
        TileMapPosition::new(IVec2::ZERO, TilePos { x: 0, y: 0 }),
        WorldObject::TimePortal,
    );
    for (pos, obj) in minimap_cache.cache.iter() {
        if get_icon_for_object(obj).is_some() {
            desired_icons.insert(*pos, *obj);
        }
    }

    for (icon_entity, icon) in icon_query.iter() {
        let should_remove = match desired_icons.get(&icon.tile_pos) {
            Some(obj) if *obj == icon.object_type => false,
            _ => true,
        };

        if should_remove {
            if let Some(entity_commands) = commands.get_entity(icon_entity) {
                entity_commands.despawn_recursive();
            }
        }
    }

    let existing_icons: HashSet<TileMapPosition> =
        icon_query.iter().map(|(_, icon)| icon.tile_pos).collect();

    let Some(map_border) = map_border_query.iter().next() else {
        return;
    };

    for (pos, obj) in desired_icons.iter() {
        if existing_icons.contains(pos) {
            continue;
        }

        let Some(ui_element) = get_icon_for_object(obj) else {
            continue;
        };

        if pos.chunk_pos.x < -num_chunks
            || pos.chunk_pos.x > num_chunks
            || pos.chunk_pos.y < -num_chunks
            || pos.chunk_pos.y > num_chunks
        {
            continue;
        }

        if pos.tile_pos.x >= tiles_per_chunk || pos.tile_pos.y >= tiles_per_chunk {
            continue;
        }

        let tile_x_calc =
            (pos.chunk_pos.x + num_chunks) * tiles_per_chunk as i32 + pos.tile_pos.x as i32;
        let tile_y_calc = (num_chunks - pos.chunk_pos.y) * tiles_per_chunk as i32
            + ((tiles_per_chunk - 1) - pos.tile_pos.y) as i32;

        if tile_x_calc < 0 || tile_y_calc < 0 {
            continue;
        }

        let tile_x_in_island = tile_x_calc as u32;
        let tile_y_in_island = tile_y_calc as u32;

        if tile_x_in_island > u32::MAX / pixels_per_tile
            || tile_y_in_island > u32::MAX / pixels_per_tile
        {
            continue;
        }

        let image_x = tile_x_in_island * pixels_per_tile;
        let image_y = tile_y_in_island * pixels_per_tile;

        if image_x >= total_pixels || image_y >= total_pixels {
            continue;
        }

        let screen_x = (image_x as f32 + 1.0 - total_pixels as f32 / 2.0) * scale_factor;
        let screen_y = (total_pixels as f32 / 2.0 - image_y as f32 - 1.0) * scale_factor;

        // Spawn icon
        let icon_entity = commands
            .spawn((
                SpriteBundle {
                    texture: graphics.get_ui_element_texture(ui_element),
                    sprite: Sprite {
                        custom_size: Some(Vec2::new(16.0, 16.0)),
                        ..Default::default()
                    },
                    transform: Transform::from_translation(Vec3::new(screen_x, screen_y, 2.5)), // Between map (2) and player (5)
                    ..Default::default()
                },
                RenderLayers::from_layers(&[3]),
                IslandMapObjectIcon {
                    tile_pos: *pos,
                    object_type: *obj,
                },
                Name::new(format!("ISLAND_MAP_ICON_{:?}", obj)),
            ))
            .id();

        if let Some(mut entity_commands) = commands.get_entity(map_border) {
            entity_commands.add_child(icon_entity);
        } else {
            if let Some(entity_commands) = commands.get_entity(icon_entity) {
                entity_commands.despawn_recursive();
            }
        }
    }
}

/// System to track which tiles the player has explored
/// Updates fog of war data based on player position (16 tile radius)
/// Runs continuously (not just when map is open) to ensure tiles are marked as explored
/// as the player moves around, so fog is already cleared when map is opened
fn update_fog_of_war(
    player_query: Query<&GlobalTransform, With<Player>>,
    mut fog_data: ResMut<FogOfWarData>,
    mut last_position: Local<Option<(IVec2, TilePos)>>,
) {
    let Ok(player_transform) = player_query.get_single() else {
        return;
    };

    let player_chunk = camera_pos_to_chunk_pos(&player_transform.translation().truncate());
    let player_tile = camera_pos_to_tile_pos(&player_transform.translation().truncate());

    let current_pos = (player_chunk, player_tile);
    let player_moved = last_position
        .as_ref()
        .map_or(true, |last| *last != current_pos);

    if !player_moved {
        return;
    }

    *last_position = Some(current_pos);

    let reveal_radius = 16;

    for dy in -reveal_radius..=reveal_radius {
        for dx in -reveal_radius..=reveal_radius {
            if (dx * dx + dy * dy) > (reveal_radius * reveal_radius) {
                continue;
            }

            let check_tile_x = player_tile.x as i32 + dx;
            let check_tile_y = player_tile.y as i32 + dy;
            let check_chunk_x = player_chunk.x + check_tile_x.div_euclid(CHUNK_SIZE as i32);
            let check_chunk_y = player_chunk.y + check_tile_y.div_euclid(CHUNK_SIZE as i32);
            let check_tile_x = check_tile_x.rem_euclid(CHUNK_SIZE as i32) as u32;
            let check_tile_y = check_tile_y.rem_euclid(CHUNK_SIZE as i32) as u32;

            let tile_pos = TileMapPosition::new(
                IVec2::new(check_chunk_x, check_chunk_y),
                TilePos {
                    x: check_tile_x,
                    y: check_tile_y,
                },
            );

            // Insert the tile into explored set
            // Bevy's ResMut automatically handles change detection
            fog_data.explored_tiles.insert(tile_pos);
        }
    }
}

/// System to update the fog overlay on the map
/// Creates a semi-transparent overlay that hides unexplored areas
fn update_fog_overlay_on_map(
    mut commands: Commands,
    map_open: Res<IslandMapOpen>,
    fog_data: Res<FogOfWarData>,
    fog_query: Query<Entity, With<IslandMapFogOverlay>>,
    map_border_query: Query<Entity, With<IslandMap>>,
    mut assets: ResMut<Assets<Image>>,
    mut color_mat: ResMut<Assets<ColorMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    game: GameParam,
) {
    if !map_open.0 {
        if map_open.is_changed() {
            for fog in fog_query.iter() {
                commands.entity(fog).insert(Visibility::Hidden);
            }
        }
        return;
    }

    if map_open.is_changed() {
        for fog in fog_query.iter() {
            commands.entity(fog).insert(Visibility::Inherited);
        }
    }

    let existing_fog_count = fog_query.iter().count();
    if existing_fog_count > 1 {
        warn!("Multiple fog overlays detected: {}", existing_fog_count);
    }

    let fog_exists = existing_fog_count > 0;

    let map_just_opened = map_open.is_changed() && map_open.0;

    // Only rebuild when:
    // 1. Map is open but no fog exists (need to create initial fog), OR
    // 2. Fog data changes (new tiles explored while map is open), OR
    // 3. Map just opened (to show all areas explored while map was closed)
    // The lag was from insert(Visibility) calls, not texture rebuilding
    let needs_rebuild = (map_open.0 && !fog_exists) || fog_data.is_changed() || map_just_opened;

    if !needs_rebuild {
        return;
    }

    for fog in fog_query.iter() {
        commands.entity(fog).despawn_recursive();
    }

    let num_chunks = ((ISLAND_SIZE / CHUNK_SIZE as f32) + 1.) as i32;
    let total_chunks = (num_chunks * 2 + 1) as u32;
    let tiles_per_chunk = CHUNK_SIZE;
    let pixels_per_tile = 2;
    let total_tiles = total_chunks * tiles_per_chunk;
    let total_pixels = total_tiles * pixels_per_tile;

    let size = Extent3d {
        width: total_pixels,
        height: total_pixels,
        depth_or_array_layers: 1,
    };

    let mut data = Vec::with_capacity((total_pixels * total_pixels * 4) as usize);

    for image_y in 0..total_pixels {
        for image_x in 0..total_pixels {
            let tile_x_in_island = image_x / pixels_per_tile;
            let tile_y_in_island = image_y / pixels_per_tile;

            let chunk_x = (tile_x_in_island as i32 / tiles_per_chunk as i32) - num_chunks;
            let chunk_y = num_chunks - (tile_y_in_island as i32 / tiles_per_chunk as i32);

            let tile_x_in_chunk = tile_x_in_island % tiles_per_chunk;
            let tile_y_in_chunk = (tiles_per_chunk - 1) - (tile_y_in_island % tiles_per_chunk);

            let map_pos = TileMapPosition::new(
                IVec2::new(chunk_x, chunk_y),
                TilePos {
                    x: tile_x_in_chunk,
                    y: tile_y_in_chunk,
                },
            );

            if fog_data.explored_tiles.contains(&map_pos) {
                data.push(0);
                data.push(0);
                data.push(0);
                data.push(0);
            } else {
                data.push(60);
                data.push(60);
                data.push(60);
                data.push(255);
            }
        }
    }

    let image = Image::new(
        size,
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
    );
    let handle = assets.add(image);
    let mat = color_mat.add(ColorMaterial::from(handle));

    let map_display_size = f32::min(
        game.resolution.game_height * 0.85,
        game.resolution.game_width * 0.85,
    );

    if map_border_query.iter().next().is_some() {
        commands.spawn((
            MaterialMesh2dBundle {
                mesh: meshes
                    .add(
                        shape::Quad {
                            size: Vec2::new(map_display_size, map_display_size),
                            ..Default::default()
                        }
                        .into(),
                    )
                    .into(),
                transform: Transform::from_translation(Vec3::new(0., 0., 903.)),
                material: mat,
                ..default()
            },
            RenderLayers::from_layers(&[3]),
            IslandMapFogOverlay,
            Name::new("ISLAND_MAP_FOG_OVERLAY"),
        ));
    }
}

/// System to close the map when the player dies
fn close_map_on_game_over(
    mut game_over_events: EventReader<GameOverEvent>,
    mut map_open: ResMut<IslandMapOpen>,
) {
    if !game_over_events.is_empty() {
        map_open.0 = false;
        game_over_events.clear();
    }
}
