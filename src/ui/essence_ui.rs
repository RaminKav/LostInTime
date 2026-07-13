use crate::ui::game_fonts as gf;
use bevy::{ecs::system::ParamSet, prelude::*, render::view::RenderLayers};
use bevy_aseprite::{anim::AsepriteAnimation, aseprite, AsepriteBundle};
use bevy_proto::prelude::ProtoCommands;
use rand::{seq::SliceRandom, Rng};
use strum::IntoEnumIterator;

use crate::{
    animations::DoneAnimation,
    assets::Graphics,
    attributes::{
        attribute_helpers::create_new_random_item_stack_with_attributes, AttributeChangeEvent,
        ItemGlow, ItemRarity, LootRateBonus,
    },
    chaos::ChaosTracker,
    colors::{LIGHT_RED, RED, SHRINE_GREEN, WHITE},
    custom_commands::CommandsExt,
    gamepad_bindings::{get_gamepad_display_name, GamepadBindingButton},
    inventory::{Inventory, ItemStack},
    item::WorldObject,
    juice::bounce::BounceOnHit,
    player::{
        currency::CoinCurrency,
        levels::PlayerLevel,
        skills::{Heirloom, HeirloomChoiceQueue, HeirloomRarity, HeirloomWithRarity, PlayerSkills},
        unlocks::RunUnlockState,
        ModifyCurencyEvent, Player,
    },
    proto::proto_param::ProtoParam,
    ui::{
        interactions::Interaction,
        item_chest::{spawn_chest_button, ChestButtonKind},
        key_input_guide::InteractionGuideTrigger,
        minimap::UpdateMiniMapEvent,
        skill_choice_ui::SkillChoiceFlash,
        ui_helpers::spawn_full_screen_ui_overlay_tuned,
    },
    GameParam, ScreenResolution,
};

/// Resource to track how many purchases the player has made at blacksmith merchants
/// This determines price scaling (~49% increase per purchase)
#[derive(Resource, Default, Debug, Clone)]
pub struct BlacksmithPurchaseTracker {
    pub purchases_made: u32,
}

impl BlacksmithPurchaseTracker {
    /// Get the price multiplier based on purchases made
    /// Each purchase increases prices by ~49%
    pub fn get_price_multiplier(&self) -> f32 {
        1.4875_f32.powi(self.purchases_made as i32)
    }
}

/// Reset the blacksmith purchase tracker on new run
pub fn reset_blacksmith_tracker(mut tracker: ResMut<BlacksmithPurchaseTracker>) {
    info!(
        "Resetting blacksmith purchase tracker from {} purchases",
        tracker.purchases_made
    );
    tracker.purchases_made = 0;
}

pub const MERCHANT_SLOT_COUNT: usize = 7;
pub const MERCHANT_REROLL_ICON_PATH: &str = "ui/icons/RerollIcon.png";
pub const MERCHANT_REROLL_ICON_SIZE: Vec2 = Vec2::new(10., 10.);

const MERCHANT_TITLE_Y: f32 = 72.;
const MERCHANT_CATEGORY_Y_OFFSET: f32 = -6.;
const MERCHANT_HEIRLOOM_LABEL_Y: f32 = 56. + MERCHANT_CATEGORY_Y_OFFSET;
const MERCHANT_HEIRLOOM_REROLL_Y: f32 = 6. + MERCHANT_CATEGORY_Y_OFFSET;
const MERCHANT_SIDE_LABEL_Y: f32 = -8. + MERCHANT_CATEGORY_Y_OFFSET;
const MERCHANT_SIDE_REROLL_Y: f32 = -60. + MERCHANT_CATEGORY_Y_OFFSET;
const MERCHANT_CATEGORY_REROLL_BADGE_SIZE: Vec2 = Vec2::new(14., 12.);
const MERCHANT_COIN_COUNTER_POS: Vec2 = Vec2::new(116., 72.);
const MERCHANT_REROLL_COUNTER_POS: Vec2 = Vec2::new(116., 46.);
const MERCHANT_ICON_HIT_SIZE: f32 = 28.;
const MERCHANT_ICON_Z: f32 = 15.;
const MERCHANT_ICON_HIT_Z: f32 = 16.;
const MERCHANT_DONE_BUTTON_Y: f32 = -84.;
pub const MERCHANT_CONTAINER_UI_SIZE: Vec2 = Vec2::new(160., 188.);

/// Prevents rapid interact spam from re-requesting `UIState::Essence` while the shop is
/// already open (which would toggle it closed via `handle_new_ui_state`).
#[derive(Resource)]
pub struct MerchantShopOpenLock(pub Timer);

/// Ticks [`MerchantShopOpenLock`] after the merchant UI opens.
pub fn tick_merchant_shop_open_lock(
    time: Res<Time>,
    mut lock: Option<ResMut<MerchantShopOpenLock>>,
    mut commands: Commands,
) {
    let Some(mut lock) = lock else {
        return;
    };
    if lock.0.tick(time.delta()).finished() {
        commands.remove_resource::<MerchantShopOpenLock>();
    }
}

/// Caches shop contents by tile position so they persist across chunk load/unload.
/// Cleared between runs and between non-dungeon era transitions.
#[derive(Resource, Default, Debug, Clone)]
pub struct EssenceShopCache {
    pub shops: std::collections::HashMap<crate::world::TileMapPosition, [MerchantShopSlot; 7]>,
    /// The slot index the player has marked to track for each shop (max 1 per shop).
    pub marked: std::collections::HashMap<crate::world::TileMapPosition, usize>,
}

/// Path to the marker icon spawned on top of a right-clicked shop item.
pub const MERCHANT_MARKER_ICON_PATH: &str = "ui/icons/MerchantMarker.png";
pub const MERCHANT_MARKER_ICON_SIZE: Vec2 = Vec2::new(16., 16.);
/// World-space vertical offset of the tracked-item display above the merchant.
const MERCHANT_WORLD_MARKER_Y_OFFSET: f32 = 34.;
/// Z used so the world-space tracked-item display renders above all y-sorted world entities.
const MERCHANT_WORLD_MARKER_Z: f32 = 990.;

use super::{
    heirloom_tooltip::{HeirloomTooltipRequest, HeirloomTooltipShow},
    spawn_item_stack_icon,
    tooltips::{ToolTipUpdateEvent, TooltipTeardownEvent},
    Focusable, Interactable, UIElement, UIState, CURRENCY_BACKGROUND_SIZE, KEYBIND_BADGE_COLOR,
    TOOLTIP_INFO_BOX_SIZE,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MerchantCategory {
    Heirlooms,
    Equipment,
    Materials,
}

impl MerchantCategory {
    pub fn slot_indices(self) -> &'static [usize] {
        match self {
            MerchantCategory::Heirlooms => &[0, 1, 2],
            MerchantCategory::Equipment => &[3, 4],
            MerchantCategory::Materials => &[5, 6],
        }
    }

    pub fn reroll_position(self) -> Vec2 {
        match self {
            MerchantCategory::Heirlooms => Vec2::new(0., MERCHANT_HEIRLOOM_REROLL_Y),
            MerchantCategory::Equipment => Vec2::new(-32., MERCHANT_SIDE_REROLL_Y),
            MerchantCategory::Materials => Vec2::new(32., MERCHANT_SIDE_REROLL_Y),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            MerchantCategory::Heirlooms => "HEIRLOOMS",
            MerchantCategory::Equipment => "EQUIPMENT",
            MerchantCategory::Materials => "MATERIALS",
        }
    }

    pub fn label_position(self) -> Vec2 {
        match self {
            MerchantCategory::Heirlooms => Vec2::new(0., MERCHANT_HEIRLOOM_LABEL_Y),
            MerchantCategory::Equipment => Vec2::new(-36., MERCHANT_SIDE_LABEL_Y),
            MerchantCategory::Materials => Vec2::new(36., MERCHANT_SIDE_LABEL_Y),
        }
    }

    pub fn flash_position(self) -> Vec3 {
        let p = match self {
            MerchantCategory::Heirlooms => Vec2::new(0., 32. + MERCHANT_CATEGORY_Y_OFFSET),
            MerchantCategory::Equipment => Vec2::new(-32., -10. + MERCHANT_CATEGORY_Y_OFFSET),
            MerchantCategory::Materials => Vec2::new(32., -10. + MERCHANT_CATEGORY_Y_OFFSET),
        };
        Vec3::new(p.x, p.y, 15.)
    }

    pub fn focus_index(self) -> u32 {
        match self {
            MerchantCategory::Heirlooms => 20,
            MerchantCategory::Equipment => 21,
            MerchantCategory::Materials => 22,
        }
    }
}

#[derive(Clone, Debug)]
pub enum MerchantItemKind {
    Heirloom {
        heirloom: Heirloom,
        rarity: HeirloomRarity,
        time_fragment_cost: u32,
    },
    Equipment(ItemStack),
    Material(ItemStack),
}

#[derive(Clone, Debug)]
pub struct MerchantShopSlot {
    pub kind: MerchantItemKind,
    pub base_coin_cost: f32,
    pub coin_cost: u32,
    pub purchased: bool,
}

impl Default for MerchantShopSlot {
    fn default() -> Self {
        Self {
            kind: MerchantItemKind::Material(ItemStack::crate_icon_stack(WorldObject::UpgradeTome)),
            base_coin_cost: 0.,
            coin_cost: 0,
            purchased: true,
        }
    }
}

#[derive(Resource, Component, Clone, Default)]
pub struct EssenceShopChoices {
    pub slots: [MerchantShopSlot; MERCHANT_SLOT_COUNT],
    pub owner_entity: Option<Entity>,
    pub tile_pos: Option<crate::world::TileMapPosition>,
    /// Slot index the player marked to track (1 per shop). `None` when nothing is tracked.
    pub marked_slot: Option<usize>,
}

impl EssenceShopChoices {
    pub fn all_purchased(&self) -> bool {
        self.slots.iter().all(|s| s.purchased)
    }

    pub fn category_fully_purchased(&self, category: MerchantCategory) -> bool {
        category
            .slot_indices()
            .iter()
            .all(|i| self.slots[*i].purchased)
    }
}

#[derive(Debug)]
pub struct SubmitMerchantPurchase {
    pub slot_index: usize,
}

#[derive(Debug)]
pub struct MerchantCategoryRerollEvent {
    pub category: MerchantCategory,
}

#[derive(Resource, Default)]
pub struct MerchantShopUiDirty {
    pub slots: Vec<usize>,
    pub disable_reroll_categories: Vec<MerchantCategory>,
}

#[derive(Component)]
pub struct EssenceUI;

#[derive(Component)]
pub struct BlacksmithCoinsText;

#[derive(Component)]
pub struct BlacksmithRerollsText;

#[derive(Component)]
pub struct MerchantPriceText {
    pub slot_index: usize,
}

#[derive(Component)]
pub struct MerchantShopSlotIndex(pub usize);

#[derive(Component)]
pub struct MerchantSlotUi {
    pub slot_index: usize,
}

#[derive(Component)]
pub struct MerchantDoneButton;

#[derive(Component, Clone, Copy)]
pub struct MerchantCategoryReroll(pub MerchantCategory);

#[derive(Component)]
pub struct MerchantCategoryRerollButton(pub MerchantCategory);

#[derive(Component)]
pub struct MerchantCategoryRerollIcon;

#[derive(Component)]
pub struct MerchantSlotIcon;

/// The `MerchantMarker.png` overlay shown on top of the marked item icon inside the shop UI.
#[derive(Component)]
pub struct MerchantMarkerOverlay {
    pub slot_index: usize,
}

/// Root of the world-space tracked-item display shown above a merchant.
#[derive(Component)]
pub struct MerchantWorldMarkerDisplay {
    pub owner: Entity,
    pub slot_index: usize,
}

/// World-space coin cost text for the tracked item (turns green when affordable).
#[derive(Component)]
pub struct MerchantWorldMarkerPriceText {
    pub coin_cost: u32,
}

/// Marks the shop-side info box explaining the right-click-to-track action.
#[derive(Component)]
pub struct MerchantTrackInfoBox;

/// Header line inside [`MerchantTrackInfoBox`]; updated when input device / bindings change.
#[derive(Component)]
pub struct MerchantTrackInfoBoxHeaderText;

#[derive(Component)]
pub struct MerchantHeirloomHover {
    pub heirloom: Heirloom,
    pub rarity: HeirloomRarity,
}

fn merchant_slot_position(slot_index: usize) -> Vec2 {
    match slot_index {
        0 => Vec2::new(-38., 42. + MERCHANT_CATEGORY_Y_OFFSET),
        1 => Vec2::new(0., 42. + MERCHANT_CATEGORY_Y_OFFSET),
        2 => Vec2::new(38., 42. + MERCHANT_CATEGORY_Y_OFFSET),

        3 => Vec2::new(-52., -26. + MERCHANT_CATEGORY_Y_OFFSET),
        4 => Vec2::new(-20., -26. + MERCHANT_CATEGORY_Y_OFFSET),

        5 => Vec2::new(20., -26. + MERCHANT_CATEGORY_Y_OFFSET),
        6 => Vec2::new(52., -26. + MERCHANT_CATEGORY_Y_OFFSET),
        _ => Vec2::ZERO,
    }
}

fn item_rarity_cost_inc(rarity: &ItemRarity) -> f32 {
    match rarity {
        ItemRarity::Common => 1.,
        ItemRarity::Uncommon => 1.2,
        ItemRarity::Rare => 1.6,
        ItemRarity::Legendary => 2.5,
    }
}

fn heirloom_rarity_cost_inc(rarity: &HeirloomRarity) -> f32 {
    match rarity {
        HeirloomRarity::Common => 1.,
        HeirloomRarity::Uncommon => 1.2,
        HeirloomRarity::Rare => 1.6,
        HeirloomRarity::Legendary => 2.5,
    }
}

const MIN_MERCHANT_COIN_COST: u32 = 10;

fn apply_purchase_multiplier(base: f32, multiplier: f32) -> u32 {
    let cost = (base * multiplier).trunc() as u32;
    if cost == 0 {
        return 0;
    }
    cost.max(MIN_MERCHANT_COIN_COST)
}

fn purchase_multiplier_for_level(player_level: u8) -> f32 {
    1.0 + player_level as f32 * 0.53
}

/// How often live merchant shops recompute their prices against the player's current level.
pub const MERCHANT_PRICE_REFRESH_SECS: f32 = 45.0;

/// Drives [`refresh_merchant_prices_on_timer`]; shops only recompute prices on spawn, so without
/// this the displayed prices never scale while the player stays near the shop and levels up.
#[derive(Resource)]
pub struct MerchantPriceRefreshTimer(pub Timer);

impl Default for MerchantPriceRefreshTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(
            MERCHANT_PRICE_REFRESH_SECS,
            TimerMode::Repeating,
        ))
    }
}

fn recompute_slot_prices(slots: &mut [MerchantShopSlot; MERCHANT_SLOT_COUNT], multiplier: f32) {
    for slot in slots.iter_mut() {
        if !slot.purchased && slot.base_coin_cost > 0. {
            slot.coin_cost = apply_purchase_multiplier(slot.base_coin_cost, multiplier);
        }
    }
}

/// Every 45s, recompute prices for the cached shops, the live merchant entities, and the currently
/// open shop so they stay scaled to the player's current level without needing a despawn/respawn.
pub fn refresh_merchant_prices_on_timer(
    time: Res<Time>,
    mut timer: ResMut<MerchantPriceRefreshTimer>,
    player_level: Query<&PlayerLevel, With<Player>>,
    mut cache: ResMut<EssenceShopCache>,
    mut merchants: Query<&mut EssenceShopChoices>,
    open_shop: Option<ResMut<EssenceShopChoices>>,
    ui_dirty: Option<ResMut<MerchantShopUiDirty>>,
    mut commands: Commands,
    world_markers: Query<Entity, With<MerchantWorldMarkerDisplay>>,
) {
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }

    let level = player_level.get_single().map(|l| l.level).unwrap_or(1);
    let multiplier = purchase_multiplier_for_level(level);

    for slots in cache.shops.values_mut() {
        recompute_slot_prices(slots, multiplier);
    }
    for mut shop in merchants.iter_mut() {
        recompute_slot_prices(&mut shop.slots, multiplier);
    }

    // World-space tracked-item markers cache their price at spawn; despawn them so
    // `sync_merchant_world_marker_displays` rebuilds them next frame with the new price.
    for e in world_markers.iter() {
        commands.entity(e).despawn_recursive();
    }

    // Refresh the open shop's badges so the displayed numbers update immediately.
    if let (Some(mut shop), Some(mut dirty)) = (open_shop, ui_dirty) {
        recompute_slot_prices(&mut shop.slots, multiplier);
        for idx in 0..MERCHANT_SLOT_COUNT {
            if !shop.slots[idx].purchased && !dirty.slots.contains(&idx) {
                dirty.slots.push(idx);
            }
        }
    }
}

pub fn sync_merchant_shop_to_world(
    shop: &EssenceShopChoices,
    commands: &mut Commands,
    cache: &mut EssenceShopCache,
) {
    if let Some(owner) = shop.owner_entity {
        commands.entity(owner).insert(shop.clone());
    }
    if let Some(tile_pos) = shop.tile_pos {
        cache.shops.insert(tile_pos, shop.slots.clone());
        match shop.marked_slot {
            Some(slot) => {
                cache.marked.insert(tile_pos, slot);
            }
            None => {
                cache.marked.remove(&tile_pos);
            }
        }
    }
}

fn spawn_currency_counter(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    parent: Entity,
    center: Vec2,
    count: impl ToString,
    text_marker: impl Bundle,
    icon_spawner: impl FnOnce(&mut Commands, Entity, &Graphics, &AssetServer) -> Entity,
) {
    let bg = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::CurrencyBackground),
            sprite: Sprite {
                custom_size: Some(CURRENCY_BACKGROUND_SIZE),
                ..default()
            },
            transform: Transform::from_translation(Vec3::new(center.x, center.y, 2.)),
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIState::Essence)
        .id();

    let text = commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    count.to_string(),
                    gf::DISPLAY.text_style(&asset_server, WHITE),
                )
                .with_alignment(TextAlignment::Center),
                text_anchor: bevy::sprite::Anchor::CenterLeft,
                transform: Transform {
                translation: Vec3::new(-4., 0., 2.),
                scale: gf::DISPLAY.transform_scale(),
                ..Default::default()
            },
                ..default()
            },
            RenderLayers::from_layers(&[3]),
            text_marker,
        ))
        .id();

    let icon_e = icon_spawner(commands, text, graphics, asset_server);
    commands.entity(icon_e).set_parent(text);
    commands.entity(text).set_parent(bg);
    commands.entity(bg).set_parent(parent);
}

fn spawn_reroll_icon_sprite(
    commands: &mut Commands,
    asset_server: &AssetServer,
    parent: Entity,
) -> Entity {
    commands
        .spawn(SpriteBundle {
            texture: asset_server.load(MERCHANT_REROLL_ICON_PATH),
            sprite: Sprite {
                custom_size: Some(MERCHANT_REROLL_ICON_SIZE),
                ..default()
            },
            transform: Transform::from_translation(Vec3::new(-12., 0., 2.)),
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .set_parent(parent)
        .id()
}

fn spawn_price_badge(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    parent: Entity,
    pos: Vec2,
    slot_index: usize,
    coin_cost: u32,
) {
    let count_str = coin_cost.to_string();
    let badge_width = 12. + (count_str.len().saturating_sub(1) as f32 * 14.);

    let row = commands
        .spawn((
            SpatialBundle::from_transform(Transform::from_translation(Vec3::new(
                pos.x,
                pos.y,
                MERCHANT_ICON_Z - 1.,
            ))),
            RenderLayers::from_layers(&[3]),
            UIState::Essence,
            Name::new("Merchant Price Row"),
        ))
        .set_parent(parent)
        .id();

    let coin_icon = spawn_item_stack_icon(
        commands,
        graphics,
        &ItemStack::crate_icon_stack(WorldObject::Coin).copy_with_count(1),
        asset_server,
        Vec2::new(-12., 0.),
        Vec2::ZERO,
        3,
    );
    commands.entity(coin_icon).set_parent(row);

    let badge = commands
        .spawn(SpriteBundle {
            sprite: Sprite {
                color: KEYBIND_BADGE_COLOR,
                custom_size: Some(Vec2::new(badge_width, 14.)),
                ..default()
            },
            transform: Transform::from_translation(Vec3::new(-5., 0., 1.)),
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .id();

    commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    count_str,
                    gf::BODY.text_style(&asset_server, WHITE),
                )
                .with_alignment(TextAlignment::Center),
                text_anchor: bevy::sprite::Anchor::Center,
                transform: Transform {
                translation: Vec3::new(5., 0., 2.),
                scale: gf::BODY.transform_scale(),
                ..Default::default()
            },
                ..default()
            },
            RenderLayers::from_layers(&[3]),
            MerchantPriceText { slot_index },
        ))
        .set_parent(badge);

    commands.entity(badge).set_parent(row);
}

/// First line of the merchant track info box (`"Press X:"`, `"Right click:"`, etc.).
pub fn format_merchant_track_action_header(
    mouseless: &crate::inputs::MouselessModeState,
    active_device: &crate::gamepad_input::ActiveInputDevice,
) -> String {
    // Mouse mark is always RMB in `handle_merchant_shop_interactions` (hardcoded). Do not use
    // `shop_mark` here — it defaults to keyboard `KeyCode::X`, which produced a fake "Press X:"
    // while `active_device` was already KeyboardMouse.
    let use_controller_prompt = mouseless.0
        || active_device.0 == crate::gamepad_input::InputDeviceKind::Gamepad;
    let action = if use_controller_prompt {
        format!(
            "Press {}",
            get_gamepad_display_name(GamepadBindingButton::West)
        )
    } else {
        "Right click".to_string()
    };
    format!("{action}:")
}

/// Keeps the merchant track info-box header in sync with keyboard vs controller input.
pub fn update_merchant_track_info_box_label(
    mouseless: Res<crate::inputs::MouselessModeState>,
    active_device: Res<crate::gamepad_input::ActiveInputDevice>,
    mut texts: Query<&mut Text, With<MerchantTrackInfoBoxHeaderText>>,
    mut last_logged: Local<Option<(bool, crate::gamepad_input::InputDeviceKind, String)>>,
) {
    let header = format_merchant_track_action_header(&mouseless, &active_device);
    let snapshot = (mouseless.0, active_device.0, header.clone());
    if last_logged.as_ref() != Some(&snapshot) {
        info!(
            "[EssenceMarkPrompt] mouseless={} active_device={:?} → \"{}\"",
            snapshot.0, snapshot.1, snapshot.2
        );
        *last_logged = Some(snapshot);
    }
    for mut text in texts.iter_mut() {
        if let Some(section) = text.sections.first_mut() {
            if section.value != header {
                section.value = header.clone();
            }
        }
    }
}

/// Reuses the tooltip info-box art to explain right-click tracking, placed to the right of
/// the shop beneath the currency counters.
fn spawn_merchant_track_info_box(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    parent: Entity,
    header: &str,
) {
    // Sit fully to the right of the merchant container (no overlap) with a small gap.
    let pos = Vec2::new(
        MERCHANT_CONTAINER_UI_SIZE.x / 2. + TOOLTIP_INFO_BOX_SIZE.x / 2. + 6.,
        8.,
    );
    let box_e = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::TooltipInfoBox),
            sprite: Sprite {
                custom_size: Some(TOOLTIP_INFO_BOX_SIZE),
                ..default()
            },
            transform: Transform::from_translation(Vec3::new(pos.x, pos.y, 2.)),
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIState::Essence)
        .insert(MerchantTrackInfoBox)
        .insert(Name::new("Merchant Track Info Box"))
        .insert(crate::item::item_drop_outline::UiShadow::container())
        .set_parent(parent)
        .id();

    commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    header.to_string(),
                    gf::BODY.text_style(&asset_server, WHITE),
                )
                .with_alignment(TextAlignment::Center),
                text_anchor: bevy::sprite::Anchor::Center,
                transform: Transform {
                translation: Vec3::new(0., 5., 2.),
                scale: gf::BODY.transform_scale(),
                ..Default::default()
            },
                ..default()
            },
            RenderLayers::from_layers(&[3]),
            MerchantTrackInfoBoxHeaderText,
        ))
        .set_parent(box_e);

    commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    "Mark a shop item to track".to_string(),
                    gf::BODY.text_style(&asset_server, WHITE),
                )
                .with_alignment(TextAlignment::Center),
                text_anchor: bevy::sprite::Anchor::Center,
                transform: Transform {
                translation: Vec3::new(0., -6., 2.),
                scale: gf::BODY.transform_scale(),
                ..Default::default()
            },
                ..default()
            },
            RenderLayers::from_layers(&[3]),
        ))
        .set_parent(box_e);
}

fn spawn_section_label(
    commands: &mut Commands,
    asset_server: &AssetServer,
    parent: Entity,
    label: &str,
    pos: Vec2,
) {
    commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    label.to_string(),
                    gf::BODY.text_style(&asset_server, WHITE),
                )
                .with_alignment(TextAlignment::Center),
                text_anchor: bevy::sprite::Anchor::Center,
                transform: Transform {
                translation: Vec3::new(pos.x, pos.y, 2.),
                scale: gf::BODY.transform_scale(),
                ..Default::default()
            },
                ..default()
            },
            RenderLayers::from_layers(&[3]),
            UIState::Essence,
        ))
        .set_parent(parent);
}

fn attach_merchant_icon_glow(
    commands: &mut Commands,
    graphics: &Graphics,
    icon_e: Entity,
    glow: ItemGlow,
    faded: bool,
) {
    let color = if faded {
        Color::rgb(0.45, 0.45, 0.45)
    } else {
        Color::WHITE
    };
    commands
        .spawn(SpriteBundle {
            texture: graphics.get_item_glow(glow),
            sprite: Sprite {
                custom_size: Some(Vec2::new(20., 20.)),
                color,
                ..default()
            },
            transform: Transform::from_translation(Vec3::new(0., 0., -1.)),
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIState::Essence)
        .set_parent(icon_e);
}

fn spawn_merchant_slot_icon(
    commands: &mut Commands,
    graphics: &Graphics,
    slot_root: Entity,
    atlas_sprite: TextureAtlasSprite,
    hit_extra: impl Bundle,
    item_glow: Option<ItemGlow>,
    faded: bool,
    slot_index: usize,
) {
    let hit_size = Vec2::splat(MERCHANT_ICON_HIT_SIZE);

    let icon_e = commands
        .spawn(SpriteSheetBundle {
            sprite: atlas_sprite,
            texture_atlas: graphics.texture_atlas.as_ref().unwrap().clone(),
            transform: Transform::from_translation(Vec3::new(0., 0., MERCHANT_ICON_Z)),
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIState::Essence)
        .insert(MerchantSlotIcon)
        .set_parent(slot_root)
        .id();

    if let Some(glow) = item_glow {
        attach_merchant_icon_glow(commands, graphics, icon_e, glow, faded);
    }

    let mut hit_cmd = commands.spawn((
        SpriteBundle {
            sprite: Sprite {
                color: Color::NONE,
                custom_size: Some(hit_size),
                ..default()
            },
            transform: Transform::from_translation(Vec3::new(0., 0., MERCHANT_ICON_HIT_Z)),
            ..default()
        },
        RenderLayers::from_layers(&[3]),
        UIState::Essence,
        hit_extra,
    ));
    if !faded {
        hit_cmd
            .insert(Interactable::default())
            .insert(MerchantShopSlotIndex(slot_index))
            .insert(Focusable {
                group: UIState::Essence,
                index: slot_index as u32,
            });
    }
    hit_cmd.set_parent(slot_root);
}

pub fn spawn_merchant_slot_ui(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    parent: Entity,
    slot_index: usize,
    slot: &MerchantShopSlot,
) {
    let pos = merchant_slot_position(slot_index);
    let faded = slot.purchased;

    let slot_root_e = commands
        .spawn((
            SpatialBundle::from_transform(Transform::from_translation(Vec3::new(pos.x, pos.y, 0.))),
            UIState::Essence,
            MerchantSlotUi { slot_index },
            Name::new(format!("Merchant Slot {slot_index}")),
        ))
        .id();

    let icon_color = if faded {
        Color::rgb(0.45, 0.45, 0.45)
    } else {
        Color::WHITE
    };

    match &slot.kind {
        MerchantItemKind::Heirloom {
            heirloom, rarity, ..
        } => {
            let mut heirloom_sprite = graphics.get_heirloom_icon(heirloom.clone());
            heirloom_sprite.color = icon_color;
            spawn_merchant_slot_icon(
                commands,
                graphics,
                slot_root_e,
                heirloom_sprite,
                MerchantHeirloomHover {
                    heirloom: heirloom.clone(),
                    rarity: rarity.clone(),
                },
                rarity.get_item_glow(),
                faded,
                slot_index,
            );
        }
        MerchantItemKind::Equipment(stack) | MerchantItemKind::Material(stack) => {
            let mut atlas_sprite = graphics
                .spritesheet_map
                .as_ref()
                .unwrap()
                .get(&stack.obj_type)
                .cloned()
                .expect("merchant item icon");
            atlas_sprite.color = icon_color;
            spawn_merchant_slot_icon(
                commands,
                graphics,
                slot_root_e,
                atlas_sprite,
                (),
                stack.rarity.get_item_glow(),
                faded,
                slot_index,
            );
        }
    }

    if !faded {
        spawn_price_badge(
            commands,
            graphics,
            asset_server,
            slot_root_e,
            Vec2::new(5., -18.),
            slot_index,
            slot.coin_cost,
        );
        if let MerchantItemKind::Heirloom {
            time_fragment_cost, ..
        } = &slot.kind
        {
            if *time_fragment_cost > 0 {
                let tf_icon = spawn_item_stack_icon(
                    commands,
                    graphics,
                    &ItemStack::crate_icon_stack(WorldObject::TimeFragment)
                        .copy_with_count(*time_fragment_cost as usize),
                    asset_server,
                    Vec2::new(0., -30.),
                    Vec2::ZERO,
                    3,
                );
                commands.entity(tf_icon).set_parent(slot_root_e);
            }
        }
    }

    commands.entity(slot_root_e).set_parent(parent);
}

/// Keeps the in-shop `MerchantMarker.png` overlay attached to the marked slot's icon.
/// Re-parents/rebuilds it after purchases or rerolls despawn and respawn slot roots.
pub fn sync_merchant_marker_overlay(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    shop: Res<EssenceShopChoices>,
    slot_ui: Query<(Entity, &MerchantSlotUi)>,
    overlays: Query<(Entity, &MerchantMarkerOverlay)>,
) {
    let desired = shop
        .marked_slot
        .filter(|&slot| slot < MERCHANT_SLOT_COUNT && !shop.slots[slot].purchased);

    // When a slot root is despawned (purchase/reroll) the overlay child dies with it, so a
    // stale overlay never lingers; we only need to compare the surviving overlay's slot.
    let existing = overlays.iter().next();
    if existing.map(|(_, o)| o.slot_index) == desired {
        return;
    }

    let desired_root = desired.and_then(|slot| {
        slot_ui
            .iter()
            .find(|(_, ui)| ui.slot_index == slot)
            .map(|(e, _)| e)
    });

    for (e, _) in overlays.iter() {
        commands.entity(e).despawn_recursive();
    }

    if let (Some(slot), Some(root)) = (desired, desired_root) {
        commands
            .spawn((
                SpriteBundle {
                    texture: asset_server.load(MERCHANT_MARKER_ICON_PATH),
                    sprite: Sprite {
                        custom_size: Some(MERCHANT_MARKER_ICON_SIZE),
                        ..default()
                    },
                    transform: Transform::from_translation(Vec3::new(
                        0.,
                        0.,
                        MERCHANT_ICON_HIT_Z + 1.,
                    )),
                    ..default()
                },
                RenderLayers::from_layers(&[3]),
                UIState::Essence,
                MerchantMarkerOverlay { slot_index: slot },
                Name::new("Merchant Marker Overlay"),
            ))
            .set_parent(root);
    }
}

/// World-space (game camera, default render layer) icon for the tracked item.
fn merchant_world_marker_icon_sprite(
    graphics: &Graphics,
    slot: &MerchantShopSlot,
) -> TextureAtlasSprite {
    match &slot.kind {
        MerchantItemKind::Heirloom { heirloom, .. } => graphics.get_heirloom_icon(heirloom.clone()),
        MerchantItemKind::Equipment(stack) | MerchantItemKind::Material(stack) => graphics
            .spritesheet_map
            .as_ref()
            .unwrap()
            .get(&stack.obj_type)
            .cloned()
            .expect("merchant item icon"),
    }
}

fn merchant_world_marker_glow(slot: &MerchantShopSlot) -> Option<ItemGlow> {
    match &slot.kind {
        MerchantItemKind::Heirloom { rarity, .. } => rarity.get_item_glow(),
        MerchantItemKind::Equipment(stack) | MerchantItemKind::Material(stack) => {
            stack.rarity.get_item_glow()
        }
    }
}

/// Builds the world-space tracked-item display (icon + glow + coin cost) above a merchant.
/// Mirrors the shop badge layout but renders on the game camera instead of the UI camera.
fn spawn_merchant_world_marker_display(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    coins: u32,
    merchant_pos: Vec2,
    owner: Entity,
    slot_index: usize,
    slot: &MerchantShopSlot,
) {
    let root = commands
        .spawn((
            SpatialBundle::from_transform(Transform::from_translation(Vec3::new(
                merchant_pos.x,
                merchant_pos.y + MERCHANT_WORLD_MARKER_Y_OFFSET,
                MERCHANT_WORLD_MARKER_Z,
            ))),
            MerchantWorldMarkerDisplay { owner, slot_index },
            Name::new("Merchant World Marker Display"),
        ))
        .id();

    // Black background behind the icon for contrast against the world.
    commands
        .spawn(SpriteBundle {
            sprite: Sprite {
                color: KEYBIND_BADGE_COLOR,
                custom_size: Some(Vec2::new(20., 20.)),
                ..default()
            },
            transform: Transform::from_translation(Vec3::new(0., 0., 0.)),
            ..default()
        })
        .set_parent(root);

    let icon_e = commands
        .spawn(SpriteSheetBundle {
            sprite: merchant_world_marker_icon_sprite(graphics, slot),
            texture_atlas: graphics.texture_atlas.as_ref().unwrap().clone(),
            transform: Transform::from_translation(Vec3::new(0., 0., 2.)),
            ..default()
        })
        .set_parent(root)
        .id();

    if let Some(glow) = merchant_world_marker_glow(slot) {
        commands
            .spawn(SpriteBundle {
                texture: graphics.get_item_glow(glow),
                sprite: Sprite {
                    custom_size: Some(Vec2::new(20., 20.)),
                    ..default()
                },
                transform: Transform::from_translation(Vec3::new(0., 0., -1.)),
                ..default()
            })
            .set_parent(icon_e);
    }

    // Coin cost badge: coin icon + black background + number (green when affordable).
    let count_str = slot.coin_cost.to_string();
    let badge_width = 12. + (count_str.len().saturating_sub(1) as f32 * 14.);
    let price_color = if coins >= slot.coin_cost {
        SHRINE_GREEN
    } else {
        LIGHT_RED
    };

    let row = commands
        .spawn((
            SpatialBundle::from_transform(Transform::from_translation(Vec3::new(5., -18., 0.5))),
            Name::new("Merchant World Marker Price"),
        ))
        .set_parent(root)
        .id();

    let coin_sprite = graphics
        .icons
        .as_ref()
        .unwrap()
        .get(&WorldObject::Coin)
        .or_else(|| {
            graphics
                .spritesheet_map
                .as_ref()
                .unwrap()
                .get(&WorldObject::Coin)
        })
        .cloned()
        .expect("coin icon");
    commands
        .spawn(SpriteSheetBundle {
            sprite: coin_sprite,
            texture_atlas: graphics.texture_atlas.as_ref().unwrap().clone(),
            transform: Transform::from_translation(Vec3::new(-12., 0., 1.)),
            ..default()
        })
        .set_parent(row);

    let badge = commands
        .spawn(SpriteBundle {
            sprite: Sprite {
                color: KEYBIND_BADGE_COLOR,
                custom_size: Some(Vec2::new(badge_width, 14.)),
                ..default()
            },
            transform: Transform::from_translation(Vec3::new(-5., 0., 1.)),
            ..default()
        })
        .set_parent(row)
        .id();

    commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    count_str,
                    gf::BODY.text_style(&asset_server, price_color),
                )
                .with_alignment(TextAlignment::Center),
                text_anchor: bevy::sprite::Anchor::Center,
                transform: Transform {
                translation: Vec3::new(5., 0., 2.),
                scale: gf::BODY.transform_scale(),
                ..Default::default()
            },
                ..default()
            },
            MerchantWorldMarkerPriceText {
                coin_cost: slot.coin_cost,
            },
        ))
        .set_parent(badge);
}

/// Keeps the world-space tracked-item display in sync with each merchant's marked slot.
pub fn sync_merchant_world_marker_displays(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    coins: Res<CoinCurrency>,
    merchants: Query<(Entity, &EssenceShopChoices, &GlobalTransform)>,
    displays: Query<(Entity, &MerchantWorldMarkerDisplay)>,
) {
    // Desired (owner -> slot) tracked items that are still valid.
    let mut desired: std::collections::HashMap<Entity, usize> = std::collections::HashMap::new();
    for (entity, shop, _) in merchants.iter() {
        if let Some(slot) = shop
            .marked_slot
            .filter(|&slot| slot < MERCHANT_SLOT_COUNT && !shop.slots[slot].purchased)
        {
            desired.insert(entity, slot);
        }
    }

    // Despawn displays that no longer match a desired (owner, slot).
    for (e, display) in displays.iter() {
        if desired.get(&display.owner) != Some(&display.slot_index) {
            commands.entity(e).despawn_recursive();
        }
    }

    // Spawn displays for merchants that need one but don't have a matching display yet.
    for (entity, shop, transform) in merchants.iter() {
        let Some(&slot) = desired.get(&entity) else {
            continue;
        };
        let already = displays
            .iter()
            .any(|(_, d)| d.owner == entity && d.slot_index == slot);
        if already {
            continue;
        }
        spawn_merchant_world_marker_display(
            &mut commands,
            &graphics,
            &asset_server,
            coins.coins,
            transform.translation().truncate(),
            entity,
            slot,
            &shop.slots[slot],
        );
    }
}

/// Recolors world-space tracked-item cost text green once the player can afford it.
pub fn update_merchant_world_marker_price_colors(
    coins: Res<CoinCurrency>,
    mut price_texts: Query<(&MerchantWorldMarkerPriceText, &mut Text)>,
) {
    if !coins.is_changed() {
        return;
    }
    for (price, mut text) in price_texts.iter_mut() {
        let color = if coins.coins >= price.coin_cost {
            SHRINE_GREEN
        } else {
            LIGHT_RED
        };
        if let Some(section) = text.sections.first_mut() {
            section.style.color = color;
        }
    }
}

pub fn spawn_merchant_category_reroll_button(
    commands: &mut Commands,
    asset_server: &AssetServer,
    parent: Entity,
    category: MerchantCategory,
    enabled: bool,
) {
    let pos = category.reroll_position();
    let color = if enabled {
        Color::WHITE
    } else {
        Color::rgb(0.45, 0.45, 0.45)
    };

    let mut btn = commands.spawn(SpriteBundle {
        sprite: Sprite {
            color: KEYBIND_BADGE_COLOR,
            custom_size: Some(MERCHANT_CATEGORY_REROLL_BADGE_SIZE),
            ..default()
        },
        transform: Transform::from_translation(Vec3::new(pos.x, pos.y, 3.)),
        ..default()
    });
    btn.insert(RenderLayers::from_layers(&[3]))
        .insert(UIState::Essence)
        .insert(MerchantCategoryRerollButton(category))
        .insert(Name::new(format!("Merchant Reroll {:?}", category.label())));

    if enabled {
        btn.insert(Interactable::default()).insert(Focusable {
            group: UIState::Essence,
            index: category.focus_index(),
        });
    }

    let btn_e = btn.id();

    commands
        .spawn(SpriteBundle {
            texture: asset_server.load(MERCHANT_REROLL_ICON_PATH),
            sprite: Sprite {
                custom_size: Some(MERCHANT_REROLL_ICON_SIZE),
                color,
                ..default()
            },
            transform: Transform::from_translation(Vec3::new(0., 0., 1.)),
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(MerchantCategoryRerollIcon)
        .set_parent(btn_e);

    commands.entity(btn_e).set_parent(parent);
}

pub fn refresh_merchant_category_ui(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    shop: &EssenceShopChoices,
    ui_root: Entity,
    category: MerchantCategory,
    run_unlocks: &RunUnlockState,
    slot_ui: &Query<(Entity, &MerchantSlotUi)>,
    reroll_buttons: &Query<(Entity, &MerchantCategoryRerollButton)>,
) {
    for (e, ui) in slot_ui.iter() {
        if category.slot_indices().contains(&ui.slot_index) {
            commands.entity(e).despawn_recursive();
        }
    }
    for (e, btn) in reroll_buttons.iter() {
        if btn.0 == category {
            commands.entity(e).despawn_recursive();
        }
    }

    for &idx in category.slot_indices() {
        spawn_merchant_slot_ui(
            commands,
            graphics,
            asset_server,
            ui_root,
            idx,
            &shop.slots[idx],
        );
    }

    let reroll_enabled =
        run_unlocks.rerolls_remaining > 0 && !shop.category_fully_purchased(category);
    spawn_merchant_category_reroll_button(
        commands,
        asset_server,
        ui_root,
        category,
        reroll_enabled,
    );
}

pub fn spawn_merchant_reroll_flash(
    commands: &mut Commands,
    asset_server: &AssetServer,
    ui_root: Entity,
    pos: Vec3,
    category: MerchantCategory,
) {
    commands
        .spawn(AsepriteBundle {
            animation: AsepriteAnimation::from(SkillChoiceFlash::tags::FLASH),
            aseprite: asset_server.load(SkillChoiceFlash::PATH),
            transform: Transform {
                translation: pos,
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(VisibilityBundle::default())
        .insert(UIState::Essence)
        .insert(MerchantCategoryReroll(category))
        .insert(DoneAnimation)
        .set_parent(ui_root);
}

/// Activate bounce on the visible atlas icon sibling of a merchant slot hit target.
pub fn bounce_merchant_slot_icon(
    commands: &mut Commands,
    hit_entity: Entity,
    parents: &Query<&Parent>,
    children: &Query<&Children>,
    icons: &Query<Entity, With<MerchantSlotIcon>>,
) {
    let Ok(parent) = parents.get(hit_entity) else {
        return;
    };
    let Ok(kids) = children.get(parent.get()) else {
        return;
    };
    for child in kids.iter() {
        if icons.get(*child).is_ok() {
            commands.entity(*child).insert(BounceOnHit::new());
        }
    }
}

pub fn handle_essence_heirloom_tooltip(
    mut tooltip_requests: EventWriter<HeirloomTooltipRequest>,
    heirloom_hovers: Query<(&MerchantHeirloomHover, &super::interactions::Interactable)>,
    mut last_hovered: Local<Option<Heirloom>>,
) {
    let currently_hovered = heirloom_hovers
        .iter()
        .find(|(_, interactable)| matches!(interactable.current(), Interaction::Hovering))
        .map(|(hover, _)| hover.heirloom.clone());

    if *last_hovered == currently_hovered {
        return;
    }

    match &currently_hovered {
        None => tooltip_requests.send(HeirloomTooltipRequest::Clear),
        Some(hovered_heirloom) => {
            for (hover, interactable) in heirloom_hovers.iter() {
                if matches!(interactable.current(), Interaction::Hovering)
                    && hover.heirloom == *hovered_heirloom
                {
                    tooltip_requests.send(HeirloomTooltipRequest::Show(HeirloomTooltipShow {
                        heirloom: hover.heirloom.clone(),
                        rarity: hover.rarity.clone(),
                        position: Vec3::new(-170., 0., 15.),
                        scaling_text: None,
                        trigger_count: 0,
                        ui_state: Some(UIState::Essence),
                    }));
                    break;
                }
            }
        }
    }

    *last_hovered = currently_hovered;
}

pub fn handle_merchant_item_tooltip(
    shop: Res<EssenceShopChoices>,
    item_slots: Query<(&MerchantShopSlotIndex, &Interactable), Without<MerchantHeirloomHover>>,
    player_inv: Query<&Inventory, With<Player>>,
    proto: ProtoParam,
    mut tooltip_update_events: EventWriter<ToolTipUpdateEvent>,
    mut tooltip_teardown_events: EventWriter<TooltipTeardownEvent>,
    mut last_hovered: Local<Option<usize>>,
) {
    let currently_hovered = item_slots
        .iter()
        .find(|(idx, interactable)| {
            !shop.slots[idx.0].purchased && matches!(interactable.current(), Interaction::Hovering)
        })
        .map(|(idx, _)| idx.0);

    if *last_hovered == currently_hovered {
        return;
    }

    tooltip_teardown_events.send_default();

    if let Some(slot_index) = currently_hovered {
        let item_stack = match &shop.slots[slot_index].kind {
            MerchantItemKind::Equipment(s) | MerchantItemKind::Material(s) => s.clone(),
            MerchantItemKind::Heirloom { .. } => {
                *last_hovered = currently_hovered;
                return;
            }
        };

        tooltip_update_events.send(ToolTipUpdateEvent {
            item_stack: item_stack.clone(),
            is_recipe: false,
            show_range: false,
            ..Default::default()
        });

        if matches!(shop.slots[slot_index].kind, MerchantItemKind::Equipment(_)) {
            if let Ok(inv) = player_inv.get_single() {
                if let Some(displaced) =
                    super::item_chest::displaced_equipped_item_stack(inv, &item_stack, &proto)
                {
                    tooltip_update_events.send(ToolTipUpdateEvent {
                        item_stack: displaced,
                        is_recipe: false,
                        show_range: false,
                        anchor_ui: None,
                        info_boxes: vec![],
                        position_override: Some(Vec2::new(
                            MERCHANT_CONTAINER_UI_SIZE.x + 20.,
                            -MERCHANT_CONTAINER_UI_SIZE.y / 2. + 40.,
                        )),
                        header_text: Some("Currently Equipped".to_string()),
                        world_anchor: None,
                        ui_state_tag: None,
                        pin_right: false,
                    });
                }
            }
        }
    }

    *last_hovered = currently_hovered;
}

pub fn update_blacksmith_coin_display(
    coins: Res<CoinCurrency>,
    mut q: Query<&mut Text, With<BlacksmithCoinsText>>,
) {
    if !coins.is_changed() {
        return;
    }
    for mut text in q.iter_mut() {
        if let Some(section) = text.sections.first_mut() {
            section.value = coins.coins.to_string();
        }
    }
}

pub fn update_merchant_price_text_colors(
    coins: Res<CoinCurrency>,
    shop: Res<EssenceShopChoices>,
    mut price_texts: Query<(&MerchantPriceText, &mut Text)>,
) {
    for (tag, mut text) in price_texts.iter_mut() {
        let slot = &shop.slots[tag.slot_index];
        if slot.purchased {
            continue;
        }
        let color = if coins.coins < slot.coin_cost {
            RED
        } else {
            WHITE
        };
        if let Some(section) = text.sections.first_mut() {
            section.style.color = color;
        }
    }
}

pub fn update_blacksmith_reroll_display(
    run_unlocks: Res<RunUnlockState>,
    mut q: Query<&mut Text, With<BlacksmithRerollsText>>,
) {
    if !run_unlocks.is_changed() {
        return;
    }
    for mut text in q.iter_mut() {
        if let Some(section) = text.sections.first_mut() {
            section.value = run_unlocks.rerolls_remaining.to_string();
        }
    }
}

pub fn update_merchant_reroll_button_states(
    run_unlocks: Res<RunUnlockState>,
    shop: Res<EssenceShopChoices>,
    mut sprites: ParamSet<(
        Query<(&mut Sprite, &MerchantCategoryRerollButton), With<Interactable>>,
        Query<&mut Sprite, With<MerchantCategoryRerollIcon>>,
    )>,
) {
    if !run_unlocks.is_changed() && !shop.is_changed() {
        return;
    }

    for (mut sprite, btn) in sprites.p0().iter_mut() {
        let enabled = run_unlocks.rerolls_remaining > 0 && !shop.category_fully_purchased(btn.0);
        sprite.color = if enabled {
            KEYBIND_BADGE_COLOR
        } else {
            Color::rgba(62. / 255., 58. / 255., 58. / 255., 0.45)
        };
    }
    let icon_color = if run_unlocks.rerolls_remaining > 0 {
        Color::WHITE
    } else {
        Color::rgb(0.45, 0.45, 0.45)
    };
    for mut sprite in sprites.p1().iter_mut() {
        sprite.color = icon_color;
    }
}

pub fn setup_essence_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    shop: Res<EssenceShopChoices>,
    resolution: Res<ScreenResolution>,
    coins: Res<CoinCurrency>,
    run_unlocks: Res<RunUnlockState>,
    mouseless: Res<crate::inputs::MouselessModeState>,
    active_device: Res<crate::gamepad_input::ActiveInputDevice>,
    orphan_reroll_flashes: Query<Entity, (With<MerchantCategoryReroll>, Without<UIState>)>,
    existing_ui: Query<Entity, With<EssenceUI>>,
) {
    if !existing_ui.is_empty() {
        return;
    }

    for e in orphan_reroll_flashes.iter() {
        commands.entity(e).despawn_recursive();
    }

    let overlay = spawn_full_screen_ui_overlay_tuned(&mut commands, &resolution, 0.0, 0.95, 9.);
    commands.entity(overlay).insert(UIState::Essence);

    let essence_ui_e = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::MerchantContainer),
            sprite: Sprite {
                custom_size: Some(MERCHANT_CONTAINER_UI_SIZE),
                ..Default::default()
            },
            transform: Transform {
                translation: Vec3::new(0., 0., 10.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(EssenceUI)
        .insert(Name::new("SHOP UI"))
        .insert(UIState::Essence)
        .insert(RenderLayers::from_layers(&[3]))
        .insert(crate::item::item_drop_outline::UiShadow::container())
        .id();

    commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    "Merchant".to_string(),
                    gf::DISPLAY.text_style(&asset_server, WHITE),
                )
                .with_alignment(TextAlignment::Center),
                text_anchor: bevy::sprite::Anchor::Center,
                transform: Transform {
                translation: Vec3::new(0., MERCHANT_TITLE_Y, 3.),
                scale: gf::DISPLAY.transform_scale(),
                ..Default::default()
            },
                ..default()
            },
            RenderLayers::from_layers(&[3]),
            UIState::Essence,
        ))
        .set_parent(essence_ui_e);

    spawn_currency_counter(
        &mut commands,
        &graphics,
        &asset_server,
        essence_ui_e,
        MERCHANT_REROLL_COUNTER_POS,
        run_unlocks.rerolls_remaining,
        (BlacksmithRerollsText, Name::new("Blacksmith Rerolls")),
        |commands, text_parent, _graphics, asset_server| {
            spawn_reroll_icon_sprite(commands, asset_server, text_parent)
        },
    );

    spawn_currency_counter(
        &mut commands,
        &graphics,
        &asset_server,
        essence_ui_e,
        MERCHANT_COIN_COUNTER_POS,
        coins.coins,
        (BlacksmithCoinsText, Name::new("Blacksmith Coins")),
        |commands, text_parent, graphics, asset_server| {
            let icon = spawn_item_stack_icon(
                commands,
                graphics,
                &ItemStack::crate_icon_stack(WorldObject::Coin).copy_with_count(1),
                asset_server,
                Vec2::new(-12., 0.),
                Vec2::ZERO,
                3,
            );
            commands.entity(icon).set_parent(text_parent);
            icon
        },
    );

    spawn_merchant_track_info_box(
        &mut commands,
        &graphics,
        &asset_server,
        essence_ui_e,
        &format_merchant_track_action_header(&mouseless, &active_device),
    );

    for category in [
        MerchantCategory::Heirlooms,
        MerchantCategory::Equipment,
        MerchantCategory::Materials,
    ] {
        spawn_section_label(
            &mut commands,
            &asset_server,
            essence_ui_e,
            category.label(),
            category.label_position(),
        );
    }

    for slot_index in 0..MERCHANT_SLOT_COUNT {
        spawn_merchant_slot_ui(
            &mut commands,
            &graphics,
            &asset_server,
            essence_ui_e,
            slot_index,
            &shop.slots[slot_index],
        );
    }

    for category in [
        MerchantCategory::Heirlooms,
        MerchantCategory::Equipment,
        MerchantCategory::Materials,
    ] {
        let enabled = run_unlocks.rerolls_remaining > 0 && !shop.category_fully_purchased(category);
        spawn_merchant_category_reroll_button(
            &mut commands,
            &asset_server,
            essence_ui_e,
            category,
            enabled,
        );
    }

    let done_btn = spawn_chest_button(
        &mut commands,
        &asset_server,
        &graphics,
        essence_ui_e,
        ChestButtonKind::Done,
        Vec3::new(0., MERCHANT_DONE_BUTTON_Y, 1.),
        true,
        UIState::Essence,
    );
    commands
        .entity(done_btn)
        .insert(MerchantDoneButton)
        .insert(Focusable {
            group: UIState::Essence,
            index: 100,
        });

    commands.insert_resource(MerchantShopUiDirty::default());
}

pub fn refresh_merchant_shop_ui_dirty(
    mut commands: Commands,
    mut ui_dirty: ResMut<MerchantShopUiDirty>,
    shop: Res<EssenceShopChoices>,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    essence_ui: Query<Entity, With<EssenceUI>>,
    slot_ui: Query<(Entity, &MerchantSlotUi)>,
    reroll_buttons: Query<(Entity, &MerchantCategoryRerollButton)>,
) {
    if ui_dirty.slots.is_empty() && ui_dirty.disable_reroll_categories.is_empty() {
        return;
    }

    let Ok(ui_root) = essence_ui.get_single() else {
        ui_dirty.slots.clear();
        ui_dirty.disable_reroll_categories.clear();
        return;
    };

    for slot_index in ui_dirty.slots.drain(..) {
        if let Some((e, _)) = slot_ui.iter().find(|(_, ui)| ui.slot_index == slot_index) {
            commands.entity(e).despawn_recursive();
        }
        spawn_merchant_slot_ui(
            &mut commands,
            &graphics,
            &asset_server,
            ui_root,
            slot_index,
            &shop.slots[slot_index],
        );
    }

    for category in ui_dirty.disable_reroll_categories.drain(..) {
        for (e, btn) in reroll_buttons.iter() {
            if btn.0 == category {
                commands.entity(e).remove::<Interactable>();
            }
        }
    }
}

pub fn handle_submit_merchant_purchase(
    mut commands: Commands,
    mut ev: EventReader<SubmitMerchantPurchase>,
    mut next_inv_state: ResMut<NextState<UIState>>,
    mut currency_event: EventWriter<ModifyCurencyEvent>,
    mut attribute_event: EventWriter<AttributeChangeEvent>,
    mut params: ParamSet<(
        GameParam,
        Query<(Entity, &mut PlayerSkills, &GlobalTransform), With<Player>>,
    )>,
    mut shop: ResMut<EssenceShopChoices>,
    mut purchase_tracker: ResMut<BlacksmithPurchaseTracker>,
    mut proto_commands: ProtoCommands,
    proto: ProtoParam,
    mut minimap_event: EventWriter<UpdateMiniMapEvent>,
    mut cache: ResMut<EssenceShopCache>,
    mut ui_dirty: ResMut<MerchantShopUiDirty>,
    mut inv: Query<&mut Inventory, With<Player>>,
    mut chaos_tracker: ResMut<ChaosTracker>,
    mut analytics: EventWriter<crate::client::analytics::AnalyticsUpdateEvent>,
) {
    for purchase in ev.iter() {
        let slot_index = purchase.slot_index;
        if slot_index >= MERCHANT_SLOT_COUNT {
            continue;
        }

        let slot = &shop.slots[slot_index];
        if slot.purchased {
            continue;
        }

        let time_fragment_cost = match &slot.kind {
            MerchantItemKind::Heirloom {
                time_fragment_cost, ..
            } => *time_fragment_cost,
            _ => 0,
        };

        let time_fragments = params.p0().get_time_fragments();
        let coins = params.p0().get_coins();

        if time_fragments < time_fragment_cost as i32 || coins < slot.coin_cost {
            continue;
        }

        currency_event.send(ModifyCurencyEvent {
            delta: -(time_fragment_cost as i32),
            obj: WorldObject::TimeFragment,
        });
        currency_event.send(ModifyCurencyEvent {
            delta: -(slot.coin_cost as i32),
            obj: WorldObject::Coin,
        });

        match &slot.kind {
            MerchantItemKind::Heirloom {
                heirloom, rarity, ..
            } => {
                if let Ok((player_entity, mut player_skills, player_transform)) =
                    params.p1().get_single_mut()
                {
                    let heirloom_with_rarity = HeirloomWithRarity {
                        heirloom: heirloom.clone(),
                        rarity: rarity.clone(),
                    };
                    player_skills.heirlooms.push(heirloom_with_rarity.clone());
                    heirloom_with_rarity.heirloom.add_heirloom_components(
                        player_entity,
                        &mut commands,
                        player_skills.clone(),
                    );
                    heirloom_with_rarity
                        .heirloom
                        .apply_acquisition_effects(&mut chaos_tracker);
                    let player_pos = player_transform.translation().truncate();
                    if let Some((drop, count)) = heirloom_with_rarity.heirloom.get_instant_drop() {
                        proto_commands.spawn_item_from_proto(
                            drop,
                            &proto,
                            player_pos + Vec2::new(0., -18.),
                            count,
                            None,
                        );
                    }
                    attribute_event.send(AttributeChangeEvent);
                }
            }
            MerchantItemKind::Equipment(stack) | MerchantItemKind::Material(stack) => {
                let collected_obj = stack.obj_type;
                let has_room = inv
                    .get_single()
                    .ok()
                    .and_then(|inventory| {
                        inventory
                            .items
                            .get_first_empty_player_slot_for_pickup(stack, &proto)
                    })
                    .is_some();
                if has_room {
                    if let Ok(mut inventory) = inv.get_single_mut() {
                        let mut game = params.p0();
                        stack.clone().add_to_inventory(
                            &mut inventory.items,
                            &mut game.inv_slot_query,
                            &proto,
                        );
                    }
                    // Direct inventory grant skips ground pickup — still credit Find* achievements.
                    analytics.send(crate::client::analytics::AnalyticsUpdateEvent {
                        update_type: crate::client::analytics::AnalyticsTrigger::ItemCollected(
                            collected_obj,
                        ),
                    });
                } else if let Ok((_, _, player_transform)) = params.p1().get_single() {
                    let player_pos = player_transform.translation().truncate();
                    let mut game = params.p0();
                    stack
                        .clone()
                        .spawn_as_drop(&mut commands, &mut game, player_pos);
                }
            }
        }

        shop.slots[slot_index].purchased = true;
        if shop.marked_slot == Some(slot_index) {
            shop.marked_slot = None;
        }
        purchase_tracker.purchases_made += 1;

        sync_merchant_shop_to_world(&shop, &mut commands, &mut cache);

        ui_dirty.slots.push(slot_index);
        for category in [
            MerchantCategory::Heirlooms,
            MerchantCategory::Equipment,
            MerchantCategory::Materials,
        ] {
            if category.slot_indices().contains(&slot_index)
                && shop.category_fully_purchased(category)
                && !ui_dirty.disable_reroll_categories.contains(&category)
            {
                ui_dirty.disable_reroll_categories.push(category);
            }
        }

        if shop.all_purchased() {
            next_inv_state.set(UIState::Closed);
            if let Some(owner_e) = shop.owner_entity {
                commands
                    .entity(owner_e)
                    .insert(WorldObject::BlacksmithMerchantDone)
                    .remove::<InteractionGuideTrigger>();

                if let Some(tile_pos) = shop.tile_pos {
                    params
                        .p0()
                        .add_object_to_chunk_cache(tile_pos, WorldObject::BlacksmithMerchantDone);
                    minimap_event.send(UpdateMiniMapEvent {
                        pos: Some(tile_pos),
                        new_tile: Some(WorldObject::BlacksmithMerchantDone),
                    });
                }
            }
        }
    }
}

fn generate_heirloom_slot(
    rng: &mut rand::rngs::ThreadRng,
    heirloom_queue: &HeirloomChoiceQueue,
    loot_bonus: i32,
    player_level: u8,
    purchase_multiplier: f32,
    exclude: &[Heirloom],
) -> Option<MerchantShopSlot> {
    let rarity = HeirloomChoiceQueue::gen_rarity(rng, loot_bonus);
    let picked = heirloom_queue.get_skill_of_rarity(rarity.clone(), rng, player_level, &|h| {
        !exclude.contains(&h.heirloom)
    })?;
    let rarity_cost_inc = heirloom_rarity_cost_inc(&picked.rarity);
    let time_fragment_cost = 0;
    let base_coin_cost = 7. * rarity_cost_inc + rng.gen_range(2.0..5.0) * rarity_cost_inc;
    Some(MerchantShopSlot {
        kind: MerchantItemKind::Heirloom {
            heirloom: picked.heirloom,
            rarity: picked.rarity,
            time_fragment_cost,
        },
        base_coin_cost,
        coin_cost: apply_purchase_multiplier(base_coin_cost, purchase_multiplier),
        purchased: false,
    })
}

fn generate_weapon_slot(
    rng: &mut impl Rng,
    commands: &mut Commands,
    proto: &ProtoParam,
    loot_bonus: i32,
    player_level: u8,
    purchase_multiplier: f32,
) -> MerchantShopSlot {
    let weapons: Vec<_> = WorldObject::iter()
        .filter(|o| o.is_weapon() && *o != WorldObject::PlasmaStaff)
        .collect();
    let pick = weapons.choose(rng).expect("weapon pool");
    let mut stack = proto.get_item_data(pick.clone()).unwrap().clone();
    let max_item_level = ((player_level as i32 / 2) - 5).clamp(1, 5) as u8
        + (player_level as i32 / 10).clamp(0, 10) as u8;
    stack.metadata.level = Some(rng.gen_range(1..=max_item_level.max(1)));
    let stack =
        create_new_random_item_stack_with_attributes(&stack, proto, commands, loot_bonus, false);
    let base_coin_cost = 12. * item_rarity_cost_inc(&stack.rarity)
        + rng.gen_range(2.0..4.0) * item_rarity_cost_inc(&stack.rarity);
    MerchantShopSlot {
        kind: MerchantItemKind::Equipment(stack.clone()),
        base_coin_cost,
        coin_cost: apply_purchase_multiplier(base_coin_cost, purchase_multiplier),
        purchased: false,
    }
}

fn generate_armor_slot(
    rng: &mut impl Rng,
    commands: &mut Commands,
    proto: &ProtoParam,
    loot_bonus: i32,
    player_level: u8,
    purchase_multiplier: f32,
) -> MerchantShopSlot {
    let armor: Vec<_> = WorldObject::iter()
        .filter(|o| o.is_armor() || o.is_accessory())
        .collect();
    let pick = armor.choose(rng).expect("armor pool");
    let mut stack = proto.get_item_data(pick.clone()).unwrap().clone();
    let max_item_level = ((player_level as i32 / 2) - 5).clamp(1, 5) as u8
        + (player_level as i32 / 10).clamp(0, 10) as u8;
    stack.metadata.level = Some(rng.gen_range(1..=max_item_level.max(1)));
    let stack =
        create_new_random_item_stack_with_attributes(&stack, proto, commands, loot_bonus, false);
    let base_coin_cost = 10. * item_rarity_cost_inc(&stack.rarity)
        + rng.gen_range(2.0..4.0) * item_rarity_cost_inc(&stack.rarity);
    MerchantShopSlot {
        kind: MerchantItemKind::Equipment(stack.clone()),
        base_coin_cost,
        coin_cost: apply_purchase_multiplier(base_coin_cost, purchase_multiplier),
        purchased: false,
    }
}

fn generate_material_slot(
    rng: &mut impl Rng,
    proto: &ProtoParam,
    purchase_multiplier: f32,
) -> MerchantShopSlot {
    let pool = [
        WorldObject::UpgradeTome,
        WorldObject::MagicGem,
        WorldObject::OrbOfTransformation,
        WorldObject::SmallPotion,
        WorldObject::LargePotion,
        WorldObject::AttackSpeedPotion,
        WorldObject::MovementSpeedPotion,
    ];
    let pick = pool.choose(rng).unwrap();
    let stack = proto.get_item_data(*pick).unwrap().clone();
    let base = match pick {
        WorldObject::UpgradeTome => 6.,
        WorldObject::MagicGem => 8.,
        WorldObject::OrbOfTransformation => 12.,
        WorldObject::SmallPotion => 3.,
        WorldObject::LargePotion => 4.,
        WorldObject::AttackSpeedPotion => 4.,
        WorldObject::MovementSpeedPotion => 4.,
        _ => 6.,
    };
    let base_coin_cost = base + rng.gen_range(1.0..3.0);
    MerchantShopSlot {
        kind: MerchantItemKind::Material(stack),
        base_coin_cost,
        coin_cost: apply_purchase_multiplier(base_coin_cost, purchase_multiplier),
        purchased: false,
    }
}

fn reroll_merchant_category(
    category: MerchantCategory,
    slots: &mut [MerchantShopSlot; MERCHANT_SLOT_COUNT],
    rng: &mut rand::rngs::ThreadRng,
    commands: &mut Commands,
    proto: &ProtoParam,
    heirloom_queue: &HeirloomChoiceQueue,
    loot_bonus: i32,
    player_level: u8,
    purchase_multiplier: f32,
) {
    match category {
        MerchantCategory::Heirlooms => {
            let mut exclude = Vec::new();
            for &idx in category.slot_indices() {
                if slots[idx].purchased {
                    if let MerchantItemKind::Heirloom { heirloom, .. } = &slots[idx].kind {
                        exclude.push(heirloom.clone());
                    }
                    continue;
                }
                if let Some(new_slot) = generate_heirloom_slot(
                    rng,
                    heirloom_queue,
                    loot_bonus,
                    player_level,
                    purchase_multiplier,
                    &exclude,
                ) {
                    if let MerchantItemKind::Heirloom { heirloom, .. } = &new_slot.kind {
                        exclude.push(heirloom.clone());
                    }
                    slots[idx] = new_slot;
                }
            }
        }
        MerchantCategory::Equipment => {
            for &idx in category.slot_indices() {
                if slots[idx].purchased {
                    continue;
                }
                slots[idx] = if idx == 3 {
                    generate_weapon_slot(
                        rng,
                        commands,
                        proto,
                        loot_bonus,
                        player_level,
                        purchase_multiplier,
                    )
                } else {
                    generate_armor_slot(
                        rng,
                        commands,
                        proto,
                        loot_bonus,
                        player_level,
                        purchase_multiplier,
                    )
                };
            }
        }
        MerchantCategory::Materials => {
            for &idx in category.slot_indices() {
                if slots[idx].purchased {
                    continue;
                }
                slots[idx] = generate_material_slot(rng, proto, purchase_multiplier);
            }
        }
    }
}

pub fn handle_merchant_category_reroll_event(
    mut ev: EventReader<MerchantCategoryRerollEvent>,
    mut shop: ResMut<EssenceShopChoices>,
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    essence_ui: Query<Entity, With<EssenceUI>>,
    player_atts: Query<(&LootRateBonus, &PlayerLevel), With<Player>>,
    heirloom_queue: Res<HeirloomChoiceQueue>,
    proto: ProtoParam,
    run_unlocks: Res<RunUnlockState>,
    mut cache: ResMut<EssenceShopCache>,
    slot_ui: Query<(Entity, &MerchantSlotUi)>,
    reroll_buttons: Query<(Entity, &MerchantCategoryRerollButton)>,
) {
    let Ok(ui_root) = essence_ui.get_single() else {
        return;
    };

    for request in ev.iter() {
        apply_merchant_category_reroll(
            request.category,
            &mut shop,
            &mut commands,
            &graphics,
            &asset_server,
            ui_root,
            &player_atts,
            &heirloom_queue,
            &proto,
            &run_unlocks,
            &mut cache,
            &slot_ui,
            &reroll_buttons,
        );
    }
}

pub fn apply_merchant_category_reroll(
    category: MerchantCategory,
    shop: &mut EssenceShopChoices,
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    ui_root: Entity,
    player_atts: &Query<(&LootRateBonus, &PlayerLevel), With<Player>>,
    heirloom_queue: &HeirloomChoiceQueue,
    proto: &ProtoParam,
    run_unlocks: &RunUnlockState,
    cache: &mut EssenceShopCache,
    slot_ui: &Query<(Entity, &MerchantSlotUi)>,
    reroll_buttons: &Query<(Entity, &MerchantCategoryRerollButton)>,
) {
    let (loot_bonus, player_level) = player_atts
        .get_single()
        .map(|a| (a.0 .0, a.1.level))
        .unwrap_or((0, 1));
    let purchase_multiplier = purchase_multiplier_for_level(player_level);
    let mut rng = rand::thread_rng();

    reroll_merchant_category(
        category,
        &mut shop.slots,
        &mut rng,
        commands,
        proto,
        heirloom_queue,
        loot_bonus,
        player_level,
        purchase_multiplier,
    );

    sync_merchant_shop_to_world(shop, commands, cache);

    refresh_merchant_category_ui(
        commands,
        graphics,
        asset_server,
        shop,
        ui_root,
        category,
        run_unlocks,
        slot_ui,
        reroll_buttons,
    );
}

pub fn handle_populate_essence_shop_on_new_spawn(
    mut new_spawns: Query<
        (Entity, &mut EssenceShopChoices, Option<&GlobalTransform>),
        Added<EssenceShopChoices>,
    >,
    player_atts: Query<(&LootRateBonus, &PlayerLevel), With<crate::player::Player>>,
    heirloom_queue: Res<HeirloomChoiceQueue>,
    mut shop_cache: ResMut<EssenceShopCache>,
    mut commands: Commands,
    proto: ProtoParam,
) {
    for (entity, mut shop, transform) in new_spawns.iter_mut() {
        let mut rng = rand::thread_rng();

        // Authoritative tile key is set when the object is placed (see
        // `handle_placing_world_object`). Only fall back to transform math for legacy spawns.
        if shop.tile_pos.is_none() {
            if let Some(t) = transform {
                let world_pos = t.translation().truncate() - Vec2::new(0., 12.);
                shop.tile_pos = Some(crate::world::world_helpers::world_pos_to_tile_pos(
                    world_pos,
                ));
            }
        }
        let Some(tile_pos) = shop.tile_pos else {
            warn!("Blacksmith merchant spawned without tile_pos; skipping shop populate");
            continue;
        };

        let player_level = player_atts.get_single().map(|a| a.1.level).unwrap_or(1);
        let loot_bonus = player_atts.get_single().map(|a| a.0 .0).unwrap_or(0);
        let purchase_multiplier = purchase_multiplier_for_level(player_level);

        if let Some(cached) = shop_cache.shops.get(&tile_pos) {
            shop.slots = cached.clone();
            for slot in shop.slots.iter_mut() {
                if let MerchantItemKind::Heirloom {
                    time_fragment_cost, ..
                } = &mut slot.kind
                {
                    *time_fragment_cost = 0;
                }
                if !slot.purchased && slot.base_coin_cost > 0. {
                    slot.coin_cost =
                        apply_purchase_multiplier(slot.base_coin_cost, purchase_multiplier);
                }
            }
            // Filter banished heirlooms from unpurchased heirloom slots
            for idx in MerchantCategory::Heirlooms.slot_indices() {
                if shop.slots[*idx].purchased {
                    continue;
                }
                if let MerchantItemKind::Heirloom { heirloom, .. } = &shop.slots[*idx].kind {
                    if heirloom_queue.banned.contains(heirloom) {
                        shop.slots[*idx] = generate_heirloom_slot(
                            &mut rng,
                            &heirloom_queue,
                            loot_bonus,
                            player_level,
                            purchase_multiplier,
                            &[],
                        )
                        .unwrap_or_default();
                    }
                }
            }
        } else {
            let mut slots: [MerchantShopSlot; MERCHANT_SLOT_COUNT] =
                std::array::from_fn(|_| MerchantShopSlot::default());
            let mut exclude = Vec::new();
            for idx in 0..3 {
                if let Some(slot) = generate_heirloom_slot(
                    &mut rng,
                    &heirloom_queue,
                    loot_bonus,
                    player_level,
                    purchase_multiplier,
                    &exclude,
                ) {
                    if let MerchantItemKind::Heirloom { heirloom, .. } = &slot.kind {
                        exclude.push(heirloom.clone());
                    }
                    slots[idx] = slot;
                }
            }
            slots[3] = generate_weapon_slot(
                &mut rng,
                &mut commands,
                &proto,
                loot_bonus,
                player_level,
                purchase_multiplier,
            );
            slots[4] = generate_armor_slot(
                &mut rng,
                &mut commands,
                &proto,
                loot_bonus,
                player_level,
                purchase_multiplier,
            );
            for idx in 5..7 {
                slots[idx] = generate_material_slot(&mut rng, &proto, purchase_multiplier);
            }
            shop.slots = slots;
        }

        // Restore the tracked-item marker; drop it if that slot is now purchased.
        shop.marked_slot = shop_cache
            .marked
            .get(&tile_pos)
            .copied()
            .filter(|&slot| slot < MERCHANT_SLOT_COUNT && !shop.slots[slot].purchased);
        if shop.marked_slot.is_none() {
            shop_cache.marked.remove(&tile_pos);
        }

        shop_cache.shops.insert(tile_pos, shop.slots.clone());
        shop.owner_entity = Some(entity);
    }
}
