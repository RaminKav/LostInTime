use bevy::{prelude::*, render::view::RenderLayers, sprite::Anchor};

use crate::{
    assets::{asset_helpers::spawn_sprite, Graphics},
    attributes::{
        Attack, AttributeQuality, AttributeValue, BonusDamage, CritChance, CritDamage, Defence,
        Dodge, Healing, HealthRegen, ItemAttributes, ItemRarity, Lifesteal, LootRateBonus,
        MaxHealth, RawItemBaseAttributes, RawItemBonusAttributes, Speed, Thorns, XpRateBonus,
    },
    colors::{BLACK, DARK_GREEN, GREY, LIGHT_GREEN, LIGHT_GREY, LIGHT_RED},
    inventory::ItemStack,
    item::{item_actions::ItemActions, EquipmentType, Recipes, WorldObject},
    juice::bounce::BounceOnHit,
    player::{stats::StatType, Player},
    proto::proto_param::ProtoParam,
    ui::{spawn_item_stack_icon, TOOLTIP_UI_SIZE},
};

use super::{
    EssenceUI, InventoryUI, UIElement, UIState, CHEST_INVENTORY_UI_SIZE,
    CRAFTING_INVENTORY_UI_SIZE, ESSENCE_UI_SIZE, FURNACE_INVENTORY_UI_SIZE, INVENTORY_UI_SIZE,
};
#[derive(Component)]
pub struct PlayerStatsTooltip;
#[derive(Component)]
pub struct ItemOrRecipeTooltip;

#[derive(Component)]
pub struct RecipeIngredientTooltipIcon;

#[derive(Resource, Clone)]
pub struct TooltipsManager {
    pub timer: Timer,
}

#[derive(Debug, Clone)]

pub struct ToolTipUpdateEvent {
    pub item_stack: ItemStack,
    pub is_recipe: bool,
    pub show_range: bool,
}

#[derive(Debug, Clone, Default)]

pub struct ShowInvPlayerStatsEvent {
    pub stat: Option<StatType>,
}

pub struct TooltipTextProps {
    pub text: Vec<String>,
    pub quality: AttributeQuality,
    pub offset: f32,
    pub anchor: Anchor,
}
impl TooltipTextProps {
    pub fn new(text: Vec<String>, offset: f32, quality: AttributeQuality, anchor: Anchor) -> Self {
        Self {
            text,
            quality,
            offset,
            anchor,
        }
    }
}

#[derive(Default)]
pub struct TooltipTeardownEvent;

pub fn tick_tooltip_timer(time: Res<Time>, mut tooltip_manager: ResMut<TooltipsManager>) {
    if tooltip_manager.timer.finished() {
        return;
    }
    tooltip_manager.timer.tick(time.delta());
}

pub fn handle_tooltip_teardown(
    mut commands: Commands,
    mut updates: EventReader<TooltipTeardownEvent>,
    tooltip: Query<Entity, With<ItemOrRecipeTooltip>>,
    mut tooltip_manager: ResMut<TooltipsManager>,
) {
    if updates.iter().count() > 0 {
        for t in tooltip.iter() {
            commands.entity(t).despawn_recursive();
        }

        // begin delay for next tooltip
        if tooltip.iter().count() > 0 {
            tooltip_manager.timer.reset();
        }
    }
}

pub fn handle_spawn_inv_item_tooltip(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    mut updates: EventReader<ToolTipUpdateEvent>,
    inv: Query<Entity, With<InventoryUI>>,
    essence: Query<Entity, With<EssenceUI>>,
    cur_inv_state: Res<State<UIState>>,
    recipes: Res<Recipes>,
    item_stacks: Query<(Entity, &ItemStack)>,
    proto: ProtoParam,
) {
    for item in updates.iter() {
        let parent_inv_size = match cur_inv_state.0 {
            UIState::Inventory => INVENTORY_UI_SIZE,
            UIState::Chest => CHEST_INVENTORY_UI_SIZE,
            UIState::Crafting => CRAFTING_INVENTORY_UI_SIZE,
            UIState::Furnace => FURNACE_INVENTORY_UI_SIZE,
            UIState::Essence => ESSENCE_UI_SIZE,
            _ => continue,
        };
        let raw_base_attributes =
            proto.get_component::<RawItemBaseAttributes, _>(item.item_stack.obj_type);
        let raw_bonus_attributes =
            proto.get_component::<RawItemBonusAttributes, _>(item.item_stack.obj_type);
        let equip_type = proto.get_component::<EquipmentType, _>(item.item_stack.obj_type);
        let item_rarity = item.item_stack.rarity.clone();
        let (attributes, score, num_attributes) = item.item_stack.attributes.get_tooltips(
            item_rarity.clone(),
            raw_base_attributes,
            raw_bonus_attributes,
        );

        //subtract 2 for the base attributes, only want bonus attributes
        let num_stars = get_num_stars(score, num_attributes - 2., item_rarity.clone(), equip_type);
        // let durability = item.item_stack.attributes.get_durability_tooltip();
        let level = item.item_stack.metadata.level;
        let item_actions = proto.get_component::<ItemActions, _>(item.item_stack.obj_type);
        let should_show_attributes = attributes.len() > 0 && !item.is_recipe;
        let size = Vec2::new(93., 120.5);
        let tooltip = commands
            .spawn((
                SpriteBundle {
                    texture: graphics
                        .get_ui_element_texture(item_rarity.clone().get_tooltip_ui_element()),
                    transform: Transform {
                        translation: Vec3::new(-(parent_inv_size.x + size.x + 2.) / 2., 0., 4.),
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
                item_rarity.get_tooltip_ui_element(),
                Name::new("TOOLTIP"),
                ItemOrRecipeTooltip,
            ))
            .id();

        let mut tooltip_text: Vec<TooltipTextProps> = vec![];
        tooltip_text.push(TooltipTextProps::new(
            vec![item.item_stack.metadata.name.clone()],
            0.,
            AttributeQuality::High,
            Anchor::Center,
        ));

        if should_show_attributes {
            for (i, (a, range, q)) in attributes.iter().enumerate().clone() {
                let d = if i >= 2 { 3. } else { 0. };
                tooltip_text.push(TooltipTextProps::new(
                    vec![a.to_string(), range.to_string()],
                    d,
                    *q,
                    Anchor::CenterLeft,
                ));
            }
            if let Some(level) = level {
                tooltip_text.push(TooltipTextProps::new(
                    vec!["Level ".to_string() + &level.to_string()],
                    size.y - (tooltip_text.len() + 1) as f32 * 10. - 14.,
                    AttributeQuality::Low,
                    Anchor::CenterLeft,
                ));
            }
        } else {
            if item.is_recipe {
                tooltip_text.push(TooltipTextProps::new(
                    vec!["".to_string()],
                    0.,
                    AttributeQuality::Low,
                    Anchor::CenterLeft,
                ));
                tooltip_text.push(TooltipTextProps::new(
                    vec!["Recipe".to_string()],
                    0.,
                    AttributeQuality::Average,
                    Anchor::CenterLeft,
                ));
            } else {
                if let Some(item_actions) = item_actions {
                    let action_type = item_actions.get_action_type();
                    let action_texts: Vec<String> = item_actions
                        .actions
                        .iter()
                        .flat_map(|a| a.get_tooltip())
                        .collect();
                    if action_texts.len() == 0 {
                        tooltip_text.push(TooltipTextProps::new(
                            vec!["".to_string()],
                            0.,
                            AttributeQuality::High,
                            Anchor::CenterLeft,
                        ));
                    }
                    tooltip_text.push(TooltipTextProps::new(
                        vec![action_type],
                        0.,
                        AttributeQuality::Low,
                        Anchor::CenterLeft,
                    ));
                    if action_texts.len() != 0 {
                        let mut combined_actions = "".to_string();
                        for action_text in action_texts.iter() {
                            combined_actions += action_text;
                            combined_actions += " ";
                        }

                        tooltip_text.push(TooltipTextProps::new(
                            vec![combined_actions],
                            0.,
                            AttributeQuality::High,
                            Anchor::CenterLeft,
                        ));
                    }
                } else {
                    tooltip_text.push(TooltipTextProps::new(
                        vec!["".to_string()],
                        0.,
                        AttributeQuality::Low,
                        Anchor::CenterLeft,
                    ));
                    tooltip_text.push(TooltipTextProps::new(
                        vec!["Material".to_string()],
                        0.,
                        AttributeQuality::Low,
                        Anchor::CenterLeft,
                    ));
                }
            }
            for (i, desc_string) in item.item_stack.metadata.desc.iter().enumerate().clone() {
                tooltip_text.push(TooltipTextProps::new(
                    vec![desc_string.to_string()],
                    4. + if item.is_recipe {
                        5. * (i + 1) as f32
                    } else {
                        0.
                    },
                    AttributeQuality::Average,
                    Anchor::CenterLeft,
                ));
            }
        }

        for (i, props) in tooltip_text.iter().enumerate() {
            let text_pos = if i == 0 {
                Vec3::new(-0.0, size.y / 2. - 12., 1.)
            } else {
                Vec3::new(
                    -size.x / 2. + 8. + if item.is_recipe { 4. } else { 0. },
                    size.y / 2. - 14. - (i as f32 * 10.) - props.offset,
                    1.,
                )
            };
            for (j, t) in props.text.clone().iter().enumerate() {
                if !item.show_range && j == 1 {
                    continue;
                }
                let text = commands
                    .spawn((
                        Text2dBundle {
                            text: Text::from_section(
                                t,
                                TextStyle {
                                    font: asset_server.load("fonts/4x5.ttf"),
                                    font_size: 5.0,
                                    color: if i == 0 {
                                        item.item_stack.rarity.get_color()
                                    } else if j == 1 {
                                        LIGHT_GREY
                                    } else {
                                        match props.quality {
                                            AttributeQuality::Low => LIGHT_GREY,
                                            AttributeQuality::Average => GREY,
                                            AttributeQuality::High => props.quality.get_color(),
                                        }
                                    },
                                },
                            ),
                            text_anchor: if j == 0 {
                                props.anchor.clone()
                            } else {
                                Anchor::CenterRight
                            },
                            transform: Transform {
                                translation: text_pos
                                    + Vec3::new(
                                        if j == 1 { TOOLTIP_UI_SIZE.x - 14. } else { 0. },
                                        0.,
                                        0.,
                                    ),
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

            if item.is_recipe && i > 2 {
                let ingredient_world_obj: Vec<WorldObject> = recipes
                    .crafting_list
                    .get(&item.item_stack.obj_type)
                    .unwrap()
                    .0
                    .iter()
                    .map(|r| r.item)
                    .collect();
                let icon_e = spawn_item_stack_icon(
                    &mut commands,
                    &graphics,
                    &ItemStack {
                        obj_type: ingredient_world_obj[i - 3],
                        count: 1,
                        ..Default::default()
                    },
                    &asset_server,
                    Vec2::ZERO,
                    3,
                );
                commands
                    .entity(icon_e)
                    .insert(RecipeIngredientTooltipIcon)
                    .insert(Transform {
                        translation: text_pos + Vec3::new(22., 0., 0.),
                        ..Default::default()
                    });
                commands.entity(tooltip).add_child(icon_e);

                // bounce
                for (e, stack) in item_stacks.iter() {
                    if stack.obj_type == ingredient_world_obj[i - 3] {
                        commands.entity(e).insert(BounceOnHit::new());
                    }
                }
            }
        }
        for i in 0..num_stars {
            let star = spawn_sprite(
                &mut commands,
                Vec3::new(36. - i as f32 * 7., -size.y / 2. + 10., 1.),
                graphics.get_ui_element_texture(UIElement::StarIcon),
                3,
            );
            commands.entity(star).set_parent(tooltip);
        }
        // add tooltip to inventory or essence ui
        //TODO: maybe we dont need to add as parent here and avoid this
        if let Ok(inv) = inv.get_single() {
            commands.entity(inv).add_child(tooltip);
        } else {
            commands
                .entity(essence.get_single().unwrap())
                .add_child(tooltip);
        }
    }
}

pub fn handle_spawn_inv_player_stats(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    mut updates: EventReader<ShowInvPlayerStatsEvent>,
    curr_ui_state: Res<State<UIState>>,
    player_stats: Query<
        (
            &Attack,
            &MaxHealth,
            &Defence,
            &CritChance,
            &CritDamage,
            &BonusDamage,
            &HealthRegen,
            &Healing,
            &Thorns,
            &Dodge,
            &Speed,
            &Lifesteal,
            &XpRateBonus,
            &LootRateBonus,
        ),
        With<Player>,
    >,
    inv: Query<Entity, With<InventoryUI>>,
    ui_state: Res<State<UIState>>,
    mut tooltip_manager: ResMut<TooltipsManager>,
    old_tooltips: Query<Entity, With<PlayerStatsTooltip>>,
) {
    if ui_state.0 == UIState::Closed {
        let d = tooltip_manager.timer.duration().clone();
        tooltip_manager.timer.tick(d);
        return;
    }
    if updates.iter().len() > 0 && tooltip_manager.timer.finished() {
        for t in old_tooltips.iter() {
            commands.entity(t).despawn_recursive();
        }
        tooltip_manager.timer.reset();
        let (Ok(parent_e), translation) = (if curr_ui_state.0 == UIState::Inventory {
            (
                inv.get_single(),
                Vec3::new(-(INVENTORY_UI_SIZE.x + TOOLTIP_UI_SIZE.x + 2.) / 2., 0., 2.),
            )
        } else {
            return;
        }) else {
            return;
        };

        let (
            attack,
            max_health,
            defence,
            crit_chance,
            crit_damage,
            bonus_damage,
            health_regen,
            healing,
            thorns,
            dodge,
            speed,
            lifesteal,
            xp_rate_bonus,
            loot_rate_bonus,
        ) = player_stats.single();

        let attributes = ItemAttributes {
            attack: AttributeValue::new(attack.0, AttributeQuality::Low, 0.),
            health: AttributeValue::new(max_health.0, AttributeQuality::Low, 0.),
            defence: AttributeValue::new(defence.0, AttributeQuality::Low, 0.),
            crit_chance: AttributeValue::new(crit_chance.0, AttributeQuality::Low, 0.),
            crit_damage: AttributeValue::new(crit_damage.0, AttributeQuality::Low, 0.),
            bonus_damage: AttributeValue::new(bonus_damage.0, AttributeQuality::Low, 0.),
            health_regen: AttributeValue::new(health_regen.0, AttributeQuality::Low, 0.),
            healing: AttributeValue::new(healing.0, AttributeQuality::Low, 0.),
            thorns: AttributeValue::new(thorns.0, AttributeQuality::Low, 0.),
            dodge: AttributeValue::new(dodge.0, AttributeQuality::Low, 0.),
            speed: AttributeValue::new(speed.0, AttributeQuality::Low, 0.),
            lifesteal: AttributeValue::new(lifesteal.0, AttributeQuality::Low, 0.),
            xp_rate: AttributeValue::new(xp_rate_bonus.0, AttributeQuality::Low, 0.),
            loot_rate: AttributeValue::new(loot_rate_bonus.0, AttributeQuality::Low, 0.),
            ..default()
        }
        .get_stats_summary();

        let tooltip = commands
            .spawn((
                SpriteBundle {
                    texture: graphics.get_ui_element_texture(UIElement::StatTooltip),
                    transform: Transform {
                        translation,
                        scale: Vec3::new(1., 1., 1.),
                        ..Default::default()
                    },
                    sprite: Sprite {
                        custom_size: Some(TOOLTIP_UI_SIZE),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                RenderLayers::from_layers(&[3]),
                UIElement::StatTooltip,
                PlayerStatsTooltip,
                Name::new("TOOLTIP"),
            ))
            .id();

        let mut tooltip_text: Vec<((String, String), f32)> = vec![];
        tooltip_text.push((("Stats".to_string(), "".to_string()), 0.));
        for (_i, a) in attributes.iter().enumerate().clone() {
            tooltip_text.push(((a.0.clone(), a.1.clone()), 0.));
        }
        let total_tooltips = tooltip_text.len();
        for (i, (text, d)) in tooltip_text.iter().enumerate() {
            let text_pos = if i == 0 {
                Vec3::new(
                    -(f32::ceil((text.0.chars().count() * 5 - 1) as f32 / 2.)) + 0.5,
                    TOOLTIP_UI_SIZE.y / 2. - 12.,
                    1.,
                )
            } else if i == total_tooltips - 1 {
                Vec3::new(
                    -TOOLTIP_UI_SIZE.x / 2. + 46.,
                    TOOLTIP_UI_SIZE.y / 2. - 12. - ((i as f32 - 1.) * 8.) - d - 2.,
                    1.,
                )
            } else if i == total_tooltips - 2 {
                Vec3::new(
                    -TOOLTIP_UI_SIZE.x / 2. + 8.,
                    TOOLTIP_UI_SIZE.y / 2. - 12. - (i as f32 * 8.) - d - 2.,
                    1.,
                )
            } else {
                Vec3::new(
                    -TOOLTIP_UI_SIZE.x / 2. + 8.,
                    TOOLTIP_UI_SIZE.y / 2. - 12. - (i as f32 * 8.) - d - 2.,
                    1.,
                )
            };

            let text_att_name = commands
                .spawn((
                    Text2dBundle {
                        text: Text::from_section(
                            text.0.to_string(),
                            TextStyle {
                                font: asset_server.load("fonts/4x5.ttf"),
                                font_size: 5.0,
                                color: if i == 0 { BLACK } else { GREY },
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
            commands.entity(tooltip).add_child(text_att_name);
            let text_att_value = commands
                .spawn((
                    Text2dBundle {
                        text: Text::from_section(
                            text.1.to_string(),
                            TextStyle {
                                font: asset_server.load("fonts/4x5.ttf"),
                                font_size: 5.0,
                                color: LIGHT_RED,
                            },
                        ),
                        text_anchor: Anchor::CenterRight,
                        transform: Transform {
                            translation: text_pos
                                + Vec3::new(
                                    if i == total_tooltips - 2 {
                                        20.
                                    } else if i == total_tooltips - 1 {
                                        39.
                                    } else {
                                        TOOLTIP_UI_SIZE.x - 16.
                                    },
                                    0.,
                                    0.,
                                ),
                            scale: Vec3::new(1., 1., 1.),
                            ..Default::default()
                        },
                        ..default()
                    },
                    Name::new("TOOLTIP TEXT"),
                    RenderLayers::from_layers(&[3]),
                ))
                .id();
            commands.entity(tooltip).add_child(text_att_value);
        }
        commands.entity(parent_e).add_child(tooltip);
    }
}

fn get_color_from_stat_hover(i: usize, stat: &Option<StatType>) -> Color {
    if let Some(stat) = stat {
        match stat {
            &StatType::STR => {
                if i == 2 {
                    return DARK_GREEN;
                }
                if i == 5 {
                    return LIGHT_GREEN;
                }
            }
            &StatType::DEX => {
                if i == 2 {
                    return LIGHT_GREEN;
                }
                if i == 4 {
                    return DARK_GREEN;
                }
                if i == 5 {
                    return DARK_GREEN;
                }
            }
            &StatType::AGI => {
                if i == 2 {
                    return LIGHT_GREEN;
                }
                if i == 9 {
                    return DARK_GREEN;
                }
                if i == 10 {
                    return DARK_GREEN;
                }
            }
            &StatType::VIT => {
                if i == 2 {
                    return LIGHT_GREEN;
                }
                if i == 1 {
                    return DARK_GREEN;
                }
                if i == 6 {
                    return DARK_GREEN;
                }
            }
        }
    }
    LIGHT_RED
}

pub fn get_num_stars(
    score: f32,
    total_atts: f32,
    rarity: ItemRarity,
    equip_type: Option<&EquipmentType>,
) -> usize {
    let mut num_stars = 0.;
    let max_possible_stars = if let Some(eqp_type) = equip_type {
        3. + total_atts - rarity.get_num_bonus_attributes(eqp_type).end().clone() as f32
    } else {
        3.
    };
    if score >= 0.87 {
        num_stars = 3.;
    } else if score > 0.7 {
        num_stars = 2.;
    } else if score > 0.45 {
        num_stars = 1.;
    }
    f32::min(f32::min(num_stars, max_possible_stars), 3.) as usize
}
