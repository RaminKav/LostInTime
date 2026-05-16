use bevy::{prelude::*, sprite::Anchor};

use crate::{
    assets::SpriteAnchor,
    inventory::{Inventory, ItemStack},
    item::{boss_shrine::BossSummonTracker, WorldObject},
    player::Player,
    GameParam,
};

use super::{damage_numbers::spawn_text, spawn_item_stack_icon, UIElement};

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
                        ItemStack::crate_icon_stack(WorldObject::Coin).copy_with_count(50),
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
            WorldObject::DungeonExit => {
                commands.entity(e).insert(InteractionGuideTrigger {
                    key: Some("F".to_string()),
                    text: Some("Exit".to_string()),
                    activation_distance: 32.,
                    icon_stack: None,
                });
            }
            WorldObject::CombatShrine => {
                commands.entity(e).insert(InteractionGuideTrigger {
                    key: Some("F".to_string()),
                    text: Some("Fight".to_string()),
                    activation_distance: 32.,
                    icon_stack: Some(ItemStack::crate_icon_stack(WorldObject::ChestBlock)),
                });
            }
            WorldObject::WeaponShrine => {
                commands.entity(e).insert(InteractionGuideTrigger {
                    key: Some("F".to_string()),
                    text: Some("Fight".to_string()),
                    activation_distance: 32.,
                    icon_stack: None,
                });
            }
            WorldObject::ArmorShrine => {
                commands.entity(e).insert(InteractionGuideTrigger {
                    key: Some("F".to_string()),
                    text: Some("Fight".to_string()),
                    activation_distance: 32.,
                    icon_stack: None,
                });
            }
            WorldObject::AccessoryShrine => {
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
            WorldObject::BlacksmithMerchant => {
                commands.entity(e).insert(InteractionGuideTrigger {
                    key: Some("F".to_string()),
                    text: Some("Purchase".to_string()),
                    activation_distance: 32.,
                    icon_stack: Some(ItemStack::crate_icon_stack(WorldObject::Coin)),
                });
            }
            WorldObject::ActiveSkillShrine => {
                commands.entity(e).insert(InteractionGuideTrigger {
                    key: Some("F".to_string()),
                    text: Some("Get Skill".to_string()),
                    activation_distance: 32.,
                    icon_stack: None,
                });
            }
            WorldObject::HeirloomShrine => {
                commands.entity(e).insert(InteractionGuideTrigger {
                    key: Some("F".to_string()),
                    text: Some("Talk ".to_string()),
                    activation_distance: 32.,
                    icon_stack: None,
                });
            }
            WorldObject::MicrowaveShrine => {
                commands.entity(e).insert(InteractionGuideTrigger {
                    key: Some("F".to_string()),
                    text: Some("Swap Heirlooms".to_string()),
                    activation_distance: 32.,
                    icon_stack: None,
                });
            }
            WorldObject::ChaosTotem => {
                commands.entity(e).insert(InteractionGuideTrigger {
                    key: Some("F".to_string()),
                    text: Some("Activate".to_string()),
                    activation_distance: 32.,
                    icon_stack: None,
                });
            }
            _ => {}
        }
    }
}

fn resolve_interaction_guide_text(
    guide: &InteractionGuideTrigger,
    world_obj: Option<WorldObject>,
    coins: u32,
    summon_cost: i32,
    boss_summon_count: u32,
    key_count: usize,
) -> Option<String> {
    let base = guide.text.clone();
    match world_obj {
        Some(WorldObject::BossShrine) => {
            if (coins as i32) < summon_cost {
                Some("Not enough coins".to_string())
            } else if boss_summon_count >= 1 {
                Some("Summon... Again?".to_string())
            } else {
                base
            }
        }
        Some(WorldObject::DungeonEntrance) => {
            if key_count < 1 {
                Some("Needs a Key".to_string())
            } else {
                base
            }
        }
        _ => base,
    }
}

pub fn spawn_shrine_interact_key_guide(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    player_query: Query<(Entity, &GlobalTransform), With<Player>>,
    player_inv: Query<&Inventory, With<Player>>,
    summon_tracker: Res<BossSummonTracker>,
    game: GameParam,
    already_exists: Query<Entity, With<InteractGuide>>,
    guides: Query<(
        &GlobalTransform,
        &InteractionGuideTrigger,
        Option<&SpriteAnchor>,
        Option<&WorldObject>,
    )>,
) {
    let (player_e, player_t) = player_query.single();
    let key_count = player_inv
        .single()
        .items
        .get_item_count_in_container(WorldObject::Key);
    let summon_cost = summon_tracker.current_cost();

    if already_exists.iter().count() == 0 {
        for (txfm, guide, anchor_option, world_obj) in guides.iter() {
            let guide_pos =
                txfm.translation().truncate() - anchor_option.unwrap_or(&SpriteAnchor::default()).0;
            if guide_pos.distance(player_t.translation().truncate()) < guide.activation_distance {
                let display_text = resolve_interaction_guide_text(
                    guide,
                    world_obj.copied(),
                    game.get_coins(),
                    summon_cost,
                    summon_tracker.summon_count,
                    key_count,
                );
                let parent_entity = commands
                    .spawn(SpatialBundle::from_transform(Transform::from_translation(
                        Vec3::new(0., 25.5, 1.),
                    )))
                    .insert(InteractGuide)
                    .set_parent(player_e)
                    .insert(Name::new("Interact Guide"))
                    .id();
                let key_entity = if let Some(key) = guide.key.clone() {
                    let x_offset = if display_text.is_some() {
                        f32::round(
                            display_text.as_ref().unwrap().chars().count() as f32 * -4. - 12.,
                        )
                    } else {
                        0.
                    };
                    Some(
                        commands
                            .spawn(SpriteBundle {
                                texture: asset_server.load(format!("textures/{}Key.png", key)),
                                transform: Transform::from_translation(Vec3::new(
                                    x_offset, 0.5, 1.,
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
                if let Some(text) = display_text {
                    let x = if key_entity.is_some() { 6. } else { 0.5 };
                    let text_e = spawn_text(
                        &mut commands,
                        &asset_server,
                        Vec3::new(x, -1., 1.),
                        Color::WHITE,
                        text,
                        if key_entity.is_some() {
                            Anchor::Center
                        } else {
                            Anchor::Center
                        },
                        1.,
                        0,
                    );
                    if let Some(key_e) = key_entity {
                        commands.entity(key_e).set_parent(text_e);
                    }
                    commands.entity(text_e).set_parent(parent_entity);
                };
                if let Some(icon_stack) = guide.icon_stack.clone() {
                    let icon = spawn_item_stack_icon(
                        &mut commands,
                        &game.graphics,
                        &icon_stack,
                        &asset_server,
                        Vec2::ZERO,
                        Vec2::new(0.0, 0.),
                        0,
                    );

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
                        .push_children(&[icon]);
                };
            }
        }
    } else {
        for (txfm, guide, anchor_option, _world_obj) in guides.iter() {
            let guide_pos =
                txfm.translation().truncate() - anchor_option.unwrap_or(&SpriteAnchor::default()).0;
            if guide_pos.distance(player_t.translation().truncate()) < guide.activation_distance {
                return;
            }
        }
        for t in already_exists.iter() {
            commands.entity(t).despawn_recursive();
        }
    }
}
