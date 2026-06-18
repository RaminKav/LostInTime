//! Player-placed map markers. While the full island map is open the player can left-click to
//! drop a colored "x" marker (one per color: red, purple, blue, in that order). Each marker also
//! spawns a screen-locked beacon (reusing [`ScreenLockedTargetWorldPos`]) that points to the
//! marked world location during normal play. Clicking near an existing marker removes it.

use bevy::{prelude::*, render::view::RenderLayers, sprite::Anchor};

use crate::{
    assets::Graphics,
    colors::{DMG_NUM_PURPLE, MAP_MARKER_BLUE, RED, WHITE},
    cursor::CursorPos,
    player::unlocks::UnlockUpgrades,
    ui::game_fonts as gf,
    world::dimension::SpawnDimension,
    world::dungeon::Dungeon,
    world::{CHUNK_SIZE, ISLAND_SIZE, TILE_SIZE},
    GameState, ScreenResolution,
};

use super::{
    damage_numbers::spawn_screen_locked_icon_to_world_pos,
    minimap::{
        hud_minimap_display_size, hud_minimap_max_icon_center_distance, hud_minimap_texture_pixels,
        island_map_display_size, island_map_legend_center_x, island_map_legend_top_y, HudMinimap,
        IslandMap, IslandMapOpen, HUD_MINIMAP_PIXELS_PER_TILE, ISLAND_MAP_X_OFFSET,
    },
    UIElement, TOOLTIP_INFO_BOX_SIZE,
};
use crate::item::WorldObject;
use crate::Player;

/// Radius (UI space) around an existing marker within which a click removes it instead of adding.
const MARKER_REMOVE_RADIUS: f32 = 5.0;
/// Marker "x" draw size on the island map.
const MAP_MARKER_FONT: gf::FontStyle = gf::FontStyle {
    path: gf::paths::SLKSCR_BOLD,
    size: 8.5,
};

pub struct MapMarkerPlugin;

impl Plugin for MapMarkerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MapMarkers>()
            .add_system(
                clear_markers_on_new_dimension
                    .run_if(in_state(GameState::Main).or_else(in_state(GameState::Initializing))),
            )
            .add_system(
                handle_map_marker_clicks
                    .run_if(in_state(GameState::Main))
                    .run_if(|dungeon: Query<&Dungeon>| dungeon.is_empty()),
            )
            .add_system(sync_map_marker_icons.run_if(in_state(GameState::Main)))
            .add_system(sync_map_marker_beacons.run_if(in_state(GameState::Main)))
            .add_system(sync_hud_minimap_markers.run_if(in_state(GameState::Main)))
            .add_system(sync_map_marker_info_box.run_if(in_state(GameState::Main)));
    }
}

/// Marker colors in fixed assignment order (lowest free index is used for the next placement).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkerColor {
    Red,
    Purple,
    Green,
}

const MARKER_COLOR_ORDER: [MarkerColor; 3] =
    [MarkerColor::Red, MarkerColor::Purple, MarkerColor::Green];

impl MarkerColor {
    fn color(&self) -> Color {
        match self {
            MarkerColor::Red => RED,
            MarkerColor::Purple => DMG_NUM_PURPLE,
            MarkerColor::Green => MAP_MARKER_BLUE,
        }
    }

    /// Beacon world object whose sprite is shown inside the screen-locked beacon slot.
    fn beacon_object(&self) -> WorldObject {
        match self {
            MarkerColor::Red => WorldObject::RedBeacon,
            MarkerColor::Purple => WorldObject::PinkBeacon,
            MarkerColor::Green => WorldObject::YellowBeacon,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MapMarker {
    pub color: MarkerColor,
    pub world_pos: Vec2,
}

#[derive(Resource, Default)]
pub struct MapMarkers {
    pub markers: Vec<MapMarker>,
}

/// On-map "x" rendered as a child of the island map border.
#[derive(Component)]
pub struct MapMarkerOnMap;

/// Screen-locked beacon "x" pointing to a marked world location during play.
#[derive(Component)]
pub struct MapMarkerBeacon;

/// Info box explaining the click-to-mark action, shown beside the open island map.
#[derive(Component)]
pub struct MapMarkerInfoBox;

/// Colored "x" shown on the circular HUD minimap; carries the marker index it tracks.
#[derive(Component)]
pub struct HudMinimapMarker(pub usize);

// --- Coordinate transforms (island map UI <-> world) --------------------------------------------
//
// These mirror the forward transform used by `update_player_marker_on_map`, expressed as pure
// continuous functions so the click->world->map round-trip is exact.

struct IslandMapTransform {
    num_chunks: i32,
    tiles_per_chunk: i32,
    pixels_per_tile: f32,
    total_pixels: f32,
    scale: f32,
}

impl IslandMapTransform {
    fn new(res: &ScreenResolution) -> Self {
        let num_chunks = ((ISLAND_SIZE / CHUNK_SIZE as f32) + 1.) as i32;
        let total_chunks = (num_chunks * 2 + 1) as u32;
        let tiles_per_chunk = CHUNK_SIZE;
        let pixels_per_tile = 2u32;
        let total_pixels = (total_chunks * tiles_per_chunk * pixels_per_tile) as f32;
        let scale = island_map_display_size(res) / total_pixels;
        Self {
            num_chunks,
            tiles_per_chunk: tiles_per_chunk as i32,
            pixels_per_tile: pixels_per_tile as f32,
            total_pixels,
            scale,
        }
    }

    fn world_to_map_local(&self, world: Vec2) -> Vec2 {
        let global_tile_x = world.x / TILE_SIZE.x;
        let global_tile_y = world.y / TILE_SIZE.y;
        let tile_fx = global_tile_x + (self.num_chunks * self.tiles_per_chunk) as f32;
        let tile_fy = ((self.num_chunks + 1) * self.tiles_per_chunk) as f32 - 1.0 - global_tile_y;
        let x = (tile_fx * self.pixels_per_tile - self.total_pixels / 2.0) * self.scale;
        let y = (self.total_pixels / 2.0 - tile_fy * self.pixels_per_tile) * self.scale;
        Vec2::new(x, y)
    }

    fn map_local_to_world(&self, local: Vec2) -> Vec2 {
        let tile_fx = (local.x / self.scale + self.total_pixels / 2.0) / self.pixels_per_tile;
        let tile_fy = (self.total_pixels / 2.0 - local.y / self.scale) / self.pixels_per_tile;
        let global_tile_x = tile_fx - (self.num_chunks * self.tiles_per_chunk) as f32;
        let global_tile_y = ((self.num_chunks + 1) * self.tiles_per_chunk) as f32 - 1.0 - tile_fy;
        Vec2::new(global_tile_x * TILE_SIZE.x, global_tile_y * TILE_SIZE.y)
    }
}

/// Pick the lowest-index color not currently in use (only ever called when capacity allows).
fn next_free_color(markers: &[MapMarker]) -> Option<MarkerColor> {
    MARKER_COLOR_ORDER
        .into_iter()
        .find(|c| !markers.iter().any(|m| m.color == *c))
}

fn handle_map_marker_clicks(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    map_open: Res<IslandMapOpen>,
    res: Res<ScreenResolution>,
    upgrades: Res<UnlockUpgrades>,
    mut markers: ResMut<MapMarkers>,
) {
    if !map_open.0 || !mouse_input.just_pressed(MouseButton::Left) {
        return;
    }

    let transform = IslandMapTransform::new(&res);
    // Map is shifted right by ISLAND_MAP_X_OFFSET; work in map-border-local space.
    let click = cursor_pos.ui_coords.truncate() - Vec2::new(ISLAND_MAP_X_OFFSET, 0.);

    // Ignore clicks outside the map panel bounds.
    let half = island_map_display_size(&res) / 2.0;
    if click.x.abs() > half || click.y.abs() > half {
        return;
    }

    // Remove if clicking near an existing marker.
    if let Some(idx) = markers.markers.iter().position(|m| {
        transform.world_to_map_local(m.world_pos).distance(click) <= MARKER_REMOVE_RADIUS
    }) {
        markers.markers.remove(idx);
        return;
    }

    // At capacity: drop the oldest marker to make room for this new one.
    let capacity = upgrades.map_marker_count() as usize;
    while markers.markers.len() >= capacity && !markers.markers.is_empty() {
        markers.markers.remove(0);
    }

    if let Some(color) = next_free_color(&markers.markers) {
        let world_pos = transform.map_local_to_world(click);
        markers.markers.push(MapMarker { color, world_pos });
    }
}

/// Rebuilds the on-map "x" markers (children of the island map border) when needed.
fn sync_map_marker_icons(
    mut commands: Commands,
    map_open: Res<IslandMapOpen>,
    res: Res<ScreenResolution>,
    markers: Res<MapMarkers>,
    asset_server: Res<AssetServer>,
    existing: Query<Entity, With<MapMarkerOnMap>>,
    map_border: Query<Entity, With<IslandMap>>,
) {
    let existing_count = existing.iter().count();

    if !map_open.0 {
        if existing_count > 0 {
            for e in existing.iter() {
                commands.entity(e).despawn_recursive();
            }
        }
        return;
    }

    // Only rebuild when something changed or the border was rebuilt (dropping our children).
    if !markers.is_changed() && !map_open.is_changed() && existing_count == markers.markers.len() {
        return;
    }

    for e in existing.iter() {
        commands.entity(e).despawn_recursive();
    }

    let Ok(border) = map_border.get_single() else {
        return;
    };

    let transform = IslandMapTransform::new(&res);
    for marker in markers.markers.iter() {
        let pos = transform.world_to_map_local(marker.world_pos);
        let icon = commands
            .spawn((
                Text2dBundle {
                    text: Text::from_section(
                        "x",
                        MAP_MARKER_FONT.text_style(&asset_server, marker.color.color()),
                    )
                    .with_alignment(TextAlignment::Center),
                    text_anchor: Anchor::Center,
                    transform: Transform::from_translation(Vec3::new(pos.x, pos.y, 6.0)),
                    ..default()
                },
                RenderLayers::from_layers(&[3]),
                MapMarkerOnMap,
                Name::new("MAP_MARKER_ON_MAP"),
            ))
            .id();
        commands.entity(border).add_child(icon);
    }
}

/// Rebuilds the screen-locked beacon icons (beacon sprite inside a slot) when markers change.
fn sync_map_marker_beacons(
    mut commands: Commands,
    graphics: Res<Graphics>,
    markers: Res<MapMarkers>,
    asset_server: Res<AssetServer>,
    existing: Query<Entity, With<MapMarkerBeacon>>,
) {
    if !markers.is_changed() {
        return;
    }

    for e in existing.iter() {
        commands.entity(e).despawn_recursive();
    }

    for marker in markers.markers.iter() {
        let beacon = spawn_screen_locked_icon_to_world_pos(
            &mut commands,
            &graphics,
            &asset_server,
            marker.color.beacon_object(),
            marker.world_pos,
        );
        commands.entity(beacon).insert(MapMarkerBeacon);
    }
}

/// Renders the markers on the circular HUD minimap (player-centered, hidden when out of range).
fn sync_hud_minimap_markers(
    mut commands: Commands,
    markers: Res<MapMarkers>,
    asset_server: Res<AssetServer>,
    resolution: Res<ScreenResolution>,
    container_query: Query<Entity, With<HudMinimap>>,
    player_query: Query<&GlobalTransform, With<Player>>,
    mut marker_query: Query<(Entity, &HudMinimapMarker, &mut Transform, &mut Visibility)>,
) {
    let Ok(player_t) = player_query.get_single() else {
        return;
    };
    let Ok(container) = container_query.get_single() else {
        // No HUD minimap (e.g. in a dungeon): drop any stale marker icons.
        for (e, _, _, _) in marker_query.iter() {
            commands.entity(e).despawn_recursive();
        }
        return;
    };

    let player_world = player_t.translation().truncate();
    let total_pixels = hud_minimap_texture_pixels();
    let display_size = hud_minimap_display_size(&resolution);
    let scale = display_size / total_pixels as f32;
    let max_dist = hud_minimap_max_icon_center_distance(scale);

    let local_pos = |marker: &MapMarker| -> Vec2 {
        let offset = marker.world_pos - player_world;
        Vec2::new(
            offset.x / TILE_SIZE.x * HUD_MINIMAP_PIXELS_PER_TILE as f32 * scale,
            offset.y / TILE_SIZE.y * HUD_MINIMAP_PIXELS_PER_TILE as f32 * scale,
        )
    };

    let needs_rebuild =
        markers.is_changed() || marker_query.iter().count() != markers.markers.len();

    if needs_rebuild {
        for (e, _, _, _) in marker_query.iter() {
            commands.entity(e).despawn_recursive();
        }
        for (i, marker) in markers.markers.iter().enumerate() {
            let pos = local_pos(marker);
            let visible = pos.length() <= max_dist;
            let icon = commands
                .spawn((
                    Text2dBundle {
                        text: Text::from_section(
                            "x",
                            MAP_MARKER_FONT.text_style(&asset_server, marker.color.color()),
                        )
                        .with_alignment(TextAlignment::Center),
                        text_anchor: Anchor::Center,
                        transform: Transform::from_translation(Vec3::new(pos.x, pos.y, 3.0)),
                        visibility: if visible {
                            Visibility::Inherited
                        } else {
                            Visibility::Hidden
                        },
                        ..default()
                    },
                    RenderLayers::from_layers(&[3]),
                    HudMinimapMarker(i),
                    Name::new("HUD_MINIMAP_MARKER"),
                ))
                .id();
            commands.entity(container).add_child(icon);
        }
        return;
    }

    for (_, hud_marker, mut tx, mut vis) in marker_query.iter_mut() {
        let Some(marker) = markers.markers.get(hud_marker.0) else {
            continue;
        };
        let pos = local_pos(marker);
        tx.translation.x = pos.x;
        tx.translation.y = pos.y;
        *vis = if pos.length() <= max_dist {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

/// Spawns / despawns the "Click to mark a location on the map" info box alongside the open map.
fn sync_map_marker_info_box(
    mut commands: Commands,
    map_open: Res<IslandMapOpen>,
    res: Res<ScreenResolution>,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    existing: Query<Entity, With<MapMarkerInfoBox>>,
) {
    if !map_open.0 {
        for e in existing.iter() {
            commands.entity(e).despawn_recursive();
        }
        return;
    }

    if !existing.is_empty() && !map_open.is_changed() && !res.is_changed() {
        return;
    }

    for e in existing.iter() {
        commands.entity(e).despawn_recursive();
    }

    // Sit on the left, above the legend icon stack.
    let pos = Vec2::new(
        island_map_legend_center_x(&res) - 22.,
        island_map_legend_top_y() + TOOLTIP_INFO_BOX_SIZE.y / 2.0 + 10.,
    );

    let box_e = commands
        .spawn((
            SpriteBundle {
                texture: graphics.get_ui_element_texture(UIElement::TooltipInfoBox),
                sprite: Sprite {
                    custom_size: Some(TOOLTIP_INFO_BOX_SIZE),
                    ..default()
                },
                transform: Transform::from_translation(Vec3::new(pos.x, pos.y, 905.)),
                ..default()
            },
            RenderLayers::from_layers(&[3]),
            MapMarkerInfoBox,
            Name::new("MAP_MARKER_INFO_BOX"),
        ))
        .id();

    for (line, y) in [("Click to mark", 5.), ("a location on the map", -6.)] {
        commands
            .spawn((
                Text2dBundle {
                    text: Text::from_section(
                        line.to_string(),
                        gf::TOOLTIP_INFO_BOX.text_style(&asset_server, WHITE),
                    )
                    .with_alignment(TextAlignment::Center),
                    text_anchor: Anchor::Center,
                    transform: Transform::from_translation(Vec3::new(0., y, 2.)),
                    ..default()
                },
                RenderLayers::from_layers(&[3]),
            ))
            .set_parent(box_e);
    }
}

/// Clear placed markers when a new dimension is generated (world coordinates change).
fn clear_markers_on_new_dimension(
    new_dim: Query<Entity, Added<SpawnDimension>>,
    mut markers: ResMut<MapMarkers>,
) {
    if !new_dim.is_empty() {
        markers.markers.clear();
    }
}
