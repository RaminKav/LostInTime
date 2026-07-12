use bevy::{ecs::system::SystemParam, prelude::*, render::view::RenderLayers, sprite::Anchor};

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
use crate::enemy::spawner::MobSpawningPaused;
use crate::item::active_skill_shrine::assign_shrine_skill_to_slot;
use crate::night::{InfiniteMode, InfiniteModeStartedEvent};
use crate::player::skills::{
    ActiveSkill, ActiveSkillChoiceState, Heirloom, HeirloomChoiceQueue, HeirloomChoiceState,
    HeirloomRarity, PlayerSkills,
};
use crate::player::unlocks::RunUnlockState;
use crate::player::ModifyCurencyEvent;
use crate::proto::proto_param::ProtoParam;
use crate::ui::focus::Focusable;
use crate::ui::game_fonts as gf;
use crate::ui::heirloom_browser_grid::{
    despawn_dev_heirloom_picker_grid_layers, grid_backdrop_size, heirloom_choice_from_full_pool,
    sorted_full_pool_grid_entries, spawn_heirloom_grid_overlay, DevHeirloomPickerGridLayer,
    HeirloomGridContext,
};
use crate::ui::skill_browser_grid::{
    despawn_dev_skill_picker_grid_layers, sorted_dev_skill_grid_entries, spawn_skill_grid_overlay,
    DevSkillPickerGridLayer, DevSkillPickerIcon,
};
use crate::ui::time_crystal_progress_ui::CrystalUnlockIcon;
use crate::ui::{
    BLUEPRINT_PAGE_BTN_CENTER_Y, BLUEPRINT_PAGE_BTN_DOWN_X, BLUEPRINT_PAGE_BTN_SIZE,
    BLUEPRINT_PAGE_BTN_UP_X, INVENTORY_BLUEPRINT_UI_SIZE, INVENTORY_CRAFTING_PANEL_UI_SIZE,
    INVENTORY_EQUIPMENT_UI_SIZE, INVENTORY_UPGRADE_UI_SIZE, INVENTORY_Y_OFFSET,
    INV_BLUEPRINT_SLOT_CENTER_X, INV_BLUEPRINT_SLOT_ICON_X_OFFSET,
    INV_BLUEPRINT_SLOT_LABEL_X_OFFSET, INV_BLUEPRINT_SLOT_ROW_GAP, INV_BLUEPRINT_SLOT_SIZE,
    INV_BLUEPRINT_SLOT_TOP_Y, INV_CRAFTING_INPUT_SLOTS_Y_LOCAL, INV_CRAFTING_INPUT_SLOT_SPACING_X,
    INV_CRAFTING_PANEL_INGREDIENT_COUNT_Y_OFFSET, INV_CRAFTING_PANEL_INGREDIENT_ICON_Y_OFFSET,
    INV_CRAFTING_PANEL_INGREDIENT_ROW_Y,
    INV_CRAFTING_PANEL_INGREDIENT_SPACING_X, INV_CRAFTING_PANEL_RESULT_Y,
    INV_CRAFTING_INGREDIENT_SLOT_SIZE,
    INV_UPGRADE_PANEL_OFFSET_Y_CRAFTING, MAX_BLUEPRINT_ROWS, UI_UPGRADE_SLOT_SIZE,
};
use crate::world::dimension::{DimensionSpawnEvent, Era};
use crate::Player;
use crate::{
    assets::Graphics,
    attributes::{
        add_item_glows, attribute_helpers::create_new_random_item_stack_with_attributes,
        AttributeChangeEvent,
    },
    inventory::{
        try_auto_equip_from_upgrade_slot, BreakDropFilter, DamageTrackerMenuOpen,
        DamageTrackerToggleButton, Inventory, InventoryItemStack, ItemStack,
        MaterialDropFilterAllButton, MaterialDropFilterEntry, MaterialDropFilterEntryX,
        MaterialDropFilterMenuOpen, MaterialDropFilterNoneButton, MaterialDropFilterPanel,
        MaterialDropsToggleButton, SortInventoryButton, BREAK_DROP_FILTER_ITEMS,
    },
    item::{item_drop_outline::UiShadow, CraftedItemEvent, Recipes, WorldObject},
    ui::{FurnaceState, CHEST_INVENTORY_UI_SIZE, INVENTORY_UI_SIZE},
    ScreenResolution,
};
use crate::{GameParam, DEBUG};

use super::{
    crafting_ui::CraftingContainer,
    icon_hover_tooltips::IconHoverTooltipText,
    interactions::{Interactable, Interaction},
    inventory_panel_center_x,
    options_ui::CheatSettings,
    player_hud::FlashExpBarEvent,
    tooltips::ItemOrRecipeTooltip,
    ui_helpers::{spawn_full_screen_ui_overlay, Z_DEPTH_HUD_ACTIVE_SKILLS},
    ShowInvPlayerStatsEvent, UIContainersParam, UIElement, CRAFTING_INVENTORY_UI_SIZE,
    FURNACE_INVENTORY_UI_SIZE, HUD_ACTION_ROW_Y_FROM_BOTTOM, HUD_HOTBAR_CENTER_X, HUD_HOTBAR_SLOTS,
    INVENTORY_GRID_COLS, INV_CHEST_SCRAPPER_GRID_OFFSET_Y, INV_CRAFTING_BASE_Y, INV_CRAFTING_COLS,
    INV_CRAFTING_NUDGE_IN_MAIN_INV, INV_CRAFTING_ROW_GAP, INV_CRAFTING_X_ANCHOR,
    INV_DAMAGE_TRACKER_TOGGLE_OFFSET_X, INV_DAMAGE_TRACKER_TOGGLE_OFFSET_Y,
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
    WellShrine,
    Crafting,
    /// Alternate inventory mode that swaps the equipment panel for the blueprints panel
    /// and the upgrade slots for three normal crafting material slots.
    /// Opened from the Cauldron shrine; Brew crafts the selected recipe into inventory.
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
    /// Main-menu archive modal: bestiary / time crystals / unlocks shortcuts.
    Archives,
    /// Gamepad-only pause overlay during a run (bound to the Start button): pauses gameplay
    /// like any other menu, but doesn't spawn its own panel — instead it lets the player
    /// navigate the always-visible HUD (heirloom icons, active skill icons, pet skill icon)
    /// with the d-pad/stick and see the same tooltips a mouse hover would show. HP/MP bars are
    /// already always-on in the HUD so nothing special is needed to "open" them for this state.
    Pause,
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
pub struct GrantHeirloomDevEvent(pub HeirloomChoiceState);

/// Event to remove one copy of an heirloom from dev mode (handled in a separate system).
pub struct RevokeHeirloomDevEvent(pub Heirloom);

/// When true, the dev heirloom picker grid is shown beside the dev buttons.
#[derive(Resource, Default)]
pub struct DevHeirloomGridOpen(pub bool);

/// Event to grant an active skill from dev mode (handled in a separate system to avoid query conflicts).
pub struct GrantSkillDevEvent {
    pub skill: ActiveSkill,
    /// Player skill slot index: 1 = 2nd slot, 2 = 3rd slot.
    pub slot: usize,
}

/// When true, the dev skill picker grid is shown beside the dev buttons.
#[derive(Resource, Default)]
pub struct DevSkillGridOpen(pub bool);

#[derive(Component, Default, Clone)]
pub struct InventoryUI;

/// Dynamic prompt text shown on the upgrade panel (Inventory mode only).
/// Text switches between "Add Upgrade Material", "Use Tome", and "Use Orb" depending on which
/// consumable is sitting in furnace slot 0 (see `update_upgrade_material_prompt_text`).
#[derive(Component, Default, Clone)]
pub struct UpgradeMaterialPromptText;

/// Brew button on the crafting side panel (`UIState::InventoryCrafting` / Cauldron).
/// Crafts the selected blueprint into the player's inventory (or drops it if full).
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
    SpawnChestHeirloom,
    SpawnTome,
    SpawnOrb,
    TeleportEra2,
    TeleportEra3,
    TriggerEndless,
    AddChaos,
    AddGold,
    DropDungeonKey,
    AddBanishCount,
}

/// Label child of the endless dev button; text switches between "endless" and "+1 min".
#[derive(Component)]
pub struct DevEndlessButtonLabel;

#[derive(SystemParam)]
pub(crate) struct DevEndlessButtonParams<'w> {
    infinite_mode: ResMut<'w, InfiniteMode>,
    infinite_mode_event: EventWriter<'w, InfiniteModeStartedEvent>,
    mob_spawning_paused: Res<'w, MobSpawningPaused>,
}

/// Dev button that toggles the full-pool heirloom picker grid.
#[derive(Component)]
pub struct DevHeirloomPickerToggleButton;

/// Dev button that toggles the full-pool active skill picker grid.
#[derive(Component)]
pub struct DevSkillPickerToggleButton;
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
pub fn cleanup_dev_heirloom_grid_on_inv_close(
    mut grid_open: ResMut<DevHeirloomGridOpen>,
    grid_layers: Query<Entity, With<DevHeirloomPickerGridLayer>>,
    mut commands: Commands,
) {
    grid_open.0 = false;
    despawn_dev_heirloom_picker_grid_layers(&mut commands, &grid_layers);
}

pub fn cleanup_dev_skill_grid_on_inv_close(
    mut grid_open: ResMut<DevSkillGridOpen>,
    grid_layers: Query<Entity, With<DevSkillPickerGridLayer>>,
    mut commands: Commands,
) {
    grid_open.0 = false;
    despawn_dev_skill_picker_grid_layers(&mut commands, &grid_layers);
}

pub fn setup_inv_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    mut inv_state: ResMut<InventoryState>,
    cur_inv_state: Res<State<UIState>>,
    mut dev_grid_open: ResMut<DevHeirloomGridOpen>,
    mut dev_skill_grid_open: ResMut<DevSkillGridOpen>,
    mut stats_event: EventWriter<ShowInvPlayerStatsEvent>,
    resolution: Res<ScreenResolution>,
    asset_server: Res<AssetServer>,
    cheat_settings: Option<Res<CheatSettings>>,
    recipes: Res<Recipes>,
    mut selected_recipe: ResMut<SelectedCraftingRecipe>,
    proto_param: ProtoParam,
    era_manager: Res<crate::world::dimension::EraManager>,
    blueprints_pagination: Res<BlueprintsPagination>,
    damage_tracker_menu: Res<DamageTrackerMenuOpen>,
) {
    dev_grid_open.0 = false;
    dev_skill_grid_open.0 = false;
    let (size, texture, pos_offset) = match cur_inv_state.0 {
        UIState::Inventory | UIState::InventoryCrafting => (
            INVENTORY_UI_SIZE,
            graphics.get_ui_element_texture(UIElement::Inventory),
            Vec2::new(
                inventory_panel_center_x(damage_tracker_menu.0),
                INVENTORY_Y_OFFSET,
            ),
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

    spawn_full_screen_ui_overlay(&mut commands, &resolution, 0.8, 9.);

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
        .insert(UiShadow::container())
        .id();
    let _inv_text = commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                "INVENTORY",
                gf::DISPLAY.text_style(&asset_server, STATS_TITLE),
            ),
            text_anchor: Anchor::Center,
            transform: Transform {
                translation: Vec3::new(3., size.y / 2. - 10., 1.),
                scale: gf::DISPLAY.transform_scale(),
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
                gf::DISPLAY.text_style(&asset_server, HOTBAR_TITLE),
            ),
            text_anchor: Anchor::Center,
            transform: Transform {
                translation: Vec3::new(3., size.y / 2. - 246., 1.),
                scale: gf::DISPLAY.transform_scale(),
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

    // Side panel: CraftingPanel + Brew only in Cauldron blueprints mode.
    // Normal Inventory no longer shows the old CraftButtonContainer under equipment.
    let upgrade_panel = if is_crafting_mode {
        let side_panel_size = INVENTORY_CRAFTING_PANEL_UI_SIZE;
        let side_panel_element = UIElement::CraftingPanel;
        let upgrade_panel = commands
            .spawn(SpriteBundle {
                texture: graphics.get_ui_element_texture(side_panel_element.clone()),
                sprite: Sprite {
                    custom_size: Some(side_panel_size),
                    ..Default::default()
                },
                transform: Transform {
                    translation: Vec3::new(
                        INV_EQUIP_PANEL_OFFSET_X + 3.,
                        INV_UPGRADE_PANEL_OFFSET_Y_CRAFTING,
                        0.,
                    ),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..Default::default()
            })
            .insert(cur_inv_state.0.clone())
            .insert(Name::new("CRAFTING PANEL"))
            .insert(side_panel_element)
            .insert(RenderLayers::from_layers(&[3]))
            .insert(UiShadow::container())
            .id();
        commands.entity(inv).add_child(upgrade_panel);

        let _upgrade_text = commands
            .spawn(Text2dBundle {
                text: Text::from_section(
                    "CRAFTING",
                    gf::DISPLAY.text_style(&asset_server, EQUIP_TITLE),
                ),
                text_anchor: Anchor::Center,
                transform: Transform {
                    translation: Vec3::new(0., side_panel_size.y / 2. - 12., 1.),
                    scale: gf::DISPLAY.transform_scale(),
                    ..Default::default()
                },
                ..default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .insert(Name::new("upgrade/crafting TITLE"))
            .insert(cur_inv_state.0.clone())
            .set_parent(upgrade_panel)
            .id();
        Some((upgrade_panel, side_panel_size))
    } else {
        None
    };

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
                    translation: Vec3::new(INV_EQUIP_PANEL_OFFSET_X, INV_EQUIP_PANEL_OFFSET_Y, 0.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..Default::default()
            })
            .insert(cur_inv_state.0.clone())
            .insert(Name::new("EQUIPMENTS"))
            .insert(RenderLayers::from_layers(&[3]))
            .insert(UiShadow::container())
            .id();
        commands.entity(inv).add_child(equip_panel);
        let _eqp_text = commands
            .spawn(Text2dBundle {
                text: Text::from_section(
                    "EQUIPMENT",
                    gf::DISPLAY.text_style(&asset_server, EQUIP_TITLE),
                ),
                text_anchor: Anchor::Center,
                transform: Transform {
                    translation: Vec3::new(0., INVENTORY_EQUIPMENT_UI_SIZE.y / 2. - 11., 1.),
                    scale: gf::DISPLAY.transform_scale(),
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
    if let Some((upgrade_panel, side_panel_size)) = upgrade_panel {
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
            .insert(UiShadow::container())
            .id();
        let _bp_text = commands
            .spawn(Text2dBundle {
                text: Text::from_section(
                    "BLUEPRINTS",
                    gf::DISPLAY.text_style(&asset_server, STATS_TITLE),
                ),
                text_anchor: Anchor::Center,
                transform: Transform {
                    translation: Vec3::new(0., INVENTORY_BLUEPRINT_UI_SIZE.y / 2. - 11., 1.),
                    scale: gf::DISPLAY.transform_scale(),
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

        // Ingredient display slots on the crafting side panel (x3). Icon + "owned/needed"
        // text are populated by `refresh_crafting_ingredient_display` when a blueprint is picked.
        for i in 0..3 {
            let local_x = 1. + (i as f32 - 1.0) * INV_CRAFTING_PANEL_INGREDIENT_SPACING_X;
            let slot_entity = commands
                .spawn(SpriteBundle {
                    texture: graphics.get_ui_element_texture(UIElement::CraftingIngredientSlot),
                    sprite: Sprite {
                        custom_size: Some(INV_CRAFTING_INGREDIENT_SLOT_SIZE),
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
                // Hover + tooltip via `handle_crafting_ingredient_tooltip_hover` (not
                // `handle_hovering`, which requires `InventorySlotState`).
                .insert(UIElement::CraftingIngredientSlot)
                .insert(Interactable::default())
                .insert(CraftingIngredientDisplaySlot { slot_index: i })
                .insert(Focusable {
                    group: cur_inv_state.0.clone(),
                    index: INV_FOCUS_CRAFT_INGREDIENT_BASE + i as u32,
                })
                .insert(cur_inv_state.0.clone())
                .insert(RenderLayers::from_layers(&[3]))
                .id();

            // "owned/needed" label (4x5 font, size 5). Starts blank.
            let count_text = commands
                .spawn(Text2dBundle {
                    text: Text::from_section("", gf::BODY.text_style(&asset_server, Color::WHITE))
                        .with_alignment(TextAlignment::Center),
                    text_anchor: Anchor::Center,
                    transform: Transform {
                        translation: Vec3::new(
                            0.,
                            INV_CRAFTING_PANEL_INGREDIENT_COUNT_Y_OFFSET,
                            2.,
                        ),
                        scale: gf::BODY.transform_scale(),
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
            // Mouse-only: controllers craft via the Brew button, so this slot is not focusable.
            .insert(cur_inv_state.0.clone())
            .insert(RenderLayers::from_layers(&[3]))
            .id();
        commands.entity(upgrade_panel).push_children(&[result_slot]);

        // Brew button on the Cauldron blueprints crafting panel.
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
            .insert(Focusable {
                group: cur_inv_state.0.clone(),
                index: INV_FOCUS_CRAFT_TOGGLE,
            })
            .insert(cur_inv_state.0.clone())
            .insert(Name::new("BREW BUTTON"))
            .id();
        let _toggle_text = commands
            .spawn(Text2dBundle {
                text: Text::from_section(
                    "Brew",
                    gf::DISPLAY.text_style(&asset_server, CRAFT_BUTTON_TEXT),
                ),
                text_anchor: Anchor::Center,
                transform: Transform {
                    translation: Vec3::new(0., -1., 1.),
                    scale: gf::DISPLAY.transform_scale(),
                    ..Default::default()
                },
                ..default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .insert(Name::new("BREW LABEL"))
            .insert(cur_inv_state.0.clone())
            .set_parent(toggle_button)
            .id();
        commands
            .entity(upgrade_panel)
            .push_children(&[toggle_button]);
    }

    inv_state.inv_size = size;

    // Dev mode buttons (far left of inventory, only when Options > Dev Mode is on)
    let dev_mode = *DEBUG || cheat_settings.map(|c| c.dev_mode).unwrap_or(false);
    if cur_inv_state.0 == UIState::Inventory && dev_mode {
        const DEV_BUTTON_WIDTH: f32 = 38.;
        const DEV_BUTTON_HEIGHT: f32 = 11.;
        const DEV_BUTTON_SPACING: f32 = 14.;
        // Left of inventory panel in local space (inv center is 22, 0.5 in world; panel half-width 109)
        let dev_x = -INVENTORY_UI_SIZE.x / 2. - DEV_BUTTON_WIDTH / 2. - 28.;
        let start_y = 108.0f32;
        let labels: [(DevButtonAction, &str); 13] = [
            (DevButtonAction::GrantXp, "+250 xp"),
            (DevButtonAction::GrantMoreXp, "+1000 xp"),
            (DevButtonAction::SpawnChest, "chest"),
            (DevButtonAction::SpawnChestHeirloom, "hrm chest"),
            (DevButtonAction::SpawnTome, "tome"),
            (DevButtonAction::SpawnOrb, "orb"),
            (DevButtonAction::TeleportEra2, "era2"),
            (DevButtonAction::TeleportEra3, "era3"),
            (DevButtonAction::TriggerEndless, "endless"),
            (DevButtonAction::AddChaos, "+chaos"),
            (DevButtonAction::AddGold, "+50 gold"),
            (DevButtonAction::DropDungeonKey, "key"),
            (DevButtonAction::AddBanishCount, "+banish"),
        ];
        let heirloom_picker_y = start_y - labels.len() as f32 * DEV_BUTTON_SPACING;
        let skill_picker_y = heirloom_picker_y - DEV_BUTTON_SPACING;
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
            let label_entity = commands
                .spawn((
                    Text2dBundle {
                        text: Text::from_section(
                            *label,
                            gf::BODY.text_style(&asset_server, DARK_WOOD_BROWN),
                        )
                        .with_alignment(TextAlignment::Center),
                        text_anchor: Anchor::Center,
                        transform: Transform {
                            translation: Vec3::new(0., 0.5, 1.),
                            scale: gf::BODY.transform_scale(),
                            ..default()
                        },
                        ..Default::default()
                    },
                    RenderLayers::from_layers(&[3]),
                    UIState::Inventory,
                    Name::new("Dev Button Label"),
                ))
                .set_parent(btn)
                .id();
            if matches!(action, DevButtonAction::TriggerEndless) {
                commands.entity(label_entity).insert(DevEndlessButtonLabel);
            }
            commands.entity(inv).add_child(btn);
        }

        let picker_btn = commands
            .spawn(SpriteBundle {
                texture: graphics.get_ui_element_texture(UIElement::XLKey).clone(),
                sprite: Sprite {
                    custom_size: Some(Vec2::new(DEV_BUTTON_WIDTH, DEV_BUTTON_HEIGHT)),
                    ..Default::default()
                },
                transform: Transform::from_xyz(dev_x, heirloom_picker_y, 10.),
                ..Default::default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .insert(UIState::Inventory)
            .insert(Interactable::default())
            .insert(DevHeirloomPickerToggleButton)
            .insert(Name::new("Dev Heirloom Picker Toggle"))
            .id();
        commands
            .spawn((
                Text2dBundle {
                    text: Text::from_section(
                        "heirlooms",
                        gf::BODY.text_style(&asset_server, DARK_WOOD_BROWN),
                    )
                    .with_alignment(TextAlignment::Center),
                    text_anchor: Anchor::Center,
                    transform: Transform {
                        translation: Vec3::new(0., 0.5, 1.),
                        scale: gf::BODY.transform_scale(),
                        ..default()
                    },
                    ..Default::default()
                },
                RenderLayers::from_layers(&[3]),
                UIState::Inventory,
                Name::new("Dev Heirloom Picker Toggle Label"),
            ))
            .set_parent(picker_btn);
        commands.entity(inv).add_child(picker_btn);

        let skill_picker_btn = commands
            .spawn(SpriteBundle {
                texture: graphics.get_ui_element_texture(UIElement::XLKey).clone(),
                sprite: Sprite {
                    custom_size: Some(Vec2::new(DEV_BUTTON_WIDTH, DEV_BUTTON_HEIGHT)),
                    ..Default::default()
                },
                transform: Transform::from_xyz(dev_x, skill_picker_y, 10.),
                ..Default::default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .insert(UIState::Inventory)
            .insert(Interactable::default())
            .insert(DevSkillPickerToggleButton)
            .insert(Name::new("Dev Skill Picker Toggle"))
            .id();
        commands
            .spawn((
                Text2dBundle {
                    text: Text::from_section(
                        "+skills",
                        gf::BODY.text_style(&asset_server, DARK_WOOD_BROWN),
                    )
                    .with_alignment(TextAlignment::Center),
                    text_anchor: Anchor::Center,
                    transform: Transform {
                        translation: Vec3::new(0., 0.5, 1.),
                        scale: gf::BODY.transform_scale(),
                        ..default()
                    },
                    ..Default::default()
                },
                RenderLayers::from_layers(&[3]),
                UIState::Inventory,
                Name::new("Dev Skill Picker Toggle Label"),
            ))
            .set_parent(skill_picker_btn);
        commands.entity(inv).add_child(skill_picker_btn);
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
    damage_tracker_menu: Res<DamageTrackerMenuOpen>,
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
                &inv_state.0,
            );
            // Drop-filter + damage-tracker toggles are inventory-only (hidden on Cauldron blueprints).
            if inv_state.0 == UIState::Inventory {
                spawn_material_drops_toggle_button(
                    &mut commands,
                    &graphics,
                    &asset_server,
                    &inv_query,
                    inv_state_res.inv_size,
                    &inv_state.0,
                );
                spawn_damage_tracker_toggle_button(
                    &mut commands,
                    &graphics,
                    &asset_server,
                    &inv_query,
                    inv_state_res.inv_size,
                    &inv_state.0,
                );
            }
            let (panel_pos_offset, panel_inv_size) =
                inventory_panel_layout(&inv_state.0, damage_tracker_menu.0);
            spawn_material_drop_filter_panel(
                &mut commands,
                &graphics,
                &asset_server,
                &proto_param,
                &inv_state,
                panel_pos_offset,
                panel_inv_size,
                &break_drop_filter,
                menu_open.0 && inv_state.0 == UIState::Inventory,
            );
        }
    }
}

/// Panel center offset and size for the active inventory-family UI state.
fn inventory_panel_layout(ui_state: &UIState, damage_tracker_visible: bool) -> (Vec2, Vec2) {
    match ui_state {
        UIState::Inventory | UIState::InventoryCrafting => (
            Vec2::new(
                inventory_panel_center_x(damage_tracker_visible),
                INVENTORY_Y_OFFSET,
            ),
            INVENTORY_UI_SIZE,
        ),
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
    ui_state: &UIState,
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
        .insert(Focusable {
            group: ui_state.clone(),
            index: INV_FOCUS_SORT,
        })
        .insert(IconHoverTooltipText(&["Sort inventory"]))
        .insert(Name::new("SORT INVENTORY BUTTON"))
        .id();

    let label = commands
        .spawn(Text2dBundle {
            text: Text::from_section("SORT", gf::BODY.text_style(&asset_server, YELLOW_2))
                .with_alignment(TextAlignment::Center),
            text_anchor: Anchor::Center,
            transform: Transform {
                translation: Vec3::new(0., 0., 1.),
                scale: gf::BODY.transform_scale(),
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
    ui_state: &UIState,
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
        .insert(Focusable {
            group: ui_state.clone(),
            index: INV_FOCUS_MATERIAL_DROPS,
        })
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

/// DMG button under the drop-filter button — toggles damage/mob stat side panels.
fn spawn_damage_tracker_toggle_button(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    inv_query: &Query<Entity, With<InventoryUI>>,
    inv_size: Vec2,
    ui_state: &UIState,
) {
    let hw = inv_size.x * 0.5;
    let hh = inv_size.y * 0.5;
    let translation = Vec3::new(
        -hw + INV_DAMAGE_TRACKER_TOGGLE_OFFSET_X,
        hh + INV_DAMAGE_TRACKER_TOGGLE_OFFSET_Y,
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
        .insert(Interactable::default())
        .insert(DamageTrackerToggleButton)
        .insert(Focusable {
            group: ui_state.clone(),
            index: INV_FOCUS_DAMAGE_TRACKER,
        })
        .insert(IconHoverTooltipText(&["Toggle damage tracker"]))
        .insert(Name::new("DAMAGE TRACKER TOGGLE BUTTON"))
        .id();

    let label = commands
        .spawn(Text2dBundle {
            text: Text::from_section("DMG", gf::BODY.text_style(&asset_server, YELLOW_2))
                .with_alignment(TextAlignment::Center),
            text_anchor: Anchor::Center,
            transform: Transform {
                translation: Vec3::new(0., 0., 1.),
                scale: gf::BODY.transform_scale(),
                ..Default::default()
            },
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("DMG LABEL"))
        .id();
    commands.entity(button).push_children(&[label]);

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
                    gf::BODY.text_style(&asset_server, YELLOW_2),
                )
                .with_alignment(TextAlignment::Center),
                text_anchor: Anchor::Center,
                transform: Transform {
                    translation: Vec3::new(0., title_y, 11.),
                    scale: gf::BODY.transform_scale(),
                    ..default()
                },
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
                text: Text::from_section(label, gf::BODY.text_style(&asset_server, Color::WHITE))
                    .with_alignment(TextAlignment::Center),
                text_anchor: Anchor::Center,
                transform: Transform {
                    translation: Vec3::new(0., 0., 1.),
                    scale: gf::BODY.transform_scale(),
                    ..default()
                },
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
                text: Text::from_section("X", gf::DISPLAY.text_style(&asset_server, RED))
                    .with_alignment(TextAlignment::Center),
                text_anchor: Anchor::Center,
                visibility: x_vis,
                transform: Transform {
                    translation: Vec3::new(0., 0., 3.),
                    scale: gf::DISPLAY.transform_scale(),
                    ..default()
                },
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

/// Stable [`Focusable::index`] for an inventory slot. Navigation itself is position-based, so
/// this only drives default-focus (lowest index = main grid slot 0) and tie-breaks. Non-grid
/// slot types get disjoint index ranges so they never collide with the main grid.
fn inv_focus_index(slot_type: InventorySlotType, slot_index: usize) -> u32 {
    let base = match slot_type {
        InventorySlotType::Normal => 0,
        InventorySlotType::Hotbar => INV_FOCUS_HOTBAR_BASE,
        InventorySlotType::Equipment => 100,
        InventorySlotType::Accessory => 110,
        InventorySlotType::Weapon => 120,
        InventorySlotType::Pet => 121,
        InventorySlotType::Trash => 130,
        InventorySlotType::Furnace => 140,
        InventorySlotType::CraftingInput => 150,
        InventorySlotType::Crafting => 160,
        InventorySlotType::Chest => 200,
        InventorySlotType::Scrapper => 300,
    };
    base + slot_index as u32
}

/// Focus index base for HUD hotbar slots (slot 0 → 90, slot 1 → 91, …).
pub const INV_FOCUS_HOTBAR_BASE: u32 = 90;
/// Focus indices for the left-edge sidebar controls (trash slot uses [`inv_focus_index`] → 130).
pub const INV_FOCUS_SORT: u32 = 131;
pub const INV_FOCUS_MATERIAL_DROPS: u32 = 132;
pub const INV_FOCUS_DAMAGE_TRACKER: u32 = 133;
pub const INV_FOCUS_CRAFT_TOGGLE: u32 = 134;
/// Ingredient display slots on the Cauldron crafting panel (0..2 → 170..172).
pub const INV_FOCUS_CRAFT_INGREDIENT_BASE: u32 = 170;
/// Blueprint rows on the blueprints panel (row i → 180 + i).
pub const INV_FOCUS_BLUEPRINT_ROW_BASE: u32 = 180;
pub const INV_FOCUS_BLUEPRINT_PREV: u32 = 190;
pub const INV_FOCUS_BLUEPRINT_NEXT: u32 = 191;

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
        let mut count_text_offset =
            if slot_index % INVENTORY_GRID_COLS == 3 || slot_index % INVENTORY_GRID_COLS == 2 {
                Vec2::new(0.5, 0.)
            } else {
                Vec2::ZERO
            };
        // HUD hotbar count labels sit near the slot's bottom edge — nudge them down a bit
        // so they don't overlap the inventory UI overlay when the inventory is opened.
        if slot_type.is_hotbar() {
            count_text_offset.y -= 2.0;
        }
        item_icon_option = Some(spawn_item_stack_icon(
            commands,
            graphics,
            &item.item_stack,
            asset_server,
            Vec2::ZERO,
            count_text_offset,
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

    // Controller/keyboard focus navigation (Track 4): panel slots get `Focusable` at spawn time.
    // HUD hotbar slots are a separate row at the bottom of the screen — they stay hidden and
    // non-focusable while the inventory panel is open; hotbar items are managed via the main
    // bag grid (Normal slots 0..N, same backing indices).
    if inv_ui_state.0.is_inv_open() && !slot_type.is_hotbar() {
        slot_entity.insert(Focusable {
            group: inv_ui_state.0.clone(),
            index: inv_focus_index(slot_type, slot_index),
        });
    }

    if let Some(icon_entity) = icon_entity_option {
        slot_entity.push_children(&[icon_entity]);
    }
    slot_entity.id()
}
/// Marker on the `Text2dBundle` child that renders an item stack's count number on top of
/// an icon. Used by `update_dragged_item_stack_count_text` to find / update / despawn the
/// label on dragged items without relying on the entity's `Name`.
#[derive(Component)]
pub struct StackCountText;

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
                        gf::BODY.text_style(&asset_server, Color::WHITE),
                    )
                    .with_alignment(TextAlignment::Center),
                    transform: Transform {
                        translation: Vec3::new(7., -5.5, 3.) + text_offset.extend(0.),
                        scale: gf::BODY.transform_scale(),
                        ..Default::default()
                    },
                    ..default()
                },
                Name::new("ITEM STACK TEXT"),
                StackCountText,
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
    item_tooltips: Query<Entity, With<ItemOrRecipeTooltip>>,
) {
    let mut despawned_item_tooltips = false;
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
            if !despawned_item_tooltips {
                for tooltip in item_tooltips.iter() {
                    commands.entity(tooltip).despawn_recursive();
                }
                despawned_item_tooltips = true;
            }
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

/// Updates the endless dev button label when endless mode starts or ends.
pub fn sync_dev_endless_button_label(
    infinite_mode: Res<InfiniteMode>,
    mut labels: Query<&mut Text, With<DevEndlessButtonLabel>>,
) {
    let label = if infinite_mode.active {
        "+1 min"
    } else {
        "endless"
    };
    for mut text in labels.iter_mut() {
        if text.sections[0].value != label {
            text.sections[0].value = label.to_string();
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
    mut endless_params: DevEndlessButtonParams,
    mut chaos_tracker: ResMut<ChaosTracker>,
    mut currency_event: EventWriter<ModifyCurencyEvent>,
    mut run_unlock_state: ResMut<RunUnlockState>,
    mut grant_heirloom_dev: EventWriter<GrantHeirloomDevEvent>,
) {
    let hit_test = super::ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None, None);
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
                        let xp_rate_bonus = game.get_xp_rate_bonus();
                        let mut player_level = game.get_player_level_mut();
                        let (did_level, gained_xp) =
                            player_level.add_xp(250, xp_rate_bonus, &mut chaos_tracker);
                        flash_event.send(FlashExpBarEvent {
                            amount: gained_xp,
                            did_level,
                        });
                    }
                    DevButtonAction::GrantMoreXp => {
                        let xp_rate_bonus = game.get_xp_rate_bonus();
                        let mut player_level = game.get_player_level_mut();
                        let (did_level, gained_xp) =
                            player_level.add_xp(1000, xp_rate_bonus, &mut chaos_tracker);
                        flash_event.send(FlashExpBarEvent {
                            amount: gained_xp,
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
                    DevButtonAction::SpawnChestHeirloom => {
                        let _ = proto_commands.spawn_item_from_proto(
                            WorldObject::HeirloomChest,
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
                        if endless_params.infinite_mode.active {
                            endless_params.infinite_mode.add_elapsed_seconds(60.0);
                        } else {
                            endless_params.infinite_mode_event.send_default();
                            if endless_params.mob_spawning_paused.paused {
                                commands.insert_resource(MobSpawningPaused { paused: false });
                            }
                        }
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
    mut player_query: Query<
        (Entity, &Transform, &mut PlayerSkills, &crate::PlayerLevel),
        With<Player>,
    >,
    mut skill_queue: ResMut<HeirloomChoiceQueue>,
    mut commands: Commands,
    mut att_event: EventWriter<AttributeChangeEvent>,
    mut proto_commands: ProtoCommands,
    proto: ProtoParam,
) {
    for GrantHeirloomDevEvent(choice) in grant_events.iter() {
        if let Ok((player_entity, transform, mut skills, level)) = player_query.get_single_mut() {
            skill_queue.grant_heirloom_from_pool(
                choice.clone(),
                &mut proto_commands,
                &proto,
                transform.translation.truncate(),
                &mut skills,
                level.level,
            );
            choice
                .heirloom
                .add_heirloom_components(player_entity, &mut commands, skills.clone());
            att_event.send(AttributeChangeEvent);
        }
    }
}

/// Removes one copy of an heirloom from the player in dev mode.
pub fn apply_revoke_heirloom_dev(
    mut revoke_events: EventReader<RevokeHeirloomDevEvent>,
    mut player_query: Query<(Entity, &mut PlayerSkills), With<Player>>,
    mut commands: Commands,
    mut att_event: EventWriter<AttributeChangeEvent>,
) {
    for RevokeHeirloomDevEvent(heirloom) in revoke_events.iter() {
        if let Ok((player_entity, mut skills)) = player_query.get_single_mut() {
            let Some(idx) = skills
                .heirlooms
                .iter()
                .position(|h| h.heirloom == *heirloom)
            else {
                continue;
            };
            skills.heirlooms.remove(idx);
            heirloom.add_heirloom_components(player_entity, &mut commands, skills.clone());
            att_event.send(AttributeChangeEvent);
        }
    }
}

/// Toggles the dev heirloom picker grid to the right of the dev buttons.
pub fn handle_dev_heirloom_picker_toggle(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut toggle_buttons: Query<(Entity, &mut Interactable), With<DevHeirloomPickerToggleButton>>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    graphics: Res<Graphics>,
    mut grid_open: ResMut<DevHeirloomGridOpen>,
    mut skill_grid_open: ResMut<DevSkillGridOpen>,
    grid_layers: Query<Entity, With<DevHeirloomPickerGridLayer>>,
    skill_grid_layers: Query<Entity, With<DevSkillPickerGridLayer>>,
    inv_ui: Query<Entity, With<InventoryUI>>,
) {
    let hit_test = super::ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None, None);
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);

    for (e, mut interactable) in toggle_buttons.iter_mut() {
        let hit = match &hit_test {
            Some((hit_ent, _, _)) => *hit_ent == e,
            None => false,
        };
        if hit {
            interactable.change(Interaction::Hovering);
            if left_mouse_pressed {
                let show = !grid_open.0;
                despawn_dev_heirloom_picker_grid_layers(&mut commands, &grid_layers);
                despawn_dev_skill_picker_grid_layers(&mut commands, &skill_grid_layers);
                skill_grid_open.0 = false;
                if show {
                    if let Ok(inv_entity) = inv_ui.get_single() {
                        const DEV_BUTTON_WIDTH: f32 = 38.;
                        let dev_x = -INVENTORY_UI_SIZE.x / 2. - DEV_BUTTON_WIDTH / 2.;
                        let entries: Vec<_> = sorted_full_pool_grid_entries()
                            .into_iter()
                            .map(|(h, r)| (h, r, true))
                            .collect();
                        let entry_count = entries.len();
                        let (backdrop_w, _, _) = grid_backdrop_size(entry_count, 400., 320.);
                        let grid_center_x = dev_x + DEV_BUTTON_WIDTH * 0.5 + 8. + backdrop_w * 0.5;
                        spawn_heirloom_grid_overlay(
                            &mut commands,
                            &asset_server,
                            &graphics,
                            Vec2::new(grid_center_x, 0.),
                            400.,
                            320.,
                            entries,
                            HeirloomGridContext::DevHeirloomPicker,
                            Some(inv_entity),
                        );
                    }
                }
                grid_open.0 = show;
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

/// Click an icon in the dev heirloom picker to grant that heirloom from the full pool.
/// Right-click removes one copy if the player already has it.
pub fn handle_dev_heirloom_picker_clicks(
    grid_open: Res<DevHeirloomGridOpen>,
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut icons: Query<
        (Entity, &mut Interactable, &CrystalUnlockIcon),
        With<DevHeirloomPickerGridLayer>,
    >,
    player_skills: Query<&PlayerSkills, With<Player>>,
    mut grant_heirloom_dev: EventWriter<GrantHeirloomDevEvent>,
    mut revoke_heirloom_dev: EventWriter<RevokeHeirloomDevEvent>,
    mut commands: Commands,
) {
    if !grid_open.0 {
        return;
    }

    let hit_test = super::ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None, None);
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);
    let right_mouse_pressed = mouse_input.just_pressed(MouseButton::Right);
    let skills = player_skills.get_single().ok();

    for (entity, mut interactable, icon) in icons.iter_mut() {
        let hit = match &hit_test {
            Some((hit_ent, _, _)) => *hit_ent == entity,
            None => false,
        };
        if hit {
            if !matches!(interactable.current(), Interaction::Hovering) {
                interactable.change(Interaction::Hovering);
            }
            if left_mouse_pressed {
                let choice = heirloom_choice_from_full_pool(icon.heirloom.clone(), icon.rarity);
                grant_heirloom_dev.send(GrantHeirloomDevEvent(choice));
                commands.spawn(crate::audio::SoundSpawner::new(
                    crate::audio::AudioSoundEffect::ButtonClick,
                    0.2,
                ));
            } else if right_mouse_pressed && skills.is_some_and(|s| s.has(icon.heirloom.clone())) {
                revoke_heirloom_dev.send(RevokeHeirloomDevEvent(icon.heirloom.clone()));
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

/// Applies dev-mode active skill grants (separate system to avoid query conflicts).
pub fn apply_grant_skill_dev(
    mut grant_events: EventReader<GrantSkillDevEvent>,
    mut player_query: Query<(Entity, &mut PlayerSkills), With<Player>>,
    mut commands: Commands,
    mut att_event: EventWriter<AttributeChangeEvent>,
) {
    for GrantSkillDevEvent { skill, slot } in grant_events.iter() {
        if *slot != 1 && *slot != 2 {
            continue;
        }
        if let Ok((player_entity, mut skills)) = player_query.get_single_mut() {
            let choice = ActiveSkillChoiceState::new(*skill, HeirloomRarity::Common);
            assign_shrine_skill_to_slot(&mut skills, *slot, choice);
            skill.add_skill_components(player_entity, &mut commands);
            att_event.send(AttributeChangeEvent);
        }
    }
}

/// Toggles the dev skill picker grid to the right of the dev buttons.
pub fn handle_dev_skill_picker_toggle(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut toggle_buttons: Query<(Entity, &mut Interactable), With<DevSkillPickerToggleButton>>,
    mut commands: Commands,
    graphics: Res<Graphics>,
    mut grid_open: ResMut<DevSkillGridOpen>,
    mut heirloom_grid_open: ResMut<DevHeirloomGridOpen>,
    grid_layers: Query<Entity, With<DevSkillPickerGridLayer>>,
    heirloom_grid_layers: Query<Entity, With<DevHeirloomPickerGridLayer>>,
    inv_ui: Query<Entity, With<InventoryUI>>,
) {
    let hit_test = super::ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None, None);
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);

    for (e, mut interactable) in toggle_buttons.iter_mut() {
        let hit = match &hit_test {
            Some((hit_ent, _, _)) => *hit_ent == e,
            None => false,
        };
        if hit {
            interactable.change(Interaction::Hovering);
            if left_mouse_pressed {
                let show = !grid_open.0;
                despawn_dev_skill_picker_grid_layers(&mut commands, &grid_layers);
                despawn_dev_heirloom_picker_grid_layers(&mut commands, &heirloom_grid_layers);
                heirloom_grid_open.0 = false;
                if show {
                    if let Ok(inv_entity) = inv_ui.get_single() {
                        const DEV_BUTTON_WIDTH: f32 = 38.;
                        let dev_x = -INVENTORY_UI_SIZE.x / 2. - DEV_BUTTON_WIDTH / 2.;
                        let entries = sorted_dev_skill_grid_entries();
                        let entry_count = entries.len();
                        let (backdrop_w, _, _) = grid_backdrop_size(entry_count, 400., 320.);
                        let grid_center_x = dev_x + DEV_BUTTON_WIDTH * 0.5 + 8. + backdrop_w * 0.5;
                        spawn_skill_grid_overlay(
                            &mut commands,
                            &graphics,
                            Vec2::new(grid_center_x, 0.),
                            400.,
                            320.,
                            entries,
                            Some(inv_entity),
                        );
                    }
                }
                grid_open.0 = show;
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

/// Click an icon in the dev skill picker to grant that skill to slot 2 (left) or 3 (right).
pub fn handle_dev_skill_picker_clicks(
    grid_open: Res<DevSkillGridOpen>,
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut icons: Query<
        (Entity, &mut Interactable, &DevSkillPickerIcon),
        With<DevSkillPickerGridLayer>,
    >,
    mut grant_skill_dev: EventWriter<GrantSkillDevEvent>,
    mut commands: Commands,
) {
    if !grid_open.0 {
        return;
    }

    let hit_test = super::ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None, None);
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);
    let right_mouse_pressed = mouse_input.just_pressed(MouseButton::Right);

    for (entity, mut interactable, icon) in icons.iter_mut() {
        let hit = match &hit_test {
            Some((hit_ent, _, _)) => *hit_ent == entity,
            None => false,
        };
        if hit {
            if !matches!(interactable.current(), Interaction::Hovering) {
                interactable.change(Interaction::Hovering);
            }
            if left_mouse_pressed {
                grant_skill_dev.send(GrantSkillDevEvent {
                    skill: icon.active_skill,
                    slot: 1,
                });
                commands.spawn(crate::audio::SoundSpawner::new(
                    crate::audio::AudioSoundEffect::ButtonClick,
                    0.2,
                ));
            } else if right_mouse_pressed {
                grant_skill_dev.send(GrantSkillDevEvent {
                    skill: icon.active_skill,
                    slot: 2,
                });
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

/// Handles hover + click on the Brew button in `UIState::InventoryCrafting` (Cauldron).
/// Crafts the selected recipe into the player's inventory, or drops it at the player if full.
pub fn handle_cursor_inventory_craft_toggle_button(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    key_input: Res<Input<KeyCode>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut toggle_buttons: Query<
        (Entity, &mut Interactable, &CraftModeToggleButton),
        Without<InventorySlotState>,
    >,
    mut commands: Commands,
    curr_ui_state: Res<State<UIState>>,
    mut brew: BrewCraftParams,
    ui_focus: Res<crate::ui::focus::UiFocus>,
    mouseless: Res<crate::inputs::MouselessModeState>,
) {
    if curr_ui_state.0 != UIState::InventoryCrafting {
        return;
    }
    let hit_test = super::ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None, None);
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);
    let focus_driving = mouseless.0 || cursor_pos.suppress_ui_hover;
    let shift_key_pressed = key_input.pressed(KeyCode::LShift);

    for (e, mut interactable, _) in toggle_buttons.iter_mut() {
        let is_focused = ui_focus.is_focused(e);
        let is_hit = hit_test.map(|(ent, _, _)| ent == e).unwrap_or(false);

        // Mouse hover only — focus-driven hover is handled by `sync_inventory_focus_hover`.
        // Clear on mouse leave unless focus is actively driving hover for this button
        // (otherwise mouseless focus can leave the button stuck in Hovering forever).
        if is_hit {
            if !matches!(interactable.current(), Interaction::Hovering) {
                interactable.change(Interaction::Hovering);
            }
        } else if matches!(interactable.current(), Interaction::Hovering)
            && !(focus_driving && is_focused)
        {
            interactable.change(Interaction::None);
        }

        if !((left_mouse_pressed && is_hit)
            || (focus_driving && is_focused && ui_focus.confirm_just_pressed))
        {
            continue;
        }

        let Some(recipe_obj) = brew.selected.0 else {
            continue;
        };
        let Some(recipe) = brew.recipes.crafting_list.get(&recipe_obj).cloned() else {
            continue;
        };
        let Ok(mut inventory) = brew.inv.get_single_mut() else {
            continue;
        };
        let can_craft = recipe
            .0
            .iter()
            .all(|ing| inventory.items.get_item_count_in_container(ing.item) >= ing.count);
        if !can_craft {
            continue;
        }

        let stack_count = recipe.2.max(1);
        let max_batches = recipe
            .0
            .iter()
            .map(|ing| inventory.items.get_item_count_in_container(ing.item) / ing.count)
            .min()
            .unwrap_or(0);
        if max_batches == 0 {
            continue;
        }
        let craft_batches = if shift_key_pressed { max_batches } else { 1 };
        let total_output = craft_batches * stack_count;

        let player_pos = brew
            .player_tf
            .get_single()
            .map(|t| t.translation().truncate())
            .unwrap_or(Vec2::ZERO);
        let loot_bonus = brew.player_atts.get_single().map(|a| a.0).unwrap_or(0);

        let (rolled, allow_hotbar_band) = {
            let proto = brew.params.p0();
            let Some(base_stack) = proto.get_item_data(recipe_obj).cloned() else {
                continue;
            };
            let allow_hotbar = proto
                .get_component::<crate::item::item_actions::ConsumableItem, _>(recipe_obj)
                .is_some();
            let rolled = create_new_random_item_stack_with_attributes(
                &base_stack.copy_with_count(total_output),
                &proto,
                &mut commands,
                loot_bonus,
                false,
            );
            (rolled, allow_hotbar)
        };

        for _ in 0..craft_batches {
            brew.crafted_event
                .send(CraftedItemEvent { obj: recipe_obj });
        }

        let add_result = {
            let mut game = brew.params.p1();
            crate::inventory::try_add_item_stack_to_inventory(
                rolled,
                &mut inventory,
                &mut game.inv_slot_query,
                allow_hotbar_band,
            )
        };
        if let Err(stack) = add_result {
            let mut game = brew.params.p1();
            stack.spawn_as_drop(&mut commands, &mut game, player_pos);
        }

        commands.spawn(crate::audio::SoundSpawner::new(
            crate::audio::AudioSoundEffect::ButtonClick,
            0.2,
        ));
    }
}

#[derive(SystemParam)]
pub struct BrewCraftParams<'w, 's> {
    pub selected: Res<'w, SelectedCraftingRecipe>,
    pub recipes: Res<'w, Recipes>,
    pub inv: Query<'w, 's, &'static mut Inventory>,
    pub crafted_event: EventWriter<'w, CraftedItemEvent>,
    pub player_atts: Query<
        'w,
        's,
        &'static crate::attributes::LootRateBonus,
        With<crate::player::Player>,
    >,
    pub player_tf: Query<'w, 's, &'static GlobalTransform, With<crate::player::Player>>,
    pub params: ParamSet<'w, 's, (ProtoParam<'w, 's>, crate::GameParam<'w, 's>)>,
}

/// Hover handler for the three ingredient display slots on the crafting side panel.
/// Shows the ingredient item tooltip (same pipeline as inventory slots, not recipe view).
pub fn handle_crafting_ingredient_tooltip_hover(
    cursor_pos: Res<CursorPos>,
    key_input: Res<Input<KeyCode>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    slot_transforms: Query<&GlobalTransform>,
    mut ingredient_slots: Query<(Entity, &mut Interactable, &CraftingIngredientDisplaySlot)>,
    selected: Res<SelectedCraftingRecipe>,
    recipes: Res<Recipes>,
    proto: ProtoParam,
    mut commands: Commands,
    graphics: Res<Graphics>,
    mut tooltip_update: EventWriter<crate::ui::ToolTipUpdateEvent>,
    mut tooltip_teardown: EventWriter<crate::ui::TooltipTeardownEvent>,
    cur_ui_state: Res<State<UIState>>,
    ui_focus: Res<crate::ui::focus::UiFocus>,
) {
    if cur_ui_state.0 != UIState::InventoryCrafting {
        return;
    }
    let hit_test = super::ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None, None);
    let shift_key_pressed = key_input.pressed(KeyCode::LShift);
    let shift_key_just_pressed = key_input.just_pressed(KeyCode::LShift);
    let shift_key_just_released = key_input.just_released(KeyCode::LShift);

    for (e, mut interactable, slot) in ingredient_slots.iter_mut() {
        let is_hit = hit_test.map(|(ent, _, _)| ent == e).unwrap_or(false);
        let is_focused = ui_focus.is_focused(e);
        let hovering = is_hit || is_focused;
        match (hovering, interactable.current()) {
            (true, Interaction::None) => {
                interactable.change(Interaction::Hovering);
                commands
                    .entity(e)
                    .insert(UIElement::CraftingIngredientSlotHover)
                    .insert(graphics.get_ui_element_texture(UIElement::CraftingIngredientSlotHover));
                commands.spawn(crate::audio::SoundSpawner::new(
                    crate::audio::AudioSoundEffect::UISlotHover,
                    0.2,
                ));
                send_crafting_ingredient_tooltip(
                    &selected,
                    &recipes,
                    &proto,
                    slot.slot_index,
                    shift_key_pressed,
                    slot_transforms
                        .get(e)
                        .ok()
                        .map(|t| t.translation().truncate()),
                    &mut tooltip_update,
                );
            }
            (true, Interaction::Hovering) => {
                if shift_key_just_pressed || shift_key_just_released {
                    tooltip_teardown.send_default();
                    send_crafting_ingredient_tooltip(
                        &selected,
                        &recipes,
                        &proto,
                        slot.slot_index,
                        shift_key_pressed,
                        slot_transforms
                            .get(e)
                            .ok()
                            .map(|t| t.translation().truncate()),
                        &mut tooltip_update,
                    );
                }
            }
            (false, Interaction::Hovering) => {
                interactable.change(Interaction::None);
                tooltip_teardown.send_default();
                commands
                    .entity(e)
                    .insert(UIElement::CraftingIngredientSlot)
                    .insert(graphics.get_ui_element_texture(UIElement::CraftingIngredientSlot));
            }
            _ => {}
        }
    }
}

fn send_crafting_ingredient_tooltip(
    selected: &SelectedCraftingRecipe,
    recipes: &Recipes,
    proto: &ProtoParam,
    slot_index: usize,
    show_range: bool,
    anchor_ui: Option<Vec2>,
    tooltip_update: &mut EventWriter<crate::ui::ToolTipUpdateEvent>,
) {
    let Some(recipe_obj) = selected.0 else {
        return;
    };
    let Some(recipe) = recipes.crafting_list.get(&recipe_obj) else {
        return;
    };
    let Some(ingredient) = recipe.0.get(slot_index) else {
        return;
    };
    if let Some(item_data) = proto.get_item_data(ingredient.item) {
        tooltip_update.send(crate::ui::ToolTipUpdateEvent {
            item_stack: item_data.clone(),
            is_recipe: false,
            show_range,
            anchor_ui,
            pin_right: true,
            ..Default::default()
        });
    }
}

/// Hover + click handler for blueprint row slots on the BlueprintsPanel (InventoryCrafting).
/// - Hover enters transition -> emit a recipe tooltip (`is_recipe = true`).
/// - Hover exits -> despawn the tooltip.
/// - Click / Confirm -> set `SelectedCraftingRecipe`.
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
    ui_focus: Res<crate::ui::focus::UiFocus>,
) {
    if cur_ui_state.0 != UIState::InventoryCrafting {
        return;
    }
    let hit_test = super::ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None, None);
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);

    for (e, mut interactable, bp) in blueprint_slots.iter_mut() {
        let is_hit = hit_test.map(|(ent, _, _)| ent == e).unwrap_or(false);
        let is_focused = ui_focus.is_focused(e);
        let hovering = is_hit || is_focused;
        let confirm_pressed =
            (is_hit && left_mouse_pressed) || (is_focused && ui_focus.confirm_just_pressed);

        match (hovering, interactable.current()) {
            (true, Interaction::None) => {
                interactable.change(Interaction::Hovering);
                if let Some(item_data) = proto.get_item_data(bp.recipe_obj) {
                    tooltip_update.send(crate::ui::ToolTipUpdateEvent {
                        item_stack: item_data.clone(),
                        is_recipe: true,
                        show_range: false,
                        ..Default::default()
                    });
                }
            }
            (true, Interaction::Hovering) => {
                if confirm_pressed {
                    selected.0 = Some(bp.recipe_obj);
                    commands.spawn(crate::audio::SoundSpawner::new(
                        crate::audio::AudioSoundEffect::ButtonClick,
                        0.2,
                    ));
                }
            }
            (false, Interaction::Hovering) => {
                interactable.change(Interaction::None);
                tooltip_teardown.send_default();
            }
            _ => {}
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
                    Vec2::new(0., INV_CRAFTING_PANEL_INGREDIENT_ICON_Y_OFFSET),
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

#[derive(SystemParam)]
pub struct CraftingResultClickParams<'w, 's> {
    pub selected: Res<'w, SelectedCraftingRecipe>,
    pub recipes: Res<'w, Recipes>,
    pub inv_q: Query<'w, 's, &'static Inventory>,
    pub proto: ProtoParam<'w, 's>,
    pub graphics: Res<'w, Graphics>,
    pub asset_server: Res<'w, AssetServer>,
    pub dragging_query: Query<'w, 's, (Entity, &'static ItemStack), With<crate::ui::DraggedItem>>,
    pub crafted_event: EventWriter<'w, CraftedItemEvent>,
    pub player_atts:
        Query<'w, 's, &'static crate::attributes::LootRateBonus, With<crate::player::Player>>,
}

/// Hover + click handler for the craft result slot on the CraftingPanel. Mouse-only — controllers
/// use the Brew button. Clicking crafts a single instance of the selected recipe onto the
/// dragged cursor stack. Repeated clicks accumulate count (up to `MAX_STACK_SIZE`).
pub fn handle_crafting_result_slot_click(
    mut commands: Commands,
    cursor_pos: Res<CursorPos>,
    mut mouse_input: ResMut<Input<MouseButton>>,
    key_input: Res<Input<KeyCode>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut result_slots: Query<(Entity, &mut Interactable), With<CraftingResultSlot>>,
    cur_ui_state: Res<State<UIState>>,
    mut params: CraftingResultClickParams,
) {
    if cur_ui_state.0 != UIState::InventoryCrafting {
        return;
    }
    let hit_test = super::ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None, None);
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);
    let shift_key_pressed = key_input.pressed(KeyCode::LShift);

    for (result_entity, mut interactable) in result_slots.iter_mut() {
        let is_hit = matches!(hit_test, Some((hit_ent, _, _)) if hit_ent == result_entity);
        let hovering = is_hit;
        let confirm_pressed = is_hit && left_mouse_pressed;

        if !hovering {
            if matches!(interactable.current(), Interaction::Hovering) {
                interactable.change(Interaction::None);
            }
            continue;
        }
        if matches!(interactable.current(), Interaction::None) {
            interactable.change(Interaction::Hovering);
        }
        if !confirm_pressed {
            continue;
        }

        // 2. Craftability check.
        let Some(recipe_obj) = params.selected.0 else {
            continue;
        };
        let Some(recipe) = params.recipes.crafting_list.get(&recipe_obj) else {
            continue;
        };
        let Ok(inv) = params.inv_q.get_single() else {
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
        let max_batches = recipe
            .0
            .iter()
            .map(|ing| inv.items.get_item_count_in_container(ing.item) / ing.count)
            .min()
            .unwrap_or(0);
        if max_batches == 0 {
            continue;
        }

        let existing_drag = params.dragging_query.iter().next();
        let current_drag_count = existing_drag.map(|(_, stack)| stack.count).unwrap_or(0);
        if existing_drag.is_some() && existing_drag.map(|(_, s)| s.obj_type) != Some(recipe_obj) {
            continue;
        }

        let space_for_batches = if current_drag_count >= crate::inventory::MAX_STACK_SIZE {
            0
        } else {
            (crate::inventory::MAX_STACK_SIZE - current_drag_count) / stack_count
        };
        let craft_batches = if shift_key_pressed {
            max_batches.min(space_for_batches.max(1))
        } else {
            1
        };
        if craft_batches == 0 {
            continue;
        }
        let total_output = craft_batches * stack_count;

        // Either start a new dragged stack or extend the one we already hold. On each
        // craft we despawn-and-respawn the icon so the stack-count text refreshes, and we
        // drive `Interaction::Dragging` on the result slot so that subsequent clicks on
        // other inventory slots route through the standard drop pipeline.
        let (new_stack, old_drag_entity) = if let Some((drag_e, drag_stack)) = existing_drag {
            let new_count = (drag_stack.count + total_output).min(crate::inventory::MAX_STACK_SIZE);
            if new_count == drag_stack.count {
                continue;
            }
            (drag_stack.copy_with_count(new_count), Some(drag_e))
        } else {
            let Some(base_stack) = params.proto.get_item_data(recipe_obj).cloned() else {
                continue;
            };
            let loot_bonus = params.player_atts.get_single().map(|a| a.0).unwrap_or(0);
            let rolled = create_new_random_item_stack_with_attributes(
                &base_stack.copy_with_count(total_output),
                &params.proto,
                &mut commands,
                loot_bonus,
                false,
            );
            (rolled, None)
        };

        for _ in 0..craft_batches {
            params.crafted_event.send(CraftedItemEvent { obj: recipe_obj });
        }

        if let Some(old) = old_drag_entity {
            commands.entity(old).despawn_recursive();
        }

        let icon = spawn_item_stack_icon(
            &mut commands,
            &params.graphics,
            &new_stack,
            &params.asset_server,
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
            .insert(Focusable {
                group: cur_inv_state.0.clone(),
                index: INV_FOCUS_BLUEPRINT_ROW_BASE + i as u32,
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
                text: Text::from_section(label, gf::BODY.text_style(&asset_server, YELLOW_2)),
                text_anchor: Anchor::CenterLeft,
                transform: Transform {
                    translation: Vec3::new(
                        -INV_BLUEPRINT_SLOT_SIZE.x * 0.5 + INV_BLUEPRINT_SLOT_LABEL_X_OFFSET,
                        0.,
                        1.,
                    ),
                    scale: gf::BODY.transform_scale(),
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
        .insert(Focusable {
            group: cur_inv_state.0.clone(),
            index: INV_FOCUS_BLUEPRINT_PREV,
        })
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
        .insert(Focusable {
            group: cur_inv_state.0.clone(),
            index: INV_FOCUS_BLUEPRINT_NEXT,
        })
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
    ui_focus: Res<crate::ui::focus::UiFocus>,
) {
    let hit = super::ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None, None);
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

    for (e, mut interactable) in prev_buttons.iter_mut() {
        let is_hit = hit.map(|(ent, _, _)| ent == e).unwrap_or(false);
        let is_focused = ui_focus.is_focused(e);
        let hovering = is_hit || is_focused;
        let confirm_pressed =
            (is_hit && left_pressed) || (is_focused && ui_focus.confirm_just_pressed);

        if hovering {
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
                    if confirm_pressed && pagination.page > 0 {
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
        let is_hit = hit.map(|(ent, _, _)| ent == e).unwrap_or(false);
        let is_focused = ui_focus.is_focused(e);
        let hovering = is_hit || is_focused;
        let confirm_pressed =
            (is_hit && left_pressed) || (is_focused && ui_focus.confirm_just_pressed);

        if hovering {
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
                    if confirm_pressed && pagination.page + 1 < total_pages {
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
