use bevy::text::Justify;
use crate::aseprite_assets::{
    InventoryStatHighlightCommon, InventoryStatHighlightLegendary, InventoryStatHighlightRare,
    InventoryStatHighlightUncommon,
};
use crate::aseprite_helpers::aseprite_bundle;
use bevy::{camera::visibility::RenderLayers, prelude::*, sprite::Anchor};

use crate::{
    assets::{asset_helpers::spawn_sprite, Graphics},
    attributes::{
        add_item_glows,
        health_regen::{effective_regen_period_secs, HealthRegenTimer, ManaRegenTimer},
        set_bonus::{EquipmentSet, SET_PIECES_REQUIRED},
        Attack, AttackSpeed, AttributeQuality, AttributeValue, BonusDamage, CritChance, CritDamage,
        CurrentHealth, CurrentMana, Defence, Dodge, Healing, HealthRegen, ItemAttributes,
        ItemRarity, Lifesteal, LootRateBonus, ManaRegen, MaxHealth, MaxMana, PickupRange,
        ProjectileSize, RawItemBaseAttributes, RawItemBonusAttributes, SkillPower, Speed, Thorns,
        XpRateBonus,
    },
    colors::{
        LIGHT_GREEN, LIGHT_GREY, ORANGE, STATS_TITLE, TOOLTIP_BLACK_2, WHITE, YELLOW, YELLOW_2,
    },
    combat::damage_tracker::{
        spawn_damage_tracker_ui, spawn_mob_stat_tracker_ui, DamageTracker, MobStatTracker,
        PetAbilityStats,
    },
    cursor::CursorPos,
    inputs::MouselessModeState,
    inventory::{Inventory, ItemStack},
    item::{
        item_actions::{ConsumableItem, ItemActions},
        item_drop_outline::UiShadow,
        EquipmentType, WorldObject,
    },
    player::{
        skills::{Heirloom, PlayerSkills},
        stats::StatType,
        Player,
    },
    proto::proto_param::ProtoParam,
    ui::{
        game_fonts::{self as gf, paths},
        spawn_item_stack_icon, INVENTORY_EQUIPMENT_UI_SIZE, INVENTORY_UPGRADE_UI_SIZE,
        TOOLTIP_UI_SIZE,
    },
};

use super::{
    item_chest::{ItemChestUI, CHEST_CONTAINER_UI_SIZE},
    tooltip_info_boxes::{
        spawn_tooltip_info_boxes_with_resolution, TooltipInfoBoxAnchor, TooltipInfoBoxSpec,
    },
    EssenceUI, InventoryUI, ScreenResolution, UIElement, UIState, CHEST_INVENTORY_UI_SIZE,
    CRAFTING_INVENTORY_UI_SIZE, FURNACE_INVENTORY_UI_SIZE, INVENTORY_UI_SIZE,
    INV_EQUIP_PANEL_OFFSET_X, INV_SIDE_STATS_BG_ALPHA, INV_SIDE_STATS_BG_PADDING,
};


/// Panel size for `LargeTooltip*` sprites (inventory item card + consumable buff HUD hover).
pub const ITEM_TOOLTIP_LARGE_CARD_SIZE: Vec2 = Vec2::new(172., 272.);
/// Z for cursor-anchored inventory item tooltips (above the inventory panel at z=10).
const INVENTORY_CURSOR_TOOLTIP_Z: f32 = 22.;
/// Wait after an inventory item tooltip closes before showing the stats tooltip again
/// (avoids flicker when moving quickly across slots).
pub const STATS_TOOLTIP_RESPAWN_DELAY_SECS: f32 = 0.18;

/// Clamps a center-anchored tooltip's X so its half-width stays inside the game viewport.
pub fn clamp_tooltip_center_x(x: f32, half_width: f32, game_width: f32, edge_pad: f32) -> f32 {
    let half_screen = game_width * 0.5;
    let min_x = -half_screen + half_width + edge_pad;
    let max_x = half_screen - half_width - edge_pad;
    x.clamp(min_x, max_x)
}

/// Clamps a center-anchored tooltip's Y so its half-height stays inside the game viewport.
pub fn clamp_tooltip_center_y(y: f32, half_height: f32, game_height: f32, edge_pad: f32) -> f32 {
    let half_screen = game_height * 0.5;
    let min_y = -half_screen + half_height + edge_pad;
    let max_y = half_screen - half_height - edge_pad;
    y.clamp(min_y, max_y)
}

/// Inventory item tooltip position beside an anchor point (cursor or slot center).
///
/// Prefers the **right** side of the anchor when there is room; falls back to the left.
/// The card is vertically centered on the anchor row with a slight upward bias. Top-edge
/// placement was collapsing most rows to one Y because the large item card is taller than
/// half the viewport.
pub fn inventory_item_tooltip_anchor_offset(
    anchor: Vec2,
    tooltip_size: Vec2,
    game_width: f32,
    game_height: f32,
) -> Vec2 {
    inventory_item_tooltip_anchor_offset_sided(
        anchor,
        tooltip_size,
        game_width,
        game_height,
        false,
        12.,
    )
}

/// Like [`inventory_item_tooltip_anchor_offset`], but can prefer the left side and use a
/// custom horizontal gap (e.g. well shrine salvage slot).
pub fn inventory_item_tooltip_anchor_offset_sided(
    anchor: Vec2,
    tooltip_size: Vec2,
    game_width: f32,
    game_height: f32,
    prefer_left: bool,
    horizontal_gap: f32,
) -> Vec2 {
    const EDGE_PAD: f32 = 8.;
    /// Anchor sits this fraction below the tooltip top (upper third of the card).
    const ANCHOR_FRAC_FROM_TOP: f32 = 0.22;
    let half_w = tooltip_size.x * 0.5;
    let half_h = tooltip_size.y * 0.5;
    let screen_half_w = game_width * 0.5;

    let right_x = anchor.x + half_w + horizontal_gap;
    let left_x = anchor.x - half_w - horizontal_gap;
    let x = if prefer_left {
        if left_x - half_w >= -screen_half_w + EDGE_PAD {
            left_x
        } else if right_x + half_w <= screen_half_w - EDGE_PAD {
            right_x
        } else {
            left_x
        }
    } else if right_x + half_w <= screen_half_w - EDGE_PAD {
        right_x
    } else if left_x - half_w >= -screen_half_w + EDGE_PAD {
        left_x
    } else {
        right_x
    };

    let anchor_bias = tooltip_size.y * ANCHOR_FRAC_FROM_TOP - half_h;
    let y = clamp_tooltip_center_y(anchor.y + anchor_bias, half_h, game_height, EDGE_PAD);

    Vec2::new(clamp_tooltip_center_x(x, half_w, game_width, EDGE_PAD), y)
}

/// Chooses the tooltip anchor for inventory item cards.
///
/// Mouse hover keeps X on the cursor while Y tracks the hovered slot when known. Controller /
/// mouseless focus uses the full slot center so the card stays beside the focused slot.
pub fn inventory_item_tooltip_placement_anchor(
    cursor: Vec2,
    slot_anchor: Option<Vec2>,
    focus_driving: bool,
) -> Vec2 {
    if focus_driving {
        slot_anchor.unwrap_or(cursor)
    } else {
        Vec2::new(cursor.x, slot_anchor.map(|a| a.y).unwrap_or(cursor.y))
    }
}

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

#[derive(Debug, Clone, Default, Message)]
pub struct ToolTipUpdateEvent {
    pub item_stack: ItemStack,
    pub is_recipe: bool,
    pub show_range: bool,
    /// Hovered inventory slot center in UI space. Y is used for mouse-hover placement; the full
    /// point anchors the card beside the slot when controller/mouseless focus is driving.
    pub anchor_ui: Option<Vec2>,
    /// When `Some`, the tooltip card is placed at exactly this `(x, y)` offset relative to
    /// the inventory / essence / item-chest parent, bypassing the `UIState`-based default
    /// position. Also marks this event as a *secondary* tooltip: the dispatcher skips the
    /// "despawn previous `ItemOrRecipeTooltip` entities" step so a secondary card can live
    /// alongside the primary hover card (used by the item chest "Currently Equipped" panel).
    pub position_override: Option<Vec2>,
    /// When `Some`, this string is rendered as a header above the tooltip card in the
    /// `TOOLTIP_ITEM_TITLE` font (alagard 15) in `DARK_WOOD_BROWN`. Used by the chest's
    /// secondary tooltip to label it "Currently Equipped".
    pub header_text: Option<String>,
    /// Optional side glossary / trigger info boxes rendered to the right of the card.
    pub info_boxes: Vec<TooltipInfoBoxSpec>,
    /// When `Some`, the tooltip card is spawned **unparented** in world space with its panel
    /// centered at this position (z included), instead of relative to an inventory/essence/chest
    /// UI root. Used by the run-start blessing choice screen, which has no inventory container.
    /// Takes precedence over `position_override`.
    pub world_anchor: Option<Vec3>,
    /// When `Some`, this `UIState` component is inserted on the spawned tooltip so the standard
    /// UI-state-exit cleanup (`handle_new_ui_state`) despawns it on leaving that state.
    pub ui_state_tag: Option<UIState>,
    /// Cauldron (`InventoryCrafting`) only: pin the card to the right of the crafting column
    /// (ingredient slots). Inventory slots use the middle crafting-column pin; blueprint
    /// recipes (`is_recipe`) use the left inventory-column pin.
    pub pin_right: bool,
}

#[derive(Debug, Clone, Default, Message)]
pub struct ShowInvPlayerStatsEvent {
    pub stat: Option<StatType>,
    pub ignore_timer: bool,
}

/// Rebuilds inventory damage/mob stat side panels without touching the player stats tooltip.
#[derive(Debug, Clone, Default, Message)]
pub struct DamageTrackerRefreshEvent;

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
        let font_size = gf::tooltip_default_size_for_font_path(font.as_str());
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

#[derive(Default, Message)]
pub struct TooltipTeardownEvent;

pub fn tick_tooltip_timer(
    time: Res<Time>,
    mut tooltip_manager: ResMut<TooltipsManager>,
    mut stats_event: MessageWriter<ShowInvPlayerStatsEvent>,
    cur_ui_state: Res<State<UIState>>,
) {
    if !tooltip_manager.timer.is_finished() {
        tooltip_manager.timer.tick(time.delta());
    }
    if let Some(ref mut delay) = tooltip_manager.stats_respawn_delay {
        delay.tick(time.delta());
        if delay.is_finished() {
            tooltip_manager.stats_respawn_delay = None;
            if *cur_ui_state.get() == UIState::Inventory {
                stats_event.write(ShowInvPlayerStatsEvent {
                    stat: None,
                    ignore_timer: true,
                });
            }
        }
    }
}

pub fn handle_tooltip_teardown(
    mut commands: Commands,
    mut updates: MessageReader<TooltipTeardownEvent>,
    tooltip: Query<Entity, With<ItemOrRecipeTooltip>>,
    mut tooltip_manager: ResMut<TooltipsManager>,
    inv: Query<&Inventory>,

    mut tooltip_update_events: MessageWriter<ToolTipUpdateEvent>,
    cur_ui_state: Res<State<UIState>>,
) {
    if updates.read().next().is_some() {
        let Ok(inv) = inv.single() else {
            return;
        };
        if let Some(item) = &inv.furnace_items.items[1] {
            tooltip_update_events.write(ToolTipUpdateEvent {
                item_stack: item.item_stack.clone(),
                is_recipe: false,
                show_range: false,
                ..Default::default()
            });
        } else {
            let had_tooltips = tooltip.iter().next().is_some();
            for t in tooltip.iter() {
                commands.entity(t).despawn();
            }
            if had_tooltips {
                tooltip_manager.timer.reset();
            }
        }
    }
}

pub fn handle_spawn_inv_item_tooltip(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    mut updates: MessageReader<ToolTipUpdateEvent>,
    inv: Query<Entity, With<InventoryUI>>,
    essence: Query<Entity, With<EssenceUI>>,
    item_chest: Query<Entity, With<ItemChestUI>>,
    cur_inv_state: Res<State<UIState>>,
    proto: ProtoParam,
    old_tooltips: Query<Entity, With<ItemOrRecipeTooltip>>,
    player_inv: Query<&Inventory, With<Player>>,
    mut tooltip_manager: ResMut<TooltipsManager>,
    cursor_pos: Res<CursorPos>,
    mouseless: Res<MouselessModeState>,
    resolution: Res<ScreenResolution>,
) {
    for item in updates.read() {
        let asset_server = asset_server.as_ref();
        tooltip_manager.stats_respawn_delay = None;
        // Secondary tooltips (e.g. the chest's "Currently Equipped" side card) explicitly
        // opt-in via `position_override` and must NOT despawn the primary card alongside
        // them. Primary tooltips clear any previous card before rendering.
        if item.position_override.is_none() {
            for t in old_tooltips.iter() {
                commands.entity(t).despawn();
            }
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
        let parent_offset = if let Some(world) = item.world_anchor {
            world.truncate()
        } else if let Some(p) = item.position_override {
            p
        } else {
            match *cur_inv_state.get() {
                UIState::Inventory => {
                    let focus_driving = mouseless.0 || cursor_pos.suppress_ui_hover;
                    let tooltip_anchor = inventory_item_tooltip_placement_anchor(
                        cursor_pos.ui_coords.truncate(),
                        item.anchor_ui,
                        focus_driving,
                    );
                    inventory_item_tooltip_anchor_offset(
                        tooltip_anchor,
                        ITEM_TOOLTIP_LARGE_CARD_SIZE,
                        resolution.game_width,
                        resolution.game_height,
                    )
                }
                UIState::InventoryCrafting => {
                    // Blueprints → left (inventory column). Inventory → middle crafting column.
                    // Ingredients → right column (`pin_right`).
                    if item.pin_right {
                        right_side_offset
                    } else if item.is_recipe {
                        Vec2::ZERO
                    } else {
                        Vec2::new(INV_EQUIP_PANEL_OFFSET_X, 0.)
                    }
                }
                UIState::Chest => Vec2::new(
                    -CHEST_INVENTORY_UI_SIZE.x - 20.,
                    -CHEST_INVENTORY_UI_SIZE.y / 2. + 40.,
                ),
                UIState::Crafting => CRAFTING_INVENTORY_UI_SIZE,
                UIState::Furnace => FURNACE_INVENTORY_UI_SIZE,
                UIState::Essence => Vec2::new(
                    -CHEST_CONTAINER_UI_SIZE.x - 40.,
                    -CHEST_CONTAINER_UI_SIZE.y / 2. + 40.,
                ),
                UIState::ItemChest => Vec2::new(
                    -CHEST_INVENTORY_UI_SIZE.x - 20.,
                    -CHEST_INVENTORY_UI_SIZE.y / 2. + 40.,
                ),
                UIState::WellShrine => {
                    crate::ui::well_shrine_ui::well_shrine_fixed_tooltip_position()
                }
                _ => continue,
            }
        };

        let use_absolute_inventory_tooltip =
            *cur_inv_state.get() == UIState::Inventory && item.position_override.is_none();
        let tooltip_z = if let Some(world) = item.world_anchor {
            world.z
        } else if use_absolute_inventory_tooltip {
            INVENTORY_CURSOR_TOOLTIP_Z
        } else if *cur_inv_state.get() == UIState::WellShrine {
            // Above the well shrine full-screen overlay (container z ≈ 50).
            70.
        } else {
            10.
        };

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
                (
                    Sprite {
                        image: graphics
                            .get_ui_element_texture(item_rarity.clone().get_tooltip_ui_element()),
                        custom_size: Some(size),
                        ..Default::default()
                    },
                    Transform {
                        translation: Vec3::new(parent_offset.x, parent_offset.y, tooltip_z),
                        scale: Vec3::new(1., 1., 1.),
                        ..Default::default()
                    },
                ),
                RenderLayers::from_layers(&[3]),
                item_rarity.get_tooltip_ui_element(),
                Name::new("TOOLTIP"),
                ItemOrRecipeTooltip,
                UiShadow::tooltip_card(),
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
                if is_item_action {
                    gf::TOOLTIP_CARD_LINE.text(&asset_server, level_string, ORANGE)
                } else {
                    gf::TOOLTIP_CARD_SUBHEAD_BOLD.text(
                        &asset_server,
                        level_string,
                        item.item_stack.rarity.get_color(),
                    )
                }
                .anchor(Anchor::CENTER_LEFT)
                .with_transform(Transform {
                    translation: Vec3::new(-16., 76., 1.),
                    scale: gf::TOOLTIP_CARD_LINE.transform_scale(),
                    ..Default::default()
                }),
                Name::new("TOOLTIP Rarity TEXT"),
                RenderLayers::from_layers(&[3]),
            ))
            .insert(ChildOf(tooltip))
            .id();

        // Accessories don't have base stats (health/defence/attack/speed). To keep their
        // tooltip clean we collapse to a single "Bonus Stats" section instead of showing an
        // empty "Base Stats" header followed by all the bonus rolls.
        let is_accessory_tooltip = equip_type.map_or(false, |e| e.is_accessory());

        // ======== header (Base Stats / Bonus Stats / Description — recipe uses ingredient row instead) ========
        if should_show_attributes {
            let _header_text = commands
                .spawn((
                    gf::TOOLTIP_CARD_SUBHEAD_BOLD
                        .text(
                            &asset_server,
                            if is_accessory_tooltip {
                                "Bonus Stats"
                            } else {
                                "Base Stats"
                            },
                            YELLOW_2,
                        )
                        .anchor(Anchor::CENTER_LEFT)
                        .with_transform(Transform {
                            translation: Vec3::new(-57., 20., 1.),
                            scale: gf::TOOLTIP_CARD_SUBHEAD_BOLD.transform_scale(),
                            ..Default::default()
                        }),
                    Name::new("TOOLTIP Rarity TEXT"),
                    RenderLayers::from_layers(&[3]),
                ))
                .insert(ChildOf(tooltip))
                .id();
        } else if !item.is_recipe {
            let _header_text = commands
                .spawn((
                    gf::TOOLTIP_CARD_SUBHEAD_BOLD
                        .text(&asset_server, "Description", YELLOW_2)
                        .anchor(Anchor::CENTER_LEFT)
                        .with_transform(Transform {
                            translation: Vec3::new(-58., -36., 1.),
                            scale: gf::TOOLTIP_CARD_SUBHEAD_BOLD.transform_scale(),
                            ..Default::default()
                        }),
                    Name::new("TOOLTIP Rarity TEXT"),
                    RenderLayers::from_layers(&[3]),
                ))
                .insert(ChildOf(tooltip))
                .id();
            // Consumable usage hint — only for proto-tagged consumables.
            if proto.get_component::<ConsumableItem, _>(obj_type).is_some() {
                let consume_text = vec!["Place in Hotbar or", "Right-Click to", "Consume Now!"];
                for (i, text) in consume_text.iter().enumerate() {
                    let _instructions_to_consume = commands
                        .spawn((
                            gf::TOOLTIP_CARD_SUBHEAD_BOLD
                                .text(&asset_server, text.to_string(), YELLOW_2)
                                .anchor(Anchor::CENTER_LEFT)
                                .with_transform(Transform {
                                    translation: Vec3::new(-58., 16. - (i as f32 * 10.), 1.),
                                    scale: gf::TOOLTIP_CARD_SUBHEAD_BOLD.transform_scale(),
                                    ..Default::default()
                                }),
                            Name::new("TOOLTIP Rarity TEXT"),
                            RenderLayers::from_layers(&[3]),
                        ))
                        .insert(ChildOf(tooltip))
                        .id();
                }
            }
        }
        // ======== rarity ========
        let _rarity_text = commands
            .spawn((
                gf::TOOLTIP_CARD_LINE
                    .text(
                        &asset_server,
                        item.item_stack.rarity.get_name(),
                        item.item_stack.rarity.get_color(),
                    )
                    .anchor(Anchor::CENTER_LEFT)
                    .with_transform(Transform {
                        translation: Vec3::new(
                            -16.,
                            if is_upgrade_material { 68. } else { 62. },
                            1.,
                        ),
                        scale: gf::TOOLTIP_CARD_LINE.transform_scale(),
                        ..Default::default()
                    }),
                Name::new("TOOLTIP Rarity TEXT"),
                RenderLayers::from_layers(&[3]),
            ))
            .insert(ChildOf(tooltip))
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
                gf::TOOLTIP_CARD_LINE
                    .text(
                        &asset_server,
                        type_string.to_string(),
                        item.item_stack.rarity.get_color(),
                    )
                    .anchor(Anchor::CENTER_LEFT)
                    .with_transform(Transform {
                        translation: Vec3::new(-16., 52., 1.),
                        scale: gf::TOOLTIP_CARD_LINE.transform_scale(),
                        ..Default::default()
                    }),
                Name::new("TOOLTIP Rarity TEXT"),
                RenderLayers::from_layers(&[3]),
            ))
            .insert(ChildOf(tooltip))
            .id();

        if should_show_attributes {
            // Skip the second "Bonus Stats" header for accessories — their stats render
            // directly under the top header (which we've already retitled to "Bonus Stats").
            if !is_accessory_tooltip {
                //======== Header 2 ========
                let _text = commands
                    .spawn((
                        gf::TOOLTIP_CARD_SUBHEAD_BOLD
                            .text(&asset_server, "Bonus Stats".to_string(), YELLOW_2)
                            .anchor(Anchor::CENTER_LEFT)
                            .with_transform(Transform {
                                translation: Vec3::new(-58., -34., 1.),
                                scale: gf::TOOLTIP_CARD_SUBHEAD_BOLD.transform_scale(),
                                ..Default::default()
                            }),
                        Name::new("TOOLTIP Rarity TEXT"),
                        RenderLayers::from_layers(&[3]),
                    ))
                    .insert(ChildOf(tooltip))
                    .id();
            }

            // Number of entries that belong to the "Base Stats" section. Base entries are
            // emitted first by `get_tooltips_from_stat_lines` and have a non-empty range like
            // "(X-Y)"; weapons also emit an "Attacks / sec" row with no range. Everything after
            // that is a bonus roll and is pushed down past the "Bonus Stats" header.
            //
            // We can't hardcode this to 2 (health + defence) because metal armor has a 3rd
            // base attribute (Speed). Accessories are rendered as a single bonus section so
            // we force their base count to 0 here.
            let num_base_stats = if is_accessory_tooltip {
                0
            } else {
                let mut count = 0;
                for (attr_name, range_text, _) in attributes.iter() {
                    if !range_text.is_empty() || attr_name.contains("Attacks / sec") {
                        count += 1;
                    } else {
                        break;
                    }
                }
                count
            };
            // The bonus section anchors at a fixed y irrespective of how many base rows came
            // before it (otherwise metal armor's 3rd base row leaves a one-row gap at the top
            // of the bonus list). Position formula: text_y = base_y - i*y_spacing - d.
            // We want bonus row j (= i - num_base_stats) to land at base_y - 54 - j*y_spacing
            // (54 = original 2 base rows at 9px + 36 header gap). Solving for d:
            //   d = 54 - num_base_stats * 9  (when i >= num_base_stats)
            // Accessories use the single-section layout so all rows render in the upper slot
            // with d=0.
            let bonus_section_offset = if is_accessory_tooltip {
                0.
            } else {
                54. - num_base_stats as f32 * 9.
            };
            for (i, (a, range, q)) in attributes.iter().enumerate().clone() {
                let d = if i >= num_base_stats {
                    bonus_section_offset
                } else {
                    0.
                };
                tooltip_text.push(TooltipTextProps::new(
                    vec![a.to_string(), range.to_string()],
                    d,
                    *q,
                    Anchor::CENTER_LEFT,
                    paths::SLKSCR.to_string(),
                ));
            }
        } else {
            if item.is_recipe {
                //======== "Description" sub-header (body text is white below) ========
                let _text = commands
                    .spawn((
                        gf::TOOLTIP_CARD_SUBHEAD_BOLD
                            .text(&asset_server, "Description".to_string(), YELLOW_2)
                            .anchor(Anchor::CENTER_LEFT)
                            .with_transform(Transform {
                                translation: Vec3::new(-58., -36., 1.),
                                scale: gf::TOOLTIP_CARD_SUBHEAD_BOLD.transform_scale(),
                                ..Default::default()
                            }),
                        Name::new("TOOLTIP Rarity TEXT"),
                        RenderLayers::from_layers(&[3]),
                    ))
                    .insert(ChildOf(tooltip))
                    .id();
            } else {
                tooltip_text.push(TooltipTextProps::new(
                    vec!["".to_string()],
                    0.,
                    AttributeQuality::Low,
                    Anchor::CENTER_LEFT,
                    paths::SLKSCR.to_string(),
                ));
            }
            for (i, desc_string) in item.item_stack.metadata.desc.iter().enumerate() {
                tooltip_text.push(TooltipTextProps::new(
                    vec![desc_string.to_string()],
                    -10. + if item.is_recipe {
                        65. + 6. * (i) as f32
                    } else {
                        65. + 6. * (i) as f32
                    },
                    if item.is_recipe {
                        AttributeQuality::Low
                    } else {
                        AttributeQuality::Average
                    },
                    Anchor::CENTER_LEFT,
                    paths::SLKSCR.to_string(),
                ));
            }
        }
        let y_spacing = if !should_show_attributes || item.is_recipe {
            5.
        } else {
            9.
        };
        for (i, props) in tooltip_text.iter().enumerate() {
            let text_pos = Vec3::new(
                -size.x / 2. + 28.,
                size.y / 2. - 126. - (i as f32 * y_spacing) - props.offset,
                2.,
            );

            for (j, t) in props.text.clone().iter().enumerate() {
                if !item.show_range && j == 1 {
                    continue;
                }
                let text = commands
                    .spawn((
                        gf::TOOLTIP_CARD_LINE
                            .text(
                                &asset_server,
                                t,
                                if j == 1 {
                                    WHITE
                                } else {
                                    match props.quality {
                                        AttributeQuality::Low | AttributeQuality::Average => WHITE,
                                        AttributeQuality::High => YELLOW,
                                    }
                                },
                            )
                            .anchor(if j == 0 {
                                props.anchor.clone()
                            } else {
                                Anchor::CENTER_RIGHT
                            })
                            .with_transform(Transform {
                                translation: text_pos
                                    + Vec3::new(
                                        if j == 1 { TOOLTIP_UI_SIZE.x - 34. } else { 0. },
                                        0.,
                                        0.,
                                    ),
                                scale: gf::TOOLTIP_CARD_LINE.transform_scale(),
                                ..Default::default()
                            }),
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
                    // Use the retained handles from Graphics so the asset + atlas
                    // stay resident; loading on demand lets it unload between
                    // tooltips, which randomly makes the highlight fail to appear.
                    let (handle, idle_tag) = match item.item_stack.rarity {
                        ItemRarity::Common => (
                            graphics.inv_stat_highlight_common_ase.clone(),
                            InventoryStatHighlightCommon::tags::IDLE,
                        ),
                        ItemRarity::Uncommon => (
                            graphics.inv_stat_highlight_uncommon_ase.clone(),
                            InventoryStatHighlightUncommon::tags::IDLE,
                        ),
                        ItemRarity::Rare => (
                            graphics.inv_stat_highlight_rare_ase.clone(),
                            InventoryStatHighlightRare::tags::IDLE,
                        ),
                        ItemRarity::Legendary => (
                            graphics.inv_stat_highlight_legendary_ase.clone(),
                            InventoryStatHighlightLegendary::tags::IDLE,
                        ),
                    };
                    commands.spawn((
                        aseprite_bundle(
                            handle.unwrap_or_default(),
                            idle_tag,
                            Transform::from_translation(box_pos),
                            Visibility::Inherited,
                            false,
                        ),
                        RenderLayers::from_layers(&[3]),
                        ChildOf(tooltip),
                    ));
                }
            }
        }

        // ======== Set Bonus line (gear sets like Leather/Metal/Forest) ========
        if let Some(set) = EquipmentSet::from_world_object(obj_type) {
            let count = if let Ok(player_inv) = player_inv.single() {
                set.count_equipped(player_inv)
            } else {
                0
            };
            let active = count >= SET_PIECES_REQUIRED;
            let label = format!(
                "Set ({}/{}): {}",
                count.min(SET_PIECES_REQUIRED),
                SET_PIECES_REQUIRED,
                set.bonus_description(),
            );
            // Place the line below the rendered tooltip rows. Mirrors the
            // bonus-stat row offset (36) plus a small gap (8) for separation.
            let row_count = tooltip_text.len() as f32;
            let set_bonus_y = size.y / 2. - 126. - (row_count * 9.) - 44.;
            commands
                .spawn((
                    gf::TOOLTIP_CARD_LINE
                        .text(
                            &asset_server,
                            label,
                            if active { LIGHT_GREEN } else { LIGHT_GREY },
                        )
                        .anchor(Anchor::CENTER_LEFT)
                        .with_transform(Transform {
                            translation: Vec3::new(-size.x / 2. + 26., set_bonus_y, 2.),
                            scale: gf::TOOLTIP_CARD_LINE.transform_scale(),
                            ..Default::default()
                        }),
                    Name::new("TOOLTIP Set Bonus TEXT"),
                    RenderLayers::from_layers(&[3]),
                ))
                .insert(ChildOf(tooltip));
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
            commands.entity(star).insert(ChildOf(tooltip));
        }
        // Optional header rendered above the card (e.g. "Currently Equipped" on the chest
        // secondary tooltip). Parented to the tooltip sprite so it inherits transform +
        // teardown lifecycle.
        if let Some(header) = item.header_text.as_ref() {
            commands
                .spawn((
                    gf::TOOLTIP_ITEM_TITLE
                        .text(&asset_server, header.clone(), WHITE)
                        .justify(Justify::Center)
                        .anchor(Anchor::CENTER)
                        .with_transform(Transform {
                            translation: Vec3::new(0., size.y / 2. + 12., 2.),
                            scale: gf::TOOLTIP_ITEM_TITLE.transform_scale(),
                            ..default()
                        }),
                    RenderLayers::from_layers(&[3]),
                    Name::new("TOOLTIP HEADER"),
                ))
                .insert(ChildOf(tooltip));
        }

        if let Some(tag) = item.ui_state_tag.clone() {
            commands.entity(tooltip).insert(tag);
        }

        // add tooltip to inventory, essence, or item chest ui
        if item.world_anchor.is_some() {
            // World-space tooltip (e.g. blessing choice screen); stays unparented.
        } else if use_absolute_inventory_tooltip {
            // Absolute UI-space position near the cursor; stats panel stays visible.
        } else if let Ok(inv) = inv.single() {
            commands.entity(inv).add_child(tooltip);
        } else if let Ok(essence) = essence.single() {
            commands.entity(essence).add_child(tooltip);
        } else if let Ok(chest) = item_chest.single() {
            commands.entity(chest).add_child(tooltip);
        }

        if !item.info_boxes.is_empty() {
            if let Some(info_root) = spawn_tooltip_info_boxes_with_resolution(
                &mut commands,
                &graphics,
                &asset_server,
                &resolution,
                TooltipInfoBoxAnchor {
                    center: Vec3::new(parent_offset.x, parent_offset.y, tooltip_z),
                    half_width: size.x * 0.5,
                    half_height: size.y * 0.5,
                    game_width: resolution.game_width,
                },
                &item.info_boxes,
            ) {
                commands.entity(info_root).insert(ChildOf(tooltip));
            }
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
    mut updates: MessageReader<ShowInvPlayerStatsEvent>,
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
            &HealthRegen,
            &ManaRegenTimer,
            &HealthRegenTimer,
            &PlayerSkills,
        ),
        With<Player>,
    >,
    inv: Query<Entity, With<InventoryUI>>,
    ui_state: Res<State<UIState>>,
    mut tooltip_manager: ResMut<TooltipsManager>,
    old_tooltips: Query<Entity, With<PlayerStatsTooltip>>,
) {
    if *ui_state == UIState::Closed {
        let d = tooltip_manager.timer.duration();
        tooltip_manager.timer.tick(d);
        return;
    }
    let pending_updates: Vec<_> = updates.read().collect();
    if !pending_updates.is_empty()
        && (tooltip_manager.timer.is_finished() || pending_updates[0].ignore_timer)
    {
        for t in old_tooltips.iter() {
            commands.entity(t).despawn();
        }
        tooltip_manager.timer.reset();
        let (Ok(parent_e), translation) = (if *curr_ui_state.get() == UIState::Inventory {
            (
                inv.single(),
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

        let Ok((
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
            health_regen,
            mana_regen_timer,
            health_regen_timer,
            skills,
        )) = player_stats.single()
        else {
            return;
        };

        let mp_regen_period_secs = effective_regen_period_secs(
            mana_regen_timer.0.duration().as_secs_f32(),
            skills.get_count(Heirloom::MPRegenCooldown),
        );
        let hp_regen_period_secs = effective_regen_period_secs(
            health_regen_timer.0.duration().as_secs_f32(),
            skills.get_count(Heirloom::HPRegenCooldown),
        );

        let mut attributes = ItemAttributes {
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
            health_regen: AttributeValue::new(health_regen.0, AttributeQuality::Low, 0.),
            ..Default::default()
        }
        .get_stats_summary(
            curr_health.0,
            curr_mana.0,
            Some(mp_regen_period_secs),
            Some(hp_regen_period_secs),
        );
        attributes.push(skills.poison_chance_stat_summary());

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

/// Value column uses bold (`slkscrbold`); parenthetical bits (mitigation %, damage mult `x`, regen `(Ns)`)
/// and the dodge `%` use regular `slkscr`.
fn stat_tooltip_value_text(
    value: &str,
    bold_font: Handle<Font>,
    regular_font: Handle<Font>,
) -> Vec<(String, Handle<Font>)> {
    const SPACE_OPEN_PAREN: &str = " (";
    if let Some(pos) = value.find(SPACE_OPEN_PAREN) {
        let after = &value[pos + SPACE_OPEN_PAREN.len()..];
        // Defence `42 (37%)` or attack `123 (1.30x)` — trailing non-bold parenthetical.
        let is_mitigation = after.ends_with(')') && after.contains('%') && !after.contains('x');
        let is_damage_mult = after.ends_with(')') && after.contains('x') && !after.contains('%');
        // Regen cooldown `15 (2.3s)` — same split; require numeric `Ns` so `(10 stacks)` etc. stay bold-only.
        let is_regen_cd_secs = after.ends_with("s)")
            && !after.contains('%')
            && !after.contains('x')
            && after
                .strip_suffix("s)")
                .and_then(|inner| inner.parse::<f32>().ok())
                .is_some();
        if is_mitigation || is_damage_mult || is_regen_cd_secs {
            return vec![
                (value[..pos].to_string(), bold_font),
                (value[pos..].to_string(), regular_font),
            ];
        }
    }
    if value.len() > 1 && value.ends_with('%') && !value.contains('(') {
        return vec![
            (value[..value.len() - 1].to_string(), bold_font),
            ("%".to_string(), regular_font),
        ];
    }
    vec![(value.to_string(), bold_font)]
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
            (
                Sprite {
                    image: graphics.get_ui_element_texture(UIElement::StatTooltip),
                    custom_size: Some(TOOLTIP_UI_SIZE),
                    ..Default::default()
                },
                Transform {
                    translation,
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
            ),
            RenderLayers::from_layers(&[3]),
            UIElement::StatTooltip,
            PlayerStatsTooltip,
            Name::new("TOOLTIP"),
            UiShadow::container(),
        ))
        .id();

    let stat_value_font_bold = gf::TOOLTIP_HEADER_BOLD.load_font(&asset_server);
    let stat_value_font_regular = gf::TOOLTIP_BODY.load_font(&asset_server);

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
                if i == 0 {
                    gf::STATS_TOOLTIP_TITLE_ROW.text(&asset_server, text.0.to_string(), STATS_TITLE)
                } else {
                    gf::STATS_TOOLTIP_ROW_NAME.text(&asset_server, text.0.to_string(), YELLOW_2)
                }
                .anchor(Anchor::CENTER_LEFT)
                .with_transform(Transform {
                    translation: text_pos,
                    scale: if i == 0 {
                        gf::STATS_TOOLTIP_TITLE_ROW.transform_scale()
                    } else {
                        gf::STATS_TOOLTIP_ROW_NAME.transform_scale()
                    },
                    ..Default::default()
                }),
                Name::new("TOOLTIP TEXT"),
                RenderLayers::from_layers(&[3]),
            ))
            .id();
        commands.entity(tooltip).add_child(_text_att_name);
        let value_spans = stat_tooltip_value_text(
            &text.1,
            stat_value_font_bold.clone(),
            stat_value_font_regular.clone(),
        );
        let mut value_commands = commands
            .spawn((
                gf::STATS_TOOLTIP_ROW_VALUE
                    .text(&asset_server, "", YELLOW_2)
                    .justify(Justify::Right)
                    .anchor(Anchor::CENTER_RIGHT)
                    .with_transform(Transform {
                        translation: text_pos
                            + Vec3::new(
                                TOOLTIP_UI_SIZE.x - 2. * STAT_TOOLTIP_INNER_PAD_X - 6.,
                                0.,
                                0.,
                            ),
                        scale: gf::STATS_TOOLTIP_ROW_VALUE.transform_scale(),
                        ..Default::default()
                    }),
                Name::new("TOOLTIP TEXT"),
                RenderLayers::from_layers(&[3]),
            ));
        value_commands.with_children(|parent| {
            for (value, font) in value_spans {
                parent.spawn((
                    TextSpan::new(value),
                    TextFont {
                        font: font.into(),
                        font_size: gf::STATS_TOOLTIP_ROW_VALUE.text_font(&asset_server).font_size,
                        font_smoothing: gf::STATS_TOOLTIP_ROW_VALUE
                            .text_font(&asset_server)
                            .font_smoothing,
                        ..default()
                    },
                    TextColor(YELLOW_2),
                ));
            }
        });
        let _text_att_value = value_commands.id();
        commands.entity(tooltip).add_child(_text_att_value);
    }
    commands.entity(parent).add_child(tooltip);
    tooltip
}

#[derive(Component)]
pub struct InventorySideStatsPanel;

fn spawn_inventory_damage_tracker_panels(
    commands: &mut Commands,
    asset_server: &AssetServer,
    inv_entity: Entity,
    old_panels: &Query<Entity, With<InventorySideStatsPanel>>,
    tracker: &DamageTracker,
    mob_tracker: &MobStatTracker,
    pet_stats: Option<&PetAbilityStats>,
) {
    for e in old_panels.iter() {
        commands.entity(e).despawn();
    }

    let panel_x = (INVENTORY_UI_SIZE.x
        + TOOLTIP_UI_SIZE.x
        + INVENTORY_UPGRADE_UI_SIZE.x
        + INVENTORY_EQUIPMENT_UI_SIZE.x
        + TOOLTIP_UI_SIZE.x
        + 86.)
        / 2.;
    let start_y = INVENTORY_UI_SIZE.y / 2. - 8.;
    let stats_width = 80.0;
    let mut next_y = start_y;
    let mut bottom_y = start_y;

    if let Some((entities, dmg_bottom_y)) = spawn_damage_tracker_ui(
        commands,
        &asset_server,
        &tracker,
        Transform::from_translation(Vec3::new(panel_x, next_y, 2.)),
        1.0,
        stats_width,
        pet_stats.as_deref(),
    ) {
        if let Some(panel) = entities.first() {
            commands.entity(*panel).insert(InventorySideStatsPanel);
            commands.entity(inv_entity).add_child(*panel);
        }
        bottom_y = next_y + dmg_bottom_y;
        next_y = bottom_y - 10.0;
    }

    if let Some((entities, mob_bottom_y)) = spawn_mob_stat_tracker_ui(
        commands,
        &asset_server,
        &mob_tracker,
        Transform::from_translation(Vec3::new(panel_x, next_y, 2.)),
        1.0,
        stats_width,
    ) {
        if let Some(panel) = entities.first() {
            commands.entity(*panel).insert(InventorySideStatsPanel);
            commands.entity(inv_entity).add_child(*panel);
        }
        bottom_y = next_y + mob_bottom_y;
    }

    let content_height = (start_y - bottom_y).abs();
    if content_height > 0.0 {
        let bg_w = stats_width + INV_SIDE_STATS_BG_PADDING * 2.0;
        let bg_h = content_height + INV_SIDE_STATS_BG_PADDING * 2.0;
        let bg_center_y = (start_y + bottom_y) * 0.5;
        let bg = commands
            .spawn((
                (
                    Sprite {
                        color: Color::srgba(0.15, 0.12, 0.10, INV_SIDE_STATS_BG_ALPHA),
                        custom_size: Some(Vec2::new(bg_w, bg_h)),
                        ..Default::default()
                    },
                    Transform::from_translation(Vec3::new(panel_x + 6., bg_center_y, 1.5)),
                ),
                RenderLayers::from_layers(&[3]),
                InventorySideStatsPanel,
                Name::new("INVENTORY SIDE STATS BACKGROUND"),
            ))
            .id();
        commands.entity(inv_entity).add_child(bg);
    }
}

pub fn spawn_damage_tracker_in_inventory(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut stats_updates: MessageReader<ShowInvPlayerStatsEvent>,
    mut tracker_refresh: MessageReader<DamageTrackerRefreshEvent>,
    inv: Query<Entity, With<InventoryUI>>,
    ui_state: Res<State<UIState>>,
    damage_tracker_menu: Res<crate::inventory::DamageTrackerMenuOpen>,
    old_panels: Query<Entity, With<InventorySideStatsPanel>>,
    tracker: Res<DamageTracker>,
    mob_tracker: Res<MobStatTracker>,
    pet_stats: Option<Res<PetAbilityStats>>,
) {
    if *ui_state != UIState::Inventory {
        return;
    }
    let should_refresh =
        tracker_refresh.read().next().is_some() || stats_updates.read().next().is_some();
    if !should_refresh {
        return;
    }

    if !damage_tracker_menu.0 {
        for e in old_panels.iter() {
            commands.entity(e).despawn();
        }
        return;
    }

    let Ok(inv_entity) = inv.single() else {
        return;
    };

    spawn_inventory_damage_tracker_panels(
        &mut commands,
        &asset_server,
        inv_entity,
        &old_panels,
        &tracker,
        &mob_tracker,
        pet_stats.as_deref(),
    );
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
            gf::TOOLTIP_ITEM_TITLE
                .text(
                    &asset_server,
                    item_stack.metadata.name.clone(),
                    item_stack.rarity.get_color(),
                )
                .anchor(Anchor::CENTER)
                .with_transform(Transform {
                    translation: Vec3::new(-ITEM_TOOLTIP_LARGE_CARD_SIZE.x / 2. + 85., 101., 1.),
                    scale: gf::TOOLTIP_ITEM_TITLE.transform_scale(),
                    ..Default::default()
                }),
            Name::new("TOOLTIP Rarity TEXT"),
            RenderLayers::from_layers(&[3]),
        ))
        .insert(ChildOf(tooltip));
}

/// World-space offset from HUD icon anchor to the large item tooltip panel center
/// (tuned so the card sits above the icon with its bottom clearing the slot).
pub fn world_item_tooltip_hud_anchor_offset() -> Vec3 {
    let size = ITEM_TOOLTIP_LARGE_CARD_SIZE;
    let legacy_panel_h = 120.;
    Vec3::new(-size.x / 2., 55. + (size.y - legacy_panel_h) / 2., 20.)
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

    let panel_center_offset = world_item_tooltip_hud_anchor_offset();

    let panel_pos = anchor_translation + panel_center_offset;

    let tooltip = commands
        .spawn((
            (
                Sprite {
                    image: graphics
                        .get_ui_element_texture(item_rarity.clone().get_tooltip_ui_element()),
                    custom_size: Some(size),
                    ..Default::default()
                },
                Transform {
                    translation: panel_pos,
                    scale: Vec3::ONE,
                    ..Default::default()
                },
            ),
            RenderLayers::from_layers(&[3]),
            item_rarity.get_tooltip_ui_element(),
            Name::new("HUD_ITEM_TOOLTIP"),
            ConsumableBuffHudTooltip,
            UiShadow::tooltip_card(),
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
                gf::TOOLTIP_CARD_LINE
                    .text(&asset_server, action_or_level, ORANGE)
                    .anchor(Anchor::CENTER_LEFT)
                    .with_transform(Transform {
                        translation: Vec3::new(-16., 78., 2.),
                        scale: gf::TOOLTIP_CARD_LINE.transform_scale(),
                        ..default()
                    }),
                RenderLayers::from_layers(&[3]),
            ))
            .insert(ChildOf(tooltip))
            .id();
    }
    for (d, line) in item_stack.metadata.desc.iter().enumerate() {
        let y = size.y / 2. - 97. - 9. * d as f32;
        let _d = commands
            .spawn((
                gf::TOOLTIP_BODY
                    .text(&asset_server, line.clone(), TOOLTIP_BLACK_2)
                    .anchor(Anchor::CENTER_LEFT)
                    .with_transform(Transform {
                        translation: Vec3::new(body_text_x, y, 2.),
                        scale: gf::TOOLTIP_BODY.transform_scale(),
                        ..default()
                    }),
                RenderLayers::from_layers(&[3]),
            ))
            .insert(ChildOf(tooltip))
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
    if let Some(_equip_type) = equip_type {
        // `total_atts` already counts only the bonus stat lines (base attributes
        // are not included in the score average), so we don't need to subtract
        // them again here. The previous formula double-subtracted and hard-capped
        // armor at 1 star and weapons at 2.
        let _ = total_atts;
        let _ = rarity;
        let num_stars = if score >= 0.72 {
            3
        } else if score > 0.6 {
            2
        } else if score > 0.47 {
            1
        } else {
            0
        };
        num_stars
    } else {
        0
    }
}
