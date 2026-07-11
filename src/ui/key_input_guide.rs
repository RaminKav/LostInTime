use bevy::{prelude::*, render::view::RenderLayers, sprite::Anchor};

use crate::{
    assets::SpriteAnchor,
    colors::{RED, WHITE},
    ecs_helpers::{safe_push_children, safe_set_parent, SafeHierarchyExt},
    gamepad_bindings::{binding_labels_dirty, format_binding_label, BindingLabel, GamepadMappings},
    inventory::{Inventory, ItemStack},
    item::{boss_shrine::BossSummonTracker, shrine_repair::ShrineRepairCosts, WorldObject},
    keybinds::InputMappings,
    player::Player,
    GameParam,
};

use super::{
    damage_numbers::spawn_text,
    game_fonts::{self as gf, FLOATING_TEXT},
    spawn_item_stack_icon, UIElement, UI_SLOT_SIZE,
};

/// Interact-guide key badge — larger than the default HUD keybind badge, with darker fill.
const INTERACT_GUIDE_KEY_BADGE_SIZE: Vec2 = Vec2::new(26., 18.);
const INTERACT_GUIDE_KEY_BADGE_COLOR: Color = Color::rgba(18. / 255., 16. / 255., 16. / 255., 0.88);

fn spawn_interact_guide_keybind_badge(
    commands: &mut Commands,
    asset_server: &AssetServer,
    label: impl Into<String>,
    transform: Transform,
    parent: Entity,
) -> (Entity, Entity) {
    let key_bg = commands
        .spawn(SpriteBundle {
            sprite: Sprite {
                color: INTERACT_GUIDE_KEY_BADGE_COLOR,
                custom_size: Some(INTERACT_GUIDE_KEY_BADGE_SIZE),
                ..default()
            },
            transform,
            ..default()
        })
        .insert(RenderLayers::from_layers(&[INTERACT_GUIDE_RENDER_LAYER]))
        .set_parent(parent)
        .id();

    let key_text = commands
        .spawn(Text2dBundle {
            text: Text::from_section(label.into(), gf::DISPLAY.text_style(&asset_server, WHITE))
                .with_alignment(TextAlignment::Center),
            text_anchor: Anchor::Center,
            transform: Transform {
                translation: Vec3::new(0., -1., 1.),
                scale: gf::DISPLAY.transform_scale(),
                ..default()
            },
            ..default()
        })
        .insert(RenderLayers::from_layers(&[INTERACT_GUIDE_RENDER_LAYER]))
        .set_parent(key_bg)
        .id();

    (key_bg, key_text)
}

const INTERACT_GUIDE_RENDER_LAYER: u8 = 0;

#[derive(Component)]
pub struct InteractGuide;

#[derive(Component)]
pub struct InteractGuideKeyBackground;

#[derive(Component)]
pub struct InteractGuideKeybindText;

#[derive(Component)]
pub struct InteractionGuideTrigger {
    pub text: Option<String>,
    pub activation_distance: f32,
    pub icon_stack: Option<ItemStack>,
}

pub fn add_guide_to_unique_objs(
    mut commands: Commands,
    new_objs: Query<(Entity, &WorldObject, &Transform, Option<&SpriteAnchor>), Added<WorldObject>>,
    cache: Res<crate::world::generation::WorldObjectCache>,
) {
    for (e, obj, transform, anchor) in new_objs.iter() {
        if crate::item::shrine_repair::can_be_broken(*obj) {
            let world_pos =
                transform.translation.truncate() - anchor.map(|a| a.0).unwrap_or(Vec2::ZERO);
            let tile_pos = crate::world::world_helpers::world_pos_to_tile_pos(world_pos);
            if let Some(costs) = cache.broken_shrine_costs.get(&tile_pos) {
                if !costs.is_empty() {
                    let icon = costs.first().map(|(item, amount)| {
                        ItemStack::crate_icon_stack(*item).copy_with_count(*amount as usize)
                    });
                    commands.entity(e).insert(InteractionGuideTrigger {
                        text: Some("Repair".to_string()),
                        activation_distance: 32.,
                        icon_stack: icon,
                    });
                    continue;
                }
            }
        }
        match obj {
            WorldObject::BossShrine => {
                commands.entity(e).insert(InteractionGuideTrigger {
                    text: Some("Summon".to_string()),
                    activation_distance: 32.,
                    icon_stack: Some(
                        ItemStack::crate_icon_stack(WorldObject::Coin).copy_with_count(50),
                    ),
                });
            }
            WorldObject::DungeonEntrance => {
                commands.entity(e).insert(InteractionGuideTrigger {
                    text: Some("Enter".to_string()),
                    activation_distance: 32.,
                    icon_stack: Some(ItemStack::crate_icon_stack(WorldObject::Key)),
                });
            }
            WorldObject::DungeonExit => {
                commands.entity(e).insert(InteractionGuideTrigger {
                    text: Some("Exit".to_string()),
                    activation_distance: 32.,
                    icon_stack: None,
                });
            }
            WorldObject::CombatShrine => {
                commands.entity(e).insert(InteractionGuideTrigger {
                    text: Some("Activate Shrine".to_string()),
                    activation_distance: 32.,
                    icon_stack: Some(ItemStack::crate_icon_stack(WorldObject::ChestBlock)),
                });
            }
            WorldObject::WeaponShrine => {
                commands.entity(e).insert(InteractionGuideTrigger {
                    text: Some("Activate Shrine".to_string()),
                    activation_distance: 32.,
                    icon_stack: None,
                });
            }
            WorldObject::ArmorShrine => {
                commands.entity(e).insert(InteractionGuideTrigger {
                    text: Some("Activate Shrine".to_string()),
                    activation_distance: 32.,
                    icon_stack: None,
                });
            }
            WorldObject::AccessoryShrine => {
                commands.entity(e).insert(InteractionGuideTrigger {
                    text: Some("Activate Shrine".to_string()),
                    activation_distance: 32.,
                    icon_stack: None,
                });
            }
            WorldObject::GambleShrine => {
                commands.entity(e).insert(InteractionGuideTrigger {
                    text: Some("Activate Shrine".to_string()),
                    activation_distance: 32.,
                    icon_stack: Some(ItemStack::crate_icon_stack(WorldObject::Coin)),
                });
            }
            WorldObject::BlacksmithMerchant => {
                commands.entity(e).insert(InteractionGuideTrigger {
                    text: Some("Activate Shrine".to_string()),
                    activation_distance: 32.,
                    icon_stack: Some(ItemStack::crate_icon_stack(WorldObject::Coin)),
                });
            }
            WorldObject::ActiveSkillShrine => {
                commands.entity(e).insert(InteractionGuideTrigger {
                    text: Some("Activate Shrine".to_string()),
                    activation_distance: 32.,
                    icon_stack: None,
                });
            }
            WorldObject::HeirloomShrine => {
                commands.entity(e).insert(InteractionGuideTrigger {
                    text: Some("Activate Shrine".to_string()),
                    activation_distance: 32.,
                    icon_stack: None,
                });
            }
            WorldObject::MicrowaveShrine => {
                commands.entity(e).insert(InteractionGuideTrigger {
                    text: Some("Activate Shrine".to_string()),
                    activation_distance: 32.,
                    icon_stack: None,
                });
            }
            WorldObject::ChaosTotem => {
                commands.entity(e).insert(InteractionGuideTrigger {
                    text: Some("Activate Shrine".to_string()),
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
    keybinds: Res<InputMappings>,
    gamepad_mappings: Res<GamepadMappings>,
    gamepads: Res<Gamepads>,
    player_query: Query<(Entity, &GlobalTransform), With<Player>>,
    player_inv: Query<&Inventory, With<Player>>,
    inv_changed: Query<Entity, (With<Player>, With<Inventory>, Changed<Inventory>)>,
    summon_tracker: Res<BossSummonTracker>,
    game: GameParam,
    already_exists: Query<Entity, With<InteractGuide>>,
    guides: Query<(
        &GlobalTransform,
        &InteractionGuideTrigger,
        Option<&SpriteAnchor>,
        Option<&WorldObject>,
        Option<&ShrineRepairCosts>,
    )>,
) {
    let interact_key = format_binding_label(
        BindingLabel::Interact,
        &keybinds,
        &gamepad_mappings,
        &gamepads,
    );
    let (player_e, player_t) = player_query.single();
    let inv = player_inv.single();
    let key_count = inv.items.get_item_count_in_container(WorldObject::Key);
    let summon_cost = summon_tracker.current_cost();
    let inventory_changed = !inv_changed.is_empty();

    if already_exists.iter().count() == 0 {
        for (txfm, guide, anchor_option, world_obj, repair_costs) in guides.iter() {
            let guide_pos =
                txfm.translation().truncate() - anchor_option.unwrap_or(&SpriteAnchor::default()).0;
            if guide_pos.distance(player_t.translation().truncate()) < guide.activation_distance {
                let interact_label = interact_key.clone();
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
                        Vec3::new(0., 57.5, 1.),
                    )))
                    .insert(InteractGuide)
                    .insert(Name::new("Interact Guide"))
                    .safe_set_parent(player_e)
                    .id();

                match display_text {
                    Some(text) => {
                        let char_count = text.chars().count() as f32;
                        let text_e = spawn_text(
                            &mut commands,
                            &asset_server,
                            Vec3::new(6., -1., 1.),
                            Color::WHITE,
                            text,
                            Anchor::Center,
                            FLOATING_TEXT,
                            INTERACT_GUIDE_RENDER_LAYER,
                            None,
                        );
                        safe_set_parent(&mut commands, text_e, parent_entity);

                        // Key badge sits left of the label.
                        let key_x_offset = f32::round(
                            char_count * -4. - 14. - INTERACT_GUIDE_KEY_BADGE_SIZE.x * 0.5,
                        );
                        let (key_bg, key_text) = spawn_interact_guide_keybind_badge(
                            &mut commands,
                            &asset_server,
                            interact_label.clone(),
                            Transform::from_translation(Vec3::new(key_x_offset, 0.5, 1.)),
                            text_e,
                        );
                        commands.entity(key_bg).insert(InteractGuideKeyBackground);
                        commands.entity(key_text).insert(InteractGuideKeybindText);
                    }
                    None => {
                        let (key_bg, key_text) = spawn_interact_guide_keybind_badge(
                            &mut commands,
                            &asset_server,
                            interact_label,
                            Transform::from_translation(Vec3::new(0., 0.5, 1.)),
                            parent_entity,
                        );
                        commands.entity(key_bg).insert(InteractGuideKeyBackground);
                        commands.entity(key_text).insert(InteractGuideKeybindText);
                    }
                }

                if let Some(repair) = repair_costs {
                    spawn_repair_cost_icons(
                        &mut commands,
                        &game,
                        &asset_server,
                        inv,
                        parent_entity,
                        &repair.materials,
                    );
                } else if let Some(mut icon_stack) = guide.icon_stack.clone() {
                    // Boss shrine summon cost scales with each summon, so reflect the
                    // current cost on the coin icon instead of the static initial value.
                    if world_obj.copied() == Some(WorldObject::BossShrine) {
                        icon_stack = icon_stack.copy_with_count(summon_cost.max(0) as usize);
                    }
                    let icon = spawn_item_stack_icon(
                        &mut commands,
                        &game.graphics,
                        &icon_stack,
                        &asset_server,
                        Vec2::ZERO,
                        Vec2::new(0.0, 0.),
                        0,
                    );

                    let slot_entity = commands
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
                        .insert(RenderLayers::from_layers(&[INTERACT_GUIDE_RENDER_LAYER]))
                        .safe_set_parent(parent_entity)
                        .id();
                    safe_push_children(&mut commands, slot_entity, &[icon]);
                }
            }
        }
    } else {
        for (txfm, guide, anchor_option, _world_obj, repair_costs) in guides.iter() {
            let guide_pos =
                txfm.translation().truncate() - anchor_option.unwrap_or(&SpriteAnchor::default()).0;
            if guide_pos.distance(player_t.translation().truncate()) < guide.activation_distance {
                // Rebuild when inventory changes so repair owned/cost counts stay live.
                if inventory_changed && repair_costs.is_some() {
                    for t in already_exists.iter() {
                        commands.entity(t).despawn_recursive();
                    }
                }
                return;
            }
        }
        for t in already_exists.iter() {
            commands.entity(t).despawn_recursive();
        }
    }
}

const REPAIR_ICON_SPACING: f32 = 28.;

fn spawn_repair_cost_icons(
    commands: &mut Commands,
    game: &GameParam,
    asset_server: &AssetServer,
    inv: &Inventory,
    parent_entity: Entity,
    materials: &[(WorldObject, u32)],
) {
    if materials.is_empty() {
        return;
    }
    let count = materials.len() as f32;
    let start_x = -((count - 1.) * REPAIR_ICON_SPACING) * 0.5;

    for (i, (item, cost)) in materials.iter().enumerate() {
        let owned = inv.items.get_item_count_in_container(*item);
        let has_enough = owned >= *cost as usize;
        let alpha = if has_enough { 1.0 } else { 0.7 };
        let x = start_x + i as f32 * REPAIR_ICON_SPACING;

        let icon_stack = ItemStack::crate_icon_stack(*item).copy_with_count(1);
        let icon = spawn_item_stack_icon(
            commands,
            &game.graphics,
            &icon_stack,
            asset_server,
            Vec2::ZERO,
            Vec2::ZERO,
            INTERACT_GUIDE_RENDER_LAYER,
        );
        // Grey out when the player cannot afford this material.
        commands.add(move |world: &mut World| {
            if let Some(mut entity) = world.get_entity_mut(icon) {
                if let Some(mut sprite) = entity.get_mut::<TextureAtlasSprite>() {
                    sprite.color.set_a(alpha);
                }
            }
        });

        let count_color = if has_enough { WHITE } else { RED };
        let count_text = commands
            .spawn(Text2dBundle {
                text: Text::from_section(
                    format!("{owned}/{cost}"),
                    gf::TITLE.text_style(asset_server, count_color),
                )
                .with_alignment(TextAlignment::Center),
                transform: Transform {
                    translation: Vec3::new(0., -18., 3.),
                    scale: gf::TITLE.transform_scale(),
                    ..default()
                },
                ..default()
            })
            .insert(RenderLayers::from_layers(&[INTERACT_GUIDE_RENDER_LAYER]))
            .insert(Name::new("REPAIR COST TEXT"))
            .id();

        let slot_entity = commands
            .spawn(SpriteBundle {
                texture: game
                    .graphics
                    .get_ui_element_texture(UIElement::InventorySlot),
                transform: Transform::from_translation(Vec3::new(x, 28., 1.)),
                sprite: Sprite {
                    custom_size: Some(UI_SLOT_SIZE),
                    color: if has_enough {
                        Color::WHITE
                    } else {
                        Color::rgba(1., 1., 1., 0.7)
                    },
                    ..Default::default()
                },
                ..Default::default()
            })
            .insert(RenderLayers::from_layers(&[INTERACT_GUIDE_RENDER_LAYER]))
            .safe_set_parent(parent_entity)
            .id();
        safe_push_children(commands, slot_entity, &[icon, count_text]);
    }
}

pub fn update_interact_guide_keybind_text(
    keybinds: Res<InputMappings>,
    gamepad_mappings: Res<GamepadMappings>,
    gamepads: Res<Gamepads>,
    mut texts: Query<&mut Text, With<InteractGuideKeybindText>>,
    mut last_gamepad_connected: Local<Option<bool>>,
) {
    if !binding_labels_dirty(
        keybinds.is_changed(),
        gamepad_mappings.is_changed(),
        &gamepads,
        &mut last_gamepad_connected,
    ) {
        return;
    }

    let label = format_binding_label(
        BindingLabel::Interact,
        &keybinds,
        &gamepad_mappings,
        &gamepads,
    );
    for mut text in texts.iter_mut() {
        text.sections[0].value = label.clone();
    }
}
