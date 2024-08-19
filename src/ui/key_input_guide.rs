use bevy::{prelude::*, sprite::Anchor};

use crate::{
    assets::SpriteAnchor, inventory::ItemStack, item::WorldObject, player::Player, GameParam,
};

use super::{damage_numbers::spawn_text, spawn_item_stack_icon, UIElement, UI_SLOT_SIZE};

#[derive(Component)]
pub struct InteractGuide;

#[derive(Component)]
pub struct InteractionGuideTrigger {
    pub key: Option<String>,
    pub text: Option<String>,
    pub activation_distance: f32,
    pub icon_stack: Option<ItemStack>,
}

pub fn add_guide_to_unique_objs(
    mut commands: Commands,
    new_objs: Query<(Entity, &WorldObject), Added<WorldObject>>,
) {
    for (e, obj) in new_objs.iter() {
        match obj {
            WorldObject::BossShrine => {
                commands.entity(e).insert(InteractionGuideTrigger {
                    key: Some("F".to_string()),
                    text: Some("Summon".to_string()),
                    activation_distance: 32.,
                    icon_stack: Some(
                        ItemStack::crate_icon_stack(WorldObject::TimeFragment).copy_with_count(10),
                    ),
                });
            }
            WorldObject::DungeonEntrance => {
                commands.entity(e).insert(InteractionGuideTrigger {
                    key: Some("F".to_string()),
                    text: Some("Enter".to_string()),
                    activation_distance: 32.,
                    icon_stack: Some(ItemStack::crate_icon_stack(WorldObject::Key)),
                });
            }
            WorldObject::CombatShrine => {
                commands.entity(e).insert(InteractionGuideTrigger {
                    key: Some("F".to_string()),
                    text: Some("Fight".to_string()),
                    activation_distance: 32.,
                    icon_stack: None,
                });
            }
            WorldObject::GambleShrine => {
                commands.entity(e).insert(InteractionGuideTrigger {
                    key: Some("F".to_string()),
                    text: Some("Pay Offering".to_string()),
                    activation_distance: 32.,
                    icon_stack: Some(ItemStack::crate_icon_stack(WorldObject::TimeFragment)),
                });
            }
            _ => {}
        }
    }
}

pub fn spawn_shrine_interact_key_guide(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    player_query: Query<(Entity, &GlobalTransform), With<Player>>,
    game: GameParam,
    already_exists: Query<Entity, With<InteractGuide>>,
    guides: Query<(&GlobalTransform, &InteractionGuideTrigger, &SpriteAnchor)>,
) {
    let (player_e, player_t) = player_query.single();

    if already_exists.iter().count() == 0 {
        for (txfm, guide, anchor) in guides.iter() {
            let guide_pos = txfm.translation().truncate() - anchor.0;
            if guide_pos.distance(player_t.translation().truncate()) < guide.activation_distance {
                let parent_entity = commands
                    .spawn(SpatialBundle::from_transform(Transform::from_translation(
                        Vec3::new(0., 25.5, 1.),
                    )))
                    .insert(InteractGuide)
                    .set_parent(player_e)
                    .insert(Name::new("Interact Guide"))
                    .id();
                let key_entity = if let Some(key) = guide.key.clone() {
                    let x_offset = if guide.text.is_some() { 10. } else { 0. };
                    Some(
                        commands
                            .spawn(SpriteBundle {
                                texture: asset_server.load(format!("textures/{}Key.png", key)),
                                transform: Transform::from_translation(Vec3::new(
                                    -29. + x_offset,
                                    0.5,
                                    1.,
                                )),
                                sprite: Sprite {
                                    custom_size: Some(Vec2::new(10., 10.)),
                                    ..Default::default()
                                },
                                ..Default::default()
                            })
                            .set_parent(parent_entity)
                            .id(),
                    )
                } else {
                    None
                };
                let text_entity = if let Some(text) = guide.text.clone() {
                    let x = if key_entity.is_some() { -10.5 } else { 0. };
                    let text_e = spawn_text(
                        &mut commands,
                        &asset_server,
                        Vec3::new(x, -1., 1.),
                        Color::WHITE,
                        text,
                        if key_entity.is_some() {
                            Anchor::CenterLeft
                        } else {
                            Anchor::Center
                        },
                        1.,
                        0,
                    );
                    commands.entity(text_e).set_parent(parent_entity);
                    Some(text_e)
                } else {
                    None
                };
                let item_icon = if let Some(icon_stack) = guide.icon_stack.clone() {
                    let icon = spawn_item_stack_icon(
                        &mut commands,
                        &game.graphics,
                        &icon_stack,
                        &asset_server,
                        Vec2::ZERO,
                        Vec2::new(0.0, 0.),
                        0,
                    );
                    Some(
                        commands
                            .spawn(SpriteBundle {
                                texture: game
                                    .graphics
                                    .get_ui_element_texture(UIElement::ScreenIconSlot),
                                transform: Transform::from_translation(Vec3::new(0., 18.5, 1.)),
                                sprite: Sprite {
                                    custom_size: Some(Vec2::new(16., 16.)),
                                    ..Default::default()
                                },
                                ..Default::default()
                            })
                            .set_parent(parent_entity)
                            .push_children(&[icon])
                            .id(),
                    )
                } else {
                    None
                };
            }
        }
    } else {
        for (txfm, guide, anchor) in guides.iter() {
            let guide_pos = txfm.translation().truncate() - anchor.0;
            if guide_pos.distance(player_t.translation().truncate()) < guide.activation_distance {
                return;
            }
        }
        for t in already_exists.iter() {
            commands.entity(t).despawn_recursive();
        }
    }
}
