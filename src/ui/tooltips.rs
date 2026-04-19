use bevy::{prelude::*, render::view::RenderLayers, sprite::Anchor};
use bevy_aseprite::{anim::AsepriteAnimation, aseprite, AsepriteBundle};

use crate::{
    assets::{asset_helpers::spawn_sprite, Graphics},
    attributes::{
        add_item_glows, Attack, AttackSpeed, AttributeQuality, AttributeValue, BonusDamage,
        CritChance, CritDamage, CurrentHealth, CurrentMana, Defence, Dodge, Healing, HealthRegen,
        ItemAttributes, ItemRarity, Lifesteal, LootRateBonus, ManaRegen, MaxHealth, MaxMana,
        PickupRange, ProjectileSize, RawItemBaseAttributes, RawItemBonusAttributes, SkillPower,
        Speed, Thorns, XpRateBonus,
    },
    colors::{ORANGE, STATS_TITLE, TOOLTIP_BLACK, TOOLTIP_BLACK_2, WHITE, YELLOW, YELLOW_2},
    combat::damage_tracker::{spawn_damage_tracker_ui, DamageTracker, PetAbilityStats},
    inventory::{Inventory, ItemStack},
    item::{item_actions::ItemActions, EquipmentType, Recipes, WorldObject},
    juice::bounce::BounceOnHit,
    player::{stats::StatType, Player},
    proto::proto_param::ProtoParam,
    ui::{
        spawn_item_stack_icon, INVENTORY_EQUIPMENT_UI_SIZE, INVENTORY_UPGRADE_UI_SIZE,
        TOOLTIP_UI_SIZE,
    },
};

use super::{
    item_chest::ItemChestUI, EssenceUI, InventoryUI, UIElement, UIState, CHEST_INVENTORY_UI_SIZE,
    CRAFTING_INVENTORY_UI_SIZE, ESSENCE_UI_SIZE, FURNACE_INVENTORY_UI_SIZE, INVENTORY_UI_SIZE,
    INVENTORY_Y_OFFSET, SKILLS_CHOICE_UI_SIZE,
};

aseprite!(pub InventoryStatHighlightCommon, "textures/effects/InventoryStatHighlightCommon.ase");
aseprite!(pub InventoryStatHighlightUncommon, "textures/effects/InventoryStatHighlightUncommon.ase");
aseprite!(pub InventoryStatHighlightRare, "textures/effects/InventoryStatHighlightRare.ase");
aseprite!(pub InventoryStatHighlightLegendary, "textures/effects/InventoryStatHighlightLegendary.ase");

/// Panel size for `LargeTooltip*` sprites (inventory item card + consumable buff HUD hover).
pub const ITEM_TOOLTIP_LARGE_CARD_SIZE: Vec2 = Vec2::new(172., 272.);
/// Horizontal spacing between recipe ingredient icons on the recipe tooltip (center-to-center).
pub const RECIPE_TOOLTIP_INGREDIENT_SPACING_X: f32 = 28.;
/// Panel-local Y for the ingredient icon row (below the title, above the type line).
pub const RECIPE_TOOLTIP_INGREDIENT_ROW_Y: f32 = 22.;
/// Y offset for the required-count label under each ingredient icon.
pub const RECIPE_TOOLTIP_INGREDIENT_COUNT_Y_OFFSET: f32 = 10.;

/// Wait after an inventory item tooltip closes before showing the stats tooltip again
/// (avoids flicker when moving quickly across slots).
pub const STATS_TOOLTIP_RESPAWN_DELAY_SECS: f32 = 0.18;

#[derive(Component)]
pub struct PlayerStatsTooltip;
#[derive(Component)]
pub struct ItemOrRecipeTooltip;

/// Marks a tooltip spawned next to the consumable-buff HUD (despawned independently of inventory).
#[derive(Component)]
pub struct ConsumableBuffHudTooltip;

#[derive(Component)]
pub struct RecipeIngredientTooltipIcon;

#[derive(Resource, Clone)]
pub struct TooltipsManager {
    pub timer: Timer,
    /// Fires [`ShowInvPlayerStatsEvent`] when finished; cleared when a new item tooltip spawns.
    pub stats_respawn_delay: Option<Timer>,
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
    pub ignore_timer: bool,
}

#[derive(Debug, Clone)]
pub struct TooltipTextProps {
    pub text: Vec<String>,
    pub quality: AttributeQuality,
    pub offset: f32,
    pub anchor: Anchor,
    pub font: String,
    pub font_size: f32,
}
impl TooltipTextProps {
    pub fn new(
        text: Vec<String>,
        offset: f32,
        quality: AttributeQuality,
        anchor: Anchor,
        font: String,
    ) -> Self {
        let font_size = match font.as_str() {
            "fonts/alagard.ttf" => 15.0,
            _ => 8.4,
        };
        Self {
            text,
            quality,
            offset,
            anchor,
            font,
            font_size,
        }
    }
}

#[derive(Default)]
pub struct TooltipTeardownEvent;

pub fn tick_tooltip_timer(
    time: Res<Time>,
    mut tooltip_manager: ResMut<TooltipsManager>,
    mut stats_event: EventWriter<ShowInvPlayerStatsEvent>,
    cur_ui_state: Res<State<UIState>>,
) {
    if !tooltip_manager.timer.finished() {
        tooltip_manager.timer.tick(time.delta());
    }
    if let Some(ref mut delay) = tooltip_manager.stats_respawn_delay {
        delay.tick(time.delta());
        if delay.finished() {
            tooltip_manager.stats_respawn_delay = None;
            if cur_ui_state.0 == UIState::Inventory {
                stats_event.send(ShowInvPlayerStatsEvent {
                    stat: None,
                    ignore_timer: true,
                });
            }
        }
    }
}

pub fn handle_tooltip_teardown(
    mut commands: Commands,
    mut updates: EventReader<TooltipTeardownEvent>,
    tooltip: Query<Entity, With<ItemOrRecipeTooltip>>,
    mut tooltip_manager: ResMut<TooltipsManager>,
    inv: Query<&Inventory>,

    mut tooltip_update_events: EventWriter<ToolTipUpdateEvent>,
    cur_ui_state: Res<State<UIState>>,
) {
    if updates.iter().count() > 0 {
        let inv = inv.single();
        if let Some(item) = &inv.furnace_items.items[1] {
            tooltip_update_events.send(ToolTipUpdateEvent {
                item_stack: item.item_stack.clone(),
                is_recipe: false,
                show_range: false,
            });
        } else {
            for t in tooltip.iter() {
                commands.entity(t).despawn_recursive();
            }
            // begin delay for next tooltip
            if tooltip.iter().count() > 0 {
                tooltip_manager.timer.reset();
            }
            if cur_ui_state.0 == UIState::Inventory {
                tooltip_manager.stats_respawn_delay = Some(Timer::from_seconds(
                    STATS_TOOLTIP_RESPAWN_DELAY_SECS,
                    TimerMode::Once,
                ));
            }
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
    item_chest: Query<Entity, With<ItemChestUI>>,
    cur_inv_state: Res<State<UIState>>,
    recipes: Res<Recipes>,
    item_stacks: Query<(Entity, &ItemStack), Without<RecipeIngredientTooltipIcon>>,
    proto: ProtoParam,
    old_tooltips: Query<Entity, With<ItemOrRecipeTooltip>>,
    player_stats_tooltips: Query<Entity, With<PlayerStatsTooltip>>,
    mut tooltip_manager: ResMut<TooltipsManager>,
) {
    for item in updates.iter() {
        tooltip_manager.stats_respawn_delay = None;
        for t in old_tooltips.iter() {
            commands.entity(t).despawn_recursive();
        }
        // Standard "right of inventory UI" tooltip anchor — used for normal inventory
        // items in both `Inventory` and `InventoryCrafting` modes.
        let right_side_offset = Vec2::new(
            ((INVENTORY_UI_SIZE.x
                + TOOLTIP_UI_SIZE.x
                + INVENTORY_UPGRADE_UI_SIZE.x
                + INVENTORY_EQUIPMENT_UI_SIZE.x
                + 8.)
                / 2.)
                .floor(),
            0.,
        );
        let parent_offset = match cur_inv_state.0 {
            UIState::Inventory => right_side_offset,
            // In `InventoryCrafting` mode:
            //   - Recipe tooltips (hovering a blueprint row) render on the LEFT, anchored
            //     on top of the main inventory panel. The BlueprintsPanel occupies the
            //     right side, so this is the only free space.
            //   - Normal item tooltips (hovering an inventory/hotbar slot) keep the same
            //     right-side position as `UIState::Inventory`.
            // The inventory panel center is at `(-185, INVENTORY_Y_OFFSET)` (see
            // `setup_inv_ui` `pos_offset`). We bias the recipe tooltip slightly right of
            // the panel center so its left edge doesn't hug the screen edge.
            UIState::InventoryCrafting => {
                if item.is_recipe {
                    Vec2::new(0., INVENTORY_Y_OFFSET)
                } else {
                    right_side_offset
                }
            }
            UIState::Chest => Vec2::new(
                -CHEST_INVENTORY_UI_SIZE.x - 20.,
                -CHEST_INVENTORY_UI_SIZE.y / 2. + 40.,
            ),
            UIState::Crafting => CRAFTING_INVENTORY_UI_SIZE,
            UIState::Furnace => FURNACE_INVENTORY_UI_SIZE,
            UIState::Essence => ESSENCE_UI_SIZE,
            UIState::ItemChest => Vec2::new(
                -CHEST_INVENTORY_UI_SIZE.x - 20.,
                -CHEST_INVENTORY_UI_SIZE.y / 2. + 40.,
            ),
            _ => continue,
        };

        if cur_inv_state.0 == UIState::Inventory {
            for t in player_stats_tooltips.iter() {
                commands.entity(t).despawn_recursive();
            }
        }

        let obj_type = item.item_stack.obj_type;
        let raw_base_attributes = proto.get_component::<RawItemBaseAttributes, _>(obj_type);
        let raw_bonus_attributes = proto.get_component::<RawItemBonusAttributes, _>(obj_type);
        let equip_type = proto.get_component::<EquipmentType, _>(obj_type);
        let item_rarity = item.item_stack.rarity.clone();
        let level = item.item_stack.metadata.level;
        let (attributes, score, num_attributes) = ItemAttributes::get_tooltips_from_stat_lines(
            &item.item_stack.metadata.bonus_stat_lines,
            &item.item_stack.attributes,
            item_rarity.clone(),
            raw_base_attributes,
            raw_bonus_attributes,
            level.unwrap_or(0) as i32,
            obj_type,
            equip_type.unwrap_or(&EquipmentType::None),
        );

        //subtract 2 for the base attributes, only want bonus attributes
        let num_stars = get_num_stars(score, num_attributes, item_rarity.clone(), equip_type);
        // let durability = item.item_stack.attributes.get_durability_tooltip();
        let item_actions = proto.get_component::<ItemActions, _>(obj_type);
        let should_show_attributes = !attributes.is_empty() && !item.is_recipe;
        let size = ITEM_TOOLTIP_LARGE_CARD_SIZE;
        let tooltip = commands
            .spawn((
                SpriteBundle {
                    texture: graphics
                        .get_ui_element_texture(item_rarity.clone().get_tooltip_ui_element()),
                    transform: Transform {
                        translation: Vec3::new(parent_offset.x, parent_offset.y, 10.),
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

        spawn_item_tooltip_icon_name_header(
            &mut commands,
            &graphics,
            &asset_server,
            tooltip,
            &item.item_stack,
        );
        if item.is_recipe {
            // spawn_recipe_ingredients_tooltip_row(
            //     &mut commands,
            //     &graphics,
            //     &asset_server,
            //     tooltip,
            //     obj_type,
            //     &recipes,
            //     &item_stacks,
            // );
        }
        let mut is_item_action = false;
        let is_upgrade_material =
            obj_type == WorldObject::UpgradeTome || obj_type == WorldObject::OrbOfTransformation;
        // ======== level ========
        let level_string = if let Some(level) = level {
            "Level ".to_string() + &level.to_string()
        } else if let Some(item_actions) = item_actions {
            let action_texts: Vec<String> = item_actions
                .actions
                .iter()
                .flat_map(|a| a.get_tooltip())
                .collect();

            if !action_texts.is_empty() {
                is_item_action = true;
                let mut combined_actions = "".to_string();
                for action_text in action_texts.iter() {
                    combined_actions += action_text;
                    combined_actions += " ";
                }

                combined_actions.to_string()
            } else {
                "".to_string()
            }
        } else {
            "".to_string()
        };

        let _level_text = commands
            .spawn((
                Text2dBundle {
                    text: Text::from_section(
                        level_string,
                        TextStyle {
                            font: asset_server.load(if is_item_action {
                                "fonts/slkscr.ttf"
                            } else {
                                "fonts/slkscrbold.ttf"
                            }),
                            font_size: 8.5,
                            color: if is_item_action {
                                ORANGE
                            } else {
                                item.item_stack.rarity.get_color()
                            },
                        },
                    ),
                    text_anchor: Anchor::CenterLeft,
                    transform: Transform {
                        translation: Vec3::new(-16., 76., 1.),
                        scale: Vec3::new(1., 1., 1.),
                        ..Default::default()
                    },
                    ..default()
                },
                Name::new("TOOLTIP Rarity TEXT"),
                RenderLayers::from_layers(&[3]),
            ))
            .set_parent(tooltip)
            .id();

        // ======== header (Base Stats / Description — recipe uses ingredient row instead) ========
        if should_show_attributes {
            let _header_text = commands
                .spawn((
                    Text2dBundle {
                        text: Text::from_section(
                            "Base Stats",
                            TextStyle {
                                font: asset_server.load("fonts/slkscrbold.ttf"),
                                font_size: 8.5,
                                color: YELLOW_2,
                            },
                        ),
                        text_anchor: Anchor::CenterLeft,
                        transform: Transform {
                            translation: Vec3::new(-57., 20., 1.),
                            scale: Vec3::new(1., 1., 1.),
                            ..Default::default()
                        },
                        ..default()
                    },
                    Name::new("TOOLTIP Rarity TEXT"),
                    RenderLayers::from_layers(&[3]),
                ))
                .set_parent(tooltip)
                .id();
        } else if !item.is_recipe {
            let _header_text = commands
                .spawn((
                    Text2dBundle {
                        text: Text::from_section(
                            "Description",
                            TextStyle {
                                font: asset_server.load("fonts/slkscrbold.ttf"),
                                font_size: 8.5,
                                color: YELLOW_2,
                            },
                        ),
                        text_anchor: Anchor::CenterLeft,
                        transform: Transform {
                            translation: Vec3::new(-57., 20., 1.),
                            scale: Vec3::new(1., 1., 1.),
                            ..Default::default()
                        },
                        ..default()
                    },
                    Name::new("TOOLTIP Rarity TEXT"),
                    RenderLayers::from_layers(&[3]),
                ))
                .set_parent(tooltip)
                .id();
        }
        // ======== rarity ========
        let _rarity_text = commands
            .spawn((
                Text2dBundle {
                    text: Text::from_section(
                        item.item_stack.rarity.get_name(),
                        TextStyle {
                            font: asset_server.load("fonts/slkscr.ttf"),
                            font_size: 8.5,
                            color: item.item_stack.rarity.get_color(),
                        },
                    ),
                    text_anchor: Anchor::CenterLeft,
                    transform: Transform {
                        translation: Vec3::new(
                            -16.,
                            if is_upgrade_material { 68. } else { 62. },
                            1.,
                        ),
                        scale: Vec3::new(1., 1., 1.),
                        ..Default::default()
                    },
                    ..default()
                },
                Name::new("TOOLTIP Rarity TEXT"),
                RenderLayers::from_layers(&[3]),
            ))
            .set_parent(tooltip)
            .id();
        // ======== type ========
        let type_string = if obj_type.is_melee_weapon() {
            "Melee Weapon"
        } else if obj_type.is_magic_weapon() {
            "Magic Weapon"
        } else if obj_type.is_ranged_weapon() {
            "Ranged Weapon"
        } else if equip_type.is_some_and(|e| e.is_tool()) {
            "Tool"
        } else if obj_type.is_armor() {
            "Armor"
        } else if obj_type.is_accessory() {
            "Accessory"
        } else if let Some(item_actions) = item_actions {
            &item_actions.get_action_type()
        } else if is_upgrade_material {
            "Upgrade\nMaterial"
        } else {
            "Material"
        };
        let _type_text = commands
            .spawn((
                Text2dBundle {
                    text: Text::from_section(
                        type_string.to_string(),
                        TextStyle {
                            font: asset_server.load("fonts/slkscr.ttf"),
                            font_size: 8.5,
                            color: item.item_stack.rarity.get_color(),
                        },
                    ),
                    text_anchor: Anchor::CenterLeft,
                    transform: Transform {
                        translation: Vec3::new(-16., 52., 1.),
                        scale: Vec3::new(1., 1., 1.),
                        ..Default::default()
                    },
                    ..default()
                },
                Name::new("TOOLTIP Rarity TEXT"),
                RenderLayers::from_layers(&[3]),
            ))
            .set_parent(tooltip)
            .id();

        if should_show_attributes {
            info!("SHOW ATTRIBUTES!");
            //======== Header 2 ========
            let _text = commands
                .spawn((
                    Text2dBundle {
                        text: Text::from_section(
                            "Bonus Stats".to_string(),
                            TextStyle {
                                font: asset_server.load("fonts/slkscrbold.ttf"),
                                font_size: 8.5,
                                color: YELLOW_2,
                            },
                        ),
                        text_anchor: Anchor::CenterLeft,
                        transform: Transform {
                            translation: Vec3::new(-58., -34., 1.),
                            scale: Vec3::new(1., 1., 1.),
                            ..Default::default()
                        },
                        ..default()
                    },
                    Name::new("TOOLTIP Rarity TEXT"),
                    RenderLayers::from_layers(&[3]),
                ))
                .set_parent(tooltip)
                .id();

            for (i, (a, range, q)) in attributes.iter().enumerate().clone() {
                let d = if i >= 2 { 36. } else { 0. };
                tooltip_text.push(TooltipTextProps::new(
                    vec![a.to_string(), range.to_string()],
                    d,
                    *q,
                    Anchor::CenterLeft,
                    "fonts/slkscr.ttf".to_string(),
                ));
            }

            // Tooltip Inspect ICON
            let tooltip_icon = spawn_item_stack_icon(
                &mut commands,
                &graphics,
                &ItemStack::crate_icon_stack(WorldObject::TooltipInspect),
                &asset_server,
                Vec2::new(TOOLTIP_UI_SIZE.x + 16., 0.),
                Vec2::new(0., 0.),
                3,
            );
            commands.entity(tooltip_icon).set_parent(tooltip);
            commands
                .spawn(SpriteBundle {
                    texture: asset_server.load("textures/ShiftKey.png"),
                    transform: Transform::from_translation(Vec3::new(0.0, 13., 1.)),
                    sprite: Sprite {
                        custom_size: Some(Vec2::new(26., 10.)),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .insert(RenderLayers::from_layers(&[3]))
                .set_parent(tooltip_icon);
        } else {
            if item.is_recipe {
                //======== "Description" sub-header (body text is white below) ========
                let _text = commands
                    .spawn((
                        Text2dBundle {
                            text: Text::from_section(
                                "Description".to_string(),
                                TextStyle {
                                    font: asset_server.load("fonts/slkscrbold.ttf"),
                                    font_size: 8.5,
                                    color: TOOLTIP_BLACK,
                                },
                            ),
                            text_anchor: Anchor::CenterLeft,
                            transform: Transform {
                                translation: Vec3::new(-58., -36., 1.),
                                scale: Vec3::new(1., 1., 1.),
                                ..Default::default()
                            },
                            ..default()
                        },
                        Name::new("TOOLTIP Rarity TEXT"),
                        RenderLayers::from_layers(&[3]),
                    ))
                    .set_parent(tooltip)
                    .id();
            } else {
                tooltip_text.push(TooltipTextProps::new(
                    vec!["".to_string()],
                    0.,
                    AttributeQuality::Low,
                    Anchor::CenterLeft,
                    "fonts/slkscr.ttf".to_string(),
                ));
            }
            for (i, desc_string) in item.item_stack.metadata.desc.iter().enumerate() {
                tooltip_text.push(TooltipTextProps::new(
                    vec![desc_string.to_string()],
                    -10. + if item.is_recipe {
                        65. + 6. * (i) as f32
                    } else {
                        0.
                    },
                    if item.is_recipe {
                        AttributeQuality::Low
                    } else {
                        AttributeQuality::Average
                    },
                    Anchor::CenterLeft,
                    "fonts/slkscr.ttf".to_string(),
                ));
            }
        }

        for (i, props) in tooltip_text.iter().enumerate() {
            let text_pos = Vec3::new(
                -size.x / 2. + 28.,
                size.y / 2. - 126. - (i as f32 * 9.) - props.offset,
                2.,
            );

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
                                    font: asset_server.load(props.font.as_str()),
                                    font_size: props.font_size,
                                    color: if j == 1 {
                                        WHITE
                                    } else {
                                        match props.quality {
                                            AttributeQuality::Low => WHITE,
                                            AttributeQuality::Average => YELLOW,
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

            if let Some(buff_line_index) = item.item_stack.metadata.inventory_buff_line_index {
                // Count base attributes that appear before bonus stat lines
                // Base attributes have range text like "({}-{})", bonus stat lines have empty range text
                let num_base_attrs = {
                    let mut count = 0;

                    for (attr_name, range_text, _) in attributes.iter() {
                        // Base attributes have non-empty range text (showing the range like "({}-{})")
                        // Bonus stat lines have empty range text
                        let has_range = !range_text.is_empty();

                        // Check if this matches a base attribute pattern
                        if attr_name.contains(" HP") && !attr_name.contains("Regen") {
                            if has_range {
                                // Base attribute (has range text)
                                count += 1;
                            } else {
                                // Bonus stat line (no range text) - stop counting base attributes
                                break;
                            }
                        } else if attr_name.contains(" Defence") {
                            if has_range {
                                count += 1;
                            } else {
                                break;
                            }
                        } else if attr_name.contains(" Speed") {
                            if has_range {
                                // Speed can be a base attribute
                                count += 1;
                            } else {
                                break;
                            }
                        } else if attr_name.contains("Attacks / sec") {
                            // Hits/s is always a base attribute (attack_cooldown); check before " Attack" so we don't break (no range text)
                            count += 1;
                        } else if attr_name.contains(" Attack") && !attr_name.contains("Speed") {
                            if has_range {
                                count += 1;
                            } else {
                                break;
                            }
                        } else {
                            // We've hit a bonus stat line (different pattern, no range text)
                            break;
                        }
                    }
                    count
                };

                let filtered_buff_line_index = {
                    // Build a set of base attribute names by checking which attributes in the tooltip have range text
                    // and match base attribute patterns
                    //TODO: this is sus, gotta be a better way
                    let mut base_attribute_names = std::collections::HashSet::new();
                    for (attr_name, range_text, _) in attributes.iter() {
                        let has_range = !range_text.is_empty();
                        if attr_name.contains(" HP") && !attr_name.contains("Regen") && has_range {
                            base_attribute_names.insert("health".to_string());
                        } else if attr_name.contains(" Defence") && has_range {
                            base_attribute_names.insert("defence".to_string());
                        } else if attr_name.contains(" Speed") && has_range {
                            base_attribute_names.insert("speed".to_string());
                        } else if attr_name.contains(" Attack")
                            && !attr_name.contains("Speed")
                            && has_range
                        {
                            base_attribute_names.insert("attack".to_string());
                        }
                        if !has_range && !attr_name.contains("Attacks / sec") {
                            break;
                        }
                    }

                    let mut filtered_index = 0;
                    for (idx, stat_line) in
                        item.item_stack.metadata.bonus_stat_lines.iter().enumerate()
                    {
                        if idx >= buff_line_index {
                            break;
                        }
                        if !base_attribute_names.contains(&stat_line.attribute_name) {
                            filtered_index += 1;
                        }
                    }
                    filtered_index
                };

                let tooltip_index = num_base_attrs + filtered_buff_line_index;
                if i == tooltip_index && i > 0 {
                    let box_x = 0.0;
                    let box_y = size.y / 2. - 126. - (i as f32 * 9.) - props.offset;
                    let box_pos = Vec3::new(box_x, box_y, 1.);
                    let (path, idle_tag) = match item.item_stack.rarity {
                        ItemRarity::Common => (
                            InventoryStatHighlightCommon::PATH,
                            InventoryStatHighlightCommon::tags::IDLE,
                        ),
                        ItemRarity::Uncommon => (
                            InventoryStatHighlightUncommon::PATH,
                            InventoryStatHighlightUncommon::tags::IDLE,
                        ),
                        ItemRarity::Rare => (
                            InventoryStatHighlightRare::PATH,
                            InventoryStatHighlightRare::tags::IDLE,
                        ),
                        ItemRarity::Legendary => (
                            InventoryStatHighlightLegendary::PATH,
                            InventoryStatHighlightLegendary::tags::IDLE,
                        ),
                    };
                    let anim = AsepriteAnimation::from(idle_tag);
                    commands
                        .spawn(AsepriteBundle {
                            aseprite: asset_server.load(path),
                            animation: anim,
                            transform: Transform::from_translation(box_pos),
                            ..Default::default()
                        })
                        .insert(RenderLayers::from_layers(&[3]))
                        .set_parent(tooltip);
                }
            }
        }
        for i in 0..num_stars {
            let star = spawn_sprite(
                &mut commands,
                Vec3::new(-56. + i as f32 * 12., size.y / 2. - 97., 1.),
                graphics.get_ui_element_texture(get_star_icon_from_rarity(
                    item.item_stack.rarity.clone(),
                )),
                3,
            );
            commands.entity(star).set_parent(tooltip);
        }
        // add tooltip to inventory, essence, or item chest ui
        if let Ok(inv) = inv.get_single() {
            commands.entity(inv).add_child(tooltip);
        } else if let Ok(essence) = essence.get_single() {
            commands.entity(essence).add_child(tooltip);
        } else if let Ok(chest) = item_chest.get_single() {
            commands.entity(chest).add_child(tooltip);
        }
    }
}
pub fn get_star_icon_from_rarity(rarity: ItemRarity) -> UIElement {
    match rarity {
        ItemRarity::Common => UIElement::StarIconCommon,
        ItemRarity::Uncommon => UIElement::StarIconUncommon,
        ItemRarity::Rare => UIElement::StarIconRare,
        ItemRarity::Legendary => UIElement::StarIconLegendary,
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
            (
                &Attack,
                &MaxHealth,
                &CurrentHealth,
                &MaxMana,
                &CurrentMana,
                &Defence,
                &CritChance,
                &CritDamage,
                &BonusDamage,
                &ManaRegen,
                &Healing,
                &Thorns,
                &Dodge,
                &Speed,
                &XpRateBonus,
            ),
            &LootRateBonus,
            &ProjectileSize,
            &SkillPower,
            &Lifesteal,
            &PickupRange,
            &AttackSpeed,
        ),
        With<Player>,
    >,
    inv: Query<Entity, With<InventoryUI>>,
    ui_state: Res<State<UIState>>,
    mut tooltip_manager: ResMut<TooltipsManager>,
    old_tooltips: Query<Entity, With<PlayerStatsTooltip>>,
    item_or_recipe_tooltips: Query<Entity, With<ItemOrRecipeTooltip>>,
) {
    if ui_state.0 == UIState::Closed {
        let d = tooltip_manager.timer.duration();
        tooltip_manager.timer.tick(d);
        return;
    }
    if updates.iter().len() > 0
        && (tooltip_manager.timer.finished() || updates.iter().next().unwrap().ignore_timer)
    {
        // Inventory attribute refreshes send this event in PostUpdate; do not respawn the stats
        // panel while an item tooltip is showing (e.g. furnace upgrade slot persistence).
        if curr_ui_state.0 == UIState::Inventory && item_or_recipe_tooltips.iter().next().is_some()
        {
            return;
        }
        for t in old_tooltips.iter() {
            commands.entity(t).despawn_recursive();
        }
        tooltip_manager.timer.reset();
        let (Ok(parent_e), translation) = (if curr_ui_state.0 == UIState::Inventory {
            (
                inv.get_single(),
                Vec3::new(
                    (INVENTORY_UI_SIZE.x
                        + TOOLTIP_UI_SIZE.x
                        + INVENTORY_UPGRADE_UI_SIZE.x
                        + INVENTORY_EQUIPMENT_UI_SIZE.x
                        + 8.)
                        / 2.,
                    0.,
                    2.,
                ),
            )
        } else {
            return;
        }) else {
            return;
        };

        let (
            (
                attack,
                max_health,
                curr_health,
                max_mana,
                curr_mana,
                defence,
                crit_chance,
                crit_damage,
                bonus_damage,
                mana_regen,
                healing,
                thorns,
                dodge,
                speed,
                xp_rate_bonus,
            ),
            loot_rate_bonus,
            size,
            skill_power,
            lifesteal,
            pickup_range,
            attack_speed,
        ) = player_stats.single();

        let attributes = ItemAttributes {
            attack: AttributeValue::new(attack.0, AttributeQuality::Low, 0.),
            health: AttributeValue::new(max_health.0, AttributeQuality::Low, 0.),
            mana: AttributeValue::new(max_mana.0, AttributeQuality::Low, 0.),
            defence: AttributeValue::new(defence.0, AttributeQuality::Low, 0.),
            crit_chance: AttributeValue::new(crit_chance.0, AttributeQuality::Low, 0.),
            crit_damage: AttributeValue::new(crit_damage.0, AttributeQuality::Low, 0.),
            bonus_damage: AttributeValue::new(bonus_damage.0, AttributeQuality::Low, 0.),
            mana_regen: AttributeValue::new(mana_regen.0, AttributeQuality::Low, 0.),
            healing: AttributeValue::new(healing.0, AttributeQuality::Low, 0.),
            thorns: AttributeValue::new(thorns.0, AttributeQuality::Low, 0.),
            dodge: AttributeValue::new(dodge.0, AttributeQuality::Low, 0.),
            speed: AttributeValue::new(speed.0, AttributeQuality::Low, 0.),
            xp_rate: AttributeValue::new(xp_rate_bonus.0, AttributeQuality::Low, 0.),
            loot_rate: AttributeValue::new(loot_rate_bonus.0, AttributeQuality::Low, 0.),
            size: AttributeValue::new(size.0, AttributeQuality::Low, 0.),
            skill_power: AttributeValue::new(skill_power.0, AttributeQuality::Low, 0.),
            lifesteal: AttributeValue::new(lifesteal.0, AttributeQuality::Low, 0.),
            pickup_range: AttributeValue::new(pickup_range.0, AttributeQuality::Low, 0.),
            attack_speed: AttributeValue::new(attack_speed.0, AttributeQuality::Low, 0.),
            ..Default::default()
        }
        .get_stats_summary(curr_health.0, curr_mana.0);

        let _ = spawn_stats_tooltip_at(
            &mut commands,
            &graphics,
            &asset_server,
            parent_e,
            translation,
            &attributes,
        );
    }
}

/// Single helper for spawning the player stats tooltip (inventory "Final Stats" hover and game over "Final Stats" hover).
/// Spawns the tooltip at `translation`, parents it to `parent`, and adds a "Stats" header plus each (name, value) row.
/// `attributes` should be the (name, value) pairs from `ItemAttributes::get_stats_summary`.
pub fn spawn_stats_tooltip_at(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    parent: Entity,
    translation: Vec3,
    attributes: &[(String, String)],
) -> Entity {
    let mut tooltip_text: Vec<((String, String), f32)> = vec![];
    tooltip_text.push((("STATS".to_string(), "".to_string()), 0.));
    for a in attributes {
        tooltip_text.push(((a.0.clone(), a.1.clone()), 0.));
    }

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

    // Horizontal inset from the tooltip sprite edges to the start of text.
    // The `StatTooltip.png` art has a ~14 px wooden frame on each side; text inside this inset
    // keeps labels and values within the inner (striped) content area instead of spilling onto
    // or past the frame.
    const STAT_TOOLTIP_INNER_PAD_X: f32 = 22.;

    for (i, (text, d)) in tooltip_text.iter().enumerate() {
        let text_pos = if i == 0 {
            Vec3::new(
                -(f32::ceil((text.0.chars().count() * 6 - 1) as f32 / 2.)) - 10.,
                TOOLTIP_UI_SIZE.y / 2. - 11.,
                1.,
            )
        } else {
            Vec3::new(
                -TOOLTIP_UI_SIZE.x / 2. + STAT_TOOLTIP_INNER_PAD_X,
                TOOLTIP_UI_SIZE.y / 2. - 23. - (i as f32 * 13.) - d - 2.,
                1.,
            )
        };

        let _text_att_name = commands
            .spawn((
                Text2dBundle {
                    text: Text::from_section(
                        text.0.to_string(),
                        TextStyle {
                            font: if i == 0 {
                                asset_server.load("fonts/alagard.ttf")
                            } else {
                                asset_server.load("fonts/slkscr.ttf")
                            },
                            font_size: if i == 0 { 15. } else { 8.4 },
                            color: if i == 0 { STATS_TITLE } else { YELLOW_2 },
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
        commands.entity(tooltip).add_child(_text_att_name);
        let _text_att_value = commands
            .spawn((
                Text2dBundle {
                    text: Text::from_section(
                        text.1.to_string(),
                        TextStyle {
                            font: asset_server.load("fonts/slkscrbold.ttf"),
                            font_size: 8.4,
                            color: YELLOW_2,
                        },
                    ),
                    text_anchor: Anchor::CenterRight,
                    transform: Transform {
                        translation: text_pos
                            + Vec3::new(
                                TOOLTIP_UI_SIZE.x - 2. * STAT_TOOLTIP_INNER_PAD_X - 6.,
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
        commands.entity(tooltip).add_child(_text_att_value);
    }
    commands.entity(parent).add_child(tooltip);
    tooltip
}

#[derive(Component)]
pub struct DamageTrackerPanel;

pub fn spawn_damage_tracker_in_inventory(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut updates: EventReader<ShowInvPlayerStatsEvent>,
    inv: Query<Entity, With<InventoryUI>>,
    ui_state: Res<State<UIState>>,
    old_panels: Query<Entity, With<DamageTrackerPanel>>,
    tracker: Res<DamageTracker>,
    pet_stats: Option<Res<PetAbilityStats>>,
) {
    if ui_state.0 != UIState::Inventory {
        return;
    }
    if updates.iter().next().is_none() {
        return;
    }

    for e in old_panels.iter() {
        commands.entity(e).despawn_recursive();
    }

    let Ok(inv_entity) = inv.get_single() else {
        return;
    };

    let panel_x = (INVENTORY_UI_SIZE.x
        + TOOLTIP_UI_SIZE.x
        + INVENTORY_UPGRADE_UI_SIZE.x
        + INVENTORY_EQUIPMENT_UI_SIZE.x
        + TOOLTIP_UI_SIZE.x
        + 86.)
        / 2.;
    let start_y = INVENTORY_UI_SIZE.y / 2. - 8.;

    if let Some(entities) = spawn_damage_tracker_ui(
        &mut commands,
        &asset_server,
        &tracker,
        Transform::from_translation(Vec3::new(panel_x, start_y, 2.)),
        1.0,
        80.0,
        pet_stats.as_deref(),
    ) {
        if let Some(panel) = entities.first() {
            commands.entity(*panel).insert(DamageTrackerPanel);
            commands.entity(inv_entity).add_child(*panel);
        }
    }
}

/// Recipe result item: ingredient icons in a horizontal row at the top of the card, with required
/// counts (always shown — `spawn_item_stack_icon` only draws stack text when count > 1).
fn spawn_recipe_ingredients_tooltip_row(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    tooltip: Entity,
    result_obj: WorldObject,
    recipes: &Recipes,
    item_stacks: &Query<(Entity, &ItemStack), Without<RecipeIngredientTooltipIcon>>,
) {
    let Some((ingredients, _, _)) = recipes.crafting_list.get(&result_obj) else {
        return;
    };
    if ingredients.is_empty() {
        return;
    }

    let n = ingredients.len() as f32;
    let span = (n - 1.).max(0.) * RECIPE_TOOLTIP_INGREDIENT_SPACING_X;
    let x_start = -span / 2.;

    for (j, ing) in ingredients.iter().enumerate() {
        let x = x_start + j as f32 * RECIPE_TOOLTIP_INGREDIENT_SPACING_X;
        let stack = ItemStack {
            obj_type: ing.item,
            count: 1,
            ..Default::default()
        };
        let icon_e = spawn_item_stack_icon(
            commands,
            graphics,
            &stack,
            asset_server,
            Vec2::new(x, RECIPE_TOOLTIP_INGREDIENT_ROW_Y),
            Vec2::ZERO,
            3,
        );
        commands.entity(icon_e).insert(RecipeIngredientTooltipIcon);
        commands.entity(tooltip).add_child(icon_e);

        commands
            .spawn((
                Text2dBundle {
                    text: Text::from_section(
                        format!("{}", ing.count),
                        TextStyle {
                            font: asset_server.load("fonts/4x5.ttf"),
                            font_size: 5.0,
                            color: WHITE,
                        },
                    ),
                    text_anchor: Anchor::Center,
                    transform: Transform::from_translation(Vec3::new(
                        x,
                        RECIPE_TOOLTIP_INGREDIENT_ROW_Y - RECIPE_TOOLTIP_INGREDIENT_COUNT_Y_OFFSET,
                        4.,
                    )),
                    ..default()
                },
                Name::new("RECIPE INGREDIENT COUNT"),
                RenderLayers::from_layers(&[3]),
            ))
            .set_parent(tooltip);

        for (e, inv_stack) in item_stacks.iter() {
            if inv_stack.obj_type == ing.item {
                if let Some(mut ec) = commands.get_entity(e) {
                    ec.insert(BounceOnHit::new());
                }
            }
        }
    }
}

/// Icon (2× scale), optional rarity glow, and title row — same layout as inventory item tooltip header.
pub fn spawn_item_tooltip_icon_name_header(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    tooltip: Entity,
    item_stack: &ItemStack,
) {
    let obj_type = item_stack.obj_type;
    let icon_e = spawn_item_stack_icon(
        commands,
        graphics,
        &ItemStack {
            obj_type,
            count: 1,
            ..item_stack.clone()
        },
        asset_server,
        Vec2::ZERO,
        Vec2::ZERO,
        3,
    );
    commands.entity(icon_e).insert(Transform {
        translation: Vec3::new(-45., 65., 2.),
        scale: Vec3::new(2., 2., 1.),
        ..Default::default()
    });
    commands.entity(tooltip).add_child(icon_e);
    if let Some(glow_e) = add_item_glows(commands, graphics, icon_e, item_stack.rarity.clone()) {
        commands
            .entity(glow_e)
            .insert(Name::new("Item Glow Effect"))
            .insert(RenderLayers::from_layers(&[3]));
    }
    commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    item_stack.metadata.name.clone(),
                    TextStyle {
                        font: asset_server.load("fonts/alagard.ttf"),
                        font_size: 15.,
                        color: item_stack.rarity.get_color(),
                    },
                ),
                text_anchor: Anchor::Center,
                transform: Transform {
                    translation: Vec3::new(-ITEM_TOOLTIP_LARGE_CARD_SIZE.x / 2. + 85., 101., 1.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..default()
            },
            Name::new("TOOLTIP Rarity TEXT"),
            RenderLayers::from_layers(&[3]),
        ))
        .set_parent(tooltip);
}

/// World-space item card for HUD buff hover (inventory UI closed).
pub fn spawn_world_item_tooltip_for_stack(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    proto: &ProtoParam,
    item_stack: &ItemStack,
    anchor_translation: Vec3,
) -> Entity {
    let item_rarity = item_stack.rarity.clone();
    let size = ITEM_TOOLTIP_LARGE_CARD_SIZE;
    let item_actions = proto.get_component::<ItemActions, _>(item_stack.obj_type);

    // Panel center offset was tuned for 130×120; shift up when using the tall card so the bottom clears the icon.
    let legacy_panel_h = 120.;
    let panel_center_offset = Vec3::new(-size.x / 2., 55. + (size.y - legacy_panel_h) / 2., 20.);

    let tooltip = commands
        .spawn((
            SpriteBundle {
                texture: graphics
                    .get_ui_element_texture(item_rarity.clone().get_tooltip_ui_element()),
                transform: Transform::from_translation(anchor_translation + panel_center_offset),
                sprite: Sprite {
                    custom_size: Some(size),
                    ..default()
                },
                ..default()
            },
            RenderLayers::from_layers(&[3]),
            item_rarity.get_tooltip_ui_element(),
            Name::new("HUD_ITEM_TOOLTIP"),
            ConsumableBuffHudTooltip,
        ))
        .id();

    spawn_item_tooltip_icon_name_header(commands, graphics, asset_server, tooltip, item_stack);

    let action_or_level = if let Some(ia) = item_actions {
        let texts: Vec<String> = ia.actions.iter().filter_map(|a| a.get_tooltip()).collect();
        if texts.is_empty() {
            String::new()
        } else {
            texts.join(" ")
        }
    } else {
        String::new()
    };

    let has_action_line = !action_or_level.is_empty();
    // Same horizontal and vertical layout as `handle_spawn_inv_item_tooltip` description rows
    // (`text_pos`: x = -size.x/2 + 13, y = size.y/2 - 98 - index*9 - props.offset; non-recipe desc
    // lines use offset -10 at tooltip_text indices 1.., i.e. y = size.y/2 - 97 - 9*d for desc line d).
    let body_text_x = -size.x / 2. + 13.;

    if has_action_line {
        let _t = commands
            .spawn((
                Text2dBundle {
                    text: Text::from_section(
                        action_or_level,
                        TextStyle {
                            font: asset_server.load("fonts/slkscr.ttf"),
                            font_size: 8.5,
                            color: ORANGE,
                        },
                    ),
                    text_anchor: Anchor::CenterLeft,
                    transform: Transform::from_translation(Vec3::new(-16., 78., 2.)),
                    ..default()
                },
                RenderLayers::from_layers(&[3]),
            ))
            .set_parent(tooltip)
            .id();
    }
    for (d, line) in item_stack.metadata.desc.iter().enumerate() {
        let y = size.y / 2. - 97. - 9. * d as f32;
        let _d = commands
            .spawn((
                Text2dBundle {
                    text: Text::from_section(
                        line.clone(),
                        TextStyle {
                            font: asset_server.load("fonts/slkscr.ttf"),
                            font_size: 8.4,
                            color: TOOLTIP_BLACK_2,
                        },
                    ),
                    text_anchor: Anchor::CenterLeft,
                    transform: Transform::from_translation(Vec3::new(body_text_x, y, 2.)),
                    ..default()
                },
                RenderLayers::from_layers(&[3]),
            ))
            .set_parent(tooltip)
            .id();
    }

    tooltip
}

pub fn get_num_stars(
    score: f32,
    total_atts: f32,
    rarity: ItemRarity,
    equip_type: Option<&EquipmentType>,
) -> usize {
    if equip_type == Some(&EquipmentType::Cape) {
        return 3;
    }
    if let Some(equip_type) = equip_type {
        let num_atts = if equip_type.is_weapon() || equip_type.is_tool() {
            total_atts - 1.
        } else if equip_type.is_armor() {
            total_atts - 2.
        } else {
            total_atts
        };
        let mut num_stars = 0.;
        let max_possible_stars =
            3. + num_atts - *rarity.get_num_bonus_attributes(equip_type).end() as f32;
        if score >= 0.87 {
            num_stars = 3.;
        } else if score > 0.7 {
            num_stars = 2.;
        } else if score > 0.42 {
            num_stars = 1.;
        }
        f32::min(f32::min(num_stars, max_possible_stars), 3.) as usize
    } else {
        0
    }
}
