use bevy::{prelude::*, render::view::RenderLayers, sprite::Anchor};

use crate::{
    assets::{Graphics, SpriteAnchor},
    ecs_helpers::{safe_push_children, safe_set_parent, SafeHierarchyExt},
    inventory::{Inventory, ItemStack},
    item::{boss_shrine::BossSummonTracker, WorldObject},
    keybinds::InputMappings,
    player::Player,
    GameParam,
};

use super::{
    damage_numbers::spawn_text,
    game_fonts::FLOATING_TEXT,
    spawn_item_stack_icon,
    ui_helpers::{get_key_size_and_element, spawn_keybind_badge},
    UIElement,
};

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
    new_objs: Query<(Entity, &WorldObject), Added<WorldObject>>,
) {
    for (e, obj) in new_objs.iter() {
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
                    text: Some("Fight".to_string()),
                    activation_distance: 32.,
                    icon_stack: Some(ItemStack::crate_icon_stack(WorldObject::ChestBlock)),
                });
            }
            WorldObject::WeaponShrine => {
                commands.entity(e).insert(InteractionGuideTrigger {
                    text: Some("Fight".to_string()),
                    activation_distance: 32.,
                    icon_stack: None,
                });
            }
            WorldObject::ArmorShrine => {
                commands.entity(e).insert(InteractionGuideTrigger {
                    text: Some("Fight".to_string()),
                    activation_distance: 32.,
                    icon_stack: None,
                });
            }
            WorldObject::AccessoryShrine => {
                commands.entity(e).insert(InteractionGuideTrigger {
                    text: Some("Fight".to_string()),
                    activation_distance: 32.,
                    icon_stack: None,
                });
            }
            WorldObject::GambleShrine => {
                commands.entity(e).insert(InteractionGuideTrigger {
                    text: Some("Pay Offering".to_string()),
                    activation_distance: 32.,
                    icon_stack: Some(ItemStack::crate_icon_stack(WorldObject::TimeFragment)),
                });
            }
            WorldObject::BlacksmithMerchant => {
                commands.entity(e).insert(InteractionGuideTrigger {
                    text: Some("Purchase".to_string()),
                    activation_distance: 32.,
                    icon_stack: Some(ItemStack::crate_icon_stack(WorldObject::Coin)),
                });
            }
            WorldObject::ActiveSkillShrine => {
                commands.entity(e).insert(InteractionGuideTrigger {
                    text: Some("Get Skill".to_string()),
                    activation_distance: 32.,
                    icon_stack: None,
                });
            }
            WorldObject::HeirloomShrine => {
                commands.entity(e).insert(InteractionGuideTrigger {
                    text: Some("Talk ".to_string()),
                    activation_distance: 32.,
                    icon_stack: None,
                });
            }
            WorldObject::MicrowaveShrine => {
                commands.entity(e).insert(InteractionGuideTrigger {
                    text: Some("Swap Heirlooms".to_string()),
                    activation_distance: 32.,
                    icon_stack: None,
                });
            }
            WorldObject::ChaosTotem => {
                commands.entity(e).insert(InteractionGuideTrigger {
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
    keybinds: Res<InputMappings>,
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
    let interact_key = keybinds.get_interact_key();
    let (_, key_width) = get_key_size_and_element(interact_key);
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
                        );
                        safe_set_parent(&mut commands, text_e, parent_entity);

                        // Key cap sits left of the label (same spacing as the old F-key sprite).
                        // Wider caps shift further left so they do not overlap the text.
                        let key_x_offset =
                            f32::round(char_count * -4. - 12. - (key_width - 10.) / 2.);
                        let (key_bg, key_text) = spawn_keybind_badge(
                            &mut commands,
                            &game.graphics,
                            &asset_server,
                            interact_key,
                            text_e,
                            Vec3::new(key_x_offset, 0.5, 1.),
                            Vec3::new(0., 1., 1.),
                            INTERACT_GUIDE_RENDER_LAYER,
                        );
                        commands.entity(key_bg).insert(InteractGuideKeyBackground);
                        commands.entity(key_text).insert(InteractGuideKeybindText);
                    }
                    None => {
                        let (key_bg, key_text) = spawn_keybind_badge(
                            &mut commands,
                            &game.graphics,
                            &asset_server,
                            interact_key,
                            parent_entity,
                            Vec3::new(0., 0.5, 1.),
                            Vec3::new(0., 1., 1.),
                            INTERACT_GUIDE_RENDER_LAYER,
                        );
                        commands.entity(key_bg).insert(InteractGuideKeyBackground);
                        commands.entity(key_text).insert(InteractGuideKeybindText);
                    }
                }
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

pub fn update_interact_guide_keybind_text(
    keybinds: Res<InputMappings>,
    mut texts: Query<&mut Text, With<InteractGuideKeybindText>>,
    mut key_backgrounds: Query<(&mut Handle<Image>, &mut Sprite), With<InteractGuideKeyBackground>>,
    graphics: Res<Graphics>,
) {
    if !keybinds.is_changed() {
        return;
    }

    let interact_key = keybinds.get_interact_key();
    for mut text in texts.iter_mut() {
        text.sections[0].value = crate::keybinds::get_key_display_name(interact_key);
    }

    for (mut texture, mut sprite) in key_backgrounds.iter_mut() {
        let (key_element, key_width) = get_key_size_and_element(interact_key);
        *texture = graphics.get_ui_element_texture(key_element);
        sprite.custom_size = Some(Vec2::new(key_width, 10.));
    }
}
