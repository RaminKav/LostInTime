use bevy::prelude::*;
use bevy::render::view::RenderLayers;
use bevy::sprite::Anchor;

use crate::{assets::Graphics, inventory::ItemStack, item::WorldObject, ScreenResolution};

use super::{
    game_fonts::{FontStyle, GLOBAL_MESSAGE},
    spawn_item_stack_icon,
};

const MESSAGE_DISPLAY_SECS: f32 = 2.5;
const TOP_MARGIN: f32 = 60.0;

const CARD_BASE_W: f32 = 16.;
const ICON_SCALE: f32 = 2.0;
const ICON_DISPLAY_W: f32 = CARD_BASE_W * ICON_SCALE;
const ICON_TEXT_GAP: f32 = 10.0;
/// Half-width estimate for Alagard at [`GLOBAL_MESSAGE`] size (tune spacing vs. overlap).
const ESTIMATED_HALF_WIDTH_FACTOR: f32 = 0.24;

#[derive(Clone)]
pub struct GlobalTextMessageEvent {
    pub text: String,
    pub color: Color,
    pub font_style: FontStyle,
    pub icon_item: Option<WorldObject>,
}

impl GlobalTextMessageEvent {
    pub fn new(text: impl Into<String>, color: Color) -> Self {
        Self {
            text: text.into(),
            color,
            font_style: GLOBAL_MESSAGE,
            icon_item: None,
        }
    }

    pub fn with_icon(mut self, item: WorldObject) -> Self {
        self.icon_item = Some(item);
        self
    }
}

#[derive(Component)]
pub struct GlobalTextMessage {
    timer: Timer,
}

#[derive(Component)]
pub(crate) struct GlobalTextMessagePart;

fn estimated_text_half_width(text: &str, font_size: f32) -> f32 {
    text.chars().count() as f32 * font_size * ESTIMATED_HALF_WIDTH_FACTOR
}

fn icon_x_left_of_text(text_half_width: f32) -> f32 {
    -text_half_width - ICON_TEXT_GAP - ICON_DISPLAY_W * 0.5
}

pub fn handle_global_text_message_events(
    mut commands: Commands,
    mut events: EventReader<GlobalTextMessageEvent>,
    asset_server: Res<AssetServer>,
    graphics: Res<Graphics>,
    resolution: Res<ScreenResolution>,
    existing: Query<Entity, With<GlobalTextMessage>>,
) {
    for event in events.iter() {
        for entity in existing.iter() {
            commands.entity(entity).despawn_recursive();
        }

        let pos = Vec3::new(0., resolution.game_height / 2. - TOP_MARGIN, 25.);
        let root = commands
            .spawn((
                SpatialBundle {
                    transform: Transform::from_translation(pos),
                    ..Default::default()
                },
                RenderLayers::from_layers(&[3]),
                GlobalTextMessage {
                    timer: Timer::from_seconds(MESSAGE_DISPLAY_SECS, TimerMode::Once),
                },
                Name::new("Global Text Message"),
            ))
            .id();

        let text_half_w = estimated_text_half_width(&event.text, event.font_style.size);

        commands
            .spawn((
                Text2dBundle {
                    text: Text::from_section(
                        event.text.clone(),
                        event.font_style.text_style(&asset_server, event.color),
                    )
                    .with_alignment(TextAlignment::Center),
                    text_anchor: Anchor::Center,
                    transform: Transform::from_translation(Vec3::ZERO),
                    ..Default::default()
                },
                RenderLayers::from_layers(&[3]),
                GlobalTextMessagePart,
                Name::new("Global Text Message Text"),
            ))
            .set_parent(root);

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
                    translation: Vec3::new(icon_x, 0., 26.),
                    scale: Vec3::splat(ICON_SCALE),
                    ..Default::default()
                },
                Name::new("Global Text Message Icon"),
            ));
            commands.entity(icon).set_parent(root);
        }
    }
}

fn fade_message_parts(
    alpha: f32,
    children: &Children,
    texts: &mut Query<&mut Text, With<GlobalTextMessagePart>>,
    sprites: &mut Query<&mut TextureAtlasSprite, With<GlobalTextMessagePart>>,
    child_q: &Query<&Children>,
) {
    for child in children.iter() {
        if let Ok(mut text) = texts.get_mut(*child) {
            for section in text.sections.iter_mut() {
                section.style.color.set_a(alpha);
            }
        }
        if let Ok(mut sprite) = sprites.get_mut(*child) {
            sprite.color.set_a(alpha);
        }
        if let Ok(grandchildren) = child_q.get(*child) {
            fade_message_parts(alpha, grandchildren, texts, sprites, child_q);
        }
    }
}

pub fn tick_global_text_messages(
    mut commands: Commands,
    time: Res<Time>,
    mut roots: Query<(Entity, &mut GlobalTextMessage, &Children)>,
    mut texts: Query<&mut Text, With<GlobalTextMessagePart>>,
    mut sprites: Query<&mut TextureAtlasSprite, With<GlobalTextMessagePart>>,
    child_q: Query<&Children>,
) {
    for (entity, mut message, children) in roots.iter_mut() {
        message.timer.tick(time.delta());

        let fade_start = 0.65;
        if message.timer.percent() > fade_start {
            let fade_t =
                ((message.timer.percent() - fade_start) / (1.0 - fade_start)).clamp(0.0, 1.0);
            let alpha = 1.0 - fade_t;
            fade_message_parts(alpha, children, &mut texts, &mut sprites, &child_q);
        }

        if message.timer.finished() {
            commands.entity(entity).despawn_recursive();
        }
    }
}
