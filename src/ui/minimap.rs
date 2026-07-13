use crate::assets::Graphics;
use crate::client::GameOverEvent;
use crate::colors::{DARK_BROWN, DESERT_TILE, DESERT_WATER, SNOW_TILE, SNOW_WATER, WHITE};
use crate::item::WorldObject;
use crate::world::dimension::{ActiveDimension, Era, SpawnDimension};
use crate::world::dungeon::Dungeon;
use crate::world::world_helpers::{
    camera_pos_to_chunk_pos, camera_pos_to_tile_pos, tile_pos_to_world_pos, world_pos_to_tile_pos,
};
use crate::world::{TileMapPosition, CHUNK_SIZE, ISLAND_SIZE, TILE_SIZE};
use crate::{CustomFlush, GameParam, GameState, InputMappings, Player, ScreenResolution, DEBUG};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::render::view::RenderLayers;
use bevy::sprite::Anchor;
use bevy::sprite::MaterialMesh2dBundle;
use bevy::utils::{HashMap, HashSet};
use bevy_ecs_tilemap::prelude::*;

use super::{game_fonts as gf, layout_sync::UiLayoutKey, ui_helpers, UIElement};

pub struct MinimapPlugin;

impl Plugin for MinimapPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(MinimapTileCache::default())
            .insert_resource(IslandMapOpen(false))
            .insert_resource(FogOfWarData::default())
            .add_event::<UpdateMiniMapEvent>()
            .add_system(
                toggle_island_map
                    .run_if(in_state(GameState::Main))
                    .run_if(|dungeon: Query<&Dungeon>| dungeon.is_empty()),
            )
            .add_system(
                clear_cache_for_new_dimensions
                    .run_if(in_state(GameState::Main).or_else(in_state(GameState::Initializing))),
            )
            .add_system(
                close_map_on_dungeon_entry
                    .run_if(in_state(GameState::Main).or_else(in_state(GameState::Initializing))),
            )
            .add_system(
                update_minimap_cache
                    .run_if(in_state(GameState::Main).or_else(in_state(GameState::Initializing)))
                    .run_if(|dungeon: Query<&Dungeon>| dungeon.is_empty()),
            )
            .add_system(
                cache_explored_chunks
                    .after(CustomFlush)
                    .run_if(in_state(GameState::Main).or_else(in_state(GameState::Initializing)))
                    .run_if(|dungeon: Query<&Dungeon>| dungeon.is_empty()),
            )
            .add_system(
                update_fog_of_war
                    .run_if(in_state(GameState::Main).or_else(in_state(GameState::Initializing)))
                    .run_if(|dungeon: Query<&Dungeon>| dungeon.is_empty()),
            )
            .add_system(
                setup_island_map
                    .after(CustomFlush)
                    .run_if(in_state(GameState::Main).or_else(in_state(GameState::Initializing)))
                    .run_if(|dungeon: Query<&Dungeon>| dungeon.is_empty()),
            )
            .add_system(
                sync_island_map_overlay
                    .after(setup_island_map)
                    .run_if(in_state(GameState::Main).or_else(in_state(GameState::Initializing)))
                    .run_if(|dungeon: Query<&Dungeon>| dungeon.is_empty()),
            )
            .add_system(
                sync_island_map_legend
                    .after(sync_island_map_overlay)
                    .run_if(in_state(GameState::Main).or_else(in_state(GameState::Initializing)))
                    .run_if(|dungeon: Query<&Dungeon>| dungeon.is_empty()),
            )
            .add_system(
                update_player_marker_on_map
                    .run_if(in_state(GameState::Main))
                    .run_if(|dungeon: Query<&Dungeon>| dungeon.is_empty()),
            )
            .add_system(
                update_object_icons_on_map
                    .after(setup_island_map)
                    .run_if(in_state(GameState::Main))
                    .run_if(|dungeon: Query<&Dungeon>| dungeon.is_empty()),
            )
            .add_system(
                update_fog_overlay_on_map
                    .after(setup_island_map)
                    .run_if(in_state(GameState::Main))
                    .run_if(|dungeon: Query<&Dungeon>| dungeon.is_empty()),
            )
            .add_system(close_map_on_game_over.run_if(in_state(GameState::Main)))
            .add_system(
                setup_hud_minimap
                    .run_if(in_state(GameState::Main))
                    .run_if(|dungeon: Query<&Dungeon>| dungeon.is_empty()),
            )
            .add_system(
                update_hud_minimap_texture
                    .after(setup_hud_minimap)
                    .run_if(in_state(GameState::Main))
                    .run_if(|dungeon: Query<&Dungeon>| dungeon.is_empty()),
            )
            .add_system(
                update_hud_minimap_icons
                    .after(update_hud_minimap_texture)
                    .run_if(in_state(GameState::Main))
                    .run_if(|dungeon: Query<&Dungeon>| dungeon.is_empty()),
            )
            .add_system(despawn_hud_minimap_in_dungeon.run_if(in_state(GameState::Main)))
            .add_system(
                invalidate_island_map_on_ui_layout_change
                    .after(crate::update_pixel_perfect_viewport),
            );
    }
}

pub const HUD_MINIMAP_RADIUS_TILES: i32 = 26;
pub const HUD_MINIMAP_PIXELS_PER_TILE: u32 = 2;
/// HUD minimap diameter in UI world units (tuned at Large / reference UI scale).
pub const HUD_MINIMAP_DISPLAY_SIZE: f32 = 90.0;
const HUD_MINIMAP_ICON_SIZE: f32 = 16.0;
/// Extra inset so icon sprites are hidden before they overlap the circular edge.
const HUD_MINIMAP_ICON_CLIP_INSET: f32 = -4.0;
pub const HUD_MINIMAP_PADDING: f32 = 8.0;
/// Horizontal nudge applied when positioning the HUD minimap from the right edge.
pub const HUD_MINIMAP_RIGHT_NUDGE: f32 = 4.0;

/// HUD minimap diameter — fixed at [`HUD_MINIMAP_DISPLAY_SIZE`] so it matches the legacy Large UI look.
pub fn hud_minimap_display_size(_res: &ScreenResolution) -> f32 {
    HUD_MINIMAP_DISPLAY_SIZE
}

/// Full island map panel size (square) for the current UI camera bucket.
pub fn island_map_display_size(res: &ScreenResolution) -> f32 {
    f32::min(res.game_height * 0.85, res.game_width * 0.85)
}

/// Horizontal UI-space shift applied to the whole island map (border, fog, legend, markers).
pub const ISLAND_MAP_X_OFFSET: f32 = 20.0;

/// Center X of the legend panel to the left of the (shifted) island map.
pub fn island_map_legend_center_x(res: &ScreenResolution) -> f32 {
    ISLAND_MAP_X_OFFSET
        - island_map_display_size(res) * 0.5
        - MINIMAP_LEGEND_GAP_FROM_MAP
        - MINIMAP_LEGEND_PANEL_HALF_WIDTH
}

/// Y of the top edge of the legend icon stack (legend is vertically centered at 0).
pub fn island_map_legend_top_y() -> f32 {
    MINIMAP_LEGEND_ROWS.len() as f32 * MINIMAP_LEGEND_ROW_HEIGHT * 0.5
}

/// World-space x for the LEFT edge of the HUD minimap sprite.
pub fn hud_minimap_left_edge_x(game_width: f32, display_size: f32) -> f32 {
    let center_x =
        game_width * 0.5 - display_size * 0.5 - HUD_MINIMAP_PADDING + HUD_MINIMAP_RIGHT_NUDGE;
    center_x - display_size * 0.5
}

pub fn hud_minimap_left_edge_x_from_res(res: &ScreenResolution) -> f32 {
    hud_minimap_left_edge_x(res.game_width, hud_minimap_display_size(res))
}
const HUD_MINIMAP_SHADOW_THICKNESS: f32 = 2.0;
const HUD_MINIMAP_SHADOW_RGB: (f32, f32, f32) = (0.1, 0.1, 0.1);
const HUD_MINIMAP_SHADOW_ALPHA: f32 = 0.45;

/// On-map icon draw size (16×16 source sprites).
const MINIMAP_ICON_DISPLAY_SIZE: f32 = 16.0;
/// Legend entries show icons at 2× native sprite size.
const MINIMAP_LEGEND_ICON_DISPLAY_SIZE: f32 = 32.0;
const MINIMAP_LEGEND_ROW_HEIGHT: f32 = 36.0;
const MINIMAP_LEGEND_GAP_FROM_MAP: f32 = 12.0;
const MINIMAP_LEGEND_PANEL_HALF_WIDTH: f32 = 48.0;
const MINIMAP_LEGEND_TEXT_OFFSET_X: f32 = 22.0;

struct MinimapLegendRow {
    icon: UIElement,
    label: &'static str,
}

const MINIMAP_LEGEND_ROWS: &[MinimapLegendRow] = &[
    MinimapLegendRow {
        icon: UIElement::PlayerMinimapIcon,
        label: "Player",
    },
    MinimapLegendRow {
        icon: UIElement::MinimapSkullIcon,
        label: "Boss Shrine",
    },
    MinimapLegendRow {
        icon: UIElement::MinimapPortalIcon,
        label: "Portal",
    },
    MinimapLegendRow {
        icon: UIElement::MinimapDungeonIcon,
        label: "Dungeon",
    },
    MinimapLegendRow {
        icon: UIElement::MinimapStarIcon,
        label: "Shrine",
    },
];

fn hud_minimap_map_diameter_pixels() -> u32 {
    (HUD_MINIMAP_RADIUS_TILES * 2 + 1) as u32 * HUD_MINIMAP_PIXELS_PER_TILE
}

/// Extra margin on each side of the texture so the outer shadow ring is not
/// clipped where the circle meets the square bounds (top/bottom/left/right).
fn hud_minimap_texture_padding_pixels() -> u32 {
    (HUD_MINIMAP_SHADOW_THICKNESS + 0.5).ceil() as u32
}

pub fn hud_minimap_texture_pixels() -> u32 {
    hud_minimap_map_diameter_pixels() + 2 * hud_minimap_texture_padding_pixels()
}

fn hud_minimap_radius_pixels() -> f32 {
    (HUD_MINIMAP_RADIUS_TILES as f32 + 0.5) * HUD_MINIMAP_PIXELS_PER_TILE as f32
}

pub fn hud_minimap_max_icon_center_distance(scale: f32) -> f32 {
    hud_minimap_radius_pixels() * scale - HUD_MINIMAP_ICON_SIZE / 2.0 - HUD_MINIMAP_ICON_CLIP_INSET
}

#[derive(Component)]
pub struct HudMinimap {
    pub image_handle: Handle<Image>,
}

#[derive(Component)]
pub struct HudMinimapSprite;

#[derive(Component)]
pub struct IslandMapImage;

#[derive(Component)]
pub struct HudMinimapPlayerMarker;

#[derive(Component)]
pub struct HudMinimapIcon {
    pub tile_pos: TileMapPosition,
    pub object_type: WorldObject,
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

/// Full-screen radial backdrop behind the island map (same tuning as item chest UI).
#[derive(Component)]
pub struct IslandMapOverlay;

/// Icon legend panel shown to the left of the full island map.
#[derive(Component)]
pub struct IslandMapLegend;

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

fn minimap_base_terrain_color_for_era(obj: WorldObject, era: &Era) -> Color {
    match era {
        Era::Main | Era::DungeonMain => obj.get_obj_color(),
        Era::Second => match obj {
            WorldObject::GrassTile => DESERT_TILE,
            WorldObject::WaterTile => DESERT_WATER,
            _ => obj.get_obj_color(),
        },
        Era::Third => match obj {
            WorldObject::GrassTile => SNOW_TILE,
            WorldObject::WaterTile => SNOW_WATER,
            _ => obj.get_obj_color(),
        },
    }
}
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
        | WorldObject::MicrowaveShrine
        | WorldObject::BlacksmithMerchant
        | WorldObject::CauldronShrine
        | WorldObject::WellShrine
        | WorldObject::ChaosTotem => Some(UIElement::MinimapStarIcon),
        WorldObject::CombatShrineDone
        | WorldObject::HeirloomShrineDone
        | WorldObject::GambleShrineDone
        | WorldObject::ActiveSkillShrineDone
        | WorldObject::WeaponShrineDone
        | WorldObject::ArmorShrineDone
        | WorldObject::AccessoryShrineDone
        | WorldObject::MicrowaveShrineDone
        | WorldObject::BlacksmithMerchantDone
        | WorldObject::CauldronShrineDone
        | WorldObject::WellShrineDone
        | WorldObject::ChaosTotemDone => None,
        _ => None,
    }
}

fn is_grass_obj(obj: &WorldObject) -> bool {
    [
        WorldObject::Grass,
        WorldObject::Grass2,
        WorldObject::Grass3,
        WorldObject::DesertGrass1,
        WorldObject::DesertGrass2,
        WorldObject::DesertGrass3,
        WorldObject::DesertGrass4,
        WorldObject::SnowGrass1,
        WorldObject::SnowGrass2,
        WorldObject::SnowGrass3,
        WorldObject::SnowGrass4,
    ]
    .contains(obj)
}

fn toggle_island_map(
    key_input: Res<Input<KeyCode>>,
    mouse_input: Res<Input<MouseButton>>,
    mut map_open: ResMut<IslandMapOpen>,
    keybinds: Res<InputMappings>,
    curr_ui_state: Res<State<super::UIState>>,
    mut next_ui_state: ResMut<NextState<super::UIState>>,
    gamepad_action_q: Query<
        &leafwing_input_manager::prelude::ActionState<crate::gamepad_input::GamepadAction>,
        With<Player>,
    >,
) {
    let gamepad_pressed = crate::gamepad_input::gamepad_action_just_pressed(
        gamepad_action_q.get_single().ok(),
        crate::gamepad_input::GamepadAction::ToggleMap,
    );
    if keybinds.check_map_input(&key_input, &mouse_input) || gamepad_pressed {
        let opening = !map_open.0;
        map_open.0 = !map_open.0;
        // Opening the map should dismiss other menus the same way inventory/options do —
        // otherwise the island map stacks on top of whatever UI was already open.
        if opening && curr_ui_state.0 != super::UIState::Closed {
            next_ui_state.set(super::UIState::Closed);
        }
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
    overlay_query: Query<Entity, With<IslandMapOverlay>>,
    legend_query: Query<Entity, With<IslandMapLegend>>,
    hud_query: Query<Entity, With<HudMinimap>>,
    dungeon_check: Query<&Dungeon>,
    game: GameParam,
) {
    for _ in new_dim.iter() {
        let is_dungeon_transition =
            dungeon_check.iter().next().is_some() || game.era.current_era.is_dungeon();

        cache.cache = HashMap::new();
        cache.explored_terrain = HashMap::new();

        if !is_dungeon_transition {
            fog_data.explored_tiles.clear();
        }
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
        for overlay in overlay_query.iter() {
            commands.entity(overlay).despawn_recursive();
        }
        for legend in legend_query.iter() {
            commands.entity(legend).despawn_recursive();
        }
        for hud in hud_query.iter() {
            commands.entity(hud).despawn_recursive();
        }

        if is_dungeon_transition {
            info!("Dungeon transition detected - preserving fog of war");
        } else {
            info!("Cleared map cache, fog of war, and map entities for new dimension");
        }
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
    game: GameParam,
    minimap_cache: Res<MinimapTileCache>,
    old_map: Query<Entity, With<IslandMap>>,
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
                                    | WorldObject::CauldronShrine
                                    | WorldObject::CauldronShrineDone
                                    | WorldObject::WellShrine
                                    | WorldObject::WellShrineDone
                                    | WorldObject::ChaosTotem
                                    | WorldObject::ChaosTotemDone
                                    | WorldObject::DungeonEntrance
                                    | WorldObject::MicrowaveShrine
                                    | WorldObject::MicrowaveShrineDone
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
                    if !is_grass_obj(cached_tile) {
                        let c = cached_tile.get_obj_color();
                        data.push((c.r() * 255.) as u8);
                        data.push((c.g() * 255.) as u8);
                        data.push((c.b() * 255.) as u8);
                        data.push(255);
                        continue;
                    }
                }

                let c = minimap_base_terrain_color_for_era(
                    explored_tile[quadrant],
                    &game.era.current_era,
                );
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
    let image_handle = assets.add(image);

    let map_display_size = island_map_display_size(&game.resolution);

    let map_border = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::Minimap),
            sprite: Sprite {
                custom_size: Some(Vec2::new(map_display_size + 4., map_display_size + 4.)),
                ..Default::default()
            },
            transform: Transform::from_translation(Vec3::new(ISLAND_MAP_X_OFFSET, 0., 900.)),
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(IslandMap)
        .insert(Name::new("ISLAND_MAP_BORDER"))
        .id();

    // SpriteBundle + `custom_size` so the map image matches the frame/fog (MaterialMesh2d
    // quads were not respecting the intended display size on some UI layout buckets).
    let map = commands
        .spawn((
            SpriteBundle {
                texture: image_handle,
                sprite: Sprite {
                    custom_size: Some(Vec2::splat(map_display_size)),
                    ..default()
                },
                transform: Transform::from_translation(Vec3::new(0., 0., 2.)),
                ..default()
            },
            IslandMapImage,
            RenderLayers::from_layers(&[3]),
            Name::new("ISLAND_MAP_IMAGE"),
        ))
        .id();

    commands.entity(map_border).add_child(map);
}

/// Spawns/despawns the full-screen radial backdrop when the island map is toggled open/closed.
fn sync_island_map_overlay(
    mut commands: Commands,
    map_open: Res<IslandMapOpen>,
    res: Res<ScreenResolution>,
    dungeon_check: Query<&Dungeon, With<ActiveDimension>>,
    existing: Query<Entity, With<IslandMapOverlay>>,
) {
    let should_show = map_open.0 && dungeon_check.is_empty();

    if should_show {
        if existing.is_empty() {
            let overlay =
                ui_helpers::spawn_full_screen_ui_overlay_tuned(&mut commands, &res, 0.0, 0.95, 9.);
            commands.entity(overlay).insert(IslandMapOverlay);
        }
    } else {
        for entity in existing.iter() {
            commands.entity(entity).despawn_recursive();
        }
    }
}

fn spawn_island_map_legend(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    map_display_size: f32,
) {
    let row_count = MINIMAP_LEGEND_ROWS.len() as f32;
    let total_height = row_count * MINIMAP_LEGEND_ROW_HEIGHT;
    let legend_center_x = ISLAND_MAP_X_OFFSET
        - map_display_size * 0.5
        - MINIMAP_LEGEND_GAP_FROM_MAP
        - MINIMAP_LEGEND_PANEL_HALF_WIDTH;

    let root = commands
        .spawn((
            SpatialBundle {
                transform: Transform::from_translation(Vec3::new(legend_center_x, 0., 901.)),
                ..default()
            },
            IslandMapLegend,
            RenderLayers::from_layers(&[3]),
            Name::new("ISLAND_MAP_LEGEND"),
        ))
        .id();

    for (i, row) in MINIMAP_LEGEND_ROWS.iter().enumerate() {
        let row_y = total_height * 0.5 - (i as f32 + 0.5) * MINIMAP_LEGEND_ROW_HEIGHT;
        let row_entity = commands
            .spawn(SpatialBundle {
                transform: Transform::from_translation(Vec3::new(0., row_y, 0.)),
                ..default()
            })
            .id();
        commands.entity(root).add_child(row_entity);

        commands
            .spawn((
                SpriteBundle {
                    texture: graphics.get_ui_element_texture(row.icon.clone()),
                    sprite: Sprite {
                        custom_size: Some(Vec2::splat(MINIMAP_LEGEND_ICON_DISPLAY_SIZE)),
                        ..default()
                    },
                    transform: Transform::from_translation(Vec3::new(
                        -MINIMAP_LEGEND_TEXT_OFFSET_X,
                        0.,
                        0.,
                    )),
                    ..default()
                },
                RenderLayers::from_layers(&[3]),
            ))
            .set_parent(row_entity);

        commands
            .spawn((
                Text2dBundle {
                    text: Text::from_section(
                        row.label,
                        gf::BODY.text_style(&asset_server, WHITE),
                    )
                    .with_alignment(TextAlignment::Left),
                    text_anchor: Anchor::CenterLeft,
                    transform: Transform {
                        translation: Vec3::new(-2., 0., 1.),
                        scale: gf::BODY.transform_scale(),
                        ..default()
                    },
                    ..default()
                },
                RenderLayers::from_layers(&[3]),
            ))
            .set_parent(row_entity);
    }
}

/// Spawns/despawns the map icon legend when the island map is toggled open/closed.
fn sync_island_map_legend(
    mut commands: Commands,
    map_open: Res<IslandMapOpen>,
    res: Res<ScreenResolution>,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    dungeon_check: Query<&Dungeon, With<ActiveDimension>>,
    existing: Query<Entity, With<IslandMapLegend>>,
) {
    let should_show = map_open.0 && dungeon_check.is_empty();

    if !should_show {
        for entity in existing.iter() {
            commands.entity(entity).despawn_recursive();
        }
        return;
    }

    if !existing.is_empty() && !map_open.is_changed() && !res.is_changed() {
        return;
    }

    for entity in existing.iter() {
        commands.entity(entity).despawn_recursive();
    }

    spawn_island_map_legend(
        &mut commands,
        &graphics,
        &asset_server,
        island_map_display_size(&res),
    );
}

/// System to dynamically update the player marker on the map
/// This runs every frame when the map is open, updating the player's position
fn update_player_marker_on_map(
    mut commands: Commands,
    map_open: Res<IslandMapOpen>,
    graphics: Res<Graphics>,
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

    let map_display_size = island_map_display_size(&game.resolution);

    let scale_factor = map_display_size / total_pixels as f32;

    let screen_x = (image_x as f32 + tile_fraction_x * pixels_per_tile as f32
        - total_pixels as f32 / 2.0)
        * scale_factor;
    let screen_y =
        (total_pixels as f32 / 2.0 - image_y as f32 - tile_fraction_y * pixels_per_tile as f32)
            * scale_factor;

    let player_texture = graphics.get_ui_element_texture(UIElement::PlayerMinimapIcon);
    let marker_sprite = Sprite {
        custom_size: Some(Vec2::splat(MINIMAP_ICON_DISPLAY_SIZE)),
        ..default()
    };

    if let Some(marker_entity) = marker_query.iter().next() {
        commands
            .entity(marker_entity)
            .insert(Transform::from_translation(Vec3::new(
                screen_x, screen_y, 5.0,
            )))
            .insert(marker_sprite);
    } else if let Some(map_border) = map_border_query.iter().next() {
        let marker = commands
            .spawn((
                SpriteBundle {
                    texture: player_texture,
                    sprite: marker_sprite,
                    transform: Transform::from_translation(Vec3::new(screen_x, screen_y, 5.0)),
                    ..default()
                },
                RenderLayers::from_layers(&[3]),
                IslandMapPlayerMarker,
                Name::new("ISLAND_MAP_PLAYER_MARKER"),
            ))
            .id();

        commands.entity(map_border).add_child(marker);
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

    let map_display_size = island_map_display_size(&game.resolution);
    let scale_factor = map_display_size / total_pixels as f32;

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
                        custom_size: Some(Vec2::splat(MINIMAP_ICON_DISPLAY_SIZE)),
                        ..default()
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
    if *DEBUG {
        return;
    }
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

    let reveal_radius = 28;

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
    if *DEBUG {
        return;
    }
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

    let map_display_size = island_map_display_size(&game.resolution);

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
                transform: Transform::from_translation(Vec3::new(ISLAND_MAP_X_OFFSET, 0., 903.)),
                material: mat,
                ..default()
            },
            RenderLayers::from_layers(&[3]),
            IslandMapFogOverlay,
            Name::new("ISLAND_MAP_FOG_OVERLAY"),
        ));
    }
}

/// System to close the map when entering a dungeon
fn close_map_on_dungeon_entry(
    dungeon_query: Query<&Dungeon, Added<Dungeon>>,
    mut map_open: ResMut<IslandMapOpen>,
) {
    if !dungeon_query.is_empty() {
        map_open.0 = false;
        info!("Closed minimap on dungeon entry");
    }
}

/// Re-anchor and resize the HUD minimap when the UI layout bucket changes.
pub fn sync_hud_minimap_layout_to_resolution(
    res: Res<ScreenResolution>,
    sync_state: Res<super::layout_sync::UiLayoutSyncState>,
    mut minimap: Query<&mut Transform, With<HudMinimap>>,
    mut sprites: ParamSet<(
        Query<&mut Sprite, With<HudMinimapSprite>>,
        Query<&mut Sprite, With<HudMinimapPlayerMarker>>,
        Query<&mut Sprite, With<HudMinimapIcon>>,
    )>,
) {
    if !super::layout_sync::ui_layout_needs_sync(&res, &sync_state) {
        return;
    }

    let display_size = hud_minimap_display_size(&res);
    let pos_x =
        res.game_width / 2.0 - display_size / 2.0 - HUD_MINIMAP_PADDING + HUD_MINIMAP_RIGHT_NUDGE;
    let pos_y = res.game_height / 2.0 - display_size / 2.0 - HUD_MINIMAP_PADDING;
    let total_pixels = hud_minimap_texture_pixels();
    let scale = display_size / total_pixels as f32;
    for mut transform in minimap.iter_mut() {
        transform.translation.x = pos_x;
        transform.translation.y = pos_y;
    }
    for mut sprite in sprites.p0().iter_mut() {
        sprite.custom_size = Some(Vec2::splat(display_size));
    }
    for mut sprite in sprites.p1().iter_mut() {
        sprite.custom_size = Some(Vec2::splat(HUD_MINIMAP_ICON_SIZE));
    }
    for mut sprite in sprites.p2().iter_mut() {
        sprite.custom_size = Some(Vec2::splat(HUD_MINIMAP_ICON_SIZE));
    }
}

/// Despawn the full island map so `setup_island_map` rebuilds it at the new UI bucket size.
fn invalidate_island_map_on_ui_layout_change(
    res: Res<ScreenResolution>,
    mut last_layout: Local<Option<UiLayoutKey>>,
    mut commands: Commands,
    maps: Query<Entity, With<IslandMap>>,
    fog: Query<Entity, With<IslandMapFogOverlay>>,
    overlay: Query<Entity, With<IslandMapOverlay>>,
    legend: Query<Entity, With<IslandMapLegend>>,
) {
    let key = UiLayoutKey::from_resolution(&res);
    if last_layout.as_ref() == Some(&key) {
        return;
    }
    *last_layout = Some(key);

    for entity in maps
        .iter()
        .chain(fog.iter())
        .chain(overlay.iter())
        .chain(legend.iter())
    {
        commands.entity(entity).despawn_recursive();
    }
}

/// Spawns the HUD minimap entity once when the player exists and we're not in a dungeon.
fn setup_hud_minimap(
    mut commands: Commands,
    mut assets: ResMut<Assets<Image>>,
    graphics: Res<Graphics>,
    existing: Query<Entity, With<HudMinimap>>,
    player_query: Query<Entity, With<Player>>,
    game: GameParam,
) {
    if !existing.is_empty() || player_query.is_empty() {
        return;
    }

    let total_pixels = hud_minimap_texture_pixels();

    let size = Extent3d {
        width: total_pixels,
        height: total_pixels,
        depth_or_array_layers: 1,
    };
    let data = vec![0u8; (total_pixels * total_pixels * 4) as usize];
    let image = Image::new(
        size,
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
    );
    let image_handle = assets.add(image);

    let res = &game.resolution;
    let display_size = hud_minimap_display_size(res);
    let pos_x =
        res.game_width / 2.0 - display_size / 2.0 - HUD_MINIMAP_PADDING + HUD_MINIMAP_RIGHT_NUDGE;
    let pos_y = res.game_height / 2.0 - display_size / 2.0 - HUD_MINIMAP_PADDING;

    let container = commands
        .spawn((
            SpatialBundle {
                transform: Transform::from_translation(Vec3::new(pos_x, pos_y, 6.0)),
                ..default()
            },
            HudMinimap {
                image_handle: image_handle.clone(),
            },
            RenderLayers::from_layers(&[3]),
            Name::new("HUD_MINIMAP"),
        ))
        .id();

    // SpriteBundle (not MaterialMesh2d): ColorMaterial bind groups only refresh on
    // material asset changes, not when the Image texture is updated each frame.
    let map_sprite = commands
        .spawn((
            SpriteBundle {
                texture: image_handle.clone(),
                sprite: Sprite {
                    custom_size: Some(Vec2::splat(display_size)),
                    ..default()
                },
                transform: Transform::from_translation(Vec3::new(0., 0., 0.)),
                ..default()
            },
            HudMinimapSprite,
            RenderLayers::from_layers(&[3]),
            Name::new("HUD_MINIMAP_IMAGE"),
        ))
        .id();
    commands.entity(container).add_child(map_sprite);

    let marker = commands
        .spawn((
            SpriteBundle {
                texture: graphics.get_ui_element_texture(UIElement::PlayerMinimapIcon),
                sprite: Sprite {
                    custom_size: Some(Vec2::splat(HUD_MINIMAP_ICON_SIZE)),
                    ..default()
                },
                transform: Transform::from_translation(Vec3::new(0., 0., 2.0)),
                ..default()
            },
            HudMinimapPlayerMarker,
            RenderLayers::from_layers(&[3]),
            Name::new("HUD_MINIMAP_PLAYER_MARKER"),
        ))
        .id();
    commands.entity(container).add_child(marker);
}

/// Re-renders the HUD minimap texture each frame around the player position with sub-tile precision.
fn update_hud_minimap_texture(
    mut assets: ResMut<Assets<Image>>,
    hud_query: Query<&HudMinimap>,
    player_query: Query<&GlobalTransform, With<Player>>,
    cache: Res<MinimapTileCache>,
    game: GameParam,
) {
    let Ok(hud) = hud_query.get_single() else {
        return;
    };
    let Ok(player_t) = player_query.get_single() else {
        return;
    };
    let Some(image) = assets.get_mut(&hud.image_handle) else {
        return;
    };

    let player_world = player_t.translation().truncate();
    let total_pixels = hud_minimap_texture_pixels();
    let center_pixel = total_pixels as f32 / 2.0;
    let radius_pixels = hud_minimap_radius_pixels();
    let border_thickness: f32 = 2.;
    let era = game.era.current_era.clone();

    let data = &mut image.data;
    let mut idx = 0usize;
    for py in 0..total_pixels {
        for px in 0..total_pixels {
            let dx = px as f32 + 0.5 - center_pixel;
            let dy = center_pixel - (py as f32 + 0.5);
            let dist = (dx * dx + dy * dy).sqrt();

            if dist > radius_pixels + HUD_MINIMAP_SHADOW_THICKNESS {
                data[idx] = 0;
                data[idx + 1] = 0;
                data[idx + 2] = 0;
                data[idx + 3] = 0;
                idx += 4;
                continue;
            }
            if dist > radius_pixels {
                data[idx] = (HUD_MINIMAP_SHADOW_RGB.0 * 255.0) as u8;
                data[idx + 1] = (HUD_MINIMAP_SHADOW_RGB.1 * 255.0) as u8;
                data[idx + 2] = (HUD_MINIMAP_SHADOW_RGB.2 * 255.0) as u8;
                data[idx + 3] = (HUD_MINIMAP_SHADOW_ALPHA * 255.0) as u8;
                idx += 4;
                continue;
            }
            if dist > radius_pixels - border_thickness {
                data[idx] = (DARK_BROWN.r() * 255.0) as u8;
                data[idx + 1] = (DARK_BROWN.g() * 255.0) as u8;
                data[idx + 2] = (DARK_BROWN.b() * 255.0) as u8;
                data[idx + 3] = 255;
                idx += 4;
                continue;
            }

            let world_offset = Vec2::new(
                dx / HUD_MINIMAP_PIXELS_PER_TILE as f32 * TILE_SIZE.x,
                dy / HUD_MINIMAP_PIXELS_PER_TILE as f32 * TILE_SIZE.y,
            );
            let sample_pos = player_world + world_offset;
            let map_pos = world_pos_to_tile_pos(sample_pos);

            let tile_world = tile_pos_to_world_pos(map_pos, false);
            let local = sample_pos - tile_world + Vec2::new(TILE_SIZE.x / 2.0, TILE_SIZE.y / 2.0);
            let qx = if local.x >= TILE_SIZE.x / 2.0 { 1 } else { 0 };
            let qy_top = local.y >= TILE_SIZE.y / 2.0;
            let quadrant = if qy_top { qx } else { 2 + qx };

            let color = if let Some(obj) = cache.cache.get(&map_pos) {
                if !is_grass_obj(obj) {
                    obj.get_obj_color()
                } else if let Some(terrain) = cache.explored_terrain.get(&map_pos) {
                    minimap_base_terrain_color_for_era(terrain[quadrant], &era)
                } else {
                    Color::rgb(0.16, 0.16, 0.16)
                }
            } else if let Some(terrain) = cache.explored_terrain.get(&map_pos) {
                minimap_base_terrain_color_for_era(terrain[quadrant], &era)
            } else {
                Color::rgb(0.16, 0.16, 0.16)
            };

            data[idx] = (color.r() * 255.0) as u8;
            data[idx + 1] = (color.g() * 255.0) as u8;
            data[idx + 2] = (color.b() * 255.0) as u8;
            data[idx + 3] = 255;
            idx += 4;
        }
    }
}

/// Spawns / repositions / removes object icons on the HUD minimap each frame.
fn update_hud_minimap_icons(
    mut commands: Commands,
    cache: Res<MinimapTileCache>,
    graphics: Res<Graphics>,
    resolution: Res<ScreenResolution>,
    container_query: Query<Entity, With<HudMinimap>>,
    mut icon_query: Query<(Entity, &HudMinimapIcon, &mut Transform)>,
    player_query: Query<&GlobalTransform, With<Player>>,
) {
    let Ok(player_t) = player_query.get_single() else {
        return;
    };
    let Ok(container) = container_query.get_single() else {
        return;
    };

    let player_world = player_t.translation().truncate();
    let total_pixels = hud_minimap_texture_pixels();
    let display_size = hud_minimap_display_size(&resolution);
    let scale = display_size / total_pixels as f32;
    let max_icon_center_dist = hud_minimap_max_icon_center_distance(scale);
    let mut desired: HashMap<TileMapPosition, WorldObject> = HashMap::new();
    desired.insert(
        TileMapPosition::new(IVec2::ZERO, TilePos { x: 0, y: 0 }),
        WorldObject::TimePortal,
    );
    for (pos, obj) in cache.cache.iter() {
        if get_icon_for_object(obj).is_some() {
            desired.insert(*pos, *obj);
        }
    }

    desired.retain(|pos, _| {
        let world = tile_pos_to_world_pos(*pos, false);
        let offset = world - player_world;
        let pixel_offset = Vec2::new(
            offset.x / TILE_SIZE.x * HUD_MINIMAP_PIXELS_PER_TILE as f32,
            offset.y / TILE_SIZE.y * HUD_MINIMAP_PIXELS_PER_TILE as f32,
        );
        pixel_offset.length() * scale <= max_icon_center_dist
    });

    let mut existing_keys: HashSet<TileMapPosition> = HashSet::new();
    for (entity, icon, _) in icon_query.iter() {
        let keep = matches!(desired.get(&icon.tile_pos), Some(o) if *o == icon.object_type);
        if keep {
            existing_keys.insert(icon.tile_pos);
        } else {
            commands.entity(entity).despawn_recursive();
        }
    }

    for (pos, obj) in desired.iter() {
        let world = tile_pos_to_world_pos(*pos, false);
        let offset = world - player_world;
        let pixel_offset_x = offset.x / TILE_SIZE.x * HUD_MINIMAP_PIXELS_PER_TILE as f32;
        let pixel_offset_y = offset.y / TILE_SIZE.y * HUD_MINIMAP_PIXELS_PER_TILE as f32;
        let local_x = pixel_offset_x * scale;
        let local_y = pixel_offset_y * scale;

        if existing_keys.contains(pos) {
            for (_, icon, mut tx) in icon_query.iter_mut() {
                if icon.tile_pos == *pos && icon.object_type == *obj {
                    tx.translation.x = local_x;
                    tx.translation.y = local_y;
                    break;
                }
            }
        } else {
            let Some(ui_element) = get_icon_for_object(obj) else {
                continue;
            };
            let icon_entity = commands
                .spawn((
                    SpriteBundle {
                        texture: graphics.get_ui_element_texture(ui_element),
                        sprite: Sprite {
                            custom_size: Some(Vec2::splat(HUD_MINIMAP_ICON_SIZE)),
                            ..default()
                        },
                        transform: Transform::from_translation(Vec3::new(local_x, local_y, 1.0)),
                        ..default()
                    },
                    HudMinimapIcon {
                        tile_pos: *pos,
                        object_type: *obj,
                    },
                    RenderLayers::from_layers(&[3]),
                    Name::new(format!("HUD_MINIMAP_ICON_{:?}", obj)),
                ))
                .id();
            commands.entity(container).add_child(icon_entity);
        }
    }
}

/// Despawns the HUD minimap when entering a dungeon (rebuilt on exit).
fn despawn_hud_minimap_in_dungeon(
    mut commands: Commands,
    dungeon_query: Query<&Dungeon, Added<Dungeon>>,
    hud_query: Query<Entity, With<HudMinimap>>,
) {
    if dungeon_query.is_empty() {
        return;
    }
    for entity in hud_query.iter() {
        commands.entity(entity).despawn_recursive();
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
