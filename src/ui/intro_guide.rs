use bevy::{prelude::*, render::view::RenderLayers, sprite::Anchor};

use crate::{
    client::GameData,
    colors::BLACK,
    gamepad_bindings::{format_binding_label, BindingLabel, GamepadMappings},
    keybinds::InputMappings,
    GameState, ScreenResolution,
};

/// Render layer shared with the rest of the on-screen HUD (see `player_hud`).
const INTRO_GUIDE_RENDER_LAYER: u8 = 3;

/// Square key-cap badge size — at least double the HUD keybind badge (19x9).
const INTRO_KEY_BADGE_SIZE: Vec2 = Vec2::new(24., 24.);
/// Alagard text size (rule: alagard uses intervals of 15.0, defaulting to 15.0).
const INTRO_FONT_SIZE: f32 = 15.0;

const FADE_IN_SECS: f32 = 0.6;
const HOLD_SECS: f32 = 12.0;
const FADE_OUT_SECS: f32 = 0.6;

const TEXT_TARGET_ALPHA: f32 = 1.0;
const BADGE_TARGET_ALPHA: f32 = 0.85;

#[derive(Component)]
pub struct IntroGuideRoot {
    elapsed: f32,
}

/// Tags every sprite / text under the guide so the fade system can drive its alpha.
/// Stores the fully-faded-in (target) alpha for this element.
#[derive(Component)]
pub struct IntroGuideVisual {
    target_alpha: f32,
}

fn key_label(
    label: BindingLabel,
    keybinds: &InputMappings,
    gamepad_mappings: &GamepadMappings,
    gamepads: &Gamepads,
) -> String {
    format_binding_label(label, keybinds, gamepad_mappings, gamepads)
}

fn spawn_key_badge(
    commands: &mut Commands,
    asset_server: &AssetServer,
    label: String,
    size: Vec2,
    transform: Transform,
    parent: Entity,
) {
    let badge = commands
        .spawn(SpriteBundle {
            sprite: Sprite {
                color: crate::ui::KEYBIND_BADGE_COLOR.with_a(0.),
                custom_size: Some(size),
                ..default()
            },
            transform,
            ..default()
        })
        .insert(RenderLayers::from_layers(&[INTRO_GUIDE_RENDER_LAYER]))
        .insert(IntroGuideVisual {
            target_alpha: BADGE_TARGET_ALPHA,
        })
        .set_parent(parent)
        .id();

    commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                label,
                TextStyle {
                    font: asset_server.load("fonts/alagard.ttf"),
                    font_size: INTRO_FONT_SIZE,
                    color: crate::colors::WHITE.with_a(0.),
                },
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: Anchor::Center,
            transform: Transform::from_translation(Vec3::new(0., 0., 1.)),
            ..default()
        })
        .insert(RenderLayers::from_layers(&[INTRO_GUIDE_RENDER_LAYER]))
        .insert(IntroGuideVisual {
            target_alpha: TEXT_TARGET_ALPHA,
        })
        .set_parent(badge);
}

fn spawn_caption(
    commands: &mut Commands,
    asset_server: &AssetServer,
    text: String,
    anchor: Anchor,
    transform: Transform,
    parent: Entity,
) {
    let render_layers = RenderLayers::from_layers(&[INTRO_GUIDE_RENDER_LAYER]);
    let font = asset_server.load("fonts/alagard.ttf");

    let caption = commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                text.clone(),
                TextStyle {
                    font: font.clone(),
                    font_size: INTRO_FONT_SIZE,
                    color: crate::colors::WHITE.with_a(0.),
                },
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: anchor.clone(),
            transform,
            ..default()
        })
        .insert(render_layers.clone())
        .insert(IntroGuideVisual {
            target_alpha: TEXT_TARGET_ALPHA,
        })
        .set_parent(parent)
        .id();

    // Shadow copy — same pattern as `spawn_floating_text_with_shadow` in damage_numbers.rs.
    commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                text,
                TextStyle {
                    font,
                    font_size: INTRO_FONT_SIZE,
                    color: BLACK.with_a(0.),
                },
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: anchor.clone(),
            transform: Transform::from_translation(Vec3::new(1., -1., -1.)),
            ..default()
        })
        .insert(render_layers)
        .insert(IntroGuideVisual {
            target_alpha: TEXT_TARGET_ALPHA,
        })
        .set_parent(caption);
}

pub fn spawn_intro_guide(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    keybinds: Res<InputMappings>,
    gamepad_mappings: Res<GamepadMappings>,
    gamepads: Res<Gamepads>,
    resolution: Res<ScreenResolution>,
    game_data: Option<Res<GameData>>,
) {
    // Only on the very first run (no completed runs recorded yet).
    let is_first_run = game_data.map(|data| data.num_runs == 0).unwrap_or(true);
    if !is_first_run {
        return;
    }

    // Sit just above the gameplay HUD (active skills at z=4) but below the inventory /
    // chest full-screen overlays (z=9) so opening the inventory while the guide is still
    // fading covers it instead of the guide drawing on top. Child offsets stay < 1.
    let root = commands
        .spawn(SpatialBundle::from_transform(Transform::from_translation(
            Vec3::new(0., 0., 5.),
        )))
        .insert(IntroGuideRoot { elapsed: 0. })
        .insert(Name::new("Intro Guide"))
        .id();

    let key = INTRO_KEY_BADGE_SIZE;
    let gap = 4.;
    let step = key.x + gap;

    // ----- Left section: WASD movement, ~25% in from the left edge. -----
    let left_x = -resolution.game_width * 0.25;
    let left_group = commands
        .spawn(SpatialBundle::from_transform(Transform::from_translation(
            Vec3::new(left_x, 0., 0.),
        )))
        .set_parent(root)
        .id();

    // "Move" caption sits above the keys.
    spawn_caption(
        &mut commands,
        &asset_server,
        "Move".to_string(),
        Anchor::Center,
        Transform::from_translation(Vec3::new(0., key.y * 1.5 + gap * 2. + 12., 1.)),
        left_group,
    );

    let top_row_y = key.y * 0.5 + gap * 0.5;
    let bottom_row_y = -key.y * 0.5 - gap * 0.5;
    // W centered on the top row.
    spawn_key_badge(
        &mut commands,
        &asset_server,
        "W".to_string(),
        key,
        Transform::from_translation(Vec3::new(0., top_row_y, 0.)),
        left_group,
    );
    // A S D across the bottom row.
    for (i, label) in ["A", "S", "D"].iter().enumerate() {
        let x = (i as f32 - 1.) * step;
        spawn_key_badge(
            &mut commands,
            &asset_server,
            label.to_string(),
            key,
            Transform::from_translation(Vec3::new(x, bottom_row_y, 0.)),
            left_group,
        );
    }

    // ----- Right section: action keybinds, ~25% in from the right edge. -----
    let right_x = resolution.game_width * 0.25;
    let right_group = commands
        .spawn(SpatialBundle::from_transform(Transform::from_translation(
            Vec3::new(right_x, 0., 0.),
        )))
        .set_parent(root)
        .id();

    let rows = [
        (
            key_label(
                BindingLabel::AttackAutoTarget,
                &keybinds,
                &gamepad_mappings,
                &gamepads,
            ),
            "Auto Aim",
        ),
        (
            key_label(
                BindingLabel::Inventory,
                &keybinds,
                &gamepad_mappings,
                &gamepads,
            ),
            "Inventory",
        ),
        (
            key_label(
                BindingLabel::Minimap,
                &keybinds,
                &gamepad_mappings,
                &gamepads,
            ),
            "Map",
        ),
    ];
    let row_step = key.y + gap * 2.5;
    let total_height = row_step * (rows.len() as f32 - 1.);
    let label_x = key.x * 0.5 + gap;
    for (i, (badge_label, caption)) in rows.iter().enumerate() {
        let y = total_height * 0.5 - i as f32 * row_step;
        let badge_size = if *caption == "Inventory" {
            Vec2::new(key.x + 8., key.y)
        } else {
            key
        };
        let badge_x = -badge_size.x * 0.5 - gap;
        spawn_key_badge(
            &mut commands,
            &asset_server,
            badge_label.clone(),
            badge_size,
            Transform::from_translation(Vec3::new(badge_x, y, 0.)),
            right_group,
        );
        spawn_caption(
            &mut commands,
            &asset_server,
            caption.to_string(),
            Anchor::CenterLeft,
            Transform::from_translation(Vec3::new(label_x, y, 1.)),
            right_group,
        );
    }
}

/// Fades the guide in, holds it, fades it back out, then despawns the whole tree.
pub fn tick_intro_guide(
    mut commands: Commands,
    time: Res<Time>,
    mut roots: Query<(Entity, &mut IntroGuideRoot)>,
    mut visuals: Query<
        (&IntroGuideVisual, Option<&mut Sprite>, Option<&mut Text>),
        Or<(With<Sprite>, With<Text>)>,
    >,
) {
    let Ok((root_entity, mut root)) = roots.get_single_mut() else {
        return;
    };

    root.elapsed += time.delta_seconds();
    let t = root.elapsed;

    let fade = if t < FADE_IN_SECS {
        (t / FADE_IN_SECS).clamp(0., 1.)
    } else if t < FADE_IN_SECS + HOLD_SECS {
        1.0
    } else if t < FADE_IN_SECS + HOLD_SECS + FADE_OUT_SECS {
        1.0 - ((t - FADE_IN_SECS - HOLD_SECS) / FADE_OUT_SECS).clamp(0., 1.)
    } else {
        commands.entity(root_entity).despawn_recursive();
        return;
    };

    for (visual, sprite, text) in visuals.iter_mut() {
        let alpha = visual.target_alpha * fade;
        if let Some(mut sprite) = sprite {
            sprite.color.set_a(alpha);
        }
        if let Some(mut text) = text {
            for section in text.sections.iter_mut() {
                section.style.color.set_a(alpha);
            }
        }
    }
}

pub struct IntroGuidePlugin;

impl Plugin for IntroGuidePlugin {
    fn build(&self, app: &mut App) {
        app.add_system(
            spawn_intro_guide
                .run_if(crate::run_once_per_run())
                .in_schedule(OnEnter(GameState::Main)),
        )
        .add_system(tick_intro_guide.run_if(in_state(GameState::Main)));
    }
}
