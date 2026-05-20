use bevy::{prelude::*, render::view::RenderLayers, sprite::Anchor};

use crate::{
    colors::WHITE,
    cursor::CursorPos,
    ui::{
        game_fonts as gf, interactions::Interactable, tooltips::clamp_tooltip_center_x, ui_helpers,
        UIState, KEYBIND_BADGE_COLOR, UI_SLOT_SIZE,
    },
    ScreenResolution,
};

/// Grey translucent backdrop (same fill as HUD keybind badges).
pub const ICON_HOVER_TOOLTIP_BG_COLOR: Color = Color::rgba(62. / 255., 58. / 255., 58. / 255., 0.9);

/// World-space Z for icon hover tooltips (above inventory panels and slots).
pub const ICON_HOVER_TOOLTIP_Z: f32 = 30.0;

const TOOLTIP_PAD_X: f32 = 10.;
const TOOLTIP_PAD_Y: f32 = 5.;
const TOOLTIP_LINE_HEIGHT: f32 = 10.0;
/// Rough slkscr 8.5 advance for sizing the backdrop.
const TOOLTIP_CHAR_WIDTH: f32 = 4.6;
const TOOLTIP_GAP_FROM_ICON: f32 = 4.;
const TOOLTIP_SCREEN_EDGE_PAD: f32 = 6.;

/// Attach to any [`Interactable`] UI icon that should show a short hover label.
///
/// Multi-line tooltips: pass a `&'static` slice of lines, e.g.
/// `IconHoverTooltipText(&["Sort inventory", "Click to sort"])`.
#[derive(Component, Clone, Copy)]
pub struct IconHoverTooltipText(pub &'static [&'static str]);

#[derive(Component)]
pub struct IconHoverTooltip;

fn tooltip_backdrop_size(lines: &[&str]) -> Vec2 {
    let line_count = lines.len().max(1) as f32;
    let widest = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) as f32;
    let text_w = widest * TOOLTIP_CHAR_WIDTH;
    Vec2::new(
        (text_w + TOOLTIP_PAD_X * 2.).max(28.),
        TOOLTIP_LINE_HEIGHT * line_count + TOOLTIP_PAD_Y * 2.,
    )
}

fn icon_hover_tooltip_world_pos(icon_center: Vec3, size: Vec2, game_width: f32) -> Vec3 {
    let half_w = size.x * 0.5;
    let right_x = icon_center.x + UI_SLOT_SIZE.x * 0.5 + half_w + TOOLTIP_GAP_FROM_ICON;
    let left_x = icon_center.x - UI_SLOT_SIZE.x * 0.5 - half_w - TOOLTIP_GAP_FROM_ICON;
    let screen_right = game_width * 0.5 - TOOLTIP_SCREEN_EDGE_PAD;
    let screen_left = -game_width * 0.5 + TOOLTIP_SCREEN_EDGE_PAD;

    let x = if right_x + half_w <= screen_right {
        right_x
    } else if left_x - half_w >= screen_left {
        left_x
    } else {
        right_x
    };

    Vec3::new(
        clamp_tooltip_center_x(x, half_w, game_width, TOOLTIP_SCREEN_EDGE_PAD),
        icon_center.y,
        ICON_HOVER_TOOLTIP_Z,
    )
}

/// Spawns a compact tooltip beside `icon_center` (world space).
pub fn spawn_icon_hover_tooltip(
    commands: &mut Commands,
    asset_server: &AssetServer,
    lines: &[&str],
    icon_center: Vec3,
    game_width: f32,
) -> Entity {
    let size = tooltip_backdrop_size(lines);
    let pos = icon_hover_tooltip_world_pos(icon_center, size, game_width);

    let root = commands
        .spawn((
            SpatialBundle::from_transform(Transform::from_translation(pos)),
            RenderLayers::from_layers(&[3]),
            IconHoverTooltip,
            Name::new("Icon Hover Tooltip"),
        ))
        .id();

    commands
        .spawn(SpriteBundle {
            sprite: Sprite {
                color: ICON_HOVER_TOOLTIP_BG_COLOR,
                custom_size: Some(size),
                ..default()
            },
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .set_parent(root);

    // Stack each line top-down so the block is vertically centered inside the backdrop.
    let line_count = lines.len().max(1) as f32;
    let block_top = (line_count - 1.) * TOOLTIP_LINE_HEIGHT * 0.5;
    for (i, line) in lines.iter().enumerate() {
        let y = block_top - i as f32 * TOOLTIP_LINE_HEIGHT;
        commands
            .spawn(Text2dBundle {
                text: Text::from_section(
                    *line,
                    gf::ICON_HOVER_TOOLTIP.text_style(asset_server, WHITE),
                )
                .with_alignment(TextAlignment::Center),
                text_anchor: Anchor::Center,
                transform: Transform::from_translation(Vec3::new(0., y, 1.)),
                ..default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .set_parent(root);
    }

    root
}

fn inventory_family_ui_open(ui_state: &UIState) -> bool {
    matches!(
        ui_state,
        UIState::Inventory | UIState::InventoryCrafting | UIState::Crafting
    )
}

/// Pointcast-driven hover tooltips for entities tagged with [`IconHoverTooltipText`].
pub fn handle_icon_hover_tooltips(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    cursor_pos: Res<CursorPos>,
    resolution: Res<ScreenResolution>,
    ui_state: Res<State<UIState>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    tooltip_targets: Query<(&GlobalTransform, &IconHoverTooltipText)>,
    existing: Query<Entity, With<IconHoverTooltip>>,
    mut last_hovered: Local<Option<Entity>>,
) {
    if !inventory_family_ui_open(&ui_state.0) {
        for tooltip_e in existing.iter() {
            commands.entity(tooltip_e).despawn_recursive();
        }
        *last_hovered = None;
        return;
    }

    let hovered =
        ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None).and_then(|(entity, _, _)| {
            tooltip_targets
                .get(entity)
                .ok()
                .map(|(transform, text)| (entity, transform.translation(), text.0))
        });

    if *last_hovered == hovered.map(|(e, _, _)| e) {
        return;
    }

    for tooltip_e in existing.iter() {
        commands.entity(tooltip_e).despawn_recursive();
    }

    if let Some((_, icon_center, lines)) = hovered {
        spawn_icon_hover_tooltip(
            &mut commands,
            &asset_server,
            lines,
            icon_center,
            resolution.game_width,
        );
    }

    *last_hovered = hovered.map(|(e, _, _)| e);
}
