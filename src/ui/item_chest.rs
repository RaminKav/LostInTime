use bevy::{prelude::*, render::view::RenderLayers, sprite::Anchor};
use bevy_aseprite::aseprite;
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
    player::skills::{Heirloom, HeirloomChoiceQueue, HeirloomChoiceState, HeirloomRarity},
    proto::proto_param::ProtoParam,
    GameParam, ScreenResolution,
};

use super::{
    game_fonts as gf,
    heirloom_tooltip::{HeirloomTooltipRequest, HeirloomTooltipShow},
    interactions::Interaction,
    ui_helpers, Interactable, ToolTipUpdateEvent, TooltipTeardownEvent, UIElement, UIState,
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
// Reveal text stack (top → bottom). Sits above the chest icon once it shifts down to
// `CHEST_ICON_Y` after opening — the chest UI lives in render-layer 3 world space.
const CHEST_REVEAL_TITLE_Y: f32 = 48.;
const CHEST_REVEAL_RARITY_Y: f32 = 26.;
const CHEST_REVEAL_TYPE_Y: f32 = 14.;

aseprite!(pub SkillChoiceFlash, "ui/SkillChoiceFlash.aseprite");

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
) -> Entity {
    let label = match kind {
        ChestButtonKind::Open => "OPEN",
        ChestButtonKind::Take => "TAKE",
        ChestButtonKind::Equip => "EQUIP",
        ChestButtonKind::Banish => "BANISH",
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
        Color::rgb(0.5, 0.5, 0.5)
    };

    let mut button = commands.spawn(SpriteBundle {
        texture: graphics.get_ui_element_texture(ui_element.clone()),
        sprite: Sprite {
            custom_size: Some(CHEST_BUTTON_SIZE),
            color,
            ..Default::default()
        },
        transform: Transform::from_translation(translation),
        ..Default::default()
    });
    button
        .insert(ui_element)
        .insert(UIState::ItemChest)
        .insert(ItemChestButton { kind })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new(format!("ITEM CHEST BUTTON {label}")));
    if enabled {
        button.insert(Interactable::default());
    }
    let button_entity = button.id();

    let text_color = if enabled {
        WHITE
    } else {
        Color::rgb(0.7, 0.7, 0.7)
    };
    let pos = if kind == ChestButtonKind::Equip {
        Vec3::new(1., 0., 1.)
    } else {
        Vec3::new(0., 0., 1.)
    };
    commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    label,
                    gf::SKILL_CHOICE_MICRO.text_style(asset_server, text_color),
                )
                .with_alignment(TextAlignment::Center),
                text_anchor: Anchor::Center,
                transform: Transform::from_translation(pos),
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            UIState::ItemChest,
            ChestButtonLabel,
            Name::new(format!("ITEM CHEST BUTTON LABEL {label}")),
        ))
        .set_parent(button_entity);

    commands.entity(parent).add_child(button_entity);
    button_entity
}

/// Tints the child label of any hovered chest button yellow, restoring white when the
/// pointer leaves. Disabled buttons have no `Interactable` so they keep their dim color.
pub fn update_chest_button_label_hover(
    buttons: Query<(&Interactable, &Children), With<ItemChestButton>>,
    mut labels: Query<&mut Text, With<ChestButtonLabel>>,
) {
    for (interactable, children) in buttons.iter() {
        let hovered = matches!(interactable.current(), Interaction::Hovering);
        let color = if hovered { YELLOW } else { WHITE };
        for child in children.iter() {
            if let Ok(mut text) = labels.get_mut(*child) {
                for section in text.sections.iter_mut() {
                    section.style.color = color;
                }
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
) {
    // // title bar
    // let title_sprite = commands
    //     .spawn(SpriteBundle {
    //         texture: graphics.get_ui_element_texture(UIElement::TitleBar).clone(),
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
    //     Anchor::Center,
    //     2.,
    //     3,
    // );
    // commands
    //     .entity(title_text)
    //     .insert(UIState::Skills)
    //     .set_parent(title_sprite);

    ui_helpers::spawn_full_screen_ui_overlay_tuned(&mut commands, &res, 0.0, 0.95, 9.);

    let ui_element = UIElement::ChestContainer;

    let item_chest_ui = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(ui_element.clone()),
            sprite: Sprite {
                custom_size: Some(CHEST_CONTAINER_UI_SIZE),
                ..Default::default()
            },
            transform: Transform {
                translation: Vec3::new(0., 0., 10.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(ui_element)
        .insert(UIState::ItemChest)
        .insert(ItemChestUI)
        .insert(Name::new("ITEM CHEST"))
        .insert(RenderLayers::from_layers(&[3]))
        .id();

    // closed chest icon (replaced when the open animation starts)
    commands
        .spawn(SpriteSheetBundle {
            sprite: graphics
                .spritesheet_map
                .as_ref()
                .unwrap()
                .get(match item_chest_state.chest_type {
                    ChestType::Item => &WorldObject::ChestBlock,
                    ChestType::Heirloom => &WorldObject::HeirloomChest,
                })
                .unwrap()
                .clone(),
            texture_atlas: graphics.texture_atlas.as_ref().unwrap().clone(),

            transform: Transform {
                translation: Vec2::new(0., CHEST_OPENING_Y).extend(4.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(ItemChest)
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("Chest Icon"))
        .set_parent(item_chest_ui);

    spawn_chest_button(
        &mut commands,
        &asset_server,
        &graphics,
        item_chest_ui,
        ChestButtonKind::Open,
        Vec3::new(0., CHEST_BUTTON_Y, 1.),
        true,
    );
}

pub fn toggle_item_chest_visibility(
    mut next_inv_state: ResMut<NextState<UIState>>,
    curr_ui_state: Res<State<UIState>>,
) {
    if curr_ui_state.0 == UIState::ActiveSkills || curr_ui_state.0 == UIState::ItemChest {
        return;
    }
    next_inv_state.set(UIState::ItemChest);
}

pub fn shuffle_items(
    mut item_chest_state: ResMut<ItemChestState>,
    time: Res<Time>,
    mut commands: Commands,
    graphics: Res<Graphics>,
    mut events: EventWriter<ItemChestAnimChangeEvent>,
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
            events.send(ItemChestAnimChangeEvent {
                state: ItemChestAnimState::Done,
                set_ui_rarity: None,
            });
        }
        return;
    }
    // Use unified rarity getter that works for both chest types
    let picked_rarity = item_chest_state.get_picked_rarity();
    if item_chest_state.shuffle_duration_timer.percent() >= 0.25
        && item_chest_state.current_ui_rarity == ItemRarity::Common
        && picked_rarity != ItemRarity::Common
    {
        item_chest_state.current_ui_rarity = ItemRarity::Uncommon;

        events.send(ItemChestAnimChangeEvent {
            state: ItemChestAnimState::Opening,
            set_ui_rarity: Some(ItemRarity::Uncommon),
        });
    } else if item_chest_state.shuffle_duration_timer.percent() >= 0.48
        && item_chest_state.current_ui_rarity == ItemRarity::Uncommon
        && picked_rarity != ItemRarity::Uncommon
    {
        item_chest_state.current_ui_rarity = ItemRarity::Rare;
        events.send(ItemChestAnimChangeEvent {
            state: ItemChestAnimState::Opening,
            set_ui_rarity: Some(ItemRarity::Rare),
        });
    } else if item_chest_state.shuffle_duration_timer.percent() >= 0.7
        && item_chest_state.current_ui_rarity == ItemRarity::Rare
        && picked_rarity != ItemRarity::Rare
    {
        item_chest_state.current_ui_rarity = ItemRarity::Legendary;
        events.send(ItemChestAnimChangeEvent {
            state: ItemChestAnimState::Opening,
            set_ui_rarity: Some(ItemRarity::Legendary),
        });
    }
    if item_chest_state.shuffle_timer.finished()
        && !item_chest_state.shuffle_duration_timer.finished()
    {
        if let Some(current_entity) = item_chest_state.current_entity {
            if commands.get_entity(current_entity).is_some() {
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
                    .spawn(SpriteSheetBundle {
                        sprite: graphics
                            .spritesheet_map
                            .as_ref()
                            .unwrap()
                            .get(&pick_new_item.clone())
                            .unwrap()
                            .clone(),
                        texture_atlas: graphics.texture_atlas.as_ref().unwrap().clone(),
                        transform: Transform {
                            translation: Vec3::new(-1., CHEST_ICON_Y, 15.),
                            scale: Vec3::new(1., 1., 1.),
                            ..Default::default()
                        },
                        ..Default::default()
                    })
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
                    .spawn(SpriteSheetBundle {
                        sprite: graphics.get_heirloom_icon(picked_heirloom),
                        texture_atlas: graphics.texture_atlas.as_ref().unwrap().clone(),
                        transform: Transform {
                            translation: Vec3::new(-1., CHEST_ICON_Y, 15.),
                            scale: Vec3::new(1., 1., 1.),
                            ..Default::default()
                        },
                        ..Default::default()
                    })
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
    let title_style = gf::TOOLTIP_ITEM_TITLE.text_style(asset_server, rarity.get_color());
    commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(name, title_style).with_alignment(TextAlignment::Center),
                text_anchor: Anchor::Center,
                transform: Transform::from_translation(Vec3::new(0., CHEST_REVEAL_TITLE_Y, 2.)),
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            UIState::ItemChest,
            ChestRevealUI,
            Name::new("CHEST REVEAL TITLE"),
        ))
        .set_parent(parent);

    let rarity_style = gf::TOOLTIP_CARD_LINE.text_style(asset_server, rarity.get_color());
    commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(rarity.get_name(), rarity_style)
                    .with_alignment(TextAlignment::Center),
                text_anchor: Anchor::Center,
                transform: Transform::from_translation(Vec3::new(0., CHEST_REVEAL_RARITY_Y, 2.)),
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            UIState::ItemChest,
            ChestRevealUI,
            Name::new("CHEST REVEAL RARITY"),
        ))
        .set_parent(parent);

    if !type_label.is_empty() {
        let type_style = gf::TOOLTIP_CARD_LINE.text_style(asset_server, WHITE);
        commands
            .spawn((
                Text2dBundle {
                    text: Text::from_section(type_label, type_style)
                        .with_alignment(TextAlignment::Center),
                    text_anchor: Anchor::Center,
                    transform: Transform::from_translation(Vec3::new(0., CHEST_REVEAL_TYPE_Y, 2.)),
                    ..Default::default()
                },
                RenderLayers::from_layers(&[3]),
                UIState::ItemChest,
                ChestRevealUI,
                Name::new("CHEST REVEAL TYPE"),
            ))
            .set_parent(parent);
    }
}

/// Return the item that would be displaced if `picked` were equipped right now, or `None`
/// if there is at least one empty valid slot. Mirrors [`equip_item_chest_reward`]'s slot
/// resolution so the UI preview matches the action's behavior 1:1.
fn displaced_equipped_item_stack(
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
    mut events: EventReader<ItemChestAnimChangeEvent>,
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
    run_unlocks: Res<crate::player::unlocks::RunUnlockState>,
) {
    for event in events.iter() {
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

                            let loot_bonus = player_atts.get_single().map(|a| a.0).unwrap_or(0);
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
                            let loot_bonus = player_atts.get_single().map(|a| a.0).unwrap_or(0);

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
                    commands.entity(entity).despawn_recursive();
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
                    .spawn(SpriteBundle {
                        texture: graphics.get_ui_element_texture(ui_element.clone()),
                        sprite: Sprite {
                            custom_size: Some(Vec2::splat(40.)),
                            ..Default::default()
                        },
                        transform: Transform {
                            translation: Vec3::new(0., CHEST_OPENING_Y + 9., 14.),
                            scale: Vec3::new(1., 1., 1.),
                            ..Default::default()
                        },
                        ..Default::default()
                    })
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
                        commands.entity(btn_e).despawn_recursive();
                    }
                }
                if let Ok(root) = chest_ui_root.get_single() {
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
                    );
                    let right_enabled = match right_kind {
                        ChestButtonKind::Banish => run_unlocks.banishes_remaining > 0,
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
                    );

                    if matches!(item_chest_state.chest_type, ChestType::Heirloom) {
                        commands
                            .spawn((
                                Text2dBundle {
                                    text: Text::from_section(
                                        format!("{}", run_unlocks.banishes_remaining),
                                        gf::SKILL_CHOICE_MICRO.text_style(&asset_server, WHITE),
                                    )
                                    .with_alignment(TextAlignment::Center),
                                    text_anchor: Anchor::Center,
                                    transform: Transform::from_translation(Vec3::new(
                                        CHEST_BUTTON_X_OFFSET,
                                        CHEST_BANISH_COUNT_Y,
                                        11.,
                                    )),
                                    ..Default::default()
                                },
                                RenderLayers::from_layers(&[3]),
                                UIState::ItemChest,
                                ChestBanishCountText,
                                Name::new("CHEST BANISH COUNT TEXT"),
                            ))
                            .set_parent(root);
                    }
                }

                // Spawn final item icon based on chest type
                match item_chest_state.chest_type {
                    ChestType::Item => {
                        let picked_item = item_chest_state.picked_item.clone().unwrap();
                        commands
                            .spawn(SpriteBundle {
                                sprite: Sprite {
                                    color: Color::NONE,
                                    custom_size: Some(Vec2::new(32., 32.)),
                                    ..default()
                                },
                                transform: Transform {
                                    translation: Vec3::new(-1., CHEST_ICON_Y, 15.),
                                    scale: Vec3::new(1., 1., 1.),
                                    ..Default::default()
                                },
                                ..Default::default()
                            })
                            .insert(
                                graphics
                                    .spritesheet_map
                                    .as_ref()
                                    .unwrap()
                                    .get(&picked_item.obj_type)
                                    .unwrap()
                                    .clone(),
                            )
                            .insert(graphics.texture_atlas.as_ref().unwrap().clone())
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
                        if let Ok(root) = chest_ui_root.get_single() {
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
                                .spawn(SpriteBundle {
                                    sprite: Sprite {
                                        color: Color::NONE,
                                        custom_size: Some(Vec2::new(32., 32.)),
                                        ..default()
                                    },
                                    transform: Transform {
                                        translation: Vec3::new(-1., CHEST_ICON_Y, 15.),
                                        scale: Vec3::new(1., 1., 1.),
                                        ..Default::default()
                                    },
                                    ..Default::default()
                                })
                                .insert(
                                    graphics.get_heirloom_icon(picked_heirloom.heirloom.clone()),
                                )
                                .insert(graphics.texture_atlas.as_ref().unwrap().clone())
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
                            if let Ok(root) = chest_ui_root.get_single() {
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
    mut tooltip_update_events: EventWriter<ToolTipUpdateEvent>,
    mut tooltip_teardown_events: EventWriter<TooltipTeardownEvent>,
    player_inv: Query<&Inventory, With<crate::player::Player>>,
    proto: ProtoParam,
) {
    use super::CHEST_INVENTORY_UI_SIZE;

    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    for (e, mut interactable, item_stack) in final_items.iter_mut() {
        match hit_test {
            Some(hit_ent) if hit_ent.0 == e => {
                match interactable.current() {
                    Interaction::None => {
                        interactable.change(Interaction::Hovering);
                        // Primary tooltip — picked item, uses default ItemChest position
                        // (left side of the chest container) as resolved by
                        // `handle_spawn_inv_item_tooltip`.
                        tooltip_update_events.send(ToolTipUpdateEvent {
                            item_stack: item_stack.clone(),
                            is_recipe: false,
                            show_range: false,
                            ..Default::default()
                        });

                        // Secondary "Currently Equipped" tooltip — only when equipping
                        // would displace an existing piece. Same spacing as the primary
                        // card but mirrored to the right of the chest container.
                        if let Ok(inv) = player_inv.get_single() {
                            if let Some(displaced) =
                                displaced_equipped_item_stack(inv, item_stack, &proto)
                            {
                                tooltip_update_events.send(ToolTipUpdateEvent {
                                    item_stack: displaced,
                                    is_recipe: false,
                                    show_range: false,
                                    position_override: Some(Vec2::new(
                                        CHEST_INVENTORY_UI_SIZE.x + 20.,
                                        -CHEST_INVENTORY_UI_SIZE.y / 2. + 40.,
                                    )),
                                    header_text: Some("Currently Equipped".to_string()),
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
                    tooltip_teardown_events.send_default();
                }
            }
        }
    }
}

/// Handle hovering on the final heirloom in the heirloom chest to show tooltip
pub fn handle_heirloom_chest_final_item_hover(
    mut tooltip_requests: EventWriter<HeirloomTooltipRequest>,
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

    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);

    for (e, transform, mut interactable, heirloom_data) in final_heirlooms.iter_mut() {
        match hit_test {
            Some(hit_ent) if hit_ent.0 == e => match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);

                    let icon_pos = transform.translation();
                    let tooltip_pos = Vec3::new(icon_pos.x - 130., icon_pos.y + 12., 15.);

                    tooltip_requests.send(HeirloomTooltipRequest::Show(HeirloomTooltipShow {
                        heirloom: heirloom_data.heirloom.heirloom.clone(),
                        rarity: heirloom_data.heirloom.rarity,
                        position: tooltip_pos,
                        scaling_text: None,
                        trigger_count_text: None,
                        ui_state: Some(super::UIState::ItemChest),
                    }));
                }
                Interaction::Hovering => {}
                _ => {}
            },
            _ => {
                if matches!(interactable.current(), Interaction::Hovering) {
                    interactable.change(Interaction::None);
                    tooltip_requests.send(HeirloomTooltipRequest::Clear);
                }
            }
        }
    }
}
