use bevy::{prelude::*, render::view::RenderLayers, sprite::Anchor};

use crate::assets::Graphics;

use super::{ToolTipUpdateEvent, UIElement};

pub fn handle_spawn_inv_item_tooltip(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    mut updates: EventReader<ToolTipUpdateEvent>,
) {
    for item in updates.iter() {
        let attributes = item.item_stack.attributes.get_tooltips();
        let durability = item.item_stack.attributes.get_durability_tooltip();
        let has_attributes = attributes.len() > 0;
        let size = if has_attributes {
            Vec2::new(80., 80.)
        } else {
            Vec2::new(64., 24.)
        };
        let tooltip = commands
            .spawn((
                SpriteBundle {
                    texture: graphics
                        .ui_image_handles
                        .as_ref()
                        .unwrap()
                        .get(if has_attributes {
                            &UIElement::LargeTooltip
                        } else {
                            &UIElement::Tooltip
                        })
                        .unwrap()
                        .clone(),
                    transform: Transform {
                        translation: Vec3::new(0., if has_attributes { 48. } else { 20. }, 2.),
                        scale: Vec3::new(1., 1., 1.),
                        ..Default::default()
                    },
                    sprite: Sprite {
                        custom_size: Some(size),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                RenderLayers::from_layers(&[3]),
                UIElement::LargeTooltip,
                Name::new("TOOLTIP"),
            ))
            .id();

        let mut tooltip_text: Vec<(String, f32)> = vec![];
        tooltip_text.push((item.item_stack.metadata.name.clone(), 0.));
        // tooltip_text.push(item.item_stack.metadata.desc.clone());
        for (i, a) in attributes.iter().enumerate().clone() {
            let d = if i == 0 { 2. } else { 0. };
            tooltip_text.push((a.to_string(), d));
        }
        if has_attributes {
            tooltip_text.push((durability.clone(), 28.));
        }

        for (i, (text, d)) in tooltip_text.iter().enumerate() {
            let text_pos = if has_attributes {
                Vec3::new(-size.x / 2. + 8., 28. - (i as f32 * 10.) - d, 1.)
            } else {
                Vec3::new(0., 0., 1.)
            };
            let text = commands
                .spawn((
                    Text2dBundle {
                        text: Text::from_section(
                            text,
                            TextStyle {
                                font: asset_server.load("fonts/Kitchen Sink.ttf"),
                                font_size: 8.0,
                                color: Color::Rgba {
                                    red: 75. / 255.,
                                    green: 61. / 255.,
                                    blue: 68. / 255.,
                                    alpha: 1.,
                                },
                            },
                        ),
                        text_anchor: if has_attributes {
                            Anchor::CenterLeft
                        } else {
                            Anchor::Center
                        },
                        transform: Transform {
                            translation: text_pos,
                            scale: Vec3::new(1., 1., 1.),
                            ..Default::default()
                        },
                        ..default()
                    },
                    Name::new("TOOLTIP TEXT"),
                    RenderLayers::from_layers(&[3]),
                ))
                .id();
            commands.entity(tooltip).add_child(text);
        }
        commands.entity(item.parent_slot_entity).add_child(tooltip);
    }
}
