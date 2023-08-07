use bevy::{prelude::*, render::view::RenderLayers, sprite::Anchor};

use crate::{
    assets::Graphics,
    colors::{GOLD, LIGHT_GREY},
};

use super::{
    InventoryUI, InventoryUIState, ToolTipUpdateEvent, CHEST_INVENTORY_UI_SIZE, INVENTORY_UI_SIZE,
};

pub fn handle_spawn_inv_item_tooltip(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    mut updates: EventReader<ToolTipUpdateEvent>,
    inv: Query<Entity, With<InventoryUI>>,
    cur_inv_state: Res<State<InventoryUIState>>,
) {
    for item in updates.iter() {
        let inv = inv.single();
        let parent_inv_size = match cur_inv_state.0 {
            InventoryUIState::Open => INVENTORY_UI_SIZE,
            InventoryUIState::Chest => CHEST_INVENTORY_UI_SIZE,
            _ => unreachable!(),
        };
        let attributes = item.item_stack.attributes.get_tooltips();
        let durability = item.item_stack.attributes.get_durability_tooltip();
        let has_attributes = attributes.len() > 0;
        let size = Vec2::new(93., 120.5);
        let tooltip = commands
            .spawn((
                SpriteBundle {
                    texture: graphics
                        .ui_image_handles
                        .as_ref()
                        .unwrap()
                        .get(&item.item_stack.rarity.get_tooltip_ui_element())
                        .unwrap()
                        .clone(),
                    transform: Transform {
                        translation: Vec3::new(-(parent_inv_size.x + size.x + 2.) / 2., 0., 2.),
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
                item.item_stack.rarity.get_tooltip_ui_element(),
                Name::new("TOOLTIP"),
            ))
            .id();

        let mut tooltip_text: Vec<(String, f32)> = vec![];
        tooltip_text.push((item.item_stack.metadata.name.clone(), 0.));
        // tooltip_text.push(item.item_stack.metadata.desc.clone());
        for (_i, a) in attributes.iter().enumerate().clone() {
            tooltip_text.push((a.to_string(), 0.));
        }
        if has_attributes {
            tooltip_text.push((
                durability.clone(),
                size.y - (tooltip_text.len() + 1) as f32 * 10. - 14.,
            ));
        }

        for (i, (text, d)) in tooltip_text.iter().enumerate() {
            let text_pos = if i == 0 {
                Vec3::new(
                    -(f32::ceil((text.chars().count() * 6 - 1) as f32 / 2.)) + 0.5,
                    size.y / 2. - 12.,
                    1.,
                )
            } else {
                Vec3::new(
                    -size.x / 2. + 8.,
                    size.y / 2. - 12. - (i as f32 * 10.) - d - 2.,
                    1.,
                )
            };
            let text = commands
                .spawn((
                    Text2dBundle {
                        text: Text::from_section(
                            text,
                            TextStyle {
                                font: asset_server.load("fonts/Kitchen Sink.ttf"),
                                font_size: 8.0,
                                color: if i == 0 {
                                    item.item_stack.rarity.get_color()
                                } else if i > 1 && i == tooltip_text.len() - 1 {
                                    LIGHT_GREY
                                } else if i > 2 {
                                    GOLD
                                } else {
                                    LIGHT_GREY
                                },
                            },
                        ),
                        text_anchor: Anchor::CenterLeft,
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
        commands.entity(inv).add_child(tooltip);
    }
}
