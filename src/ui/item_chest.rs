use bevy::text::Justify;
use crate::aseprite_assets::SkillChoiceFlash;
use bevy::ecs::system::SystemParam;
use bevy::{camera::visibility::RenderLayers, prelude::*, sprite::Anchor};
use itertools::Itertools;
use rand::{seq::SliceRandom, Rng};
use strum::IntoEnumIterator;

use crate::{
    assets::Graphics,
    attributes::{
        attribute_helpers::{create_new_random_item_stack_with_attributes, spawn_rarity_animation},
        ItemRarity,
    },
    colors::{WHITE, YELLOW},
    cursor::CursorPos,
    inventory::{Inventory, ItemStack},
    item::{EquipmentType, WorldObject},
    juice::bounce::BounceOnHit,
    player::{
        skills::{Heirloom, HeirloomChoiceQueue, HeirloomChoiceState, HeirloomRarity},
        time_crystals::TimeCrystals,
        unlocks::RunUnlockState,
    },
    proto::proto_param::ProtoParam,
    GameParam, ScreenResolution,
};

use super::{
    banish_tracker_ui::spawn_banish_tracker,
    essence_ui::{MERCHANT_REROLL_ICON_PATH, MERCHANT_REROLL_ICON_SIZE},
    game_fonts as gf,
    heirloom_tooltip::{HeirloomTooltipRequest, HeirloomTooltipShow},
    interactions::Interaction,
    ui_helpers, Focusable, Interactable, ToolTipUpdateEvent, TooltipTeardownEvent, UIElement,
    UIState, CURRENCY_BACKGROUND_SIZE, KEYBIND_BADGE_COLOR,
};


/// Background container art size (`assets/ui/ChestContainer.png`).
pub const CHEST_CONTAINER_UI_SIZE: Vec2 = Vec2::new(130., 148.);
/// Per-button art size (`assets/ui/ChestButton.png`).
pub const CHEST_BUTTON_SIZE: Vec2 = Vec2::new(34., 12.);
/// Y of the chest icon / opening anim / final reward sprite (centered horizontally).
/// Sits in the lower portion of the container so the reveal text stack (title / rarity /
/// type) has clean space above it once the chest opens.
const CHEST_ICON_Y: f32 = -6.;
const CHEST_OPENING_Y: f32 = -30.;
/// Y of action buttons along the bottom edge.
const CHEST_BUTTON_Y: f32 = -64.;
/// Horizontal offset for the paired Take/Equip / Take/Banish buttons.
const CHEST_BUTTON_X_OFFSET: f32 = 22.;
/// Y for the heirloom banishes-remaining counter text (sits above the buttons).
const CHEST_BANISH_COUNT_Y: f32 = -57.;
/// Y for the heirloom-chest reroll badge (above Take/Banish).
const CHEST_REROLL_BUTTON_Y: f32 = -46.;
const CHEST_REROLL_BADGE_SIZE: Vec2 = Vec2::new(14., 12.);
/// Right-side rerolls-remaining counter (CurrencyBackground), relative to chest root.
const CHEST_REROLL_COUNTER_POS: Vec2 = Vec2::new(116., 20.);
// Reveal text stack (top → bottom). Sits above the chest icon once it shifts down to
// `CHEST_ICON_Y` after opening — the chest UI lives in render-layer 3 world space.
const CHEST_REVEAL_TITLE_Y: f32 = 48.;
const CHEST_REVEAL_RARITY_Y: f32 = 26.;
const CHEST_REVEAL_TYPE_Y: f32 = 14.;

/// Type of chest being opened - determines what content is picked and how it's granted
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChestType {
    Item,
    Heirloom,
}

/// Unified state for both Item and Heirloom chests - reuses the same UI and animation logic
#[derive(Resource, Debug, Clone)]
pub struct ItemChestState {
    pub chest_type: ChestType,
    pub shuffle_timer: Timer,
    pub shuffle_duration_timer: Timer,
    pub state: ItemChestAnimState,
    pub current_entity: Option<Entity>,
    pub current_ui_rarity: ItemRarity,
    // Item chest specific
    pub picked_item: Option<ItemStack>,
    pub current_item: Option<WorldObject>,
    // Heirloom chest specific
    pub picked_heirloom: Option<HeirloomChoiceState>,
    pub current_heirloom: Option<Heirloom>,
    pub target_heirloom_rarity: Option<HeirloomRarity>, // Rarity to pick for heirloom chests
}

impl ItemChestState {
    pub fn new_item_chest() -> Self {
        Self {
            chest_type: ChestType::Item,
            shuffle_timer: Timer::from_seconds(0.06, TimerMode::Once),
            shuffle_duration_timer: Timer::from_seconds(1.5, TimerMode::Once),
            state: ItemChestAnimState::Closed,
            current_entity: None,
            current_ui_rarity: ItemRarity::Common,
            picked_item: None,
            current_item: None,
            picked_heirloom: None,
            current_heirloom: None,
            target_heirloom_rarity: None,
        }
    }

    pub fn new_heirloom_chest() -> Self {
        Self {
            chest_type: ChestType::Heirloom,
            shuffle_timer: Timer::from_seconds(0.06, TimerMode::Once),
            shuffle_duration_timer: Timer::from_seconds(1.5, TimerMode::Once),
            state: ItemChestAnimState::Closed,
            current_entity: None,
            current_ui_rarity: ItemRarity::Common,
            picked_item: None,
            current_item: None,
            picked_heirloom: None,
            current_heirloom: None,
            target_heirloom_rarity: None,
        }
    }

    /// Get the rarity of the picked content (works for both chest types)
    pub fn get_picked_rarity(&self) -> ItemRarity {
        match self.chest_type {
            ChestType::Item => self
                .picked_item
                .as_ref()
                .map(|i| i.rarity.clone())
                .unwrap_or(ItemRarity::Common),
            ChestType::Heirloom => self
                .picked_heirloom
                .as_ref()
                .map(|h| heirloom_rarity_to_item_rarity(&h.rarity))
                .unwrap_or(ItemRarity::Common),
        }
    }
}

/// Convert HeirloomRarity to ItemRarity for UI consistency
pub fn heirloom_rarity_to_item_rarity(rarity: &HeirloomRarity) -> ItemRarity {
    match rarity {
        HeirloomRarity::Common => ItemRarity::Common,
        HeirloomRarity::Uncommon => ItemRarity::Uncommon,
        HeirloomRarity::Rare => ItemRarity::Rare,
        HeirloomRarity::Legendary => ItemRarity::Legendary,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemChestAnimState {
    Closed,
    Opening,
    Done,
}

#[derive(Component)]
pub struct ItemChest;

#[derive(Component)]
pub struct ItemChestUI;

#[derive(Component)]
pub struct ItemChestFinalItem;

/// Component to store heirloom data on the final heirloom entity in chest
#[derive(Component)]
pub struct ItemChestFinalHeirloom {
    pub heirloom: HeirloomChoiceState,
}

/// Which action a [`ItemChestButton`] performs when clicked.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChestButtonKind {
    /// Visible while the chest is `Closed` / `Opening`. First click starts the open,
    /// further clicks fast-forward the shuffle (see `advance_*_chest_state`).
    Open,
    /// Spawn the picked item as a floor drop next to the player (item chest only).
    Take,
    /// Auto-equip the picked item into the matching slot, displacing any current piece (item chest only).
    Equip,
    /// Add the picked heirloom to the banned pool and decrement banishes_remaining (heirloom chest only).
    Banish,
    /// Close the merchant shop without deactivating the world object.
    Done,
}

impl ChestButtonKind {
    pub fn focus_index(self) -> u32 {
        match self {
            ChestButtonKind::Open => 0,
            ChestButtonKind::Take => 1,
            ChestButtonKind::Equip => 2,
            ChestButtonKind::Banish => 3,
            ChestButtonKind::Done => 4,
        }
    }
}

#[derive(Component)]
pub struct ItemChestButton {
    pub kind: ChestButtonKind,
}

/// Marker for the label text spawned as a child of a chest button — kept distinct
/// so future restyles can re-query just the label without picking up other text on
/// the chest UI.
#[derive(Component)]
pub struct ChestButtonLabel;

/// Marker for the "Banishes: N" counter displayed above the heirloom-chest Banish button.
#[derive(Component)]
pub struct ChestBanishCountText;

/// Clickable reroll badge shown after a heirloom chest finishes revealing its prize.
#[derive(Component)]
pub struct HeirloomChestRerollButton;

#[derive(Component)]
pub struct HeirloomChestRerollIcon;

/// Marker for the rerolls-remaining number in the chest-side currency counter.
#[derive(Component)]
pub struct HeirloomChestRerollsText;

/// Marker for any text/sprite spawned by the chest reveal stack (title / rarity / type /
/// currently-equipped tooltip + header). Kept distinct from the chest UI root so the
/// reveal can be re-spawned without rebuilding the container.
#[derive(Component)]
pub struct ChestRevealUI;

/// Human-friendly label for an `EquipmentType` used in the chest reveal "type" row.
pub fn equipment_type_display_name(et: &EquipmentType) -> &'static str {
    match et {
        EquipmentType::Head => "Helmet",
        EquipmentType::Chest => "Chestplate",
        EquipmentType::Legs => "Pants",
        EquipmentType::Feet => "Boots",
        EquipmentType::Ring => "Ring",
        EquipmentType::Pendant => "Pendant",
        EquipmentType::Trinket => "Trinket",
        EquipmentType::Weapon => "Weapon",
        EquipmentType::Cape => "Cape",
        EquipmentType::Axe => "Axe",
        EquipmentType::Pickaxe => "Pickaxe",
        EquipmentType::None => "",
    }
}

#[derive(Message)]
pub struct ItemChestAnimChangeEvent {
    pub state: ItemChestAnimState,
    pub set_ui_rarity: Option<ItemRarity>,
}

/// Spawn a single chest action button as a child of `parent`. Returns the button entity.
/// Layout: art is 26×12 (`ChestButton.png`), centered at `translation` with a small label
/// text rendered at the visual center (slightly nudged so the 4×5 font sits above the
/// baseline visually).
pub fn spawn_chest_button(
    commands: &mut Commands,
    asset_server: &AssetServer,
    graphics: &Graphics,
    parent: Entity,
    kind: ChestButtonKind,
    translation: Vec3,
    enabled: bool,
    ui_state: UIState,
) -> Entity {
    let label = match kind {
        ChestButtonKind::Open => "OPEN",
        ChestButtonKind::Take => "TAKE",
        ChestButtonKind::Equip => "EQUIP",
        ChestButtonKind::Banish => "BANISH",
        ChestButtonKind::Done => "DONE",
    };
    // Banish uses its own art so the destructive action reads as distinct from the
    // standard chest buttons; everything else shares `ChestButton.png`.
    let ui_element = match kind {
        ChestButtonKind::Banish => UIElement::ChestButtonBanish,
        _ => UIElement::ChestButton,
    };
    let color = if enabled {
        Color::WHITE
    } else {
        Color::srgb(0.5, 0.5, 0.5)
    };

    let mut button = commands.spawn((
        Sprite {
            image: graphics.get_ui_element_texture(ui_element.clone()),
            custom_size: Some(CHEST_BUTTON_SIZE),
            color,
            ..default()
        },
        Transform::from_translation(translation),
    ));
    button
        .insert(ui_element)
        .insert(ui_state.clone())
        .insert(ItemChestButton { kind })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new(format!("ITEM CHEST BUTTON {label}")));
    if enabled {
        button.insert(Interactable::default()).insert(Focusable {
            group: ui_state.clone(),
            index: kind.focus_index(),
        });
    }
    let button_entity = button.id();

    let text_color = if enabled {
        WHITE
    } else {
        Color::srgb(0.7, 0.7, 0.7)
    };
    let pos = if kind == ChestButtonKind::Equip {
        Vec3::new(1., 0., 1.)
    } else {
        Vec3::new(0., 0., 1.)
    };
    commands
        .spawn((
            gf::SKILL_CHOICE_MICRO
                .text(&asset_server, label, text_color)
                .justify(Justify::Center)
                .anchor(Anchor::CENTER)
                .with_transform(Transform {
                    translation: pos,
                    scale: gf::SKILL_CHOICE_MICRO.transform_scale(),
                    ..Default::default()
                }),
            RenderLayers::from_layers(&[3]),
            ui_state.clone(),
            ChestButtonLabel,
            Name::new(format!("ITEM CHEST BUTTON LABEL {label}")),
        ))
        .insert(ChildOf(button_entity));

    commands.entity(parent).add_child(button_entity);
    button_entity
}

/// Tints the child label of any hovered chest button yellow, restoring white when the
/// pointer leaves. Disabled buttons have no `Interactable` so they keep their dim color.
pub fn update_chest_button_label_hover(
    buttons: Query<(&Interactable, &Children), With<ItemChestButton>>,
    mut labels: Query<&mut TextColor, With<ChestButtonLabel>>,
) {
    for (interactable, children) in buttons.iter() {
        let hovered = matches!(interactable.current(), Interaction::Hovering);
        let color = if hovered { YELLOW } else { WHITE };
        for child in children.iter() {
            if let Ok(mut text_color) = labels.get_mut(child) {
                text_color.0 = color;
            }
        }
    }
}

pub fn setup_item_chest_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    res: Res<ScreenResolution>,
    item_chest_state: ResMut<ItemChestState>,
    skill_queue: Res<HeirloomChoiceQueue>,
    time_crystals: Res<TimeCrystals>,
) {
    // // title bar
    // let title_sprite = commands
    //     .spawn(SpriteBundle {
    //         sprite: Sprite { image: graphics.get_ui_element_texture(UIElement::TitleBar).clone(), ..default() },
    //         sprite: Sprite {
    //             custom_size: Some(Vec2::new(168., 16.)),
    //             ..Default::default()
    //         },
    //         transform: Transform {
    //             translation: Vec3::new(0., 80., 10.),
    //             scale: Vec3::new(1., 1., 1.),
    //             ..Default::default()
    //         },
    //         ..Default::default()
    //     })
    //     .insert(RenderLayers::from_layers(&[3]))
    //     .insert(UIElement::TitleBar)
    //     .insert(UIState::Skills)
    //     .insert(Name::new("SKILL ICON!!"))
    //     .id();

    // let title_text = spawn_text(
    //     &mut commands,
    //     &asset_server,
    //     Vec3::new(0., 0., 1.),
    //     BLACK,
    //     "Choose a new skill".to_string(),
    //     Anchor::CENTER,
    //     2.,
    //     3,
    // );
    // commands
    //     .entity(title_text)
    //     .insert(UIState::Skills)
    //     .insert(ChildOf(title_sprite));

    ui_helpers::spawn_full_screen_ui_overlay_tuned(&mut commands, &res, 0.0, 0.95, 9.);

    let ui_element = UIElement::ChestContainer;

    let item_chest_ui = commands
        .spawn((
            Sprite {
                image: graphics.get_ui_element_texture(ui_element.clone()),
                custom_size: Some(CHEST_CONTAINER_UI_SIZE),
                ..default()
            },
            Transform {
                translation: Vec3::new(0., 0., 10.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
        ))
        .insert(ui_element)
        .insert(UIState::ItemChest)
        .insert(ItemChestUI)
        .insert(Name::new("ITEM CHEST"))
        .insert(RenderLayers::from_layers(&[3]))
        .insert(crate::item::item_drop_outline::UiShadow::container())
        .id();

    // closed chest icon (replaced when the open animation starts)
    commands
        .spawn((
            (graphics
                    .spritesheet_map
                    .as_ref()
                    .unwrap()
                    .get(match item_chest_state.chest_type {
                        ChestType::Item => &WorldObject::ChestBlock,
                        ChestType::Heirloom => &WorldObject::HeirloomChest,
                    })
                    .unwrap()
                    .clone()),
            Transform {
                translation: Vec2::new(0., CHEST_OPENING_Y).extend(4.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
        ))
        .insert(ItemChest)
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("Chest Icon"))
        .insert(ChildOf(item_chest_ui));

    spawn_chest_button(
        &mut commands,
        &asset_server,
        &graphics,
        item_chest_ui,
        ChestButtonKind::Open,
        Vec3::new(0., CHEST_BUTTON_Y, 1.),
        true,
        UIState::ItemChest,
    );

    if matches!(item_chest_state.chest_type, ChestType::Heirloom) {
        spawn_banish_tracker(
            &mut commands,
            &asset_server,
            &graphics,
            skill_queue.as_ref(),
            time_crystals.as_ref(),
            &res,
            UIState::ItemChest,
        );
    }
}

pub fn toggle_item_chest_visibility(
    mut next_inv_state: ResMut<NextState<UIState>>,
    curr_ui_state: Res<State<UIState>>,
    chest_state: Option<Res<ItemChestState>>,
    mut tutorial_popup_events: MessageWriter<crate::ui::tutorial_ui::TutorialPopupEvent>,
    seen_tutorial_chunks: Option<Res<crate::ui::tutorial_ui::SeenTutorialChunks>>,
    tutorial_ui: Query<(), With<crate::ui::tutorial_ui::TutorialUI>>,
) {
    if *curr_ui_state.get() == UIState::ActiveSkills || *curr_ui_state.get() == UIState::ItemChest {
        return;
    }
    if chest_state
        .as_ref()
        .map(|c| c.chest_type == ChestType::Item)
        .unwrap_or(false)
    {
        if let Some(seen_tutorial_chunks) = seen_tutorial_chunks.as_ref() {
            crate::ui::tutorial_ui::try_equipment_chest_tutorial(
                &mut tutorial_popup_events,
                seen_tutorial_chunks,
                &tutorial_ui,
            );
        }
    }
    next_inv_state.set(UIState::ItemChest);
}

pub fn shuffle_items(
    mut item_chest_state: ResMut<ItemChestState>,
    time: Res<Time>,
    mut commands: Commands,
    graphics: Res<Graphics>,
    mut events: MessageWriter<ItemChestAnimChangeEvent>,
    choices_queue: Res<HeirloomChoiceQueue>,
) {
    if item_chest_state.state != ItemChestAnimState::Opening {
        return;
    }
    item_chest_state.shuffle_timer.tick(time.delta());
    item_chest_state.shuffle_duration_timer.tick(time.delta());
    if item_chest_state.shuffle_duration_timer.just_finished() {
        if let Some(current_entity) = item_chest_state.current_entity {
            commands.entity(current_entity).despawn();
            item_chest_state.current_entity = None;
            events.write(ItemChestAnimChangeEvent {
                state: ItemChestAnimState::Done,
                set_ui_rarity: None,
            });
        }
        return;
    }
    // Use unified rarity getter that works for both chest types
    let picked_rarity = item_chest_state.get_picked_rarity();
    if item_chest_state.shuffle_duration_timer.fraction() >= 0.25
        && item_chest_state.current_ui_rarity == ItemRarity::Common
        && picked_rarity != ItemRarity::Common
    {
        item_chest_state.current_ui_rarity = ItemRarity::Uncommon;

        events.write(ItemChestAnimChangeEvent {
            state: ItemChestAnimState::Opening,
            set_ui_rarity: Some(ItemRarity::Uncommon),
        });
    } else if item_chest_state.shuffle_duration_timer.fraction() >= 0.48
        && item_chest_state.current_ui_rarity == ItemRarity::Uncommon
        && picked_rarity != ItemRarity::Uncommon
    {
        item_chest_state.current_ui_rarity = ItemRarity::Rare;
        events.write(ItemChestAnimChangeEvent {
            state: ItemChestAnimState::Opening,
            set_ui_rarity: Some(ItemRarity::Rare),
        });
    } else if item_chest_state.shuffle_duration_timer.fraction() >= 0.7
        && item_chest_state.current_ui_rarity == ItemRarity::Rare
        && picked_rarity != ItemRarity::Rare
    {
        item_chest_state.current_ui_rarity = ItemRarity::Legendary;
        events.write(ItemChestAnimChangeEvent {
            state: ItemChestAnimState::Opening,
            set_ui_rarity: Some(ItemRarity::Legendary),
        });
    }
    if item_chest_state.shuffle_timer.is_finished()
        && !item_chest_state.shuffle_duration_timer.is_finished()
    {
        if let Some(current_entity) = item_chest_state.current_entity {
            if commands.get_entity(current_entity).is_ok() {
                commands.entity(current_entity).despawn();
            }
        }
        let mut rng = rand::thread_rng();

        // Branch based on chest type for icon spawning
        let icon = match item_chest_state.chest_type {
            ChestType::Item => {
                let filtered_items = WorldObject::iter()
                    .filter(|obj| {
                        item_chest_state
                            .current_item
                            .map(|old_item| old_item != *obj)
                            .unwrap_or(true)
                            && (obj.is_weapon() || obj.is_armor() || obj.is_accessory())
                            && obj != &WorldObject::PlasmaStaff
                    })
                    .collect_vec();
                let pick_new_item = filtered_items.choose(&mut rng).expect("No items found");
                item_chest_state.current_item = Some(pick_new_item.clone());
                commands
                    .spawn((
                        (graphics
                                .spritesheet_map
                                .as_ref()
                                .unwrap()
                                .get(&pick_new_item.clone())
                                .unwrap()
                                .clone()),
                        Transform {
                            translation: Vec3::new(-1., CHEST_ICON_Y, 15.),
                            scale: Vec3::new(1., 1., 1.),
                            ..Default::default()
                        },
                    ))
                    .insert(UIState::ItemChest)
                    .insert(RenderLayers::from_layers(&[3]))
                    .insert(Name::new("Chest Icon!!"))
                    .id()
            }
            ChestType::Heirloom => {
                // Filter by target rarity if set, otherwise show any
                let target_rarity = item_chest_state.target_heirloom_rarity.clone();
                let available_heirlooms: Vec<&HeirloomChoiceState> = choices_queue
                    .pool
                    .iter()
                    .filter(|choice| {
                        // Never show Heirloom::None (banish placeholder has no icon)
                        let valid_heirloom = choice.heirloom != Heirloom::None;
                        // Don't show the same heirloom twice in a row
                        let not_same = item_chest_state
                            .current_heirloom
                            .as_ref()
                            .map(|old| *old != choice.heirloom)
                            .unwrap_or(true);
                        // Match target rarity if set, otherwise allow any
                        let matches_rarity = target_rarity
                            .as_ref()
                            .map(|rarity| choice.rarity == *rarity)
                            .unwrap_or(true);
                        // Not banned
                        let not_banned = !choices_queue.banned.contains(&choice.heirloom);
                        valid_heirloom && not_same && matches_rarity && not_banned
                    })
                    .collect_vec();

                let picked_heirloom = if let Some(picked) = available_heirlooms.choose(&mut rng) {
                    picked.heirloom.clone()
                } else {
                    // Fallback: if no heirlooms match, just pick any (shouldn't happen normally)
                    let fallback_heirlooms: Vec<&HeirloomChoiceState> = choices_queue
                        .pool
                        .iter()
                        .filter(|choice| {
                            choice.heirloom != Heirloom::None
                                && item_chest_state
                                    .current_heirloom
                                    .as_ref()
                                    .map(|old| *old != choice.heirloom)
                                    .unwrap_or(true)
                                && !choices_queue.banned.contains(&choice.heirloom)
                        })
                        .collect();
                    if let Some(picked) = fallback_heirlooms.choose(&mut rng) {
                        picked.heirloom.clone()
                    } else {
                        // No heirlooms available at all - skip this shuffle cycle
                        return;
                    }
                };

                item_chest_state.current_heirloom = Some(picked_heirloom.clone());

                commands
                    .spawn((
                        graphics.get_heirloom_icon(picked_heirloom),
                        Transform {
                            translation: Vec3::new(-1., CHEST_ICON_Y, 15.),
                            scale: Vec3::new(1., 1., 1.),
                            ..Default::default()
                        },
                    ))
                    .insert(UIState::ItemChest)
                    .insert(RenderLayers::from_layers(&[3]))
                    .insert(Name::new("Chest Icon!!"))
                    .id()
            }
        };
        item_chest_state.current_entity = Some(icon);
        item_chest_state.shuffle_timer.reset();
    }
}

/// Spawn the three reveal labels (name / rarity / type) above the chest icon when the
/// chest finishes opening. All entities are tagged `UIState::ItemChest` + `ChestRevealUI`
/// so they tear down with the rest of the chest UI.
fn spawn_chest_reveal_text(
    commands: &mut Commands,
    asset_server: &AssetServer,
    parent: Entity,
    name: String,
    rarity: ItemRarity,
    type_label: &str,
) {
    // Parented to the chest UI root so the text inherits its world transform (the
    // container sprite renders at z=10; local z values here stack above it).
    commands
        .spawn((
            gf::TOOLTIP_ITEM_TITLE
                .text(&asset_server, name, rarity.get_color())
                .justify(Justify::Center)
                .anchor(Anchor::CENTER)
                .with_transform(Transform {
                    translation: Vec3::new(0., CHEST_REVEAL_TITLE_Y, 2.),
                    scale: gf::TOOLTIP_ITEM_TITLE.transform_scale(),
                    ..Default::default()
                }),
            RenderLayers::from_layers(&[3]),
            UIState::ItemChest,
            ChestRevealUI,
            Name::new("CHEST REVEAL TITLE"),
        ))
        .insert(ChildOf(parent));

    commands
        .spawn((
            gf::TOOLTIP_CARD_LINE
                .text(&asset_server, rarity.get_name(), rarity.get_color())
                .justify(Justify::Center)
                .anchor(Anchor::CENTER)
                .with_transform(Transform {
                    translation: Vec3::new(0., CHEST_REVEAL_RARITY_Y, 2.),
                    scale: gf::TOOLTIP_CARD_LINE.transform_scale(),
                    ..Default::default()
                }),
            RenderLayers::from_layers(&[3]),
            UIState::ItemChest,
            ChestRevealUI,
            Name::new("CHEST REVEAL RARITY"),
        ))
        .insert(ChildOf(parent));

    if !type_label.is_empty() {
        commands
            .spawn((
                gf::TOOLTIP_CARD_LINE
                    .text(&asset_server, type_label, WHITE)
                    .justify(Justify::Center)
                    .anchor(Anchor::CENTER)
                    .with_transform(Transform {
                        translation: Vec3::new(0., CHEST_REVEAL_TYPE_Y, 2.),
                        scale: gf::TOOLTIP_CARD_LINE.transform_scale(),
                        ..Default::default()
                    }),
                RenderLayers::from_layers(&[3]),
                UIState::ItemChest,
                ChestRevealUI,
                Name::new("CHEST REVEAL TYPE"),
            ))
            .insert(ChildOf(parent));
    }
}

fn spawn_heirloom_chest_reroll_button(
    commands: &mut Commands,
    asset_server: &AssetServer,
    parent: Entity,
    enabled: bool,
) {
    let color = if enabled {
        Color::WHITE
    } else {
        Color::srgb(0.45, 0.45, 0.45)
    };

    let mut btn = commands.spawn((
        Sprite {
            color: KEYBIND_BADGE_COLOR,
            custom_size: Some(CHEST_REROLL_BADGE_SIZE),
            ..default()
        },
        Transform::from_translation(Vec3::new(0., CHEST_REROLL_BUTTON_Y, 3.)),
    ));
    btn.insert(RenderLayers::from_layers(&[3]))
        .insert(UIState::ItemChest)
        .insert(HeirloomChestRerollButton)
        .insert(Name::new("Heirloom Chest Reroll"));

    if enabled {
        btn.insert(Interactable::default()).insert(Focusable {
            group: UIState::ItemChest,
            index: 5,
        });
    }

    let btn_e = btn.id();

    commands
        .spawn((
            Sprite {
                image: asset_server.load(MERCHANT_REROLL_ICON_PATH),
                custom_size: Some(MERCHANT_REROLL_ICON_SIZE),
                color,
                ..default()
            },
            Transform::from_translation(Vec3::new(0., 0., 1.)),
        ))
        .insert(RenderLayers::from_layers(&[3]))
        .insert(HeirloomChestRerollIcon)
        .insert(ChildOf(btn_e));

    commands.entity(btn_e).insert(ChildOf(parent));
}

fn spawn_heirloom_chest_reroll_counter(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    parent: Entity,
    rerolls_remaining: u32,
) {
    let bg = commands
        .spawn((
            Sprite {
                image: graphics.get_ui_element_texture(UIElement::CurrencyBackground),
                custom_size: Some(CURRENCY_BACKGROUND_SIZE),
                ..default()
            },
            Transform::from_translation(Vec3::new(
                CHEST_REROLL_COUNTER_POS.x,
                CHEST_REROLL_COUNTER_POS.y,
                2.,
            )),
        ))
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIState::ItemChest)
        .insert(Name::new("Heirloom Chest Rerolls Counter"))
        .id();

    let text = commands
        .spawn((
            gf::DISPLAY
                .text(&asset_server, rerolls_remaining.to_string(), WHITE)
                .justify(Justify::Center)
                .anchor(Anchor::CENTER_LEFT)
                .with_transform(Transform {
                    translation: Vec3::new(-4., 0., 2.),
                    scale: gf::DISPLAY.transform_scale(),
                    ..Default::default()
                }),
            RenderLayers::from_layers(&[3]),
            HeirloomChestRerollsText,
            Name::new("Heirloom Chest Rerolls Text2d"),
        ))
        .id();

    commands
        .spawn((
            Sprite {
                image: asset_server.load(MERCHANT_REROLL_ICON_PATH),
                custom_size: Some(MERCHANT_REROLL_ICON_SIZE),
                ..default()
            },
            Transform::from_translation(Vec3::new(-12., 0., 2.)),
        ))
        .insert(RenderLayers::from_layers(&[3]))
        .insert(ChildOf(text));

    commands.entity(text).insert(ChildOf(bg));
    commands.entity(bg).insert(ChildOf(parent));
}

/// Re-roll the revealed heirloom prize in place (same rarity), refreshing icon + reveal text.
pub fn reroll_heirloom_chest_reward(
    commands: &mut Commands,
    item_chest_state: &mut ItemChestState,
    choices_queue: &HeirloomChoiceQueue,
    graphics: &Graphics,
    asset_server: &AssetServer,
    player_level: u8,
    chest_root: Entity,
    final_items: &Query<Entity, With<ItemChestFinalItem>>,
    reveal_ui: &Query<Entity, With<ChestRevealUI>>,
    tooltip_teardown: &mut MessageWriter<TooltipTeardownEvent>,
    heirloom_tooltip_clear: &mut MessageWriter<HeirloomTooltipRequest>,
) {
    let Some(current) = item_chest_state.picked_heirloom.clone() else {
        return;
    };
    let target_rarity = item_chest_state
        .target_heirloom_rarity
        .clone()
        .unwrap_or(current.rarity.clone());

    let mut rng = rand::thread_rng();
    let exclude = current.heirloom.clone();
    let picked = choices_queue
        .get_skill_of_rarity(target_rarity.clone(), &mut rng, player_level, &|h| {
            h.heirloom != exclude
        })
        .or_else(|| {
            choices_queue.get_skill_of_rarity(target_rarity, &mut rng, player_level, &|_| true)
        });

    let Some(picked) = picked else {
        return;
    };
    if picked.heirloom == Heirloom::None {
        return;
    }

    item_chest_state.picked_heirloom = Some(picked.clone());
    item_chest_state.current_heirloom = Some(picked.heirloom.clone());

    for e in final_items.iter() {
        commands.entity(e).despawn();
    }
    for e in reveal_ui.iter() {
        commands.entity(e).despawn();
    }
    tooltip_teardown.write_default();
    heirloom_tooltip_clear.write(HeirloomTooltipRequest::Clear);

    commands
        .spawn((
            graphics.get_heirloom_icon(picked.heirloom.clone()),
            Transform {
                translation: Vec3::new(-1., CHEST_ICON_Y, 15.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
        ))
        .insert(UIState::ItemChest)
        .insert(RenderLayers::from_layers(&[3]))
        .insert(ItemChestFinalItem)
        .insert(ItemChestFinalHeirloom {
            heirloom: picked.clone(),
        })
        .insert(Interactable::default())
        .insert(Name::new("Chest Final Heirloom"));

    spawn_chest_reveal_text(
        commands,
        asset_server,
        chest_root,
        picked.heirloom.get_title(),
        heirloom_rarity_to_item_rarity(&picked.rarity),
        "Heirloom",
    );
}

/// Return the item that would be displaced if `picked` were equipped right now, or `None`
/// if there is at least one empty valid slot. Mirrors [`equip_item_chest_reward`]'s slot
/// resolution so the UI preview matches the action's behavior 1:1.
pub(crate) fn displaced_equipped_item_stack(
    inventory: &Inventory,
    picked: &ItemStack,
    proto: &ProtoParam,
) -> Option<ItemStack> {
    let eq_type = proto
        .get_component::<EquipmentType, _>(picked.obj_type)
        .cloned()?;
    let eq_type = match eq_type {
        EquipmentType::None => {
            if picked.obj_type.is_weapon() {
                EquipmentType::Weapon
            } else {
                return None;
            }
        }
        other => other,
    };
    let slot_type = eq_type.get_valid_slot_type();
    let valid_slots = eq_type.get_valid_slots();
    if valid_slots.is_empty() {
        return None;
    }
    let container = inventory.get_items_from_slot_type(slot_type);
    let has_empty = valid_slots
        .iter()
        .any(|i| container.items.get(*i).map_or(false, |s| s.is_none()));
    if has_empty {
        return None;
    }
    container
        .items
        .get(valid_slots[0])
        .and_then(|s| s.as_ref())
        .map(|s| s.item_stack.clone())
}

pub fn handle_anim_events(
    mut commands: Commands,
    mut events: MessageReader<ItemChestAnimChangeEvent>,
    mut item_chest_state: ResMut<ItemChestState>,
    query: Query<Entity, With<ItemChest>>,
    chest_ui_root: Query<Entity, With<ItemChestUI>>,
    open_buttons: Query<(Entity, &ItemChestButton)>,
    graphics: Res<Graphics>,
    proto: ProtoParam,
    game: GameParam,
    asset_server: Res<AssetServer>,
    player_atts: Query<&crate::attributes::LootRateBonus, With<crate::player::Player>>,
    choices_queue: Res<HeirloomChoiceQueue>,
    run_unlocks: Res<RunUnlockState>,
    time_crystals: Res<TimeCrystals>,
) {
    for event in events.read() {
        match event.state {
            ItemChestAnimState::Opening => {
                // Pick content based on chest type (only if not already picked)
                match item_chest_state.chest_type {
                    ChestType::Item => {
                        if item_chest_state.picked_item.is_none() {
                            let mut rng = rand::thread_rng();
                            let filtered_items = WorldObject::iter()
                                .filter(|obj| {
                                    (obj.is_weapon() || obj.is_armor() || obj.is_accessory())
                                        && obj != &WorldObject::PlasmaStaff
                                })
                                .collect_vec();
                            // Weight armor 2x so the pool isn't dominated by weapons
                            let pick_new_item = filtered_items
                                .choose_weighted(&mut rng, |obj| {
                                    if obj.is_armor() || obj.is_accessory() {
                                        3
                                    } else {
                                        1
                                    }
                                })
                                .expect("No items found");
                            let mut stack =
                                proto.get_item_data(pick_new_item.clone()).unwrap().clone();
                            let max_item_level = ((game.get_player_level() as i32 / 2) - 5)
                                .clamp(1, 5) as u8
                                + (game.get_player_level() as i32 / 10).clamp(0, 10) as u8;
                            let level = rng.gen_range(1..=max_item_level);
                            stack.metadata.level = Some(level);

                            let loot_bonus = player_atts.single().map(|a| a.0).unwrap_or(0);
                            item_chest_state.picked_item =
                                Some(create_new_random_item_stack_with_attributes(
                                    &stack,
                                    &proto,
                                    &mut commands,
                                    loot_bonus,
                                    true,
                                ));
                        }
                    }
                    ChestType::Heirloom => {
                        if item_chest_state.picked_heirloom.is_none() {
                            let mut rng = rand::thread_rng();
                            let loot_bonus = player_atts.single().map(|a| a.0).unwrap_or(0);

                            // Generate rarity first (same as heirloom shrine)
                            let target_rarity = if item_chest_state.target_heirloom_rarity.is_none()
                            {
                                let rarity = HeirloomChoiceQueue::gen_rarity(&mut rng, loot_bonus);
                                item_chest_state.target_heirloom_rarity = Some(rarity.clone());
                                rarity
                            } else {
                                item_chest_state
                                    .target_heirloom_rarity
                                    .as_ref()
                                    .unwrap()
                                    .clone()
                            };

                            // Pick a heirloom of the target rarity
                            if let Some(picked_heirloom) = choices_queue.get_skill_of_rarity(
                                target_rarity,
                                &mut rng,
                                game.get_player_level(),
                                &|_| true, // No additional filter needed
                            ) {
                                item_chest_state.picked_heirloom = Some(picked_heirloom);
                            } else {
                                // Fallback: if no heirloom of target rarity exists, pick any available (excluding None)
                                let available_heirlooms: Vec<&HeirloomChoiceState> = choices_queue
                                    .pool
                                    .iter()
                                    .filter(|c| c.heirloom != Heirloom::None)
                                    .collect_vec();
                                if let Some(pick_new_heirloom) =
                                    available_heirlooms.choose(&mut rng)
                                {
                                    item_chest_state.picked_heirloom =
                                        Some((*pick_new_heirloom).clone());
                                }
                            }
                        }
                    }
                }
                // Handle opening animation (same for both chest types)
                item_chest_state.state = ItemChestAnimState::Opening;
                for entity in query.iter() {
                    commands.entity(entity).despawn();
                }
                let rarity = event.set_ui_rarity.clone().unwrap_or(ItemRarity::Common);
                spawn_rarity_animation(
                    rarity.clone(),
                    &mut commands,
                    &asset_server,
                    Vec3::new(-1., CHEST_ICON_Y, 16.),
                );

                let ui_element = match item_chest_state.chest_type {
                    ChestType::Item => match rarity {
                        ItemRarity::Common => UIElement::ItemChestOpeningCommon,
                        ItemRarity::Uncommon => UIElement::ItemChestOpeningUncommon,
                        ItemRarity::Rare => UIElement::ItemChestOpeningRare,
                        ItemRarity::Legendary => UIElement::ItemChestOpeningLegendary,
                    },
                    ChestType::Heirloom => match rarity {
                        ItemRarity::Common => UIElement::HeirloomChestOpeningCommon,
                        ItemRarity::Uncommon => UIElement::HeirloomChestOpeningUncommon,
                        ItemRarity::Rare => UIElement::HeirloomChestOpeningRare,
                        ItemRarity::Legendary => UIElement::HeirloomChestOpeningLegendary,
                    },
                };
                commands
                    .spawn((
                        Sprite {
                            image: graphics.get_ui_element_texture(ui_element.clone()),
                            custom_size: Some(Vec2::splat(40.)),
                            ..default()
                        },
                        Transform {
                            translation: Vec3::new(0., CHEST_OPENING_Y + 9., 14.),
                            scale: Vec3::new(1., 1., 1.),
                            ..Default::default()
                        },
                    ))
                    .insert(ui_element)
                    .insert(ItemChest)
                    .insert(UIState::ItemChest)
                    .insert(BounceOnHit::new())
                    .insert(Name::new("ITEM CHEST"))
                    .insert(RenderLayers::from_layers(&[3]));
            }
            ItemChestAnimState::Done => {
                item_chest_state.state = ItemChestAnimState::Done;

                if let Some(current_entity) = item_chest_state.current_entity {
                    commands.entity(current_entity).despawn();
                    item_chest_state.current_entity = None;
                }

                // Swap the OPEN button for the action buttons (Take/Equip or Take/Banish).
                for (btn_e, btn) in open_buttons.iter() {
                    if btn.kind == ChestButtonKind::Open {
                        commands.entity(btn_e).despawn();
                    }
                }
                if let Ok(root) = chest_ui_root.single() {
                    let (left_kind, right_kind) = match item_chest_state.chest_type {
                        ChestType::Item => (ChestButtonKind::Take, ChestButtonKind::Equip),
                        ChestType::Heirloom => (ChestButtonKind::Take, ChestButtonKind::Banish),
                    };
                    spawn_chest_button(
                        &mut commands,
                        &asset_server,
                        &graphics,
                        root,
                        left_kind,
                        Vec3::new(-CHEST_BUTTON_X_OFFSET, CHEST_BUTTON_Y, 1.),
                        true,
                        UIState::ItemChest,
                    );
                    let right_enabled = match right_kind {
                        ChestButtonKind::Banish => {
                            let picked = item_chest_state.picked_heirloom.as_ref();
                            run_unlocks.banishes_remaining > 0
                                && picked
                                    .map(|h| {
                                        choices_queue
                                            .banish_allowed_for_heirloom(time_crystals.as_ref(), h)
                                    })
                                    .unwrap_or(false)
                        }
                        _ => true,
                    };
                    spawn_chest_button(
                        &mut commands,
                        &asset_server,
                        &graphics,
                        root,
                        right_kind,
                        Vec3::new(CHEST_BUTTON_X_OFFSET, CHEST_BUTTON_Y, 1.),
                        right_enabled,
                        UIState::ItemChest,
                    );

                    if matches!(item_chest_state.chest_type, ChestType::Heirloom) {
                        commands
                            .spawn((
                                gf::SKILL_CHOICE_MICRO
                                    .text(
                                        &asset_server,
                                        format!("{}", run_unlocks.banishes_remaining),
                                        WHITE,
                                    )
                                    .justify(Justify::Center)
                                    .anchor(Anchor::CENTER)
                                    .with_transform(Transform {
                                        translation: Vec3::new(
                                            CHEST_BUTTON_X_OFFSET,
                                            CHEST_BANISH_COUNT_Y,
                                            11.,
                                        ),
                                        scale: gf::SKILL_CHOICE_MICRO.transform_scale(),
                                        ..Default::default()
                                    }),
                                RenderLayers::from_layers(&[3]),
                                UIState::ItemChest,
                                ChestBanishCountText,
                                Name::new("CHEST BANISH COUNT TEXT"),
                            ))
                            .insert(ChildOf(root));

                        let reroll_enabled = run_unlocks.rerolls_remaining > 0;
                        spawn_heirloom_chest_reroll_button(
                            &mut commands,
                            &asset_server,
                            root,
                            reroll_enabled,
                        );
                        spawn_heirloom_chest_reroll_counter(
                            &mut commands,
                            &graphics,
                            &asset_server,
                            root,
                            run_unlocks.rerolls_remaining,
                        );
                    }
                }

                // Spawn final item icon based on chest type
                match item_chest_state.chest_type {
                    ChestType::Item => {
                        let picked_item = item_chest_state.picked_item.clone().unwrap();
                        commands
                            .spawn((
                                Sprite {
                                    color: Color::NONE,
                                    custom_size: Some(Vec2::new(32., 32.)),
                                    ..default()
                                },
                                Transform {
                                    translation: Vec3::new(-1., CHEST_ICON_Y, 15.),
                                    scale: Vec3::new(1., 1., 1.),
                                    ..Default::default()
                                },
                            ))
                            .insert(
                                (graphics
                                        .spritesheet_map
                                        .as_ref()
                                        .unwrap()
                                        .get(&picked_item.obj_type)
                                        .unwrap()
                                        .clone()),
                            )
                            .insert(UIState::ItemChest)
                            .insert(RenderLayers::from_layers(&[3]))
                            .insert(ItemChestFinalItem)
                            .insert(Interactable::default())
                            .insert(picked_item.clone())
                            .insert(Name::new("Chest Final Item"));

                        // Reveal text stack (title / rarity / type) — only for item chests
                        // since heirloom rewards use their own hover-tooltip card.
                        // The currently-equipped side tooltip is spawned on hover (see
                        // `handle_item_chest_final_item_hover`) rather than here.
                        if let Ok(root) = chest_ui_root.single() {
                            let type_label = proto
                                .get_component::<EquipmentType, _>(picked_item.obj_type)
                                .map(equipment_type_display_name)
                                .unwrap_or("");
                            spawn_chest_reveal_text(
                                &mut commands,
                                &asset_server,
                                root,
                                picked_item.metadata.name.clone(),
                                picked_item.rarity.clone(),
                                type_label,
                            );
                        }
                    }
                    ChestType::Heirloom => {
                        let picked_heirloom = item_chest_state.picked_heirloom.clone().unwrap();
                        // Defensive: Heirloom::None has no icon and must not be shown (e.g. from contaminated pool)
                        if picked_heirloom.heirloom != Heirloom::None {
                            commands
                                .spawn((
                                    Sprite {
                                        color: Color::NONE,
                                        custom_size: Some(Vec2::new(32., 32.)),
                                        ..default()
                                    },
                                    Transform {
                                        translation: Vec3::new(-1., CHEST_ICON_Y, 15.),
                                        scale: Vec3::new(1., 1., 1.),
                                        ..Default::default()
                                    },
                                ))
                                .insert(graphics.get_heirloom_icon(picked_heirloom.heirloom.clone()))
                                .insert(UIState::ItemChest)
                                .insert(RenderLayers::from_layers(&[3]))
                                .insert(ItemChestFinalItem)
                                .insert(ItemChestFinalHeirloom {
                                    heirloom: picked_heirloom.clone(),
                                })
                                .insert(Interactable::default())
                                .insert(Name::new("Chest Final Heirloom"));

                            // Reveal text stack — same layout as item chests; "Heirloom"
                            // stands in for the equipment-type label.
                            if let Ok(root) = chest_ui_root.single() {
                                spawn_chest_reveal_text(
                                    &mut commands,
                                    &asset_server,
                                    root,
                                    picked_heirloom.heirloom.get_title(),
                                    heirloom_rarity_to_item_rarity(&picked_heirloom.rarity),
                                    "Heirloom",
                                );
                            }
                        }
                    }
                }
            }
            ItemChestAnimState::Closed => {
                // handle closed state
            }
        }
    }
}
/// Handle hovering on the final item in the item chest to show tooltip. On hover-enter we
/// send two `ToolTipUpdateEvent`s: the primary card (picked item, default ItemChest
/// position on the left of the chest container) and — when the player already has every
/// valid slot for this equipment type filled — a *secondary* card on the right showing
/// what would be displaced, labeled "Currently Equipped".
pub fn handle_item_chest_final_item_hover(
    cursor_pos: Res<CursorPos>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut final_items: Query<
        (Entity, &mut Interactable, &ItemStack),
        (With<ItemChestFinalItem>, Without<ItemChestFinalHeirloom>),
    >,
    mut tooltip_update_events: MessageWriter<ToolTipUpdateEvent>,
    mut tooltip_teardown_events: MessageWriter<TooltipTeardownEvent>,
    player_inv: Query<&Inventory, With<crate::player::Player>>,
    proto: ProtoParam,
) {
    use super::CHEST_INVENTORY_UI_SIZE;

    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None, None);
    for (e, mut interactable, item_stack) in final_items.iter_mut() {
        match hit_test {
            Some(hit_ent) if hit_ent.0 == e => {
                match interactable.current() {
                    Interaction::None => {
                        interactable.change(Interaction::Hovering);
                        // Primary tooltip — picked item, uses default ItemChest position
                        // (left side of the chest container) as resolved by
                        // `handle_spawn_inv_item_tooltip`.
                        tooltip_update_events.write(ToolTipUpdateEvent {
                            item_stack: item_stack.clone(),
                            is_recipe: false,
                            show_range: false,
                            ..Default::default()
                        });

                        // Secondary "Currently Equipped" tooltip — only when equipping
                        // would displace an existing piece. Same spacing as the primary
                        // card but mirrored to the right of the chest container.
                        if let Ok(inv) = player_inv.single() {
                            if let Some(displaced) =
                                displaced_equipped_item_stack(inv, item_stack, &proto)
                            {
                                tooltip_update_events.write(ToolTipUpdateEvent {
                                    item_stack: displaced,
                                    is_recipe: false,
                                    show_range: false,
                                    anchor_ui: None,
                                    info_boxes: vec![],
                                    position_override: Some(Vec2::new(
                                        CHEST_INVENTORY_UI_SIZE.x + 20.,
                                        -CHEST_INVENTORY_UI_SIZE.y / 2. + 40.,
                                    )),
                                    header_text: Some("Currently Equipped".to_string()),
                                    world_anchor: None,
                                    ui_state_tag: None,
                                    pin_right: false,
                                    pin_center: false,
                                });
                            }
                        }
                    }
                    Interaction::Hovering => {}
                    _ => {}
                }
            }
            _ => {
                if matches!(interactable.current(), Interaction::Hovering) {
                    interactable.change(Interaction::None);
                    tooltip_teardown_events.write_default();
                }
            }
        }
    }
}

/// Handle hovering on the final heirloom in the heirloom chest to show tooltip
pub fn handle_heirloom_chest_final_item_hover(
    mut tooltip_requests: MessageWriter<HeirloomTooltipRequest>,
    cursor_pos: Res<CursorPos>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut final_heirlooms: Query<
        (
            Entity,
            &GlobalTransform,
            &mut Interactable,
            &ItemChestFinalHeirloom,
        ),
        With<ItemChestFinalItem>,
    >,
) {
    use super::interactions::Interaction;

    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None, None);

    for (e, transform, mut interactable, heirloom_data) in final_heirlooms.iter_mut() {
        match hit_test {
            Some(hit_ent) if hit_ent.0 == e => match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);

                    let icon_pos = transform.translation();
                    let tooltip_pos = Vec3::new(icon_pos.x - 140., icon_pos.y + 12., 15.);

                    tooltip_requests.write(HeirloomTooltipRequest::Show(HeirloomTooltipShow {
                        heirloom: heirloom_data.heirloom.heirloom.clone(),
                        rarity: heirloom_data.heirloom.rarity,
                        position: tooltip_pos,
                        scaling_text: None,
                        trigger_count: 0,
                        ui_state: Some(super::UIState::ItemChest),
                    }));
                }
                Interaction::Hovering => {}
                _ => {}
            },
            _ => {
                if matches!(interactable.current(), Interaction::Hovering) {
                    interactable.change(Interaction::None);
                    tooltip_requests.write(HeirloomTooltipRequest::Clear);
                }
            }
        }
    }
}

#[derive(SystemParam)]
pub struct HeirloomChestRerollParams<'w, 's> {
    pub item_chest_state: ResMut<'w, ItemChestState>,
    pub run_unlocks: ResMut<'w, RunUnlockState>,
    pub choices_queue: Res<'w, HeirloomChoiceQueue>,
    pub graphics: Res<'w, Graphics>,
    pub asset_server: Res<'w, AssetServer>,
    pub chest_ui_root: Query<'w, 's, Entity, With<ItemChestUI>>,
    pub final_items: Query<'w, 's, Entity, With<ItemChestFinalItem>>,
    pub reveal_ui: Query<'w, 's, Entity, With<ChestRevealUI>>,
    pub player_level:
        Query<'w, 's, &'static crate::player::levels::PlayerLevel, With<crate::player::Player>>,
    pub tooltip_teardown: MessageWriter<'w, TooltipTeardownEvent>,
    pub heirloom_tooltip_clear: MessageWriter<'w, HeirloomTooltipRequest>,
    pub focus_input: crate::ui::focus::FocusInput<'w>,
}

pub fn handle_heirloom_chest_reroll_button(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    mut sprites: ParamSet<(
        Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
        Query<&mut Sprite, With<HeirloomChestRerollIcon>>,
    )>,
    mut reroll_buttons: Query<
        (Entity, &mut Interactable, &Children),
        With<HeirloomChestRerollButton>,
    >,
    mut reroll_text: Query<&mut Text2d, With<HeirloomChestRerollsText>>,
    mut commands: Commands,
    mut params: HeirloomChestRerollParams,
) {
    if params.item_chest_state.chest_type != ChestType::Heirloom
        || params.item_chest_state.state != ItemChestAnimState::Done
    {
        return;
    }

    let hit_entity = {
        let ui_sprites = sprites.p0();
        ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None, None).map(|(e, _, _)| e)
    };
    let left_pressed = mouse_input.just_pressed(MouseButton::Left);

    for (e, mut interactable, children) in reroll_buttons.iter_mut() {
        let enabled = params.run_unlocks.rerolls_remaining > 0;
        let is_hit = hit_entity == Some(e);
        let is_focused = params.focus_input.is_focused(e);
        let confirm_pressed =
            (is_hit && left_pressed) || (is_focused && params.focus_input.confirm_just_pressed());

        if is_hit || is_focused {
            match interactable.current() {
                Interaction::None if enabled => {
                    interactable.change(Interaction::Hovering);
                    for child in children.iter() {
                        if let Ok(mut sprite) = sprites.p1().get_mut(child) {
                            sprite.color = YELLOW;
                        }
                    }
                }
                Interaction::Hovering => {
                    if confirm_pressed && enabled {
                        let Ok(root) = params.chest_ui_root.single() else {
                            return;
                        };
                        let level = params.player_level.single().map(|l| l.level).unwrap_or(1);

                        params.run_unlocks.rerolls_remaining =
                            params.run_unlocks.rerolls_remaining.saturating_sub(1);
                        commands.spawn(crate::audio::SoundSpawner::new(
                            crate::audio::AudioSoundEffect::UISkillReRoll,
                            0.4,
                        ));

                        reroll_heirloom_chest_reward(
                            &mut commands,
                            &mut params.item_chest_state,
                            &params.choices_queue,
                            &params.graphics,
                            &params.asset_server,
                            level,
                            root,
                            &params.final_items,
                            &params.reveal_ui,
                            &mut params.tooltip_teardown,
                            &mut params.heirloom_tooltip_clear,
                        );

                        for mut text in reroll_text.iter_mut() {
                            text.0 = params.run_unlocks.rerolls_remaining.to_string();
                        }

                        interactable.change(Interaction::None);
                        let icon_color = if params.run_unlocks.rerolls_remaining > 0 {
                            Color::WHITE
                        } else {
                            Color::srgb(0.45, 0.45, 0.45)
                        };
                        for child in children.iter() {
                            if let Ok(mut sprite) = sprites.p1().get_mut(child) {
                                sprite.color = icon_color;
                            }
                        }
                        if params.run_unlocks.rerolls_remaining == 0 {
                            commands
                                .entity(e)
                                .remove::<Interactable>()
                                .remove::<Focusable>();
                        }
                    }
                }
                _ => {}
            }
        } else if matches!(interactable.current(), Interaction::Hovering) {
            interactable.change(Interaction::None);
            let icon_color = if enabled {
                Color::WHITE
            } else {
                Color::srgb(0.45, 0.45, 0.45)
            };
            for child in children.iter() {
                if let Ok(mut sprite) = sprites.p1().get_mut(child) {
                    sprite.color = icon_color;
                }
            }
        }
    }
}
