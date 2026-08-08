use bevy::text::Justify;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::{
    assets::Graphics,
    colors::{RED, WHITE},
    inventory::ItemStack,
    item::WorldObject,
    night::NightTracker,
    world::dimension::Era,
    ScreenResolution,
};

use super::{
    game_fonts::{FontStyle, GLOBAL_MESSAGE, GLOBAL_MESSAGE_SUBTEXT},
    spawn_item_stack_icon,
    ui_helpers::Z_DEPTH_GLOBAL_TEXT_MESSAGE,
};

const MESSAGE_DISPLAY_SECS: f32 = 5.;
const TOP_MARGIN: f32 = 140.0;

/// Matches tutorial / contextual tip panels in [`super::tutorial_ui`].
const MESSAGE_PANEL_COLOR: Color = Color::srgba(0.15, 0.12, 0.10, 0.75);
const PANEL_HORIZONTAL_PADDING: f32 = 28.0;
const PANEL_VERTICAL_PADDING: f32 = 8.0;
const PANEL_MIN_WIDTH: f32 = 100.0;
const PANEL_MAX_WIDTH_FRACTION: f32 = 0.6;
const PANEL_MIN_HEIGHT: f32 = 38.0;
const PANEL_LINE_HEIGHT_FACTOR: f32 = 1.35;

const SUBTEXT_PANEL_HORIZONTAL_PADDING: f32 = 20.0;
const SUBTEXT_PANEL_VERTICAL_PADDING: f32 = 0.0;
const SUBTEXT_PANEL_MIN_HEIGHT: f32 = 16.0;
const SUBTEXT_PANEL_MIN_WIDTH: f32 = 40.0;

const CARD_BASE_W: f32 = 16.;
const ICON_SCALE: f32 = 2.0;
const ICON_DISPLAY_W: f32 = CARD_BASE_W * ICON_SCALE;
const ICON_TEXT_GAP: f32 = 10.0;
/// Half-width estimate for Alagard at [`GLOBAL_MESSAGE`] size (tune spacing vs. overlap).
const ESTIMATED_HALF_WIDTH_FACTOR: f32 = 0.24;

#[derive(Clone, Message)]
pub struct GlobalTextMessageEvent {
    pub text: String,
    pub color: Color,
    pub font_style: FontStyle,
    pub icon_item: Option<WorldObject>,
    /// Exact panel size; overrides [`Self::panel_width`].
    pub panel_size: Option<Vec2>,
    /// Panel width only; height is still derived from the message text.
    pub panel_width: Option<f32>,
    pub sub_text: Option<String>,
    pub sub_text_color: Option<Color>,
    /// Subtitle panel width only; height is derived from [`Self::sub_text`].
    pub sub_panel_width: Option<f32>,
}

impl GlobalTextMessageEvent {
    pub fn new(text: impl Into<String>, color: Color) -> Self {
        Self {
            text: text.into(),
            color,
            font_style: GLOBAL_MESSAGE,
            icon_item: None,
            panel_size: None,
            panel_width: None,
            sub_text: None,
            sub_text_color: None,
            sub_panel_width: None,
        }
    }

    pub fn with_icon(mut self, item: WorldObject) -> Self {
        self.icon_item = Some(item);
        self
    }

    pub fn with_panel_size(mut self, size: Vec2) -> Self {
        self.panel_size = Some(size);
        self.panel_width = None;
        self
    }

    pub fn with_panel_width(mut self, width: f32) -> Self {
        self.panel_width = Some(width);
        self.panel_size = None;
        self
    }

    pub fn with_sub_text(mut self, text: impl Into<String>, color: Color) -> Self {
        self.sub_text = Some(text.into());
        self.sub_text_color = Some(color);
        self
    }

    pub fn with_sub_panel_width(mut self, width: f32) -> Self {
        self.sub_panel_width = Some(width);
        self
    }

    pub fn day_announcement(day: u8, color: Color) -> Self {
        Self::new(format!("Day {}", day), color)
    }

    pub fn era_start_announcement(era: Era) -> Option<Self> {
        let label = era.run_start_announcement_label()?;
        Some(Self::new(label, era.minimap_grass_background_color()))
    }

    /// Shown when the era 3 boss is defeated and endless mode begins.
    pub fn final_endless_survive_prompt() -> Self {
        Self::new("Survive...?", RED)
    }
}

/// Queued when an era title should appear in [`crate::GameState::Main`].
#[derive(Resource)]
pub enum PendingEraAnnouncement {
    /// New run: wait for [`super::main_menu::GameStartFadein`] before showing.
    DuringRunFadeIn,
    /// Era 2/3 portal transition: show as soon as Main loads (no run-start fade).
    OnEraEnter(Era),
}

#[derive(Component)]
pub struct GlobalTextMessage {
    timer: Timer,
}

#[derive(Component)]
pub(crate) struct GlobalTextMessagePart;

/// Vertical layout for main + optional subtitle panels (shared center at root origin).
struct MessageStackLayout {
    main_center_y: f32,
    sub_center_y: f32,
}

fn estimated_text_half_width(text: &str, font_size: f32) -> f32 {
    text.lines()
        .map(|line| line.chars().count() as f32 * font_size * ESTIMATED_HALF_WIDTH_FACTOR)
        .fold(0.0, f32::max)
}

fn estimated_text_full_width(text: &str, font_size: f32) -> f32 {
    estimated_text_half_width(text, font_size) * 2.0
}

fn icon_x_left_of_text(text_half_width: f32) -> f32 {
    -text_half_width - ICON_TEXT_GAP - ICON_DISPLAY_W * 0.5
}

fn message_panel_height(text: &str, font_size: f32) -> f32 {
    let line_count = text.lines().count().max(1);
    let content_h = line_count as f32 * font_size * PANEL_LINE_HEIGHT_FACTOR;
    (content_h + PANEL_VERTICAL_PADDING * 2.0).max(PANEL_MIN_HEIGHT)
}

fn subtext_panel_height(text: &str) -> f32 {
    let font_size = GLOBAL_MESSAGE_SUBTEXT.size;
    let line_count = text.lines().count().max(1);
    let content_h = line_count as f32 * font_size * PANEL_LINE_HEIGHT_FACTOR;
    (content_h + SUBTEXT_PANEL_VERTICAL_PADDING * 2.0).max(SUBTEXT_PANEL_MIN_HEIGHT)
}

fn message_panel_width(
    text: &str,
    font_size: f32,
    game_width: f32,
    has_icon: bool,
    panel_width: Option<f32>,
) -> f32 {
    if let Some(width) = panel_width {
        return width;
    }
    let text_w = estimated_text_full_width(text, font_size);
    let content_w = if has_icon {
        text_w + ICON_DISPLAY_W + ICON_TEXT_GAP
    } else {
        text_w
    };
    (content_w + PANEL_HORIZONTAL_PADDING * 2.0)
        .clamp(PANEL_MIN_WIDTH, game_width * PANEL_MAX_WIDTH_FRACTION)
}

fn subtext_panel_width(text: &str, game_width: f32, sub_panel_width: Option<f32>) -> f32 {
    if let Some(width) = sub_panel_width {
        return width;
    }
    let text_w = estimated_text_full_width(text, GLOBAL_MESSAGE_SUBTEXT.size);
    (text_w + SUBTEXT_PANEL_HORIZONTAL_PADDING * 2.0).clamp(
        SUBTEXT_PANEL_MIN_WIDTH,
        game_width * PANEL_MAX_WIDTH_FRACTION,
    )
}

fn message_panel_size(event: &GlobalTextMessageEvent, game_width: f32) -> Vec2 {
    if let Some(size) = event.panel_size {
        return size;
    }
    let height = message_panel_height(&event.text, event.font_style.size);
    let width = message_panel_width(
        &event.text,
        event.font_style.size,
        game_width,
        event.icon_item.is_some(),
        event.panel_width,
    );
    Vec2::new(width, height)
}

fn message_stack_layout(main_h: f32, sub_h: f32) -> MessageStackLayout {
    if sub_h <= 0.0 {
        return MessageStackLayout {
            main_center_y: 0.0,
            sub_center_y: 0.0,
        };
    }
    // Stack top→bottom: main panel, then sub panel flush below (no gap).
    MessageStackLayout {
        main_center_y: sub_h * 0.5,
        sub_center_y: -main_h * 0.5,
    }
}

fn spawn_panel_sprite(commands: &mut Commands, parent: Entity, size: Vec2, center: Vec3) {
    commands
        .spawn((
            (
                Sprite {
                    color: MESSAGE_PANEL_COLOR,
                    custom_size: Some(size),
                    ..default()
                },
                Transform::from_translation(center),
            ),
            RenderLayers::from_layers(&[3]),
            GlobalTextMessagePart,
            Name::new("Global Text Message Panel"),
        ))
        .insert(ChildOf(parent));
}

fn spawn_main_text(
    commands: &mut Commands,
    parent: Entity,
    asset_server: &AssetServer,
    text: &str,
    style: FontStyle,
    color: Color,
    center: Vec3,
) {
    commands
        .spawn((
            style
                .text(asset_server, text, color)
                .justify(Justify::Center)
                .anchor(Anchor::CENTER)
                .with_transform(Transform {
                    translation: center,
                    scale: style.transform_scale(),
                    ..default()
                }),
            RenderLayers::from_layers(&[3]),
            GlobalTextMessagePart,
            Name::new("Global Text Message Text"),
        ))
        .insert(ChildOf(parent));
}

fn spawn_sub_text(
    commands: &mut Commands,
    parent: Entity,
    asset_server: &AssetServer,
    text: &str,
    color: Color,
    center: Vec3,
) {
    commands
        .spawn((
            GLOBAL_MESSAGE_SUBTEXT
                .text(asset_server, text, color)
                .justify(Justify::Center)
                .anchor(Anchor::CENTER)
                .with_transform(Transform {
                    translation: center,
                    scale: GLOBAL_MESSAGE_SUBTEXT.transform_scale(),
                    ..default()
                }),
            RenderLayers::from_layers(&[3]),
            GlobalTextMessagePart,
            Name::new("Global Text Message Subtext"),
        ))
        .insert(ChildOf(parent));
}

pub fn show_pending_era_announcement(
    pending: Option<Res<PendingEraAnnouncement>>,
    fade: Query<(), With<super::main_menu::GameStartFadein>>,
    night: Res<NightTracker>,
    mut events: MessageWriter<GlobalTextMessageEvent>,
    mut commands: Commands,
) {
    let Some(pending) = pending else {
        return;
    };
    let era = match &*pending {
        PendingEraAnnouncement::DuringRunFadeIn => {
            if fade.is_empty() {
                return;
            }
            Era::Main
        }
        PendingEraAnnouncement::OnEraEnter(era) => era.clone(),
    };
    if let Some(event) = GlobalTextMessageEvent::era_start_announcement(era) {
        events.write(
            event
                .with_sub_text(format!("Day {}", night.display_day()), WHITE)
                .with_sub_panel_width(80.),
        );
    }
    commands.remove_resource::<PendingEraAnnouncement>();
}

pub fn handle_global_text_message_events(
    mut commands: Commands,
    mut events: MessageReader<GlobalTextMessageEvent>,
    asset_server: Res<AssetServer>,
    graphics: Res<Graphics>,
    resolution: Res<ScreenResolution>,
    existing: Query<Entity, With<GlobalTextMessage>>,
) {
    for event in events.read() {
        for entity in existing.iter() {
            commands.entity(entity).despawn();
        }

        let pos = Vec3::new(
            0.,
            resolution.game_height / 2. - TOP_MARGIN,
            Z_DEPTH_GLOBAL_TEXT_MESSAGE,
        );
        let root = commands
            .spawn((
                (Transform::from_translation(pos), Visibility::default()),
                RenderLayers::from_layers(&[3]),
                GlobalTextMessage {
                    timer: Timer::from_seconds(MESSAGE_DISPLAY_SECS, TimerMode::Once),
                },
                Name::new("Global Text Message"),
            ))
            .id();

        let main_panel_size = message_panel_size(event, resolution.game_width);
        let sub_panel_size = event.sub_text.as_ref().map(|sub| {
            Vec2::new(
                subtext_panel_width(sub, resolution.game_width, event.sub_panel_width),
                subtext_panel_height(sub),
            )
        });
        let layout = message_stack_layout(
            main_panel_size.y,
            sub_panel_size.map(|s| s.y).unwrap_or(0.0),
        );

        let main_center = Vec3::new(0., layout.main_center_y, 0.);
        let main_text_z = Vec3::new(0., layout.main_center_y, 1.);

        spawn_panel_sprite(&mut commands, root, main_panel_size, main_center);
        spawn_main_text(
            &mut commands,
            root,
            &asset_server,
            &event.text,
            event.font_style,
            event.color,
            main_text_z,
        );

        let text_half_w = estimated_text_half_width(&event.text, event.font_style.size);
        if let Some(obj) = event.icon_item {
            let icon_x = icon_x_left_of_text(text_half_w);
            let icon = spawn_item_stack_icon(
                &mut commands,
                &graphics,
                &ItemStack::crate_icon_stack(obj),
                &asset_server,
                Vec2::new(icon_x, 0.),
                Vec2::ZERO,
                3,
            );
            commands.entity(icon).insert((
                GlobalTextMessagePart,
                Transform {
                    translation: Vec3::new(icon_x, layout.main_center_y, 2.),
                    scale: Vec3::splat(ICON_SCALE),
                    ..Default::default()
                },
                Name::new("Global Text Message Icon"),
            ));
            commands.entity(icon).insert(ChildOf(root));
        }

        if let (Some(sub_text), Some(sub_size)) = (&event.sub_text, sub_panel_size) {
            let sub_color = event.sub_text_color.unwrap_or(event.color);
            let sub_center = Vec3::new(0., layout.sub_center_y, 0.);
            let sub_text_z = Vec3::new(0., layout.sub_center_y + 2., 1.);

            spawn_panel_sprite(&mut commands, root, sub_size, sub_center);
            spawn_sub_text(
                &mut commands,
                root,
                &asset_server,
                sub_text,
                sub_color,
                sub_text_z,
            );
        }
    }
}

fn fade_message_parts(
    alpha: f32,
    children: &Children,
    texts: &mut Query<&mut TextColor, With<GlobalTextMessagePart>>,
    sprites: &mut Query<&mut Sprite, With<GlobalTextMessagePart>>,
    child_q: &Query<&Children>,
) {
    for child in children.iter() {
        if let Ok(mut color) = texts.get_mut(child) {
            color.0 = color.0.with_alpha(alpha);
        }
        if let Ok(mut sprite) = sprites.get_mut(child) {
            sprite.color = sprite.color.with_alpha(alpha.min(0.75));
        }
        if let Ok(grandchildren) = child_q.get(child) {
            fade_message_parts(alpha, grandchildren, texts, sprites, child_q);
        }
    }
}

pub fn tick_global_text_messages(
    mut commands: Commands,
    time: Res<Time>,
    mut roots: Query<(Entity, &mut GlobalTextMessage, &Children)>,
    mut texts: Query<&mut TextColor, With<GlobalTextMessagePart>>,
    mut sprites: Query<&mut Sprite, With<GlobalTextMessagePart>>,
    child_q: Query<&Children>,
) {
    for (entity, mut message, children) in roots.iter_mut() {
        message.timer.tick(time.delta());

        let fade_start = 0.65;
        if message.timer.fraction() > fade_start {
            let fade_t =
                ((message.timer.fraction() - fade_start) / (1.0 - fade_start)).clamp(0.0, 1.0);
            let alpha = 1.0 - fade_t;
            fade_message_parts(alpha, children, &mut texts, &mut sprites, &child_q);
        }

        if message.timer.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}
