use bevy::{prelude::*, render::view::RenderLayers, sprite::Anchor};

use bevy_aseprite::{anim::AsepriteAnimation, aseprite, Aseprite, AsepriteBundle};
use bevy_proto::prelude::ProtoCommands;

aseprite!(pub CraftingArrowAse, "textures/effects/CraftingArrow.aseprite");

/// Marker for the indicator arrow spawned over the crafting result slot when the
/// player has all the ingredients needed for the selected recipe.
#[derive(Component)]
pub struct CraftingArrowIndicator;

use crate::chaos::ChaosTracker;
use crate::colors::{
    CRAFT_BUTTON_TEXT, DARK_WOOD_BROWN, EQUIP_TITLE, HOTBAR_TITLE, RED, STATS_TITLE, YELLOW_2,
};
use crate::cursor::CursorPos;
use crate::custom_commands::CommandsExt;
use crate::night::EraTimer;
use crate::player::skills::{Heirloom, HeirloomRarity, HeirloomWithRarity, PlayerSkills};
use crate::player::unlocks::RunUnlockState;
use crate::player::ModifyCurencyEvent;
use crate::proto::proto_param::ProtoParam;
use crate::ui::{
    BLUEPRINT_PAGE_BTN_CENTER_Y, BLUEPRINT_PAGE_BTN_DOWN_X, BLUEPRINT_PAGE_BTN_SIZE,
    BLUEPRINT_PAGE_BTN_UP_X, INVENTORY_BLUEPRINT_UI_SIZE, INVENTORY_CRAFTING_PANEL_UI_SIZE,
    INVENTORY_EQUIPMENT_UI_SIZE, INVENTORY_UPGRADE_UI_SIZE, INVENTORY_Y_OFFSET,
    INV_BLUEPRINT_SLOT_CENTER_X, INV_BLUEPRINT_SLOT_ICON_X_OFFSET,
    INV_BLUEPRINT_SLOT_LABEL_X_OFFSET, INV_BLUEPRINT_SLOT_ROW_GAP, INV_BLUEPRINT_SLOT_SIZE,
    INV_BLUEPRINT_SLOT_TOP_Y, INV_CRAFTING_INPUT_SLOTS_Y_LOCAL, INV_CRAFTING_INPUT_SLOT_SPACING_X,
    INV_CRAFTING_PANEL_INGREDIENT_COUNT_Y_OFFSET, INV_CRAFTING_PANEL_INGREDIENT_ROW_Y,
    INV_CRAFTING_PANEL_INGREDIENT_SPACING_X, INV_CRAFTING_PANEL_RESULT_Y,
    INV_UPGRADE_PANEL_OFFSET_Y_CRAFTING, MAX_BLUEPRINT_ROWS, UI_UPGRADE_SLOT_SIZE,
};
use crate::world::dimension::{DimensionSpawnEvent, Era};
use crate::GameParam;
use crate::Player;
use crate::{
    assets::Graphics,
    attributes::{
        add_item_glows, attribute_helpers::create_new_random_item_stack_with_attributes,
        AttributeChangeEvent,
    },
    inventory::{
        try_auto_equip_from_upgrade_slot, BreakDropFilter, Inventory, InventoryItemStack,
        ItemStack, MaterialDropFilterAllButton, MaterialDropFilterEntry, MaterialDropFilterEntryX,
        MaterialDropFilterMenuOpen, MaterialDropFilterNoneButton, MaterialDropFilterPanel,
        MaterialDropsToggleButton, SortInventoryButton, BREAK_DROP_FILTER_ITEMS,
    },
    item::{CraftedItemEvent, Recipes, WorldObject},
    ui::{crafting_ui::UpgradeButton, FurnaceState, CHEST_INVENTORY_UI_SIZE, INVENTORY_UI_SIZE},
    ScreenResolution,
};

use super::{
    crafting_ui::CraftingContainer,
    icon_hover_tooltips::IconHoverTooltipText,
    interactions::{Interactable, Interaction},
    options_ui::CheatSettings,
    player_hud::FlashExpBarEvent,
    ui_helpers::{spawn_ui_overlay, Z_DEPTH_HUD_ACTIVE_SKILLS},
    ShowInvPlayerStatsEvent, UIContainersParam, UIElement, CRAFTING_INVENTORY_UI_SIZE,
    FURNACE_INVENTORY_UI_SIZE, HUD_ACTION_ROW_Y_FROM_BOTTOM, HUD_HOTBAR_CENTER_X, HUD_HOTBAR_SLOTS,
    INVENTORY_GRID_COLS, INV_CHEST_SCRAPPER_GRID_OFFSET_Y, INV_CRAFTING_BASE_Y, INV_CRAFTING_COLS,
    INV_CRAFTING_NUDGE_IN_MAIN_INV, INV_CRAFTING_ROW_GAP, INV_CRAFTING_X_ANCHOR,
    INV_DROP_FILTER_BUTTON_ROW_HEIGHT, INV_DROP_FILTER_BUTTON_SPACING, INV_DROP_FILTER_ICON_GAP,
    INV_DROP_FILTER_ICON_SIZE, INV_DROP_FILTER_PANEL_COLS, INV_DROP_FILTER_PANEL_GAP,
    INV_DROP_FILTER_PANEL_PADDING, INV_DROP_FILTER_PANEL_Z, INV_DROP_FILTER_TITLE_ROW_HEIGHT,
    INV_EQUIP_GRID_ROW_BOT_Y, INV_EQUIP_GRID_ROW_MID_Y, INV_EQUIP_GRID_ROW_TOP_Y,
    INV_EQUIP_GRID_SPACING, INV_EQUIP_PANEL_OFFSET_X, INV_EQUIP_PANEL_OFFSET_Y, INV_FURNACE_SLOT_0,
    INV_FURNACE_SLOT_1, INV_GRID_FIRST_ROW_NUDGE_Y, INV_GRID_INSET_BOTTOM, INV_GRID_INSET_LEFT,
    INV_MATERIAL_DROPS_TOGGLE_OFFSET_X, INV_MATERIAL_DROPS_TOGGLE_OFFSET_Y, INV_SLOT_SPACING_X,
    INV_SLOT_SPACING_Y, INV_SORT_BUTTON_OFFSET_X, INV_SORT_BUTTON_OFFSET_Y, INV_TRASH_OFFSET_X,
    INV_TRASH_OFFSET_Y, INV_UI_PARENT_OFFSET_CRAFTING, UI_SLOT_SIZE,
};

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States, Component)]
pub enum UIState {
    #[default]
    Closed,
    Inventory,
    NPC,
    Chest,
    Skills,
    ItemChest,
    ActiveSkills,
    ActiveSkillShrine,
    MicrowaveShrine,
    Crafting,
    /// Alternate inventory mode that swaps the equipment panel for the blueprints panel
    /// and the upgrade slots for three normal crafting material slots.
    /// Toggled via the CRAFT / UPGRADE button on the upgrade / crafting side panel.
    InventoryCrafting,
    Furnace,
    Essence,
    Unlocks,
    Options,
    Achievements,
    Scrapper,
    ClassSelection,
    EnterName,
    BlessingChoice,
    TimeCrystalProgress,
    /// Main-menu overview of all time crystals and their heirloom unlocks.
    TimeCrystalsBrowser,
    /// Main-menu bestiary browser: 3x3 grid of mob cards plus a detail panel.
    BeastiaryBrowser,
}
impl UIState {
    pub fn is_inv_open(&self) -> bool {
        self == &UIState::Inventory
            || self == &UIState::InventoryCrafting
            || self == &UIState::Chest
            || self == &UIState::Scrapper
            || self == &UIState::Crafting
            || self == &UIState::Furnace
    }
    /// Whether this state renders the main inventory panel (as opposed to a side-container panel).
    /// `Inventory` and `InventoryCrafting` both use the same inventory sprite + hotbar layout.
    pub fn is_main_inventory(&self) -> bool {
        self == &UIState::Inventory || self == &UIState::InventoryCrafting
    }
}

/// Event to grant an heirloom from dev mode (handled in a separate system to avoid query conflicts).
pub struct GrantHeirloomDevEvent(pub Heirloom);

#[derive(Component, Default, Clone)]
pub struct InventoryUI;

/// Dynamic prompt text shown on the upgrade panel (Inventory mode only).
/// Text switches between "Add Upgrade Material", "Use Tome", and "Use Orb" depending on which
/// consumable is sitting in furnace slot 0 (see `update_upgrade_material_prompt_text`).
#[derive(Component, Default, Clone)]
pub struct UpgradeMaterialPromptText;

/// Clickable hit area on the bottom of the upgrade / crafting side panel that toggles the UI
/// between `UIState::Inventory` (CRAFT label → switch to crafting) and
/// `UIState::InventoryCrafting` (UPGRADE label → switch back).
#[derive(Component, Default, Clone)]
pub struct CraftModeToggleButton;

/// Tag for a single row slot on the blueprints panel in `UIState::InventoryCrafting`.
/// Holds the recipe [`WorldObject`] that this blueprint produces.
#[derive(Component, Clone, Debug)]
pub struct BlueprintSlot {
    pub recipe_obj: WorldObject,
}

/// Tag for one of the three ingredient display slots on the crafting side panel in
/// `UIState::InventoryCrafting`. Purely visual — does not accept item drops.
#[derive(Component, Clone, Debug)]
pub struct CraftingIngredientDisplaySlot {
    pub slot_index: usize,
}

/// Tag for the craftable result slot that sits above the ingredient row on the crafting panel
/// in `UIState::InventoryCrafting`. Clicking the slot crafts one of the selected recipe into the
/// dragged cursor stack (see `handle_crafting_result_slot_click`).
#[derive(Component, Default, Clone, Debug)]
pub struct CraftingResultSlot;

/// Tag text child under an ingredient slot that displays "owned/needed" (e.g. "2/3").
#[derive(Component, Default, Clone, Debug)]
pub struct CraftingIngredientCountText {
    pub slot_index: usize,
}

/// Marker on the currently-rendered item icon for one of the three ingredient display slots.
/// Re-spawned whenever [`SelectedCraftingRecipe`] or the inventory changes.
#[derive(Component, Default, Clone, Debug)]
pub struct CraftingIngredientIcon {
    pub slot_index: usize,
}

/// Marker on the currently-rendered item icon inside the craft result slot.
#[derive(Component, Default, Clone, Debug)]
pub struct CraftingResultIcon;

/// Resource holding the player's currently-selected blueprint in `UIState::InventoryCrafting`.
/// Populated by clicking a blueprint row slot; cleared when the UI closes.
#[derive(Resource, Default, Clone, Debug)]
pub struct SelectedCraftingRecipe(pub Option<WorldObject>);

/// Current page (0-indexed) for the blueprints panel in `UIState::InventoryCrafting`.
/// Each page shows up to [`MAX_BLUEPRINT_ROWS`] recipes.  Navigated via the up/down arrow
/// buttons next to the panel.  Reset to 0 when the inventory closes.
#[derive(Resource, Default, Clone, Debug)]
pub struct BlueprintsPagination {
    pub page: usize,
}

/// Marker on the blueprints panel up-arrow button (page--).
#[derive(Component, Default, Clone, Debug)]
pub struct BlueprintsPrevButton;

/// Marker on the blueprints panel down-arrow button (page++).
#[derive(Component, Default, Clone, Debug)]
pub struct BlueprintsNextButton;

/// Dev-only button shown in inventory when dev mode is enabled (Options > Dev Mode).
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum DevButtonAction {
    GrantXp,
    GrantMoreXp,
    SpawnChest,
    SpawnTome,
    SpawnOrb,
    TeleportEra2,
    TeleportEra3,
    TriggerEndless,
    AddChaos,
    AddGold,
    DropDungeonKey,
    AddBanishCount,
    AddLoadedDice,
}
#[derive(Component, FromReflect, Reflect, Clone, Debug)]
pub struct InventorySlotState {
    pub slot_index: usize,
    pub item: Option<Entity>,
    pub count: Option<usize>,
    pub obj_type: Option<WorldObject>,
    pub r#type: InventorySlotType,
    pub dirty: bool,
}
#[derive(Resource, Default, Debug)]
pub struct InventoryState {
    pub inv_size: Vec2,
    pub furnace_state: FurnaceState,
}
#[derive(FromReflect, PartialEq, Reflect, Debug, Clone, Copy)]
pub enum InventorySlotType {
    Normal,
    Hotbar,
    Crafting,
    Equipment,
    Accessory,
    Weapon,
    Pet,
    Chest,
    Furnace,
    Scrapper,
    Trash,
    /// Three slots on the "CRAFTING" side panel shown in `UIState::InventoryCrafting`.
    /// Behaves like a `Normal` inventory slot (drag/drop, right-click split) but is backed by
    /// `Inventory::crafting_inputs_items` instead of the main grid.
    CraftingInput,
}
impl InventorySlotType {
    pub fn is_crafting(self) -> bool {
        self == InventorySlotType::Crafting
    }
    pub fn is_furnace(self) -> bool {
        self == InventorySlotType::Furnace
    }
    pub fn is_hotbar(self) -> bool {
        self == InventorySlotType::Hotbar
    }
    pub fn is_equipment(self) -> bool {
        self == InventorySlotType::Equipment
    }
    pub fn is_accessory(self) -> bool {
        self == InventorySlotType::Accessory
    }
    pub fn is_weapon_slot(self) -> bool {
        self == InventorySlotType::Weapon
    }
    pub fn is_pet_slot(self) -> bool {
        self == InventorySlotType::Pet
    }
    pub fn is_inventory(self) -> bool {
        self == InventorySlotType::Normal
    }
    pub fn is_chest(self) -> bool {
        self == InventorySlotType::Chest
    }
    pub fn is_scrapper(self) -> bool {
        self == InventorySlotType::Scrapper
    }
    pub fn is_trash(self) -> bool {
        self == InventorySlotType::Trash
    }
    pub fn is_crafting_input(self) -> bool {
        self == InventorySlotType::CraftingInput
    }
}
pub fn setup_inv_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    mut inv_state: ResMut<InventoryState>,
    cur_inv_state: Res<State<UIState>>,
    mut stats_event: EventWriter<ShowInvPlayerStatsEvent>,
    resolution: Res<ScreenResolution>,
    asset_server: Res<AssetServer>,
    cheat_settings: Option<Res<CheatSettings>>,
    recipes: Res<Recipes>,
    mut selected_recipe: ResMut<SelectedCraftingRecipe>,
    proto_param: ProtoParam,
    era_manager: Res<crate::world::dimension::EraManager>,
    blueprints_pagination: Res<BlueprintsPagination>,
) {
    let (size, texture, pos_offset) = match cur_inv_state.0 {
        UIState::Inventory | UIState::InventoryCrafting => (
            INVENTORY_UI_SIZE,
            graphics.get_ui_element_texture(UIElement::Inventory),
            Vec2::new(-185., INVENTORY_Y_OFFSET),
        ),
        UIState::Chest => (
            CHEST_INVENTORY_UI_SIZE,
            graphics.get_ui_element_texture(UIElement::ChestInventory),
            Vec2::new(22.5, 0.),
        ),
        UIState::Scrapper => (
            CHEST_INVENTORY_UI_SIZE,
            graphics.get_ui_element_texture(UIElement::ChestInventory),
            Vec2::new(22.5, 0.),
        ),
        UIState::Crafting => (
            CRAFTING_INVENTORY_UI_SIZE,
            graphics.get_ui_element_texture(UIElement::CraftingInventory),
            Vec2::new(22.5, INVENTORY_Y_OFFSET),
        ),
        UIState::Furnace => (
            FURNACE_INVENTORY_UI_SIZE,
            graphics.get_ui_element_texture(UIElement::FurnaceInventory),
            Vec2::new(22.5, 0.),
        ),
        _ => return,
    };

    spawn_ui_overlay(
        &mut commands,
        Vec2::new(resolution.game_width + 10., resolution.game_height + 20.),
        0.8,
        9.,
    );

    let inv = commands
        .spawn(SpriteBundle {
            texture,
            sprite: Sprite {
                custom_size: Some(size),
                ..Default::default()
            },
            transform: Transform {
                translation: Vec3::new(pos_offset.x, pos_offset.y, 10.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(InventoryUI)
        .insert(cur_inv_state.0.clone())
        .insert(Name::new("INVENTORY"))
        .insert(RenderLayers::from_layers(&[3]))
        .id();
    let _inv_text = commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                "INVENTORY",
                TextStyle {
                    font: asset_server.load("fonts/alagard.ttf"),
                    font_size: 15.0,
                    color: STATS_TITLE,
                },
            ),
            text_anchor: Anchor::Center,
            transform: Transform {
                translation: Vec3::new(3., size.y / 2. - 10., 1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("INVENTORY TITLE"))
        .insert(cur_inv_state.0.clone())
        .set_parent(inv)
        .id();
    let _hotbar_text = commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                "HOTBAR",
                TextStyle {
                    font: asset_server.load("fonts/alagard.ttf"),
                    font_size: 15.0,
                    color: HOTBAR_TITLE,
                },
            ),
            text_anchor: Anchor::Center,
            transform: Transform {
                translation: Vec3::new(3., size.y / 2. - 246., 1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("HOTBAR TITLE"))
        .insert(cur_inv_state.0.clone())
        .set_parent(inv)
        .id();

    let is_crafting_mode = cur_inv_state.0 == UIState::InventoryCrafting;
    let (side_panel_size, side_panel_element) = if is_crafting_mode {
        (INVENTORY_CRAFTING_PANEL_UI_SIZE, UIElement::CraftingPanel)
    } else {
        (INVENTORY_UPGRADE_UI_SIZE, UIElement::UpgradePanel)
    };
    let upgrade_panel_y_local = if is_crafting_mode {
        INV_UPGRADE_PANEL_OFFSET_Y_CRAFTING
    } else {
        -75.0
    };
    let upgrade_panel = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(side_panel_element.clone()),
            sprite: Sprite {
                custom_size: Some(side_panel_size),
                ..Default::default()
            },
            transform: Transform {
                translation: Vec3::new(
                    pos_offset.x + 150.,
                    pos_offset.y + upgrade_panel_y_local,
                    10.,
                ),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(cur_inv_state.0.clone())
        .insert(Name::new(if is_crafting_mode {
            "CRAFTING PANEL"
        } else {
            "UPGRADES"
        }))
        .insert(side_panel_element)
        .insert(RenderLayers::from_layers(&[3]))
        .id();
    let upgrade_title = if is_crafting_mode {
        "CRAFTING"
    } else {
        "UPGRADES"
    };
    let _upgrade_text = commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                upgrade_title,
                TextStyle {
                    font: asset_server.load("fonts/alagard.ttf"),
                    font_size: 15.0,
                    color: EQUIP_TITLE,
                },
            ),
            text_anchor: Anchor::Center,
            transform: Transform {
                translation: Vec3::new(0., side_panel_size.y / 2. - 12., 1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("upgrade/crafting TITLE"))
        .insert(cur_inv_state.0.clone())
        .set_parent(upgrade_panel)
        .id();

    // Equipment panel (only in standard Inventory mode — crafting mode hides equipment).
    if cur_inv_state.0 == UIState::Inventory {
        let equip_panel = commands
            .spawn(SpriteBundle {
                texture: graphics.get_ui_element_texture(UIElement::EquipmentPanel),
                sprite: Sprite {
                    custom_size: Some(INVENTORY_EQUIPMENT_UI_SIZE),
                    ..Default::default()
                },
                transform: Transform {
                    translation: Vec3::new(pos_offset.x + 150., pos_offset.y + 86., 10.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..Default::default()
            })
            .insert(cur_inv_state.0.clone())
            .insert(Name::new("EQUIPMENTS"))
            .insert(RenderLayers::from_layers(&[3]))
            .id();
        let _eqp_text = commands
            .spawn(Text2dBundle {
                text: Text::from_section(
                    "EQUIPMENT",
                    TextStyle {
                        font: asset_server.load("fonts/alagard.ttf"),
                        font_size: 15.0,
                        color: EQUIP_TITLE,
                    },
                ),
                text_anchor: Anchor::Center,
                transform: Transform {
                    translation: Vec3::new(0., INVENTORY_EQUIPMENT_UI_SIZE.y / 2. - 11., 1.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .insert(Name::new("eqp TITLE"))
            .insert(UIState::Inventory)
            .set_parent(equip_panel)
            .id();
    }

    // Blueprint panel (only in `InventoryCrafting` — replaces the stats tooltip column).
    if is_crafting_mode {
        // Mirror the stats tooltip positioning from `handle_spawn_inv_player_stats` so this sits
        // in exactly the same on-screen location as the stats panel it replaces.
        let blueprint_x_local = (INVENTORY_UI_SIZE.x
            + INVENTORY_BLUEPRINT_UI_SIZE.x
            + INVENTORY_UPGRADE_UI_SIZE.x
            + INVENTORY_EQUIPMENT_UI_SIZE.x
            + 8.)
            / 2.;
        let blueprint_panel = commands
            .spawn(SpriteBundle {
                texture: graphics.get_ui_element_texture(UIElement::BlueprintsPanel),
                sprite: Sprite {
                    custom_size: Some(INVENTORY_BLUEPRINT_UI_SIZE),
                    ..Default::default()
                },
                transform: Transform {
                    translation: Vec3::new(blueprint_x_local, 0., 2.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..Default::default()
            })
            .insert(cur_inv_state.0.clone())
            .insert(Name::new("BLUEPRINTS"))
            .insert(UIElement::BlueprintsPanel)
            .insert(RenderLayers::from_layers(&[3]))
            .id();
        let _bp_text = commands
            .spawn(Text2dBundle {
                text: Text::from_section(
                    "BLUEPRINTS",
                    TextStyle {
                        font: asset_server.load("fonts/alagard.ttf"),
                        font_size: 15.0,
                        color: STATS_TITLE,
                    },
                ),
                text_anchor: Anchor::Center,
                transform: Transform {
                    translation: Vec3::new(0., INVENTORY_BLUEPRINT_UI_SIZE.y / 2. - 11., 1.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .insert(Name::new("blueprint TITLE"))
            .insert(cur_inv_state.0.clone())
            .set_parent(blueprint_panel)
            .id();
        commands.entity(inv).push_children(&[blueprint_panel]);

        // Reset any previously-held selection; refreshing logic re-populates on pick.
        selected_recipe.0 = None;

        render_blueprint_rows_and_nav(
            &mut commands,
            blueprint_panel,
            &graphics,
            &asset_server,
            &proto_param,
            &recipes,
            &era_manager,
            &blueprints_pagination,
            &cur_inv_state,
        );

        // Ingredient display slots on the crafting side panel (x3). Purely visual; the
        // icon + "owned/needed" text are populated by `refresh_crafting_ingredient_display`
        // when the player clicks a blueprint row.
        for i in 0..3 {
            let local_x = 1. + (i as f32 - 1.0) * INV_CRAFTING_PANEL_INGREDIENT_SPACING_X;
            let slot_entity = commands
                .spawn(SpriteBundle {
                    texture: graphics.get_ui_element_texture(UIElement::InventorySlot),
                    sprite: Sprite {
                        custom_size: Some(UI_SLOT_SIZE),
                        color: Color::Rgba {
                            red: 0.,
                            green: 0.,
                            blue: 0.,
                            alpha: 0.,
                        },
                        ..Default::default()
                    },
                    transform: Transform {
                        translation: Vec3::new(local_x, INV_CRAFTING_PANEL_INGREDIENT_ROW_Y, 1.),
                        scale: Vec3::new(1., 1., 1.),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .insert(Name::new(format!("CRAFTING INGREDIENT SLOT {}", i)))
                .insert(UIElement::InventorySlot)
                .insert(CraftingIngredientDisplaySlot { slot_index: i })
                .insert(cur_inv_state.0.clone())
                .insert(RenderLayers::from_layers(&[3]))
                .id();

            // "owned/needed" label (4x5 font, size 5). Starts blank.
            let count_text = commands
                .spawn(Text2dBundle {
                    text: Text::from_section(
                        "",
                        TextStyle {
                            font: asset_server.load("fonts/4x5.ttf"),
                            font_size: 5.0,
                            color: Color::WHITE,
                        },
                    )
                    .with_alignment(TextAlignment::Center),
                    text_anchor: Anchor::Center,
                    transform: Transform {
                        translation: Vec3::new(
                            0.,
                            INV_CRAFTING_PANEL_INGREDIENT_COUNT_Y_OFFSET,
                            2.,
                        ),
                        scale: Vec3::new(1., 1., 1.),
                        ..Default::default()
                    },
                    ..default()
                })
                .insert(RenderLayers::from_layers(&[3]))
                .insert(CraftingIngredientCountText { slot_index: i })
                .insert(cur_inv_state.0.clone())
                .insert(Name::new("CRAFTING INGREDIENT COUNT"))
                .set_parent(slot_entity)
                .id();
            let _ = count_text;
            commands.entity(upgrade_panel).push_children(&[slot_entity]);
        }

        // Craft result slot (hotbar-style). Clicking it crafts one into the dragged cursor
        // stack via `handle_crafting_result_slot_click`.
        let result_slot = commands
            .spawn(SpriteBundle {
                texture: graphics.get_ui_element_texture(UIElement::InventorySlotHotbar),
                sprite: Sprite {
                    custom_size: Some(UI_SLOT_SIZE),
                    color: Color::Rgba {
                        red: 0.,
                        green: 0.,
                        blue: 0.,
                        alpha: 0.,
                    },
                    ..Default::default()
                },
                transform: Transform {
                    translation: Vec3::new(1., INV_CRAFTING_PANEL_RESULT_Y, 1.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..Default::default()
            })
            .insert(Name::new("CRAFTING RESULT SLOT"))
            .insert(UIElement::InventorySlotHotbar)
            .insert(Interactable::default())
            .insert(CraftingResultSlot)
            .insert(cur_inv_state.0.clone())
            .insert(RenderLayers::from_layers(&[3]))
            .id();
        commands.entity(upgrade_panel).push_children(&[result_slot]);
    }

    inv_state.inv_size = size;
    // Furnace "accept" arrow button — only meaningful in standard Inventory mode (upgrades).
    // Hidden in Scrapper (no upgrade flow) and InventoryCrafting (panel contents differ).
    if cur_inv_state.0 == UIState::Inventory {
        let upgrade_button = commands
            .spawn(SpriteBundle {
                texture: graphics
                    .get_ui_element_texture(UIElement::UpgradeButton)
                    .clone(),
                sprite: Sprite {
                    custom_size: Some(Vec2::new(100., 20.)),
                    ..Default::default()
                },
                transform: Transform {
                    translation: Vec3::new(-1., -INVENTORY_UPGRADE_UI_SIZE.y / 2. + 66., 1.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..Default::default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .insert(UIElement::UpgradeButton)
            .insert(Interactable::default())
            .insert(UIState::Inventory)
            .insert(UpgradeButton)
            .insert(Name::new("UPGRADE BUTTON"))
            .id();
        commands
            .entity(upgrade_panel)
            .push_children(&[upgrade_button]);

        let upgrade_material_text = commands
            .spawn(Text2dBundle {
                text: Text::from_section(
                    "Add Materials",
                    TextStyle {
                        font: asset_server.load("fonts/slkscrbold.ttf"),
                        font_size: 8.4,
                        color: YELLOW_2,
                    },
                ),
                text_anchor: Anchor::Center,
                transform: Transform {
                    translation: Vec3::new(0., 0., 1.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .insert(Name::new("upgrade material prompt"))
            .insert(UIState::Inventory)
            .insert(UpgradeMaterialPromptText)
            .id();
        commands
            .entity(upgrade_button)
            .push_children(&[upgrade_material_text]);
    }

    // CRAFT / UPGRADE toggle button — shown on both `Inventory` and `InventoryCrafting`
    // so the player can flip between the two side-panel layouts.  The visual is the
    // `CraftButton` sprite (which has a matching `CraftButtonHover` variant handled by the
    // hover highlight system), and the label text sits as a child of that sprite.
    let toggle_label = if is_crafting_mode { "Back" } else { "Craft" };
    let toggle_button = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::CraftButton),
            sprite: Sprite {
                custom_size: Some(Vec2::new(60., 18.)),
                ..Default::default()
            },
            transform: Transform {
                translation: Vec3::new(0., -side_panel_size.y / 2. + 36., 2.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Interactable::default())
        .insert(UIElement::CraftButton)
        .insert(CraftModeToggleButton)
        .insert(cur_inv_state.0.clone())
        .insert(Name::new("CRAFT/UPGRADE TOGGLE"))
        .id();
    let _toggle_text = commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                toggle_label,
                TextStyle {
                    font: asset_server.load("fonts/alagard.ttf"),
                    font_size: 15.0,
                    color: CRAFT_BUTTON_TEXT,
                },
            ),
            text_anchor: Anchor::Center,
            transform: Transform {
                translation: Vec3::new(0., -1., 1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("CRAFT/UPGRADE LABEL"))
        .insert(cur_inv_state.0.clone())
        .set_parent(toggle_button)
        .id();
    commands
        .entity(upgrade_panel)
        .push_children(&[toggle_button]);

    // Dev mode buttons (far left of inventory, only when Options > Dev Mode is on)
    let dev_mode = cheat_settings.map(|c| c.dev_mode).unwrap_or(false);
    if cur_inv_state.0 == UIState::Inventory && dev_mode {
        const DEV_BUTTON_WIDTH: f32 = 38.;
        const DEV_BUTTON_HEIGHT: f32 = 11.;
        const DEV_BUTTON_SPACING: f32 = 14.;
        // Left of inventory panel in local space (inv center is 22, 0.5 in world; panel half-width 109)
        let dev_x = -INVENTORY_UI_SIZE.x / 2. - DEV_BUTTON_WIDTH / 2. - 0.;
        let start_y = 48.0f32;
        let labels: [(DevButtonAction, &str); 13] = [
            (DevButtonAction::GrantXp, "+250 xp"),
            (DevButtonAction::GrantMoreXp, "+1000 xp"),
            (DevButtonAction::SpawnChest, "chest"),
            (DevButtonAction::SpawnTome, "tome"),
            (DevButtonAction::SpawnOrb, "orb"),
            (DevButtonAction::TeleportEra2, "era2"),
            (DevButtonAction::TeleportEra3, "era3"),
            (DevButtonAction::TriggerEndless, "endless"),
            (DevButtonAction::AddChaos, "+chaos"),
            (DevButtonAction::AddGold, "+50 gold"),
            (DevButtonAction::DropDungeonKey, "key"),
            (DevButtonAction::AddBanishCount, "+banish"),
            (DevButtonAction::AddLoadedDice, "Loaded Dice"),
        ];
        for (i, (action, label)) in labels.iter().enumerate() {
            let y = start_y - i as f32 * DEV_BUTTON_SPACING;
            let btn = commands
                .spawn(SpriteBundle {
                    texture: graphics.get_ui_element_texture(UIElement::XLKey).clone(),
                    sprite: Sprite {
                        custom_size: Some(Vec2::new(DEV_BUTTON_WIDTH, DEV_BUTTON_HEIGHT)),
                        ..Default::default()
                    },
                    transform: Transform::from_xyz(dev_x, y, 10.),
                    ..Default::default()
                })
                .insert(RenderLayers::from_layers(&[3]))
                .insert(UIState::Inventory)
                .insert(Interactable::default())
                .insert(*action)
                .insert(Name::new(format!("Dev Button {:?}", action)))
                .id();
            commands
                .spawn((
                    Text2dBundle {
                        text: Text::from_section(
                            *label,
                            TextStyle {
                                font: asset_server.load("fonts/4x5.ttf"),
                                font_size: 5.0,
                                color: DARK_WOOD_BROWN,
                            },
                        )
                        .with_alignment(TextAlignment::Center),
                        text_anchor: Anchor::Center,
                        transform: Transform::from_xyz(0., 0.5, 1.),
                        ..Default::default()
                    },
                    RenderLayers::from_layers(&[3]),
                    UIState::Inventory,
                    Name::new("Dev Button Label"),
                ))
                .set_parent(btn);
            commands.entity(inv).add_child(btn);
        }
    }

    stats_event.send(ShowInvPlayerStatsEvent {
        stat: None,
        ignore_timer: true,
    });
}

pub fn setup_inv_slots_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    inv_query: Query<Entity, With<InventoryUI>>,
    inv_state_res: Res<InventoryState>,
    inv_state: Res<State<UIState>>,
    inv_spawn_check: Query<Entity, Added<InventoryUI>>,

    asset_server: Res<AssetServer>,
    mut inv: Query<&mut Inventory>,
    crafting_container: Option<Res<CraftingContainer>>,
    resolution: Res<ScreenResolution>,
    break_drop_filter: Res<BreakDropFilter>,
    menu_open: Res<MaterialDropFilterMenuOpen>,
    proto_param: ProtoParam,
) {
    if inv_spawn_check.get_single().is_err() {
        return;
    }
    let (should_spawn_equipment, crafting_items_option) = match inv_state.0 {
        UIState::Inventory => (true, Some(inv.single().crafting_items.clone())),
        UIState::InventoryCrafting => (false, None),
        UIState::Crafting => (true, Some(crafting_container.unwrap().items.clone())),
        UIState::Chest => (false, None),
        UIState::Scrapper => (false, None),
        UIState::Furnace => (true, None),
        _ => return,
    };
    for (slot_index, item) in inv.single_mut().items.items.iter().enumerate() {
        spawn_inv_slot(
            &mut commands,
            &inv_state,
            &graphics,
            slot_index,
            Interaction::None,
            &inv_state_res,
            &inv_query,
            &asset_server,
            InventorySlotType::Normal,
            item.clone(),
            &resolution,
        );

        // equipment slots
        if slot_index < 4 && should_spawn_equipment {
            spawn_inv_slot(
                &mut commands,
                &inv_state,
                &graphics,
                slot_index,
                Interaction::None,
                &inv_state_res,
                &inv_query,
                &asset_server,
                InventorySlotType::Equipment,
                None,
                &resolution,
            );
        }
        // accessoyr slots
        if slot_index < 3 && should_spawn_equipment {
            spawn_inv_slot(
                &mut commands,
                &inv_state,
                &graphics,
                slot_index,
                Interaction::None,
                &inv_state_res,
                &inv_query,
                &asset_server,
                InventorySlotType::Accessory,
                None,
                &resolution,
            );
        }
    }
    // Single-slot Weapon + Pet cells in the equipment panel's mid row.
    // Both containers are size 1 (see `player::spawn_player`), so we spawn them outside
    // the main-grid loop with `slot_index = 0`.
    // World weapon pickups auto-fill these when empty via `inventory::try_auto_equip_weapon_on_pickup`
    // (`check_item_drop_collisions`): main weapon first, then pet slot if the player has a pet.
    if should_spawn_equipment {
        let weapon_item = inv
            .single_mut()
            .weapon_items
            .items
            .get(0)
            .and_then(|x| x.clone());
        spawn_inv_slot(
            &mut commands,
            &inv_state,
            &graphics,
            0,
            Interaction::None,
            &inv_state_res,
            &inv_query,
            &asset_server,
            InventorySlotType::Weapon,
            weapon_item,
            &resolution,
        );
        let pet_item = inv
            .single_mut()
            .pet_items
            .items
            .get(0)
            .and_then(|x| x.clone());
        spawn_inv_slot(
            &mut commands,
            &inv_state,
            &graphics,
            0,
            Interaction::None,
            &inv_state_res,
            &inv_query,
            &asset_server,
            InventorySlotType::Pet,
            pet_item,
            &resolution,
        );
    }
    if inv_state.0 != UIState::Scrapper {
        // Upgrade tome / orb slots live on the upgrade side panel; in `InventoryCrafting`
        // the side panel is replaced by the three crafting inputs so we skip these.
        if inv_state.0 != UIState::InventoryCrafting {
            if let Some(furnace_items) = inv.single_mut().furnace_items.clone().into() {
                for (slot_index, item) in furnace_items.items.iter().enumerate() {
                    spawn_inv_slot(
                        &mut commands,
                        &inv_state,
                        &graphics,
                        slot_index,
                        Interaction::None,
                        &inv_state_res,
                        &inv_query,
                        &asset_server,
                        InventorySlotType::Furnace,
                        item.to_owned(),
                        &resolution,
                    );
                }
            }
        }
        // In crafting mode the upgrade panel is replaced by the `CraftingPanel`, which hosts
        // three ingredient *display* slots plus the craft result slot. Those are spawned by
        // `spawn_inventory_crafting_side_panel_slots` since they are not regular droppable
        // inventory slots.
        // Spawn trash slot (only in regular inventory, not scrapper)
        if inv_state.0 == UIState::Inventory
            || inv_state.0 == UIState::InventoryCrafting
            || inv_state.0 == UIState::Crafting
        {
            let trash_item = inv
                .single_mut()
                .trash_items
                .items
                .get(0)
                .and_then(|x| x.clone());
            spawn_inv_slot(
                &mut commands,
                &inv_state,
                &graphics,
                0,
                Interaction::None,
                &inv_state_res,
                &inv_query,
                &asset_server,
                InventorySlotType::Trash,
                trash_item,
                &resolution,
            );

            // Sort button — slot-sized tile sitting directly under the trash slot. Clicking
            // sorts the main inventory grid (see `handle_sort_inventory_button_click`).
            spawn_sort_inventory_button(
                &mut commands,
                &graphics,
                &asset_server,
                &inv_query,
                inv_state_res.inv_size,
            );
            spawn_material_drops_toggle_button(
                &mut commands,
                &graphics,
                &asset_server,
                &inv_query,
                inv_state_res.inv_size,
            );
            let (panel_pos_offset, panel_inv_size) = inventory_panel_layout(&inv_state.0);
            spawn_material_drop_filter_panel(
                &mut commands,
                &graphics,
                &asset_server,
                &proto_param,
                &inv_state,
                panel_pos_offset,
                panel_inv_size,
                &break_drop_filter,
                menu_open.0,
            );
        }
    }
}

/// Panel center offset and size for the active inventory-family UI state.
fn inventory_panel_layout(ui_state: &UIState) -> (Vec2, Vec2) {
    match ui_state {
        UIState::Inventory | UIState::InventoryCrafting => {
            (Vec2::new(-185., INVENTORY_Y_OFFSET), INVENTORY_UI_SIZE)
        }
        UIState::Crafting => (
            Vec2::new(22.5, INVENTORY_Y_OFFSET),
            CRAFTING_INVENTORY_UI_SIZE,
        ),
        _ => (Vec2::ZERO, INVENTORY_UI_SIZE),
    }
}

/// Spawns the "Sort" button directly under the trash slot on the left edge of the inventory
/// panel. The button is a slot-sized `InventorySlot` sprite with a `SORT` label so it visually
/// matches the trash slot. Clicks are handled by `handle_sort_inventory_button_click`.
fn spawn_sort_inventory_button(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    inv_query: &Query<Entity, With<InventoryUI>>,
    inv_size: Vec2,
) {
    let hw = inv_size.x * 0.5;
    let hh = inv_size.y * 0.5;
    let translation = Vec3::new(
        -hw + INV_SORT_BUTTON_OFFSET_X,
        hh + INV_SORT_BUTTON_OFFSET_Y,
        1.,
    );

    let button = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::InventorySlot),
            transform: Transform {
                translation,
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            sprite: Sprite {
                custom_size: Some(UI_SLOT_SIZE),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        // Deliberately do NOT insert `UIElement::InventorySlot` — `handle_hovering` assumes
        // any entity tagged with that element also has an `InventorySlotState` component and
        // unwraps it. The sort button owns its own hover state in
        // `handle_sort_inventory_button_click`.
        .insert(Interactable::default())
        .insert(SortInventoryButton)
        .insert(IconHoverTooltipText(&["Sort inventory"]))
        .insert(Name::new("SORT INVENTORY BUTTON"))
        .id();

    let label = commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                "SORT",
                TextStyle {
                    font: asset_server.load("fonts/4x5.ttf"),
                    font_size: 5.0,
                    color: YELLOW_2,
                },
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: Anchor::Center,
            transform: Transform {
                translation: Vec3::new(0., 0., 1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("SORT LABEL"))
        .id();
    commands.entity(button).push_children(&[label]);

    if let Ok(inv_e) = inv_query.get_single() {
        commands.entity(button).set_parent(inv_e);
    }
}

/// Filter button under the sort button — opens the drop-filter side panel.
fn spawn_material_drops_toggle_button(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    inv_query: &Query<Entity, With<InventoryUI>>,
    inv_size: Vec2,
) {
    let hw = inv_size.x * 0.5;
    let hh = inv_size.y * 0.5;
    let translation = Vec3::new(
        -hw + INV_MATERIAL_DROPS_TOGGLE_OFFSET_X,
        hh + INV_MATERIAL_DROPS_TOGGLE_OFFSET_Y,
        1.,
    );

    let plant_sprite = graphics
        .icons
        .as_ref()
        .and_then(|m| m.get(&WorldObject::PlantFibre).cloned())
        .or_else(|| {
            graphics
                .spritesheet_map
                .as_ref()
                .and_then(|m| m.get(&WorldObject::PlantFibre).cloned())
        })
        .unwrap_or_else(|| {
            graphics
                .spritesheet_map
                .as_ref()
                .unwrap()
                .get(&WorldObject::Stick)
                .cloned()
                .expect("fallback world object sprite for material toggle")
        });

    let button = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::InventorySlot),
            transform: Transform {
                translation,
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            sprite: Sprite {
                custom_size: Some(UI_SLOT_SIZE),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Interactable::default())
        .insert(MaterialDropsToggleButton)
        .insert(IconHoverTooltipText(&["Open drop filter menu"]))
        .insert(Name::new("MATERIAL DROPS TOGGLE BUTTON"))
        .id();

    let icon = commands
        .spawn(SpriteSheetBundle {
            sprite: {
                let mut s = plant_sprite;
                s.custom_size = Some(Vec2::splat(18.));
                s
            },
            texture_atlas: graphics.texture_atlas.as_ref().unwrap().clone(),
            transform: Transform::from_translation(Vec3::new(0., 0., 0.5)),
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("MATERIAL DROPS TOGGLE ICON"))
        .id();

    commands.entity(button).push_children(&[icon]);

    if let Ok(inv_e) = inv_query.get_single() {
        commands.entity(button).set_parent(inv_e);
    }
}

/// Side panel anchored to the right of the drop-filter button. When closed it is translated
/// off-screen so its child hit-boxes never block clicks on the inventory underneath.
fn spawn_material_drop_filter_panel(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    proto_param: &ProtoParam,
    inv_state: &State<UIState>,
    inv_pos_offset: Vec2,
    inv_size: Vec2,
    break_drop_filter: &BreakDropFilter,
    menu_open: bool,
) {
    let items = BREAK_DROP_FILTER_ITEMS;
    let cols = INV_DROP_FILTER_PANEL_COLS;
    let rows = items.len().div_ceil(cols).max(1);
    let cell = INV_DROP_FILTER_ICON_SIZE + INV_DROP_FILTER_ICON_GAP;
    let grid_w = cols as f32 * INV_DROP_FILTER_ICON_SIZE
        + (cols.saturating_sub(1) as f32) * INV_DROP_FILTER_ICON_GAP;
    let grid_h = rows as f32 * INV_DROP_FILTER_ICON_SIZE
        + (rows.saturating_sub(1) as f32) * INV_DROP_FILTER_ICON_GAP;
    let panel_w = grid_w + INV_DROP_FILTER_PANEL_PADDING * 2.0;
    let panel_h = INV_DROP_FILTER_PANEL_PADDING * 2.0
        + INV_DROP_FILTER_TITLE_ROW_HEIGHT
        + INV_DROP_FILTER_BUTTON_ROW_HEIGHT
        + 4.0
        + grid_h;

    // Anchor to the filter button: button is at inv-local (-hw + OFFSET_X, hh + OFFSET_Y) and
    // in world space at inv_pos_offset + that local. Panel sits to the right of the button.
    let hw = inv_size.x * 0.5;
    let hh = inv_size.y * 0.5;
    let button_world_x = inv_pos_offset.x + (-hw + INV_MATERIAL_DROPS_TOGGLE_OFFSET_X);
    let button_world_y = inv_pos_offset.y + (hh + INV_MATERIAL_DROPS_TOGGLE_OFFSET_Y);
    let panel_open_x =
        button_world_x + UI_SLOT_SIZE.x * 0.5 + INV_DROP_FILTER_PANEL_GAP + panel_w * 0.5;
    let panel_open_y = button_world_y - panel_h * 0.5 + UI_SLOT_SIZE.y * 0.5;
    let open_translation = Vec3::new(panel_open_x, panel_open_y, INV_DROP_FILTER_PANEL_Z);
    let closed_translation = Vec3::new(50_000.0, 50_000.0, INV_DROP_FILTER_PANEL_Z);
    let initial_translation = if menu_open {
        open_translation
    } else {
        closed_translation
    };

    let panel = commands
        .spawn((
            SpriteBundle {
                sprite: Sprite {
                    color: Color::rgba(0.22, 0.18, 0.14, 0.98),
                    custom_size: Some(Vec2::new(panel_w, panel_h)),
                    ..Default::default()
                },
                transform: Transform::from_translation(initial_translation),
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            MaterialDropFilterPanel { open_translation },
            inv_state.0.clone(),
            Name::new("DROP FILTER PANEL"),
        ))
        .id();

    let top_y = panel_h * 0.5 - INV_DROP_FILTER_PANEL_PADDING;
    let title_y = top_y - INV_DROP_FILTER_TITLE_ROW_HEIGHT * 0.5;
    commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    "Toggle Item Drop Filters",
                    TextStyle {
                        font: asset_server.load("fonts/4x5.ttf"),
                        font_size: 5.0,
                        color: YELLOW_2,
                    },
                )
                .with_alignment(TextAlignment::Center),
                text_anchor: Anchor::Center,
                transform: Transform::from_translation(Vec3::new(0., title_y, 11.)),
                ..default()
            },
            RenderLayers::from_layers(&[3]),
            inv_state.0.clone(),
            Name::new("DROP FILTER TITLE"),
        ))
        .set_parent(panel);

    let button_row_y = title_y
        - INV_DROP_FILTER_TITLE_ROW_HEIGHT * 0.5
        - INV_DROP_FILTER_BUTTON_ROW_HEIGHT * 0.5
        - 2.0;
    for (label, x_offset, is_all) in [
        ("ALL", -INV_DROP_FILTER_BUTTON_SPACING * 0.5, true),
        ("NONE", INV_DROP_FILTER_BUTTON_SPACING * 0.5, false),
    ] {
        let hit_w = 28.0;
        let hit_h = 12.0;
        let mut btn_ec = commands.spawn(SpriteBundle {
            sprite: Sprite {
                color: Color::NONE,
                custom_size: Some(Vec2::new(hit_w, hit_h)),
                ..Default::default()
            },
            transform: Transform::from_translation(Vec3::new(x_offset, button_row_y, 1.)),
            ..Default::default()
        });
        btn_ec.insert(RenderLayers::from_layers(&[3]));
        btn_ec.insert(Interactable::default());
        btn_ec.insert(inv_state.0.clone());
        btn_ec.insert(Name::new(if is_all {
            "DROP FILTER ALL"
        } else {
            "DROP FILTER NONE"
        }));
        if is_all {
            btn_ec.insert(MaterialDropFilterAllButton);
        } else {
            btn_ec.insert(MaterialDropFilterNoneButton);
        }
        let btn = btn_ec.id();
        let text = commands
            .spawn(Text2dBundle {
                text: Text::from_section(
                    label,
                    TextStyle {
                        font: asset_server.load("fonts/4x5.ttf"),
                        font_size: 5.0,
                        color: Color::WHITE,
                    },
                )
                .with_alignment(TextAlignment::Center),
                text_anchor: Anchor::Center,
                transform: Transform::from_translation(Vec3::new(0., 0., 1.)),
                ..default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .id();
        commands
            .entity(btn)
            .push_children(&[text])
            .set_parent(panel);
    }

    let grid_top_y = button_row_y
        - INV_DROP_FILTER_BUTTON_ROW_HEIGHT * 0.5
        - 4.0
        - INV_DROP_FILTER_ICON_SIZE * 0.5;
    let grid_left_x = -grid_w * 0.5 + INV_DROP_FILTER_ICON_SIZE * 0.5;

    for (i, &obj) in items.iter().enumerate() {
        let col = i % cols;
        let row = i / cols;
        let x = grid_left_x + col as f32 * cell;
        let y = grid_top_y - row as f32 * cell;

        let entry = commands
            .spawn(SpriteBundle {
                sprite: Sprite {
                    color: Color::NONE,
                    custom_size: Some(Vec2::splat(INV_DROP_FILTER_ICON_SIZE)),
                    ..Default::default()
                },
                transform: Transform::from_translation(Vec3::new(x, y, 1.)),
                ..Default::default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .insert(Interactable::default())
            .insert(MaterialDropFilterEntry { obj })
            .insert(inv_state.0.clone())
            .insert(Name::new(format!("DROP FILTER ENTRY {:?}", obj)))
            .id();

        let icon_stack = proto_param
            .get_item_data(obj)
            .map(|d| d.copy_with_count(1))
            .unwrap_or_else(|| ItemStack::crate_icon_stack(obj));
        let icon = spawn_drop_filter_icon(commands, graphics, &icon_stack);
        commands.entity(icon).set_parent(entry);

        let blocked = break_drop_filter.is_blocked(obj);
        let x_vis = if blocked {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        let x_overlay = commands
            .spawn(Text2dBundle {
                text: Text::from_section(
                    "X",
                    TextStyle {
                        font: asset_server.load("fonts/alagard.ttf"),
                        font_size: 15.0,
                        color: RED,
                    },
                )
                .with_alignment(TextAlignment::Center),
                text_anchor: Anchor::Center,
                visibility: x_vis,
                transform: Transform::from_translation(Vec3::new(0., 0., 3.)),
                ..default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .insert(MaterialDropFilterEntryX)
            .id();
        commands
            .entity(entry)
            .push_children(&[x_overlay])
            .set_parent(panel);
    }
}

/// 16×16 item icon for the drop-filter grid (no slot chrome).
fn spawn_drop_filter_icon(
    commands: &mut Commands,
    graphics: &Graphics,
    item_stack: &ItemStack,
) -> Entity {
    let obj = item_stack.obj_type;
    let sprite = graphics
        .icons
        .as_ref()
        .and_then(|icons| icons.get(&obj).cloned())
        .or_else(|| {
            graphics
                .spritesheet_map
                .as_ref()
                .and_then(|m| m.get(&obj).cloned())
        })
        .unwrap_or_else(|| {
            graphics
                .spritesheet_map
                .as_ref()
                .unwrap()
                .get(&WorldObject::Stick)
                .cloned()
                .expect("fallback sprite for drop-filter icon")
        });
    let mut sprite = sprite;
    sprite.custom_size = Some(Vec2::splat(INV_DROP_FILTER_ICON_SIZE));
    commands
        .spawn(SpriteSheetBundle {
            sprite,
            texture_atlas: graphics.texture_atlas.as_ref().unwrap().clone(),
            transform: Transform::from_translation(Vec3::new(0., 0., 2.)),
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .id()
}

/// Center of a main-grid slot (`Normal`, and base for chest/scrapper) in inventory panel space.
fn main_inventory_grid_center(slot_index: usize, inv_size: Vec2) -> Vec2 {
    let hw = inv_size.x * 0.5;
    let hh = inv_size.y * 0.5;
    let col = slot_index % INVENTORY_GRID_COLS;
    let row = slot_index / INVENTORY_GRID_COLS;
    let x = (-hw + INV_GRID_INSET_LEFT + UI_SLOT_SIZE.x * 0.5 + col as f32 * INV_SLOT_SPACING_X)
        .floor();
    let y_base =
        (-hh + INV_GRID_INSET_BOTTOM + UI_SLOT_SIZE.y * 0.5 + row as f32 * INV_SLOT_SPACING_Y)
            .floor();
    let y = if row == 0 {
        y_base + INV_GRID_FIRST_ROW_NUDGE_Y
    } else {
        y_base
    };
    Vec2::new(x, y)
}

/// Maps an equipment / accessory / weapon / pet slot to its (col, row) cell in the
/// equipment panel's 3×3 grid, where col/row are each in `0..3` with `col = 0` left
/// and `row = 0` top.
///
/// Layout (matches reference art):
/// ```text
///   ┌─────────────┬─────────────┬─────────────┐
///   │ Ring  (Acc1)│ Neck  (Acc0)│ Ring  (Acc2)│   row 0 (top,   y = +26)
///   ├─────────────┼─────────────┼─────────────┤
///   │ Weapon      │ Chest (Eq2) │ Pet Weapon  │   row 1 (mid,   y =  −4)
///   ├─────────────┼─────────────┼─────────────┤
///   │ Shoes (Eq0) │ Pants (Eq1) │ Cape  (Eq3) │   row 2 (bot,   y = −34)
///   └─────────────┴─────────────┴─────────────┘
/// ```
/// Weapon / Pet slots each use slot_index 0 (single-slot containers).
fn equipment_grid_cell(slot_type: InventorySlotType, slot_index: usize) -> (usize, usize) {
    match (slot_type, slot_index) {
        (InventorySlotType::Accessory, 1) => (0, 0),
        (InventorySlotType::Accessory, 0) => (1, 0),
        (InventorySlotType::Accessory, 2) => (2, 0),
        (InventorySlotType::Weapon, _) => (0, 1),
        (InventorySlotType::Equipment, 2) => (1, 1),
        (InventorySlotType::Pet, _) => (2, 1),
        (InventorySlotType::Equipment, 0) => (0, 2),
        (InventorySlotType::Equipment, 1) => (1, 2),
        (InventorySlotType::Equipment, 3) => (2, 2),
        _ => (1, 1),
    }
}

/// Panel-local position (relative to the inventory panel center) for an equipment / accessory slot.
fn equipment_grid_position(slot_type: InventorySlotType, slot_index: usize) -> Vec2 {
    let (col, row) = equipment_grid_cell(slot_type, slot_index);
    let row_y = match row {
        0 => INV_EQUIP_GRID_ROW_TOP_Y,
        1 => INV_EQUIP_GRID_ROW_MID_Y,
        _ => INV_EQUIP_GRID_ROW_BOT_Y,
    };
    Vec2::new(
        INV_EQUIP_PANEL_OFFSET_X + (col as f32 - 1.0) * INV_EQUIP_GRID_SPACING,
        INV_EQUIP_PANEL_OFFSET_Y + row_y,
    )
}

/// Panel-local position for a slot’s center (before optional crafting-mode parent nudge).
///
/// `game_height` should be the current `ScreenResolution::game_height`, not the
/// `GAME_HEIGHT` constant — the constant is only a *target* height; the actual
/// game-space height is derived from the window size and an integer scale, so
/// anchoring HUD slots to the bottom of the screen requires the runtime value
/// or the hotbar will drift on non-matching monitor resolutions.
fn inv_slot_local_position(
    slot_type: InventorySlotType,
    slot_index: usize,
    inv_size: Vec2,
    ui_state: &UIState,
    game_height: f32,
) -> Vec2 {
    match slot_type {
        InventorySlotType::Hotbar => {
            // Hotbar shares a row with the class-skill icons in the HUD. Slots are laid out
            // symmetrically around `HUD_HOTBAR_CENTER_X`, so slot 0 is the leftmost and
            // slot `HUD_HOTBAR_SLOTS - 1` is the rightmost. Slots >= HUD_HOTBAR_SLOTS still
            // exist in the container but are not spawned into the HUD (see `setup_hotbar_hud`).
            let half_span = (HUD_HOTBAR_SLOTS as f32 - 1.0) * 0.5;
            Vec2::new(
                HUD_HOTBAR_CENTER_X + (slot_index as f32 - half_span) * (INV_SLOT_SPACING_X - 5.0),
                -game_height * 0.5 + HUD_ACTION_ROW_Y_FROM_BOTTOM,
            )
        }
        InventorySlotType::Crafting => {
            let hw = inv_size.x * 0.5;
            let hh = inv_size.y * 0.5;
            let col = slot_index % INV_CRAFTING_COLS;
            let row = slot_index / INV_CRAFTING_COLS;
            let mut x = -hw
                + INV_CRAFTING_X_ANCHOR
                + UI_SLOT_SIZE.x * 0.5
                + col as f32 * INV_SLOT_SPACING_X;
            let mut y = -(row as f32) * (INV_SLOT_SPACING_Y + INV_CRAFTING_ROW_GAP) - hh
                + INV_CRAFTING_BASE_Y;
            if matches!(ui_state, UIState::Inventory) {
                x += INV_CRAFTING_NUDGE_IN_MAIN_INV.x;
                y += INV_CRAFTING_NUDGE_IN_MAIN_INV.y;
            }
            Vec2::new(x, y)
        }
        InventorySlotType::Equipment => {
            equipment_grid_position(InventorySlotType::Equipment, slot_index)
        }
        InventorySlotType::Accessory => {
            equipment_grid_position(InventorySlotType::Accessory, slot_index)
        }
        InventorySlotType::Weapon => equipment_grid_position(InventorySlotType::Weapon, slot_index),
        InventorySlotType::Pet => equipment_grid_position(InventorySlotType::Pet, slot_index),
        InventorySlotType::Chest | InventorySlotType::Scrapper => {
            let mut p = main_inventory_grid_center(slot_index, inv_size);
            p.y += INV_CHEST_SCRAPPER_GRID_OFFSET_Y;
            p
        }
        InventorySlotType::Furnace => match slot_index {
            0 => INV_FURNACE_SLOT_0,
            1 => INV_FURNACE_SLOT_1,
            _ => Vec2::ZERO,
        },
        InventorySlotType::CraftingInput => {
            // Three slots laid out horizontally, centered on the upgrade panel's local x (150).
            // The crafting panel sits higher than the default upgrade panel, so slot y uses the
            // shared constant (see `INV_CRAFTING_INPUT_SLOTS_Y_LOCAL`).
            let center_x = 150.0;
            let x = center_x + (slot_index as f32 - 1.0) * INV_CRAFTING_INPUT_SLOT_SPACING_X;
            Vec2::new(x, INV_CRAFTING_INPUT_SLOTS_Y_LOCAL)
        }
        InventorySlotType::Trash => {
            let hw = inv_size.x * 0.5;
            let hh = inv_size.y * 0.5;
            Vec2::new(-hw + INV_TRASH_OFFSET_X, hh + INV_TRASH_OFFSET_Y)
        }
        InventorySlotType::Normal => main_inventory_grid_center(slot_index, inv_size),
    }
}

pub fn spawn_inv_slot(
    commands: &mut Commands,
    inv_ui_state: &Res<State<UIState>>,
    graphics: &Graphics,
    slot_index: usize,
    interactable_state: Interaction,
    inv_state: &InventoryState,
    inv_query: &Query<Entity, With<InventoryUI>>,
    asset_server: &AssetServer,
    slot_type: InventorySlotType,
    item_stack: Option<InventoryItemStack>,
    resolution: &ScreenResolution,
) -> Entity {
    // spawns an inv slot, with an item icon as its child if an item exists in that inv slot.
    // the slot's parent is set to the inv ui entity.
    let inv_slot_offset = match inv_ui_state.0 {
        UIState::Crafting => INV_UI_PARENT_OFFSET_CRAFTING,
        _ => Vec2::ZERO,
    };

    let local = inv_slot_local_position(
        slot_type,
        slot_index,
        inv_state.inv_size,
        &inv_ui_state.0,
        resolution.game_height,
    );
    // HUD hotbar slots must render above the `HudBar` frame (Z=1); class-skill icons use the
    // same depth via `Z_DEPTH_HUD_ACTIVE_SKILLS`.
    let translation = (local + inv_slot_offset).extend(if slot_type.is_hotbar() {
        Z_DEPTH_HUD_ACTIVE_SKILLS
    } else {
        1.
    });
    let mut item_icon_option = None;
    let mut item_type_option = None;
    let mut item_count_option = None;
    // check if we need to spawn an item icon for this slot
    if let Some(item) = item_stack {
        // player has item in this slot

        let obj_type = *item.get_obj();
        item_type_option = Some(obj_type);
        item_count_option = Some(item.item_stack.count);
        item_icon_option = Some(spawn_item_stack_icon(
            commands,
            graphics,
            &item.item_stack,
            asset_server,
            Vec2::ZERO,
            if slot_index % INVENTORY_GRID_COLS == 3 || slot_index % INVENTORY_GRID_COLS == 2 {
                Vec2::new(0.5, 0.)
            } else {
                Vec2::ZERO
            },
            3,
        ));
    }

    // Inv Slot Icon //
    let mut slot_icon = match slot_type {
        InventorySlotType::Equipment => match slot_index {
            3 => Some("ui/icons/CapeSlotIcon.png"),
            2 => Some("ui/icons/ChestSlotIcon.png"),
            1 => Some("ui/icons/PantsSlotIcon.png"),
            0 => Some("ui/icons/ShoesSlotIcon.png"),
            _ => None,
        },
        InventorySlotType::Accessory => match slot_index {
            2 => Some("ui/icons/RingSlotIcon.png"),
            1 => Some("ui/icons/RingSlotIcon.png"),
            0 => Some("ui/icons/NecklaceSlotIcon.png"),
            _ => None,
        },
        InventorySlotType::Trash => Some("ui/icons/TrashSlotIcon.png"),
        InventorySlotType::Furnace => match slot_index {
            1 => Some("ui/icons/SwordSlotIcon.png"),
            0 => Some("ui/icons/OrbSlotIcon.png"),
            _ => None,
        },
        InventorySlotType::Weapon => Some("ui/icons/SwordSlotIcon.png"),
        InventorySlotType::Pet => Some("ui/icons/PetSlotIcon.png"),
        _ => None,
    };

    if item_icon_option.is_some() {
        slot_icon = None;
    }
    let size = if slot_type.is_trash() {
        Vec2::new(20., 20.)
    } else {
        Vec2::new(18., 18.)
    };

    let icon_entity_option = slot_icon.map(|slot_icon| {
        commands
            .spawn(SpriteBundle {
                texture: asset_server.load(slot_icon),
                transform: Transform {
                    translation: Vec3::new(0., 0., 1.),
                    scale: if slot_type.is_furnace() {
                        Vec3::new(2., 2., 1.)
                    } else {
                        Vec3::new(1., 1., 1.)
                    },
                    ..Default::default()
                },
                sprite: Sprite {
                    custom_size: Some(size),
                    ..Default::default()
                },
                ..Default::default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .id()
    });

    // HUD hotbar: anchor only — slot art is baked into `HudBar.png`.
    let mut slot_entity = if slot_type.is_hotbar() {
        commands.spawn(SpatialBundle::from_transform(Transform::from_translation(
            translation,
        )))
    } else {
        commands.spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(if slot_type.is_furnace() {
                UIElement::UpgradeSlot
            } else {
                UIElement::InventorySlot
            }),
            transform: Transform {
                translation,
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            sprite: Sprite {
                custom_size: Some(if slot_type.is_furnace() {
                    UI_UPGRADE_SLOT_SIZE
                } else {
                    UI_SLOT_SIZE
                }),
                ..Default::default()
            },
            ..Default::default()
        })
    };
    slot_entity
        .insert(RenderLayers::from_layers(&[3]))
        .insert(InventorySlotState {
            slot_index,
            item: item_icon_option,
            obj_type: item_type_option,
            count: item_count_option,
            dirty: false,
            r#type: slot_type,
        })
        .insert(if slot_type.is_furnace() {
            UIElement::UpgradeSlot
        } else {
            UIElement::InventorySlot
        })
        .insert(Name::new(if slot_type.is_crafting() {
            "CRAFTING SLOT"
        } else {
            "SLOT"
        }));
    if let Some(i) = item_icon_option {
        slot_entity.push_children(&[i]);
    }
    if slot_type.is_trash() {
        slot_entity.insert(IconHoverTooltipText(&["trash: drop items here"]));
    }
    if !slot_type.is_hotbar() {
        let inv_e = inv_query.single();
        slot_entity
            .set_parent(inv_e)
            .insert(Interactable::from_state(interactable_state));
    } else {
        // Hotbar slots also get Interactable for click detection when inventory is closed
        slot_entity.insert(Interactable::from_state(interactable_state));
    }

    if let Some(icon_entity) = icon_entity_option {
        slot_entity.push_children(&[icon_entity]);
    }
    slot_entity.id()
}
pub fn spawn_item_stack_icon(
    commands: &mut Commands,
    graphics: &Graphics,
    item_stack: &ItemStack,
    asset_server: &AssetServer,
    icon_offset: Vec2,
    text_offset: Vec2,
    render_layer: u8,
) -> Entity {
    let has_icon = graphics.icons.as_ref().unwrap().get(&item_stack.obj_type);
    let sprite = if let Some(icon) = has_icon {
        icon.clone()
    } else {
        graphics
            .spritesheet_map
            .as_ref()
            .unwrap()
            .get(&item_stack.obj_type)
            .unwrap_or_else(|| panic!("No graphic for object {:?}", item_stack.obj_type))
            .clone()
    };
    let item_entity = commands
        .spawn(SpriteSheetBundle {
            sprite,
            texture_atlas: graphics.texture_atlas.as_ref().unwrap().clone(),
            transform: Transform {
                translation: Vec3::new(icon_offset.x, icon_offset.y, 2.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(item_stack.clone())
        .insert(RenderLayers::from_layers(&[render_layer]))
        .id();
    // glow effect for rarity
    if let Some(glow_e) = add_item_glows(commands, graphics, item_entity, item_stack.rarity.clone())
    {
        commands
            .entity(glow_e)
            .insert(RenderLayers::from_layers(&[3]));
    }

    if item_stack.count > 1 {
        let text = commands
            .spawn((
                Text2dBundle {
                    text: Text::from_section(
                        item_stack.count.to_string(),
                        TextStyle {
                            font: asset_server.load("fonts/4x5.ttf"),
                            font_size: 5.0,
                            color: Color::WHITE,
                        },
                    )
                    .with_alignment(TextAlignment::Center),
                    transform: Transform {
                        translation: Vec3::new(7., -5.5, 3.) + text_offset.extend(0.),
                        scale: Vec3::new(1., 1., 1.),
                        ..Default::default()
                    },
                    ..default()
                },
                Name::new("ITEM STACK TEXT"),
                RenderLayers::from_layers(&[render_layer]),
            ))
            .id();
        commands.entity(item_entity).push_children(&[text]);
    }
    item_entity
}
pub fn update_inventory_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    mut ui_elements: Query<(Entity, &mut InventorySlotState)>,
    interactables: Query<&Interactable>,
    inv_ui_state: Res<State<UIState>>,
    inv_query: Query<Entity, With<InventoryUI>>,
    asset_server: Res<AssetServer>,
    inv: Query<&mut Inventory>,
    cont_param: UIContainersParam,
    resolution: Res<ScreenResolution>,
) {
    for (e, mut slot_state) in ui_elements.iter_mut() {
        // check current inventory state against that slot's state
        // if they do not match, delete and respawn

        // hotbars are hidden when inventory is open, so defer update
        // until inv is closed again. But still mark dirty if count changed
        // so they update properly when inv closes.
        if inv_ui_state.0.is_inv_open() && slot_state.r#type.is_hotbar() {
            // Check if hotbar slot needs updating and mark dirty for later
            let hotbar_item = inv
                .single()
                .get_items_from_slot_type(slot_state.r#type)
                .items[slot_state.slot_index]
                .clone();
            let real_count = hotbar_item.as_ref().map(|i| i.item_stack.count);
            if slot_state.count != real_count {
                slot_state.dirty = true;
            }
            continue;
        }

        let interactable_option = interactables.get(e);
        let item_option = if slot_state.r#type.is_chest() {
            cont_param.chest_option.as_ref().unwrap().items.items[slot_state.slot_index].clone()
        } else if slot_state.r#type.is_scrapper() {
            cont_param.scrapper_option.as_ref().unwrap().items.items[slot_state.slot_index].clone()
        } else if slot_state.r#type.is_furnace() {
            inv.single()
                .get_items_from_slot_type(slot_state.r#type)
                .items[slot_state.slot_index]
                .clone()
        } else if slot_state.r#type.is_trash() {
            inv.single()
                .get_items_from_slot_type(slot_state.r#type)
                .items[slot_state.slot_index]
                .clone()
        } else if slot_state.r#type.is_crafting() && cont_param.crafting_option.is_some() {
            cont_param.crafting_option.as_ref().unwrap().items.items[slot_state.slot_index].clone()
        } else {
            inv.single()
                .get_items_from_slot_type(slot_state.r#type)
                .items[slot_state.slot_index]
                .clone()
        };
        let real_count = if let Some(item) = item_option.clone() {
            Some(item.item_stack.count)
        } else {
            None
        };

        if slot_state.dirty || slot_state.count != real_count {
            commands.entity(e).despawn_recursive();
            let new_slot_entity = spawn_inv_slot(
                &mut commands,
                &inv_ui_state,
                &graphics,
                slot_state.slot_index,
                if let Ok(i) = interactable_option {
                    i.current().clone()
                } else {
                    Interaction::None
                },
                &cont_param.inv_state,
                &inv_query,
                &asset_server,
                slot_state.r#type,
                item_option.clone(),
                &resolution,
            );

            // HUD hotbar keybind badges are spawned once in `setup_hotbar_hud` (bottom-anchored,
            // not parented to slot entities) and updated via `update_hotbar_keybind_text`.
        }
    }
}
/// when items in the inventory state change, update the matching entities in the UI
pub fn handle_update_inv_item_entities(
    mut inv: Query<&mut Inventory, Changed<Inventory>>,
    mut inv_slot_state: Query<&mut InventorySlotState>,
    mut att_event: EventWriter<AttributeChangeEvent>,
    mut commands: Commands,
    ui_state: Res<State<UIState>>,
) {
    if !ui_state.0.is_inv_open() {
        return;
    }
    if let Ok(inv) = inv.get_single_mut() {
        att_event.send(AttributeChangeEvent);
        for inv_item_option in inv.clone().items.items.iter() {
            if let Some(inv_item) = inv_item_option {
                let item = inv_item.item_stack.clone();
                for slot_state in inv_slot_state.iter_mut() {
                    if slot_state.slot_index == inv_item.slot
                        && (slot_state.r#type.is_inventory() || slot_state.r#type.is_hotbar())
                    {
                        if let Some(item_e) = slot_state.item {
                            if let Some(mut entity_commands) = commands.get_entity(item_e) {
                                entity_commands.insert(item.clone());
                            }
                        }
                    }
                }
            }
        }
    }
}

pub fn mark_slot_dirty(
    slot_index: usize,
    slot_type: InventorySlotType,
    inv_slots: &mut Query<&mut InventorySlotState>,
) {
    for mut state in inv_slots.iter_mut() {
        if state.slot_index == slot_index && (state.r#type == slot_type || state.r#type.is_hotbar())
        {
            state.dirty = true;
        }
    }
}

/// Handles clicks on dev mode buttons (only runs when inventory is open and dev mode is on).
pub fn handle_dev_button_clicks(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut dev_buttons: Query<(Entity, &DevButtonAction, &mut Interactable)>,
    mut commands: Commands,
    mut flash_event: EventWriter<FlashExpBarEvent>,
    mut game: GameParam,
    mut proto_commands: ProtoCommands,
    proto: ProtoParam,
    mut dimension_spawn: EventWriter<DimensionSpawnEvent>,
    mut era_timer: ResMut<EraTimer>,
    mut chaos_tracker: ResMut<ChaosTracker>,
    mut currency_event: EventWriter<ModifyCurencyEvent>,
    mut run_unlock_state: ResMut<RunUnlockState>,
    mut grant_heirloom_dev: EventWriter<GrantHeirloomDevEvent>,
) {
    let hit_test = super::ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);

    for (e, action, mut interactable) in dev_buttons.iter_mut() {
        let hit = match &hit_test {
            Some((hit_ent, _, _)) => *hit_ent == e,
            None => false,
        };
        if hit {
            interactable.change(Interaction::Hovering);
            if left_mouse_pressed {
                let player_pos = game.player().position.truncate();
                let spawn_pos = player_pos + Vec2::new(0., -20.);

                match action {
                    DevButtonAction::GrantXp => {
                        let player_skills = game.get_player_skills();
                        let mut player_level = game.get_player_level_mut();
                        let did_level =
                            player_level.add_xp(250, &player_skills, &mut chaos_tracker);
                        flash_event.send(FlashExpBarEvent {
                            amount: 250,
                            did_level,
                        });
                    }
                    DevButtonAction::GrantMoreXp => {
                        let player_skills = game.get_player_skills();
                        let mut player_level = game.get_player_level_mut();
                        let did_level =
                            player_level.add_xp(1000, &player_skills, &mut chaos_tracker);
                        flash_event.send(FlashExpBarEvent {
                            amount: 1000,
                            did_level,
                        });
                    }
                    DevButtonAction::SpawnChest => {
                        let _ = proto_commands.spawn_item_from_proto(
                            WorldObject::ChestBlock,
                            &proto,
                            spawn_pos,
                            1,
                            None,
                        );
                    }
                    DevButtonAction::SpawnTome => {
                        let _ = proto_commands.spawn_item_from_proto(
                            WorldObject::UpgradeTome,
                            &proto,
                            spawn_pos,
                            3,
                            None,
                        );
                    }
                    DevButtonAction::SpawnOrb => {
                        let _ = proto_commands.spawn_item_from_proto(
                            WorldObject::OrbOfTransformation,
                            &proto,
                            spawn_pos,
                            3,
                            None,
                        );
                    }
                    DevButtonAction::TeleportEra2 => {
                        dimension_spawn.send(DimensionSpawnEvent {
                            swap_to_dim_now: true,
                            new_era: Some(Era::Second),
                        });
                    }
                    DevButtonAction::TeleportEra3 => {
                        dimension_spawn.send(DimensionSpawnEvent {
                            swap_to_dim_now: true,
                            new_era: Some(Era::Third),
                        });
                    }
                    DevButtonAction::TriggerEndless => {
                        era_timer.remaining_seconds = 5.0;
                    }
                    DevButtonAction::AddChaos => {
                        chaos_tracker.add_chaos(1.0);
                    }
                    DevButtonAction::AddGold => {
                        currency_event.send(ModifyCurencyEvent {
                            delta: 50,
                            obj: WorldObject::Coin,
                        });
                    }
                    DevButtonAction::DropDungeonKey => {
                        let _ = proto_commands.spawn_item_from_proto(
                            WorldObject::Key,
                            &proto,
                            spawn_pos,
                            1,
                            None,
                        );
                    }
                    DevButtonAction::AddBanishCount => {
                        run_unlock_state.banishes_remaining =
                            run_unlock_state.banishes_remaining.saturating_add(1);
                        run_unlock_state.banishes_total =
                            run_unlock_state.banishes_total.saturating_add(1);
                    }
                    DevButtonAction::AddLoadedDice => {
                        grant_heirloom_dev.send(GrantHeirloomDevEvent(Heirloom::LoadedDice));
                    }
                }
                commands.spawn(crate::audio::SoundSpawner::new(
                    crate::audio::AudioSoundEffect::ButtonClick,
                    0.2,
                ));
            }
        } else if matches!(interactable.current(), Interaction::Hovering) {
            interactable.change(Interaction::None);
        }
    }
}

/// Applies dev-mode heirloom grants (separate system to avoid GameParam query conflict).
pub fn apply_grant_heirloom_dev(
    mut grant_events: EventReader<GrantHeirloomDevEvent>,
    mut player_query: Query<(Entity, &mut PlayerSkills), With<Player>>,
    mut commands: Commands,
    mut att_event: EventWriter<AttributeChangeEvent>,
) {
    for GrantHeirloomDevEvent(heirloom) in grant_events.iter() {
        if let Ok((player_entity, mut skills)) = player_query.get_single_mut() {
            skills.heirlooms.push(HeirloomWithRarity {
                heirloom: heirloom.clone(),
                rarity: HeirloomRarity::Uncommon,
            });
            heirloom.add_heirloom_components(player_entity, &mut commands, skills.clone());
            att_event.send(AttributeChangeEvent);
        }
    }
}

/// Keeps the dynamic upgrade-material prompt text in sync with furnace slot 0 (the tome/orb slot).
/// Runs while the inventory (non-crafting) UI is open so the prompt updates as soon as a material
/// is dropped onto the slot.
pub fn update_upgrade_material_prompt_text(
    inv: Query<&Inventory>,
    mut prompt: Query<&mut Text, With<UpgradeMaterialPromptText>>,
) {
    let Ok(inventory) = inv.get_single() else {
        return;
    };
    let next_label = match inventory
        .furnace_items
        .items
        .get(0)
        .and_then(|slot| slot.as_ref())
        .map(|i| i.item_stack.obj_type)
    {
        Some(WorldObject::UpgradeTome) => "Use Tome",
        Some(WorldObject::OrbOfTransformation) => "Use Orb",
        _ => "Add Materials",
    };
    for mut text in prompt.iter_mut() {
        if let Some(section) = text.sections.get_mut(0) {
            if section.value != next_label {
                section.value = next_label.to_string();
            }
        }
    }
}

/// Handles hover + click on the CRAFT / UPGRADE toggle button that flips the inventory side
/// panels between the standard equipment/upgrade layout and the crafting/blueprints layout.
pub fn handle_cursor_inventory_craft_toggle_button(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut toggle_buttons: Query<
        (Entity, &mut Interactable, &CraftModeToggleButton),
        Without<InventorySlotState>,
    >,
    mut commands: Commands,
    curr_ui_state: Res<State<UIState>>,
    mut next_ui_state: ResMut<NextState<UIState>>,
    mut inv: Query<&mut Inventory>,
    mut inv_slots: Query<&mut InventorySlotState>,
    proto: ProtoParam,
) {
    let hit_test = super::ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);

    for (e, mut interactable, _) in toggle_buttons.iter_mut() {
        match hit_test {
            Some((hit_ent, _, _)) if hit_ent == e => match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                }
                Interaction::Hovering => {
                    if left_mouse_pressed {
                        let target = match curr_ui_state.0 {
                            UIState::Inventory => UIState::InventoryCrafting,
                            UIState::InventoryCrafting => UIState::Inventory,
                            _ => continue,
                        };
                        // Leaving the upgrade panel: if an equipment piece is sitting in the
                        // upgrade slot and an appropriate equipment slot is empty, auto-equip
                        // it so the player doesn't lose sight of it behind the crafting panel.
                        if curr_ui_state.0 == UIState::Inventory
                            && target == UIState::InventoryCrafting
                        {
                            if let Ok(mut inv) = inv.get_single_mut() {
                                try_auto_equip_from_upgrade_slot(&mut inv, &proto, &mut inv_slots);
                            }
                        }
                        next_ui_state.set(target);
                        commands.spawn(crate::audio::SoundSpawner::new(
                            crate::audio::AudioSoundEffect::ButtonClick,
                            0.2,
                        ));
                    }
                }
                _ => (),
            },
            _ => {
                if matches!(interactable.current(), Interaction::Hovering) {
                    interactable.change(Interaction::None);
                }
            }
        }
    }
}

/// Hover + click handler for blueprint row slots on the BlueprintsPanel (InventoryCrafting).
/// - Hover enters transition -> emit a recipe tooltip (`is_recipe = true`).
/// - Hover exits -> despawn the tooltip.
/// - Click -> set `SelectedCraftingRecipe`.
pub fn handle_blueprint_slot_interaction(
    mut commands: Commands,
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut blueprint_slots: Query<(Entity, &mut Interactable, &BlueprintSlot)>,
    mut selected: ResMut<SelectedCraftingRecipe>,
    mut tooltip_update: EventWriter<crate::ui::ToolTipUpdateEvent>,
    mut tooltip_teardown: EventWriter<crate::ui::TooltipTeardownEvent>,
    cur_ui_state: Res<State<UIState>>,
    proto: ProtoParam,
) {
    if cur_ui_state.0 != UIState::InventoryCrafting {
        return;
    }
    let hit_test = super::ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);

    for (e, mut interactable, bp) in blueprint_slots.iter_mut() {
        match hit_test {
            Some((hit_ent, _, _)) if hit_ent == e => match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                    // Build an `ItemStack` for the recipe result so the tooltip renders its
                    // ingredients + name via the existing `is_recipe` pipeline.
                    if let Some(item_data) = proto.get_item_data(bp.recipe_obj) {
                        tooltip_update.send(crate::ui::ToolTipUpdateEvent {
                            item_stack: item_data.clone(),
                            is_recipe: true,
                            show_range: false,
                        });
                    }
                }
                Interaction::Hovering => {
                    if left_mouse_pressed {
                        selected.0 = Some(bp.recipe_obj);
                        commands.spawn(crate::audio::SoundSpawner::new(
                            crate::audio::AudioSoundEffect::ButtonClick,
                            0.2,
                        ));
                    }
                }
                _ => (),
            },
            _ => {
                if matches!(interactable.current(), Interaction::Hovering) {
                    interactable.change(Interaction::None);
                    tooltip_teardown.send_default();
                }
            }
        }
    }
}

/// Rebuilds the icon + count label on each of the three ingredient display slots and the
/// craft-result slot whenever the selection or inventory changes. Icons are faded to 40%
/// alpha if the player does not have enough of that ingredient.
pub fn refresh_crafting_ingredient_display(
    mut commands: Commands,
    cur_ui_state: Res<State<UIState>>,
    selected: Res<SelectedCraftingRecipe>,
    inv_q: Query<&Inventory>,
    inv_changed_q: Query<Entity, (With<Inventory>, Changed<Inventory>)>,
    recipes: Res<Recipes>,
    proto: ProtoParam,
    ingredient_slots: Query<(Entity, &CraftingIngredientDisplaySlot)>,
    result_slots: Query<Entity, With<CraftingResultSlot>>,
    existing_ing_icons: Query<Entity, With<CraftingIngredientIcon>>,
    existing_result_icons: Query<Entity, With<CraftingResultIcon>>,
    existing_arrows: Query<Entity, With<CraftingArrowIndicator>>,
    mut count_texts: Query<(&mut Text, &CraftingIngredientCountText)>,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
) {
    if cur_ui_state.0 != UIState::InventoryCrafting {
        return;
    }
    let needs_refresh = selected.is_changed() || !inv_changed_q.is_empty();
    if !needs_refresh {
        return;
    }
    let Ok(inv) = inv_q.get_single() else {
        return;
    };

    // Clear current icons so we can repaint from scratch.
    for e in existing_ing_icons.iter() {
        commands.entity(e).despawn_recursive();
    }
    for e in existing_result_icons.iter() {
        commands.entity(e).despawn_recursive();
    }
    for e in existing_arrows.iter() {
        commands.entity(e).despawn_recursive();
    }

    // Default: clear all count labels.
    for (mut text, _) in count_texts.iter_mut() {
        if let Some(section) = text.sections.get_mut(0) {
            section.value.clear();
        }
    }

    let Some(recipe_obj) = selected.0 else {
        return;
    };
    let Some(recipe) = recipes.crafting_list.get(&recipe_obj) else {
        return;
    };

    // Helper closure: are all ingredients satisfied?
    let all_satisfied = recipe
        .0
        .iter()
        .all(|ing| inv.items.get_item_count_in_container(ing.item) >= ing.count);

    // Ingredients.
    for (slot_entity, slot) in ingredient_slots.iter() {
        let ingredient = recipe.0.get(slot.slot_index);
        if let Some(ingredient) = ingredient {
            let owned = inv.items.get_item_count_in_container(ingredient.item);
            let has_enough = owned >= ingredient.count;
            let alpha = if has_enough { 1.0 } else { 0.4 };

            // Spawn icon.
            if let Some(base_stack) = proto.get_item_data(ingredient.item).cloned() {
                let icon = spawn_item_stack_icon(
                    &mut commands,
                    &graphics,
                    &base_stack.copy_with_count(1),
                    &asset_server,
                    Vec2::ZERO,
                    Vec2::ZERO,
                    3,
                );
                commands
                    .entity(icon)
                    .insert(CraftingIngredientIcon {
                        slot_index: slot.slot_index,
                    })
                    .insert(UIState::InventoryCrafting);
                // Fade if unavailable.
                commands.add(move |world: &mut World| {
                    if let Some(mut e) = world.get_entity_mut(icon) {
                        if let Some(mut sprite) = e.get_mut::<TextureAtlasSprite>() {
                            sprite.color.set_a(alpha);
                        }
                    }
                });
                commands.entity(slot_entity).push_children(&[icon]);
            }

            // Update count label.
            for (mut text, tag) in count_texts.iter_mut() {
                if tag.slot_index == slot.slot_index {
                    if let Some(section) = text.sections.get_mut(0) {
                        section.value = format!("{}/{}", owned, ingredient.count);
                        section.style.color = if has_enough {
                            Color::WHITE
                        } else {
                            crate::colors::RED
                        };
                    }
                }
            }
        }
    }

    // Result slot icon.
    let result_alpha = if all_satisfied { 1.0 } else { 0.4 };
    if let Ok(result_entity) = result_slots.get_single() {
        if let Some(result_stack) = proto.get_item_data(recipe_obj).cloned() {
            let stack_count = recipe.2.max(1);
            let icon = spawn_item_stack_icon(
                &mut commands,
                &graphics,
                &result_stack.copy_with_count(stack_count),
                &asset_server,
                Vec2::ZERO,
                Vec2::ZERO,
                3,
            );
            commands
                .entity(icon)
                .insert(CraftingResultIcon)
                .insert(UIState::InventoryCrafting);
            commands.add(move |world: &mut World| {
                if let Some(mut e) = world.get_entity_mut(icon) {
                    if let Some(mut sprite) = e.get_mut::<TextureAtlasSprite>() {
                        sprite.color.set_a(result_alpha);
                    }
                }
            });
            commands.entity(result_entity).push_children(&[icon]);
        }

        if all_satisfied {
            let arrow = commands
                .spawn(AsepriteBundle {
                    aseprite: asset_server.load::<Aseprite, _>(CraftingArrowAse::PATH),
                    animation: AsepriteAnimation::from(CraftingArrowAse::tags::IDLE),
                    transform: Transform::from_translation(Vec3::new(0., 2., 4.)),
                    ..Default::default()
                })
                .insert(Name::new("CRAFTING ARROW INDICATOR"))
                .insert(CraftingArrowIndicator)
                .insert(UIState::InventoryCrafting)
                .insert(RenderLayers::from_layers(&[3]))
                .id();
            commands.entity(result_entity).push_children(&[arrow]);
        }
    }
}

/// Hover + click handler for the craft result slot on the CraftingPanel. Clicking crafts a
/// single instance of the selected recipe and places it onto the player's dragged cursor
/// stack. Repeated clicks accumulate count (up to `MAX_STACK_SIZE`).
pub fn handle_crafting_result_slot_click(
    mut commands: Commands,
    cursor_pos: Res<CursorPos>,
    mut mouse_input: ResMut<Input<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut result_slots: Query<(Entity, &mut Interactable), With<CraftingResultSlot>>,
    cur_ui_state: Res<State<UIState>>,
    selected: Res<SelectedCraftingRecipe>,
    recipes: Res<Recipes>,
    inv_q: Query<&Inventory>,
    proto: ProtoParam,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    dragging_query: Query<(Entity, &ItemStack), With<crate::ui::DraggedItem>>,
    mut crafted_event: EventWriter<CraftedItemEvent>,
    player_atts: Query<&crate::attributes::LootRateBonus, With<crate::player::Player>>,
) {
    if cur_ui_state.0 != UIState::InventoryCrafting {
        return;
    }
    let hit_test = super::ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);

    for (result_entity, mut interactable) in result_slots.iter_mut() {
        // 1. Hover-enter / hover-exit bookkeeping.
        let hovering_this_frame =
            matches!(hit_test, Some((hit_ent, _, _)) if hit_ent == result_entity);
        if !hovering_this_frame {
            if matches!(interactable.current(), Interaction::Hovering) {
                interactable.change(Interaction::None);
            }
            continue;
        }
        if matches!(interactable.current(), Interaction::None) {
            interactable.change(Interaction::Hovering);
        }
        if !left_mouse_pressed {
            continue;
        }

        // 2. Craftability check.
        let Some(recipe_obj) = selected.0 else {
            continue;
        };
        let Some(recipe) = recipes.crafting_list.get(&recipe_obj) else {
            continue;
        };
        let Ok(inv) = inv_q.get_single() else {
            continue;
        };
        let can_craft = recipe
            .0
            .iter()
            .all(|ing| inv.items.get_item_count_in_container(ing.item) >= ing.count);
        if !can_craft {
            continue;
        }

        let stack_count = recipe.2.max(1);

        // 3. Either start a new dragged stack or extend the one we already hold. On each
        //    craft we despawn-and-respawn the icon so the stack-count text refreshes, and we
        //    drive `Interaction::Dragging` on the result slot so that subsequent clicks on
        //    other inventory slots route through the standard drop pipeline.
        let existing_drag = dragging_query.iter().next();
        let (new_stack, old_drag_entity) = if let Some((drag_e, drag_stack)) = existing_drag {
            if drag_stack.obj_type != recipe_obj {
                continue;
            }
            let new_count = (drag_stack.count + stack_count).min(crate::inventory::MAX_STACK_SIZE);
            if new_count == drag_stack.count {
                continue;
            }
            (drag_stack.copy_with_count(new_count), Some(drag_e))
        } else {
            let Some(base_stack) = proto.get_item_data(recipe_obj).cloned() else {
                continue;
            };
            let loot_bonus = player_atts.get_single().map(|a| a.0).unwrap_or(0);
            let rolled = create_new_random_item_stack_with_attributes(
                &base_stack.copy_with_count(stack_count),
                &proto,
                &mut commands,
                loot_bonus,
                false,
            );
            (rolled, None)
        };

        if let Some(old) = old_drag_entity {
            commands.entity(old).despawn_recursive();
        }

        let icon = spawn_item_stack_icon(
            &mut commands,
            &graphics,
            &new_stack,
            &asset_server,
            Vec2::ZERO,
            Vec2::ZERO,
            3,
        );
        commands
            .entity(icon)
            .insert(new_stack.clone())
            .insert(crate::ui::DraggedItem);

        // Put the result slot into Dragging state so the existing drop pipeline can
        // handle dropping the crafted stack into inventory / world.
        interactable.change(Interaction::Dragging {
            item: icon,
            origin_slot: crate::ui::VIRTUAL_DRAG_ORIGIN_SLOT,
        });

        crafted_event.send(CraftedItemEvent { obj: recipe_obj });
        mouse_input.clear();
    }
}

/// Deterministic sort key for the blueprints panel.
///
/// Recipes are grouped into three buckets so the panel always reads the same way:
///   0. Materials / equipment / everything that isn't a consumable.
///   1. Regular consumable foods (anything proto-tagged with [`crate::item::item_actions::ConsumableItem`]
///      that isn't one of the new permanent stat-foods).
///   2. New stat-boost foods (anything matched by
///      [`crate::item::food_recipes::food_recipe_required_eras`]) — pinned to the bottom.
///
/// Within a bucket, items are sorted alphabetically by `WorldObject` debug name so the order
/// is stable between panel rebuilds.
pub fn blueprint_sort_key(obj: WorldObject, proto: &ProtoParam) -> (u8, String) {
    let bucket = if crate::item::food_recipes::food_recipe_required_eras(obj).is_some() {
        2
    } else if proto
        .get_component::<crate::item::item_actions::ConsumableItem, _>(obj)
        .is_some()
        && obj != WorldObject::BridgeBlock
    {
        1
    } else {
        0
    };
    (bucket, format!("{:?}", obj))
}

/// Spawns/refreshes the blueprint row entities + up/down pagination buttons on `blueprint_panel`.
///
/// Used by both `setup_inv_ui` (initial render when the inventory opens in `InventoryCrafting`)
/// and `refresh_blueprints_on_pagination_change` (after the player flips pages).
pub fn render_blueprint_rows_and_nav(
    commands: &mut Commands,
    blueprint_panel: Entity,
    graphics: &Graphics,
    asset_server: &AssetServer,
    proto_param: &ProtoParam,
    recipes: &Recipes,
    era_manager: &crate::world::dimension::EraManager,
    blueprints_pagination: &BlueprintsPagination,
    cur_inv_state: &State<UIState>,
) {
    let mut recipe_list: Vec<WorldObject> = recipes
        .crafting_list
        .keys()
        .copied()
        .filter(|obj| {
            crate::item::food_recipes::is_food_recipe_unlocked(*obj, &era_manager.visited_eras)
        })
        .collect();
    recipe_list.sort_by_key(|obj| blueprint_sort_key(*obj, proto_param));
    let total_pages = if recipe_list.is_empty() {
        1
    } else {
        (recipe_list.len() + MAX_BLUEPRINT_ROWS - 1) / MAX_BLUEPRINT_ROWS
    };
    let page = blueprints_pagination
        .page
        .min(total_pages.saturating_sub(1));
    let start = page * MAX_BLUEPRINT_ROWS;
    let end = (start + MAX_BLUEPRINT_ROWS).min(recipe_list.len());
    for (i, recipe_obj) in recipe_list[start..end].iter().enumerate() {
        let row_y = INV_BLUEPRINT_SLOT_TOP_Y
            - i as f32 * (INV_BLUEPRINT_SLOT_SIZE.y + INV_BLUEPRINT_SLOT_ROW_GAP);
        let row_entity = commands
            .spawn(SpriteBundle {
                texture: graphics.get_ui_element_texture(UIElement::BlueprintSlot),
                sprite: Sprite {
                    custom_size: Some(INV_BLUEPRINT_SLOT_SIZE),
                    ..Default::default()
                },
                transform: Transform {
                    translation: Vec3::new(INV_BLUEPRINT_SLOT_CENTER_X, row_y, 1.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..Default::default()
            })
            .insert(Name::new(format!("BLUEPRINT ROW {:?}", recipe_obj)))
            .insert(UIElement::BlueprintSlot)
            .insert(Interactable::default())
            .insert(BlueprintSlot {
                recipe_obj: *recipe_obj,
            })
            .insert(cur_inv_state.0.clone())
            .insert(RenderLayers::from_layers(&[3]))
            .id();
        if let Some(item_data) = proto_param.get_item_data(*recipe_obj).cloned() {
            let icon_stack = item_data.copy_with_count(1);
            let icon_entity = spawn_item_stack_icon(
                commands,
                graphics,
                &icon_stack,
                asset_server,
                Vec2::new(
                    -INV_BLUEPRINT_SLOT_SIZE.x * 0.5 + INV_BLUEPRINT_SLOT_ICON_X_OFFSET,
                    0.,
                ),
                Vec2::ZERO,
                3,
            );
            commands
                .entity(icon_entity)
                .insert(Name::new("BLUEPRINT ROW ICON"))
                .set_parent(row_entity);
        }
        let label = proto_param
            .get_item_data(*recipe_obj)
            .map(|s| s.metadata.name.clone())
            .unwrap_or_else(|| format!("{:?}", recipe_obj));
        commands
            .spawn(Text2dBundle {
                text: Text::from_section(
                    label,
                    TextStyle {
                        font: asset_server.load("fonts/slkscrbold.ttf"),
                        font_size: 8.4,
                        color: YELLOW_2,
                    },
                ),
                text_anchor: Anchor::CenterLeft,
                transform: Transform {
                    translation: Vec3::new(
                        -INV_BLUEPRINT_SLOT_SIZE.x * 0.5 + INV_BLUEPRINT_SLOT_LABEL_X_OFFSET,
                        0.,
                        1.,
                    ),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .insert(Name::new("BLUEPRINT ROW LABEL"))
            .set_parent(row_entity);
        commands
            .entity(blueprint_panel)
            .push_children(&[row_entity]);
    }

    // Pagination arrows are always rendered for visual consistency. The click handler
    // (`handle_blueprint_pagination_clicks`) gates the actual page change on `page > 0`
    // (prev) / `page + 1 < total_pages` (next), so the buttons are inert at the
    // boundaries — useful when there's only one page or you're already at the first/last.
    let up_btn = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::ButtonPageUp),
            sprite: Sprite {
                custom_size: Some(BLUEPRINT_PAGE_BTN_SIZE),
                ..Default::default()
            },
            transform: Transform::from_translation(Vec3::new(
                BLUEPRINT_PAGE_BTN_UP_X,
                BLUEPRINT_PAGE_BTN_CENTER_Y,
                2.,
            )),
            ..Default::default()
        })
        .insert(UIElement::ButtonPageUp)
        .insert(Interactable::default())
        .insert(BlueprintsPrevButton)
        .insert(cur_inv_state.0.clone())
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("BLUEPRINTS PREV"))
        .id();
    commands.entity(blueprint_panel).push_children(&[up_btn]);

    let down_btn = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::ButtonPageDown),
            sprite: Sprite {
                custom_size: Some(BLUEPRINT_PAGE_BTN_SIZE),
                ..Default::default()
            },
            transform: Transform::from_translation(Vec3::new(
                BLUEPRINT_PAGE_BTN_DOWN_X,
                BLUEPRINT_PAGE_BTN_CENTER_Y,
                2.,
            )),
            ..Default::default()
        })
        .insert(UIElement::ButtonPageDown)
        .insert(Interactable::default())
        .insert(BlueprintsNextButton)
        .insert(cur_inv_state.0.clone())
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("BLUEPRINTS NEXT"))
        .id();
    commands.entity(blueprint_panel).push_children(&[down_btn]);
}

/// System that re-renders blueprint rows when [`BlueprintsPagination`] changes (the player
/// clicked the up/down arrows).  Despawns the existing rows + arrow buttons under the
/// `BlueprintsPanel` and re-runs [`render_blueprint_rows_and_nav`] with the new page.
pub fn refresh_blueprints_on_pagination_change(
    mut commands: Commands,
    blueprints_pagination: Res<BlueprintsPagination>,
    cur_inv_state: Res<State<UIState>>,
    blueprint_panel_q: Query<(Entity, &UIElement)>,
    existing_rows: Query<Entity, With<BlueprintSlot>>,
    existing_prev: Query<Entity, With<BlueprintsPrevButton>>,
    existing_next: Query<Entity, With<BlueprintsNextButton>>,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    proto_param: ProtoParam,
    recipes: Res<Recipes>,
    era_manager: Res<crate::world::dimension::EraManager>,
) {
    if !blueprints_pagination.is_changed() {
        return;
    }
    if cur_inv_state.0 != UIState::InventoryCrafting {
        return;
    }
    let Some((blueprint_panel, _)) = blueprint_panel_q
        .iter()
        .find(|(_, ui)| **ui == UIElement::BlueprintsPanel)
    else {
        return;
    };
    for e in existing_rows.iter() {
        if let Some(ec) = commands.get_entity(e) {
            ec.despawn_recursive();
        }
    }
    for e in existing_prev.iter() {
        if let Some(ec) = commands.get_entity(e) {
            ec.despawn_recursive();
        }
    }
    for e in existing_next.iter() {
        if let Some(ec) = commands.get_entity(e) {
            ec.despawn_recursive();
        }
    }
    render_blueprint_rows_and_nav(
        &mut commands,
        blueprint_panel,
        &graphics,
        &asset_server,
        &proto_param,
        &recipes,
        &era_manager,
        &blueprints_pagination,
        &cur_inv_state,
    );
}

fn blueprint_prev_normal_sprite(commands: &mut Commands, graphics: &Graphics, e: Entity) {
    commands
        .entity(e)
        .insert(UIElement::ButtonPageUp)
        .insert(graphics.get_ui_element_texture(UIElement::ButtonPageUp));
}

fn blueprint_next_normal_sprite(commands: &mut Commands, graphics: &Graphics, e: Entity) {
    commands
        .entity(e)
        .insert(UIElement::ButtonPageDown)
        .insert(graphics.get_ui_element_texture(UIElement::ButtonPageDown));
}

/// Click + hover handler for the blueprint pagination buttons on the bottom of the blueprints
/// panel. Updates [`BlueprintsPagination::page`] on click; swaps `ButtonPage*` / `*Hover` sprites.
pub fn handle_blueprint_pagination_clicks(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut prev_buttons: Query<
        (Entity, &mut Interactable),
        (With<BlueprintsPrevButton>, Without<BlueprintsNextButton>),
    >,
    mut next_buttons: Query<
        (Entity, &mut Interactable),
        (With<BlueprintsNextButton>, Without<BlueprintsPrevButton>),
    >,
    mut pagination: ResMut<BlueprintsPagination>,
    recipes: Res<Recipes>,
    era_manager: Res<crate::world::dimension::EraManager>,
    mut commands: Commands,
    graphics: Res<Graphics>,
) {
    let hit = super::ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    let left_pressed = mouse_input.just_pressed(MouseButton::Left);

    let total_unlocked = recipes
        .crafting_list
        .keys()
        .filter(|obj| {
            crate::item::food_recipes::is_food_recipe_unlocked(**obj, &era_manager.visited_eras)
        })
        .count();
    let total_pages = if total_unlocked == 0 {
        1
    } else {
        (total_unlocked + MAX_BLUEPRINT_ROWS - 1) / MAX_BLUEPRINT_ROWS
    };

    let Some(hit) = hit else {
        for (e, mut interactable) in prev_buttons.iter_mut() {
            if matches!(interactable.current(), Interaction::Hovering) {
                interactable.change(Interaction::None);
                blueprint_prev_normal_sprite(&mut commands, &graphics, e);
            }
        }
        for (e, mut interactable) in next_buttons.iter_mut() {
            if matches!(interactable.current(), Interaction::Hovering) {
                interactable.change(Interaction::None);
                blueprint_next_normal_sprite(&mut commands, &graphics, e);
            }
        }
        return;
    };

    for (e, mut interactable) in prev_buttons.iter_mut() {
        if hit.0 == e {
            match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                    commands
                        .entity(e)
                        .insert(UIElement::ButtonPageUpHover)
                        .insert(graphics.get_ui_element_texture(UIElement::ButtonPageUpHover));
                    commands.spawn(crate::audio::SoundSpawner::new(
                        crate::audio::AudioSoundEffect::ButtonHover,
                        0.05,
                    ));
                }
                Interaction::Hovering => {
                    if left_pressed && pagination.page > 0 {
                        pagination.page -= 1;
                        commands.spawn(crate::audio::SoundSpawner::new(
                            crate::audio::AudioSoundEffect::ButtonClick,
                            0.2,
                        ));
                    }
                }
                _ => {}
            }
        } else if matches!(interactable.current(), Interaction::Hovering) {
            interactable.change(Interaction::None);
            blueprint_prev_normal_sprite(&mut commands, &graphics, e);
        }
    }
    for (e, mut interactable) in next_buttons.iter_mut() {
        if hit.0 == e {
            match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                    commands
                        .entity(e)
                        .insert(UIElement::ButtonPageDownHover)
                        .insert(graphics.get_ui_element_texture(UIElement::ButtonPageDownHover));
                    commands.spawn(crate::audio::SoundSpawner::new(
                        crate::audio::AudioSoundEffect::ButtonHover,
                        0.05,
                    ));
                }
                Interaction::Hovering => {
                    if left_pressed && pagination.page + 1 < total_pages {
                        pagination.page += 1;
                        commands.spawn(crate::audio::SoundSpawner::new(
                            crate::audio::AudioSoundEffect::ButtonClick,
                            0.2,
                        ));
                    }
                }
                _ => {}
            }
        } else if matches!(interactable.current(), Interaction::Hovering) {
            interactable.change(Interaction::None);
            blueprint_next_normal_sprite(&mut commands, &graphics, e);
        }
    }
}
