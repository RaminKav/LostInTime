use bevy::{prelude::*, render::view::RenderLayers};
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
    cursor::CursorPos,
    inventory::ItemStack,
    item::WorldObject,
    juice::bounce::BounceOnHit,
    player::skills::{Heirloom, HeirloomChoiceQueue, HeirloomChoiceState, HeirloomRarity},
    proto::proto_param::ProtoParam,
    GameParam, ScreenResolution, GAME_HEIGHT,
};

use super::{
    interactions::Interaction, ui_helpers, ui_helpers::spawn_ui_overlay, Interactable,
    ToolTipUpdateEvent, TooltipTeardownEvent, UIElement, UIState, SKILLS_CHOICE_UI_SIZE,
};

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

#[derive(Component)]
pub struct ItemChestButton;

pub struct ItemChestAnimChangeEvent {
    pub state: ItemChestAnimState,
    pub set_ui_rarity: Option<ItemRarity>,
}

pub fn setup_item_chest_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
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

    spawn_ui_overlay(
        &mut commands,
        Vec2::new(res.game_width + 10., GAME_HEIGHT + 100.),
        0.8,
        9.,
    );

    let ui_element = UIElement::SkillChoice;
    let size = SKILLS_CHOICE_UI_SIZE;

    let item_chest_ui = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(ui_element.clone()),
            sprite: Sprite {
                custom_size: Some(size),
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

    // icon
    commands
        .spawn(SpriteSheetBundle {
            sprite: graphics
                .spritesheet_map
                .as_ref()
                .unwrap()
                .get(match item_chest_state.chest_type {
                    ChestType::Item => &WorldObject::Chest,
                    ChestType::Heirloom => &WorldObject::HeirloomChest,
                })
                .unwrap()
                .clone(),
            texture_atlas: graphics.texture_atlas.as_ref().unwrap().clone(),

            transform: Transform {
                translation: Vec2::new(0., 18.).extend(4.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(ItemChest)
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("SKILL ICON!!"))
        .set_parent(item_chest_ui);

    let upgrade_button = commands
        .spawn(SpriteBundle {
            texture: graphics
                .get_ui_element_texture(UIElement::UpgradeButton)
                .clone(),
            sprite: Sprite {
                custom_size: Some(Vec2::new(13., 13.)),
                ..Default::default()
            },
            transform: Transform {
                translation: Vec3::new(0.5, -36.5, 10.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIElement::UpgradeButton)
        .insert(Interactable::default())
        .insert(ItemChestButton)
        .insert(UIState::Inventory)
        .insert(Name::new("UPGRADE BUTTON"))
        .id();
    commands
        .entity(item_chest_ui)
        .push_children(&[upgrade_button]);
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
                            translation: Vec3::new(0., 25., 15.),
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
                            translation: Vec3::new(0., 25., 15.),
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

pub fn handle_anim_events(
    mut commands: Commands,
    mut events: EventReader<ItemChestAnimChangeEvent>,
    mut item_chest_state: ResMut<ItemChestState>,
    query: Query<Entity, With<ItemChest>>,
    graphics: Res<Graphics>,
    proto: ProtoParam,
    game: GameParam,
    asset_server: Res<AssetServer>,
    player_atts: Query<&crate::attributes::LootRateBonus, With<crate::player::Player>>,
    choices_queue: Res<HeirloomChoiceQueue>,
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
                            let pick_new_item =
                                filtered_items.choose(&mut rng).expect("No items found");
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
                    Vec3::new(0., 25., 16.),
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
                            custom_size: Some(Vec2::splat(32.)),
                            ..Default::default()
                        },
                        transform: Transform {
                            translation: Vec3::new(0., 26., 14.),
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
                                    translation: Vec3::new(0., 25., 15.),
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
                            .insert(picked_item)
                            .insert(Name::new("Chest Final Item"));
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
                                        translation: Vec3::new(0., 25., 15.),
                                        scale: Vec3::new(1., 1., 1.),
                                        ..Default::default()
                                    },
                                    ..Default::default()
                                })
                                .insert(graphics.get_heirloom_icon(picked_heirloom.heirloom.clone()))
                                .insert(graphics.texture_atlas.as_ref().unwrap().clone())
                                .insert(UIState::ItemChest)
                                .insert(RenderLayers::from_layers(&[3]))
                                .insert(ItemChestFinalItem)
                                .insert(ItemChestFinalHeirloom {
                                    heirloom: picked_heirloom.clone(),
                                })
                                .insert(Interactable::default())
                                .insert(Name::new("Chest Final Heirloom"));
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
/// Handle hovering on the final item in the item chest to show tooltip
pub fn handle_item_chest_final_item_hover(
    cursor_pos: Res<CursorPos>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut final_items: Query<
        (Entity, &mut Interactable, &ItemStack),
        (With<ItemChestFinalItem>, Without<ItemChestFinalHeirloom>),
    >,
    mut tooltip_update_events: EventWriter<ToolTipUpdateEvent>,
    mut tooltip_teardown_events: EventWriter<TooltipTeardownEvent>,
) {
    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    for (e, mut interactable, item_stack) in final_items.iter_mut() {
        match hit_test {
            Some(hit_ent) if hit_ent.0 == e => {
                match interactable.current() {
                    Interaction::None => {
                        interactable.change(Interaction::Hovering);
                        // Send tooltip event
                        tooltip_update_events.send(ToolTipUpdateEvent {
                            item_stack: item_stack.clone(),
                            is_recipe: false,
                            show_range: false,
                        });
                    }
                    Interaction::Hovering => {
                        // Already hovering, do nothing
                    }
                    _ => {}
                }
            }
            _ => {
                // Not hovering over this item
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
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
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
    existing_tooltips: Query<Entity, With<HeirloomChestTooltip>>,
) {
    use super::interactions::Interaction;
    use super::skill_choice_ui::spawn_heirloom_tooltip_card;

    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);

    for (e, transform, mut interactable, heirloom_data) in final_heirlooms.iter_mut() {
        match hit_test {
            Some(hit_ent) if hit_ent.0 == e => match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);

                    for tooltip_e in existing_tooltips.iter() {
                        commands.entity(tooltip_e).despawn_recursive();
                    }

                    let icon_pos = transform.translation();
                    let tooltip_pos = Vec3::new(icon_pos.x - 98., icon_pos.y - 25., 15.);

                    let tooltip_e = spawn_heirloom_tooltip_card(
                        &graphics,
                        &mut commands,
                        &asset_server,
                        heirloom_data.heirloom.heirloom.clone(),
                        heirloom_data.heirloom.rarity.clone(),
                        tooltip_pos,
                        None,
                        None,
                    );

                    commands
                        .entity(tooltip_e)
                        .insert(HeirloomChestTooltip)
                        .insert(RenderLayers::from_layers(&[3]));
                }
                Interaction::Hovering => {}
                _ => {}
            },
            _ => {
                if matches!(interactable.current(), Interaction::Hovering) {
                    interactable.change(Interaction::None);
                    for tooltip_e in existing_tooltips.iter() {
                        commands.entity(tooltip_e).despawn_recursive();
                    }
                }
            }
        }
    }
}

#[derive(Component)]
pub struct HeirloomChestTooltip;
