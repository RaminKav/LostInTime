use std::time::Duration;

use crate::{
    blessings::{Blessing, HeirloomStatsBonuses},
    chaos::ChaosTracker,
    player::Player,
    ui::damage_numbers::spawn_floating_text_with_shadow,
};
use bevy::prelude::*;
use bevy_proto::prelude::ProtoCommands;
use rand::Rng;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumIter};

use crate::{
    assets::Graphics,
    attributes::{
        attribute_helpers::create_new_random_item_stack_with_attributes, AttributeChangeEvent,
    },
    audio::{AudioSoundEffect, SoundSpawner},
    blessings::OwnedBlessings,
    colors::{DARK_GREEN, RED},
    inputs::CursorPos,
    inventory::{Inventory, InventoryItemStack, ItemStack},
    item::{heirloom_shrine::HeirloomShrineState, CraftedItemEvent, EquipmentType},
    pets::state::UpdatePetWeaponEvent,
    player::{
        combat_heirlooms::HallucinationStatType,
        levels::PlayerLevel,
        skills::{Heirloom, HeirloomChoiceQueue, PlayerSkills},
        stats::StatType,
        unlocks::RunUnlockState,
    },
    proto::proto_param::ProtoParam,
    ui::{
        crafting_ui::UpgradeButton,
        inventory_ui::change_hotbar_slot,
        item_chest::{
            ChestType, ItemChestAnimChangeEvent, ItemChestAnimState, ItemChestButton,
            ItemChestState,
        },
        InventoryState,
    },
    GameParam,
};

use super::{
    crafting_ui::CraftingContainer, scrapper_ui::ScrapperContainer, spawn_item_stack_icon,
    spawn_skill_choice_flash, stats_ui::StatsButtonState, ui_helpers, BanishButton, ChestContainer,
    EssenceOption, InfoModal, InventorySlotState, MenuButton, MenuButtonClickEvent, RerollDice,
    ShowInvPlayerStatsEvent, SkillChoiceUI, SubmitEssenceChoice, ToolTipUpdateEvent,
    TooltipTeardownEvent, UIContainersParam, UIState, SKILLS_CHOICE_UI_SIZE,
};

#[derive(
    Component, Debug, EnumIter, Clone, Display, Hash, PartialEq, Eq, Serialize, Deserialize,
)]
pub enum UIElement {
    Inventory,
    ChestInventory,
    InventorySlot,
    InventorySlotHotbar,
    InventorySlotHover,
    XPBarFrame,
    Tooltip,
    StatTooltip,
    LargeTooltipCommon,
    LargeTooltipUncommon,
    LargeTooltipRare,
    LargeTooltipLegendary,
    Minimap,
    LargeMinimap,
    LevelUpStats,
    StatsButton,
    StatsButtonHover,
    PlayerHUDBars,
    EliteStar,
    CraftingInventory,
    FurnaceInventory,
    TileHover,
    BlockedTileHover,
    MenuButton,
    AchievementsButton,
    UnlocksButton,
    OptionsButton,
    MenuButtonHover,
    AchievementsButtonHover,
    UnlocksButtonHover,
    OptionsButtonHover,
    MainMenu,
    Essence,
    EssenceButton,
    EssenceButtonHover,
    HealthDebuff1,
    HealthDebuff2,
    HealthDebuff3,
    HungerDebuff1,
    HungerDebuff2,
    HungerDebuff3,
    ScreenIconSlot,
    ScreenIconSlotLarge,
    Options,
    SkillChoice,
    SkillChoiceHover,
    SkillChoiceMelee,
    SkillChoiceRogue,
    SkillChoiceMagic,
    SkillChoiceMeleeHover,
    SkillChoiceRogueHover,
    SkillChoiceMagicHover,
    StarIcon,
    InfoModal,
    TitleBar,
    SkillClassTracker,
    RerollDice,
    RerollDiceHover,
    UpgradeButton,
    UpgradeButtonHover,
    BackButton,
    BackButtonHover,
    ItemChestOpeningCommon,
    ItemChestOpeningUncommon,
    ItemChestOpeningRare,
    ItemChestOpeningLegendary,
    HeirloomChestOpeningCommon,
    HeirloomChestOpeningUncommon,
    HeirloomChestOpeningRare,
    HeirloomChestOpeningLegendary,
    FairyPetIcon,
    SlimePetIcon,
    GreenPlayerSelectIcon,
    RedPlayerSelectIcon,
    BluePlayerSelectIcon,
    GreyPlayerSelectIcon,
    PlayerSelect,
    PlayerSelectSlot,
    PlayerSelectSlotHover,
    MeleePowerIcon,
    ClassWeaponSlot,
    AttackSpeedIcon,
    CritChanceIcon,
    CritDamageIcon,
    DefenceIcon,
    DodgeChanceIcon,
    HealthIcon,
    ManaIcon,
    HPRegenCooldownIcon,
    SlimeShieldIcon,
    HPRegenIcon,
    MPRegenIcon,
    SpeedIcon,
    UnknownUnlockIcon,
    Achievements,
    CheckBox,
    CheckBoxSelected,
    AchievementCrossOut,
    SmallKey,
    MediumKey,
    HeirloomHudIcon,
    LargeKey,
    XLKey,
    XLKeyHover,
    MinimapDungeonIcon,
    MinimapStarIcon,
    MinimapSkullIcon,
    MinimapPortalIcon,
}
impl UIElement {
    pub fn get_hover_state(&self) -> Option<UIElement> {
        match self {
            UIElement::InventorySlot => Some(UIElement::InventorySlotHover),
            UIElement::StatsButton => Some(UIElement::StatsButtonHover),
            UIElement::EssenceButton => Some(UIElement::EssenceButtonHover),
            UIElement::SkillChoiceMelee => Some(UIElement::SkillChoiceMeleeHover),
            UIElement::SkillChoiceRogue => Some(UIElement::SkillChoiceRogueHover),
            UIElement::SkillChoiceMagic => Some(UIElement::SkillChoiceMagicHover),
            UIElement::RerollDice => Some(UIElement::RerollDiceHover),
            UIElement::UpgradeButton => Some(UIElement::UpgradeButtonHover),
            UIElement::BackButton => Some(UIElement::BackButtonHover),
            UIElement::XLKey => Some(UIElement::XLKeyHover),
            UIElement::MenuButton => Some(UIElement::MenuButtonHover),
            UIElement::AchievementsButton => Some(UIElement::AchievementsButtonHover),
            UIElement::UnlocksButton => Some(UIElement::UnlocksButtonHover),
            UIElement::OptionsButton => Some(UIElement::OptionsButtonHover),
            _ => None,
        }
    }
    pub fn get_normal_state(&self) -> Option<UIElement> {
        match self {
            UIElement::InventorySlotHover => Some(UIElement::InventorySlot),
            UIElement::StatsButtonHover => Some(UIElement::StatsButton),
            UIElement::EssenceButtonHover => Some(UIElement::EssenceButton),
            UIElement::SkillChoiceMeleeHover => Some(UIElement::SkillChoiceMelee),
            UIElement::SkillChoiceRogueHover => Some(UIElement::SkillChoiceRogue),
            UIElement::SkillChoiceMagicHover => Some(UIElement::SkillChoiceMagic),
            UIElement::RerollDiceHover => Some(UIElement::RerollDice),
            UIElement::UpgradeButtonHover => Some(UIElement::UpgradeButton),
            UIElement::BackButtonHover => Some(UIElement::BackButton),
            UIElement::XLKeyHover => Some(UIElement::XLKey),
            UIElement::MenuButtonHover => Some(UIElement::MenuButton),
            UIElement::AchievementsButtonHover => Some(UIElement::AchievementsButton),
            UIElement::UnlocksButtonHover => Some(UIElement::UnlocksButton),
            UIElement::OptionsButtonHover => Some(UIElement::OptionsButton),
            _ => None,
        }
    }
}

#[derive(Component, Debug, Clone)]
pub struct Interactable {
    state: Interaction,
    previous_state: Interaction,
}
impl Default for Interactable {
    fn default() -> Self {
        Self {
            state: Interaction::None,
            previous_state: Interaction::None,
        }
    }
}
impl Interactable {
    pub fn from_state(state: Interaction) -> Self {
        Self {
            state,
            previous_state: Interaction::None,
        }
    }
    pub fn current(&self) -> &Interaction {
        &self.state
    }
    pub fn previous(&self) -> &Interaction {
        &self.previous_state
    }
    pub fn change(&mut self, new_state: Interaction) {
        std::mem::swap(&mut self.previous_state, &mut self.state);
        self.state = new_state;
    }
}
#[derive(Component, Default, Debug, Clone)]
pub enum Interaction {
    #[default]
    None,
    Hovering,
    Dragging {
        item: Entity,
        origin_slot: usize,
    },
}

#[derive(Component)]
pub struct DraggedItem;

#[derive(Debug, Clone)]

pub struct DropOnSlotEvent {
    pub dropped_entity: Entity,
    pub dropped_item_stack: ItemStack,
    pub drop_target_slot_state: InventorySlotState,
    pub parent_interactable_entity: Entity,
    pub stack_empty: bool,
}
#[derive(Debug, Clone)]

pub struct RemoveFromSlotEvent {
    pub removed_item_stack: ItemStack,
    pub removed_slot_state: InventorySlotState,
}

#[derive(Debug, Clone)]

pub struct DropInWorldEvent {
    pub dropped_entity: Entity,
    pub dropped_item_stack: ItemStack,
    pub parent_interactable_entity: Option<Entity>,
    pub stack_empty: bool,
}

pub fn handle_drop_in_world_events(
    mut events: EventReader<DropInWorldEvent>,
    mut game_param: GameParam,
    mut commands: Commands,
    mut interactables: Query<(Entity, &UIElement, &mut Interactable)>,
    item_stacks: Query<&ItemStack>,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
) {
    for drop_event in events.iter() {
        let pos = game_param.player().position.truncate() + Vec2::new(20., 2.);
        drop_event
            .dropped_item_stack
            .spawn_as_drop(&mut commands, &mut game_param, pos);
        commands
            .entity(drop_event.dropped_entity)
            .despawn_recursive();

        if let Some(parent_e) = drop_event.parent_interactable_entity {
            if let Ok(mut parent_interactable) = interactables.get_mut(parent_e) {
                if drop_event.stack_empty {
                    commands
                        .entity(drop_event.dropped_entity)
                        .despawn_recursive();

                    parent_interactable.2.change(Interaction::None);
                } else {
                    let new_drag_icon_entity = spawn_item_stack_icon(
                        &mut commands,
                        &graphics,
                        item_stacks.get(drop_event.dropped_entity).unwrap(),
                        &asset_server,
                        Vec2::ZERO,
                        Vec2::new(0., 0.),
                        3,
                    );

                    commands.entity(new_drag_icon_entity).insert(DraggedItem);
                    parent_interactable.2.change(Interaction::Dragging {
                        item: new_drag_icon_entity,
                        origin_slot: game_param
                            .inv_slot_query
                            .get(parent_e)
                            .expect("parent is an inv slot")
                            .slot_index,
                    });
                }
            }
        }
    }
}
pub fn handle_drop_on_slot_events(
    mut events: EventReader<DropOnSlotEvent>,
    mut game: GameParam,
    proto_param: ProtoParam,
    mut commands: Commands,
    mut interactables: Query<(Entity, &UIElement, &mut Interactable)>,
    item_stacks: Query<&ItemStack>,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    mut inv: Query<&mut Inventory>,
    mut cont_param: UIContainersParam,
) {
    for drop_event in events.iter() {
        // all we need to do here is swap spots in the inventory
        let no_more_dragging: bool;
        let slot_type = drop_event.drop_target_slot_state.r#type;
        let obj = drop_event.dropped_item_stack.obj_type;
        if slot_type.is_crafting() && !cont_param.crafting_tracker.craftable.contains(&obj) {
            continue;
        }
        let return_item = if slot_type.is_crafting()
            && cont_param.crafting_tracker.craftable.contains(&obj)
            && proto_param.get_component::<EquipmentType, _>(obj).is_none()
        {
            inv.single()
                .crafting_items
                .pick_up_and_merge_crafting_result_stack(
                    drop_event.dropped_item_stack.clone(),
                    drop_event.drop_target_slot_state.slot_index,
                    &mut cont_param,
                )
        } else {
            if slot_type.is_crafting() {
                continue;
            }
            let inv_stack = InventoryItemStack {
                item_stack: drop_event.dropped_item_stack.clone(),
                slot: drop_event.drop_target_slot_state.slot_index,
            };
            if !inv_stack.validate(slot_type, &proto_param, &cont_param) {
                return;
            }
            let mut inv = inv.single_mut();
            let container = if slot_type.is_chest() {
                &mut cont_param.chest_option.as_mut().unwrap().items
            } else if slot_type.is_scrapper() {
                &mut cont_param.scrapper_option.as_mut().unwrap().items
            } else {
                inv.get_mut_items_from_slot_type(slot_type)
            };

            inv_stack.drop_item_on_slot(container, &mut game.inv_slot_query, slot_type)
        };

        let updated_drag_item;
        if let Some(return_item) = return_item {
            updated_drag_item = return_item;
            no_more_dragging = false;
        } else {
            updated_drag_item = item_stacks.get(drop_event.dropped_entity).unwrap().clone();
            no_more_dragging = drop_event.stack_empty;
        }

        // if nothing left on cursor and dragging is done
        // despawn parent stack icon entity, set parent slot to no dragging

        commands
            .entity(drop_event.dropped_entity)
            .despawn_recursive();
        if let Ok(mut parent_interactable) =
            interactables.get_mut(drop_event.parent_interactable_entity)
        {
            if no_more_dragging {
                parent_interactable.2.change(Interaction::None);
            } else {
                let new_drag_icon_entity = spawn_item_stack_icon(
                    &mut commands,
                    &graphics,
                    &updated_drag_item,
                    &asset_server,
                    Vec2::ZERO,
                    Vec2::new(0., 0.),
                    3,
                );

                commands.entity(new_drag_icon_entity).insert(DraggedItem);
                parent_interactable.2.change(Interaction::Dragging {
                    item: new_drag_icon_entity,
                    origin_slot: game
                        .inv_slot_query
                        .get(drop_event.parent_interactable_entity)
                        .expect("parent is an inv slot")
                        .slot_index,
                });
            }
        }
    }
}

pub fn handle_dragging(
    cursor_pos: Res<CursorPos>,
    mut interactables: Query<(Entity, &mut Interactable)>,
    mut drag_query: Query<(Entity, &mut Transform)>,
) {
    // iter all interactables, find ones in dragging.
    // set translation to cursor, and bring them to the top z layer so they render on top of everything

    // check things that were just dropped (.previous == dragging)
    // check cursor pos to see if it dropped on top of another item
    // if so, swap their places in the inventory
    // if dropped outside inventory space, remove item and spawn dropped entity item stack
    // else, it is an invalid drag, reset the its original location
    for (_e, interactable) in interactables.iter_mut() {
        if let Interaction::Dragging { item, .. } = interactable.current() {
            if let Ok((_e, mut t)) = drag_query.get_mut(*item) {
                t.translation = cursor_pos.ui_coords.truncate().extend(998.);
            }
        }
    }
}
pub fn handle_hovering(
    mut interactables: Query<(
        Entity,
        &UIElement,
        &mut Interactable,
        Option<&InventorySlotState>,
        Option<&EssenceOption>,
        Option<&StatsButtonState>,
    )>,
    graphics: Res<Graphics>,
    mut commands: Commands,
    inv: Query<&Inventory>,
    chest_option: Option<Res<ChestContainer>>,
    scrapper_option: Option<Res<ScrapperContainer>>,
    crafting_option: Option<Res<CraftingContainer>>,
    mut tooltip_update_events: EventWriter<ToolTipUpdateEvent>,
    mut tooltip_teardown_events: EventWriter<TooltipTeardownEvent>,
    mut stats_update_events: EventWriter<ShowInvPlayerStatsEvent>,
    key_input: Res<Input<KeyCode>>,
) {
    // iter all interactables, find ones in hover state.
    // match the UIElement type to swap to a new image
    let shift_key_pressed = key_input.pressed(KeyCode::LShift);
    let shift_key_just_pressed = key_input.just_pressed(KeyCode::LShift);
    let shift_key_just_released = key_input.just_released(KeyCode::LShift);
    let mut spawning_new_tooltips_this_frame = false;
    let mut tearing_down_tooltips_this_frame = false;
    for (e, ui, interactable, state_option, essence_option, stats_option) in
        interactables.iter_mut()
    {
        if let Interaction::Hovering = interactable.current() {
            if ui == &UIElement::InventorySlot {
                let state = state_option.unwrap();
                // swap to hover img
                commands.entity(e).insert(UIElement::InventorySlotHover);

                commands.spawn(SoundSpawner::new(AudioSoundEffect::UISlotHover, 0.2));

                commands
                    .entity(e)
                    .insert(graphics.get_ui_element_texture(UIElement::InventorySlotHover));

                if let Some(_item_e) = state.item {
                    let item = if state.r#type.is_chest() {
                        chest_option.as_ref().unwrap().items.items[state.slot_index].clone()
                    } else if state.r#type.is_scrapper() {
                        scrapper_option.as_ref().unwrap().items.items[state.slot_index].clone()
                    } else if state.r#type.is_crafting() && crafting_option.is_some() {
                        crafting_option.as_ref().unwrap().items.items[state.slot_index].clone()
                    } else {
                        inv.single().get_items_from_slot_type(state.r#type).items[state.slot_index]
                            .clone()
                    };
                    if let Some(item) = item {
                        spawning_new_tooltips_this_frame = true;
                        tooltip_update_events.send(ToolTipUpdateEvent {
                            item_stack: item.item_stack,
                            is_recipe: state.r#type.is_crafting(),
                            show_range: shift_key_pressed,
                        });
                    }
                }
            }
            if ui == &UIElement::InventorySlotHover
                && (shift_key_just_pressed || shift_key_just_released)
            {
                tooltip_teardown_events.send_default();
                let state = state_option.unwrap();

                if let Some(_item_e) = state.item {
                    let item = if state.r#type.is_chest() {
                        chest_option.as_ref().unwrap().items.items[state.slot_index].clone()
                    } else if state.r#type.is_scrapper() {
                        scrapper_option.as_ref().unwrap().items.items[state.slot_index].clone()
                    } else if state.r#type.is_crafting() && crafting_option.is_some() {
                        crafting_option.as_ref().unwrap().items.items[state.slot_index].clone()
                    } else {
                        inv.single().get_items_from_slot_type(state.r#type).items[state.slot_index]
                            .clone()
                    };
                    if let Some(item) = item {
                        tooltip_update_events.send(ToolTipUpdateEvent {
                            item_stack: item.item_stack,
                            is_recipe: state.r#type.is_crafting(),
                            show_range: shift_key_pressed,
                        });
                    }
                }
            }
            if ui == &UIElement::StatsButton {
                // swap to hover img
                commands.entity(e).insert(UIElement::StatsButtonHover);
                commands
                    .entity(e)
                    .insert(graphics.get_ui_element_texture(UIElement::StatsButtonHover));
                stats_update_events.send(ShowInvPlayerStatsEvent {
                    stat: Some(StatType::from_index(
                        stats_option.expect("stats buttons have stats state").index,
                    )),
                    ignore_timer: true,
                });
            }
            if ui == &UIElement::EssenceButton {
                // swap to hover img
                commands.entity(e).insert(UIElement::EssenceButtonHover);
                commands
                    .entity(e)
                    .insert(graphics.get_ui_element_texture(UIElement::EssenceButtonHover));

                // Skip tooltip for heirlooms - the name is already displayed in the UI
                // let _essence = essence_option.expect("essence buttons have essence state");
            }
        }
        if let Interaction::Hovering = interactable.previous() {
            if ui == &UIElement::InventorySlotHover {
                // swap to base img

                commands
                    .entity(e)
                    .insert(UIElement::InventorySlot)
                    .insert(graphics.get_ui_element_texture(UIElement::InventorySlot));
                if let Some(InventorySlotState { item: Some(_), .. }) = state_option {
                    tearing_down_tooltips_this_frame = true;
                }
            }
            if ui == &UIElement::StatsButtonHover {
                // swap to base img
                commands
                    .entity(e)
                    .insert(UIElement::StatsButton)
                    .insert(graphics.get_ui_element_texture(UIElement::StatsButton));
                stats_update_events.send_default();
            }
            if ui == &UIElement::EssenceButtonHover {
                // swap to base img
                commands
                    .entity(e)
                    .insert(UIElement::EssenceButton)
                    .insert(graphics.get_ui_element_texture(UIElement::EssenceButton));

                tooltip_teardown_events.send_default();
            }
        }
    }
    if tearing_down_tooltips_this_frame && !spawning_new_tooltips_this_frame {
        tooltip_teardown_events.send_default();
    }
}

pub fn handle_item_drop_clicks(
    mouse_input: ResMut<Input<MouseButton>>,
    cursor_pos: Res<CursorPos>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    slot_states: Query<&mut InventorySlotState>,

    mut slot_drop_events: EventWriter<DropOnSlotEvent>,
    mut world_drop_events: EventWriter<DropInWorldEvent>,

    mut item_stack_query: Query<&mut ItemStack>,
    mut interactables: Query<(Entity, &mut Interactable)>,
    mut right_clicks: Local<Vec<usize>>,
    ui_state: Res<State<UIState>>,
) {
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);
    let right_mouse_pressed = mouse_input.pressed(MouseButton::Right);
    if !right_mouse_pressed {
        right_clicks.clear();
    }
    let inv_open = ui_state.0.is_inv_open();
    let hit_test = if inv_open {
        ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None)
    } else {
        None
    };
    for (e, interactable) in interactables.iter_mut() {
        // reset dragged interactables when mouse released
        if let Interaction::Dragging { item, origin_slot } = interactable.current() {
            if let Ok(mut item_stack) = item_stack_query.get_mut(*item) {
                if let Some(drop_target) = hit_test {
                    if let Ok(state) = slot_states.get(drop_target.0) {
                        if left_mouse_pressed {
                            slot_drop_events.send(DropOnSlotEvent {
                                dropped_entity: *item,
                                dropped_item_stack: item_stack.clone(),
                                drop_target_slot_state: state.clone(),
                                parent_interactable_entity: e,
                                stack_empty: true,
                            });
                        } else if right_mouse_pressed {
                            if right_clicks.contains(&state.slot_index)
                                || state.slot_index == *origin_slot
                            {
                                continue;
                            }
                            right_clicks.push(state.slot_index);
                            let mut valid_drop = true;
                            if let Some(target_obj_type) = state.obj_type {
                                if item_stack.obj_type != target_obj_type {
                                    valid_drop = false;
                                }
                            }
                            if valid_drop {
                                let lonely_item_stack: ItemStack = item_stack.copy_with_count(1);
                                item_stack.modify_count(-1);
                                slot_drop_events.send(DropOnSlotEvent {
                                    dropped_entity: *item,
                                    dropped_item_stack: lonely_item_stack,
                                    parent_interactable_entity: e,
                                    drop_target_slot_state: state.clone(),
                                    stack_empty: item_stack.count == 0,
                                });
                            }
                        }
                    }
                } else {
                    // we did not click on a slot, so send drop out of inv event
                    if left_mouse_pressed {
                        world_drop_events.send(DropInWorldEvent {
                            dropped_entity: *item,
                            dropped_item_stack: item_stack.clone(),
                            parent_interactable_entity: Some(e),
                            stack_empty: true,
                        });
                    } else if mouse_input.just_pressed(MouseButton::Right) {
                        let lonely_item_stack: ItemStack = item_stack.copy_with_count(1);
                        item_stack.modify_count(-1);
                        world_drop_events.send(DropInWorldEvent {
                            dropped_entity: *item,
                            dropped_item_stack: lonely_item_stack,
                            parent_interactable_entity: Some(e),
                            stack_empty: item_stack.count == 0,
                        });
                    }
                }
            }
        } else {
            continue;
        };
    }
}

pub fn handle_drop_dragged_items_on_inv_close(
    mut world_drop_events: EventWriter<DropInWorldEvent>,
    ui_state: ResMut<State<UIState>>,
    dragging_query: Query<(Entity, &ItemStack), With<DraggedItem>>,
) {
    if ui_state.0.is_inv_open() {
        return;
    }
    for (e, item_stack) in dragging_query.iter() {
        world_drop_events.send(DropInWorldEvent {
            dropped_entity: e,
            dropped_item_stack: item_stack.clone(),
            parent_interactable_entity: None,
            stack_empty: true,
        });
    }
}
pub fn handle_interaction_clicks(
    mut commands: Commands,
    cursor_pos: Res<CursorPos>,
    mut mouse_input: ResMut<Input<MouseButton>>,
    key_input: ResMut<Input<KeyCode>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut inv_slots: Query<(Entity, &mut Interactable, &mut InventorySlotState)>,
    mut inv_item_icons: Query<(Entity, &mut Transform, &ItemStack)>,
    dragging_query: Query<&DraggedItem>,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    mut inv: Query<&mut Inventory>,
    mut remove_item_event: EventWriter<RemoveFromSlotEvent>,
    mut container_param: UIContainersParam,
    proto: ProtoParam,
    ui_state: Res<State<UIState>>,
    player_atts: Query<&crate::attributes::LootRateBonus, With<crate::player::Player>>,
) {
    // get cursor resource from inputs
    // do a ray cast and get results
    if !ui_state.0.is_inv_open() {
        return;
    }

    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);
    let left_mouse_pressing = mouse_input.pressed(MouseButton::Left);
    let right_mouse_pressed = mouse_input.just_pressed(MouseButton::Right);
    let shift_key_pressed = key_input.pressed(KeyCode::LShift);
    let currently_dragging = dragging_query.iter().len() > 0;
    for (e, mut interactable, mut state) in inv_slots.iter_mut() {
        match hit_test {
            Some(hit_ent) if hit_ent.0 == e => match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                }
                Interaction::Hovering => {
                    // cape slot is not clickable
                    if state.r#type.is_equipment() && state.slot_index == 3 {
                        continue;
                    }
                    if left_mouse_pressed && !currently_dragging && !shift_key_pressed {
                        //send drag event
                        if let Some(item) = state.item {
                            if let Ok(item_icon) = inv_item_icons.get_mut(item) {
                                if state.r#type.is_crafting()
                                    && !container_param
                                        .crafting_tracker
                                        .craftable
                                        .contains(&item_icon.2.obj_type)
                                {
                                    continue;
                                }
                                commands
                                    .entity(item_icon.0)
                                    .remove_parent()
                                    .insert(DraggedItem);

                                remove_item_event.send(RemoveFromSlotEvent {
                                    removed_item_stack: item_icon.2.clone(),
                                    removed_slot_state: state.clone(),
                                });

                                interactable.change(Interaction::Dragging {
                                    item: item_icon.0,
                                    origin_slot: state.slot_index,
                                });
                                let mut inv = inv.single_mut();
                                let container_items = if state.r#type.is_chest() {
                                    &mut container_param.chest_option.as_mut().unwrap().items
                                } else if state.r#type.is_scrapper() {
                                    &mut container_param.scrapper_option.as_mut().unwrap().items
                                } else {
                                    inv.get_mut_items_from_slot_type(state.r#type)
                                };

                                if state.r#type.is_crafting() {
                                    let loot_bonus =
                                        player_atts.get_single().map(|a| a.0).unwrap_or(0);
                                    let new_stack = create_new_random_item_stack_with_attributes(
                                        item_icon.2,
                                        &proto,
                                        &mut commands,
                                        loot_bonus,
                                        false,
                                    );
                                    commands.entity(item_icon.0).insert(new_stack);
                                    container_param.crafted_event.send(CraftedItemEvent {
                                        obj: state.obj_type.unwrap(),
                                    });
                                } else {
                                    container_items.items[state.slot_index] = None;
                                }

                                state.dirty = true;
                                mouse_input.clear();
                            }
                        }
                    } else if right_mouse_pressed && !currently_dragging && !shift_key_pressed {
                        if state.r#type.is_crafting() {
                            continue;
                        }
                        if let Some(item) = state.item {
                            if let Ok(item_icon) = inv_item_icons.get_mut(item) {
                                let mut inv = inv.single_mut();
                                let container = if state.r#type.is_chest() {
                                    &mut container_param.chest_option.as_mut().unwrap().items
                                } else if state.r#type.is_scrapper() {
                                    &mut container_param.scrapper_option.as_mut().unwrap().items
                                } else {
                                    inv.get_mut_items_from_slot_type(state.r#type)
                                };
                                let inv_stack = InventoryItemStack {
                                    item_stack: item_icon.2.clone(),
                                    slot: state.slot_index,
                                };
                                let split_stack = inv_stack.split_stack(&mut state, container);
                                let e = spawn_item_stack_icon(
                                    &mut commands,
                                    &graphics,
                                    &split_stack,
                                    &asset_server,
                                    Vec2::ZERO,
                                    Vec2::new(0., 0.),
                                    3,
                                );

                                commands.entity(e).insert(DraggedItem);
                                interactable.change(Interaction::Dragging {
                                    item: e,
                                    origin_slot: state.slot_index,
                                });
                                mouse_input.clear();
                            }
                        }
                    } else if shift_key_pressed && left_mouse_pressing {
                        if state.r#type.is_crafting() {
                            continue;
                        }

                        let mut inv = inv.single_mut();

                        // Special handling for furnace slots (part of player inventory)
                        // These are always visible when inventory is open (except in Scrapper mode)
                        if ui_state.0.is_inv_open() && ui_state.0 != UIState::Scrapper {
                            if state.r#type.is_inventory() || state.r#type.is_hotbar() {
                                // Shift-clicking from inventory/hotbar - check if item should go to furnace
                                if let Some(item) = &inv.items.items[state.slot_index] {
                                    let obj_type = item.item_stack.obj_type;

                                    // Check if it's a tome/orb (goes to furnace slot 0)
                                    let is_tome_or_orb = matches!(
                                        obj_type,
                                        crate::item::WorldObject::UpgradeTome
                                            | crate::item::WorldObject::OrbOfTransformation
                                    );

                                    if is_tome_or_orb && inv.furnace_items.items[0].is_none() {
                                        // Move tome/orb to furnace slot 0
                                        let item_to_move = inv.items.items[state.slot_index].take();
                                        if let Some(mut moved_item) = item_to_move {
                                            moved_item.slot = 0;
                                            inv.furnace_items.items[0] = Some(moved_item);
                                        }
                                        state.dirty = true;
                                        continue;
                                    }

                                    // Check if it's equipment that can go to furnace slot 1
                                    // Only if there's already a tome/orb in slot 0
                                    let has_tome_in_slot_0 = inv.furnace_items.items[0].is_some();
                                    let slot_map =
                                        &container_param.inv_state.furnace_state.slot_map;
                                    let is_valid_for_slot_1 =
                                        slot_map.len() > 1 && slot_map[1].contains(&obj_type);

                                    if has_tome_in_slot_0
                                        && is_valid_for_slot_1
                                        && inv.furnace_items.items[1].is_none()
                                    {
                                        // Move equipment to furnace slot 1
                                        let item_to_move = inv.items.items[state.slot_index].take();
                                        if let Some(mut moved_item) = item_to_move {
                                            moved_item.slot = 1;
                                            inv.furnace_items.items[1] = Some(moved_item);
                                        }
                                        state.dirty = true;
                                        continue;
                                    }
                                }
                            } else if state.r#type.is_furnace() {
                                // Moving from furnace slots back to inventory
                                // Take the item from furnace first to avoid double borrow
                                let furnace_item = inv.furnace_items.items[state.slot_index].take();
                                if let Some(mut moved_item) = furnace_item {
                                    // Find first empty slot in inventory
                                    if let Some(empty_slot) = inv.items.get_first_empty_slot() {
                                        moved_item.slot = empty_slot;
                                        inv.items.items[empty_slot] = Some(moved_item);
                                    } else {
                                        // No empty slot, put it back
                                        inv.furnace_items.items[state.slot_index] =
                                            Some(moved_item);
                                    }
                                }
                                state.dirty = true;
                                continue;
                            }
                        }

                        // Default behavior for other containers
                        if let Some(active_container) =
                            container_param.get_active_ui_container_mut()
                        {
                            if state.r#type.is_inventory() {
                                inv.items.move_item_to_target_container(
                                    active_container,
                                    state.slot_index,
                                )
                            } else if !state.r#type.is_inventory() {
                                active_container
                                    .move_item_to_target_container(&mut inv.items, state.slot_index)
                            }
                        } else {
                            inv.items
                                .move_item_from_hotbar_to_inv_or_vice_versa(state.slot_index);
                        }
                        // Mark source slot dirty after any shift-click move operation
                        state.dirty = true;
                    }
                }
                _ => (),
            },
            _ => {
                // reset hovering states if we stop hovering ?
                let Interaction::Hovering = interactable.current() else {
                    continue;
                };

                interactable.change(Interaction::None);
            }
        }
    }
}

pub fn handle_cursor_skills_buttons(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut skill_choices: Query<
        (Entity, &mut Interactable, &SkillChoiceUI),
        Without<InventorySlotState>,
    >,
    mut player_skills: Query<(
        Entity,
        &mut PlayerSkills,
        &GlobalTransform,
        &PlayerLevel,
        &OwnedBlessings,
        &mut HeirloomStatsBonuses,
    )>,
    mut skill_queue: ResMut<HeirloomChoiceQueue>,
    mut next_ui_state: ResMut<NextState<UIState>>,
    curr_ui_state: Res<State<UIState>>,
    proto: ProtoParam,
    mut proto_commands: ProtoCommands,
    mut commands: Commands,
    mut att_event: EventWriter<AttributeChangeEvent>,
    graphics: Res<Graphics>,
    mut shrine_query: Query<&mut HeirloomShrineState>,
    mut chaos_tracker: ResMut<ChaosTracker>,
    asset_server: Res<AssetServer>,
) {
    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);
    let ui_state = &curr_ui_state.0;

    for (e, mut interactable, state) in skill_choices.iter_mut() {
        match hit_test {
            Some(hit_ent) if hit_ent.0 == e => match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::UISkillHover, 0.2));

                    let ui_element = state
                        .skill_choice
                        .heirloom
                        .get_ui_element_hover(state.skill_choice.rarity.clone());
                    // swap to hover img
                    commands
                        .entity(e)
                        .insert(ui_element.clone())
                        .insert(graphics.get_ui_element_texture(ui_element));
                }
                Interaction::Hovering => {
                    if left_mouse_pressed && state.interaction_lock_timer.finished() {
                        let (e, mut skills, t, level, blessings, mut heirloom_stats) =
                            player_skills.single_mut();
                        let picked_skill = state.skill_choice.clone();
                        if ui_state == &UIState::Skills {
                            skill_queue.handle_pick_skill(
                                picked_skill.clone(),
                                &mut proto_commands,
                                &proto,
                                t.translation().truncate(),
                                &mut skills,
                                level.level,
                            );
                            picked_skill.heirloom.add_skill_components(
                                e,
                                &mut commands,
                                skills.clone(),
                            );

                            // Add chaos if ChaosBoost heirloom was picked
                            if picked_skill.heirloom == Heirloom::ChaosBoost {
                                chaos_tracker.add_chaos(1.5);
                            }
                            // Add chaos if ChaosStats heirloom was picked (+2 chaos)
                            if picked_skill.heirloom == Heirloom::ChaosStats {
                                chaos_tracker.add_chaos(2.0);
                            }

                            for mut shrine in shrine_query.iter_mut() {
                                if !shrine.is_used {
                                    shrine.is_used = true;

                                    if blessings.has_blessing(Blessing::HeirloomStats) {
                                        let stat_type = HallucinationStatType::random();
                                        let mut rng = rand::thread_rng();

                                        let amount = rng.gen_range(1..=4); // Base amount for heirloom stats blessing
                                        heirloom_stats.add_stat(stat_type, amount);

                                        // Show floating text with stat gain at player position (like item pickups)
                                        let player_pos = t.translation();
                                        let drop_spread = 16.;
                                        let pos_offset = Vec3::new(
                                            rng.gen_range(-drop_spread..drop_spread),
                                            rng.gen_range(0.0..drop_spread) + 10.,
                                            2.,
                                        );
                                        spawn_floating_text_with_shadow(
                                            &mut commands,
                                            &asset_server,
                                            player_pos + pos_offset,
                                            stat_type.color(),
                                            format!("+{} {}", amount, stat_type.name()),
                                        );

                                        info!(
                                            "HeirloomStats blessing: Gained +{} {:?}",
                                            amount, stat_type
                                        );
                                    }
                                }
                            }

                            if skill_queue.active_heirloom_limbo.is_some() {
                                next_ui_state.set(UIState::ActiveSkills);
                            } else {
                                next_ui_state.set(UIState::Closed);
                            }
                            att_event.send(AttributeChangeEvent);
                        } else if ui_state == &UIState::ActiveSkills {
                            match state.index {
                                0 => {
                                    skills.roll_skill_slot =
                                        skill_queue.active_heirloom_limbo.clone();
                                }
                                1 => {
                                    skills.active_skill_slot_1 =
                                        skill_queue.active_heirloom_limbo.clone();
                                }
                                _ => (),
                            }

                            // Add skill components for the new active skill
                            if let Some(ref new_skill) = skill_queue.active_heirloom_limbo {
                                new_skill
                                    .active_skill
                                    .add_skill_components(e, &mut commands);
                            }

                            skill_queue.active_heirloom_limbo = None;
                            next_ui_state.set(UIState::Closed);
                            att_event.send(AttributeChangeEvent);
                        }
                    }
                }
                _ => (),
            },
            _ => {
                // reset hovering states if we stop hovering ?
                let Interaction::Hovering = interactable.current() else {
                    continue;
                };
                let ui_element = state
                    .skill_choice
                    .heirloom
                    .get_ui_element(state.skill_choice.rarity.clone());

                interactable.change(Interaction::None);
                commands
                    .entity(e)
                    .insert(ui_element.clone())
                    .insert(graphics.get_ui_element_texture(ui_element));
            }
        }
    }
}
pub fn handle_cursor_reroll_dice_buttons(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut reroll_dice: Query<(Entity, &mut Interactable, &RerollDice), Without<InventorySlotState>>,
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    mut run_unlocks: ResMut<RunUnlockState>,
) {
    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);

    for (e, mut interactable, state) in reroll_dice.iter_mut() {
        let rerolls_available = run_unlocks.rerolls_remaining > 0;
        match hit_test {
            Some(hit_ent) if hit_ent.0 == e => match interactable.current() {
                Interaction::None => {
                    if !rerolls_available {
                        continue;
                    }
                    interactable.change(Interaction::Hovering);
                    let ui_element = UIElement::RerollDiceHover;
                    commands
                        .entity(e)
                        .insert(ui_element.clone())
                        .insert(graphics.get_ui_element_texture(ui_element));
                }
                Interaction::Hovering => {
                    if left_mouse_pressed && rerolls_available {
                        run_unlocks.rerolls_remaining =
                            run_unlocks.rerolls_remaining.saturating_sub(1);
                        interactable.change(Interaction::None);
                        commands
                            .entity(e)
                            .insert(UIElement::RerollDice)
                            .insert(graphics.get_ui_element_texture(UIElement::RerollDice));
                        commands.spawn(SoundSpawner::new(AudioSoundEffect::UISkillReRoll, 0.4));

                        spawn_skill_choice_flash(
                            &mut commands,
                            &asset_server,
                            Vec3::new(
                                (state.0 as f32 - 1.) * (SKILLS_CHOICE_UI_SIZE.x + 16.) + 4.,
                                4.,
                                15.,
                            ),
                            state.0,
                        );
                    }
                }
                _ => (),
            },
            _ => {
                let Interaction::Hovering = interactable.current() else {
                    continue;
                };
                let ui_element = UIElement::RerollDice;

                interactable.change(Interaction::None);
                commands
                    .entity(e)
                    .insert(ui_element.clone())
                    .insert(graphics.get_ui_element_texture(ui_element));
            }
        }
    }
}

pub fn handle_cursor_banish_buttons(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut banish_buttons: Query<
        (Entity, &mut Interactable, &BanishButton),
        Without<InventorySlotState>,
    >,
    mut commands: Commands,
    graphics: Res<Graphics>,
    mut run_unlocks: ResMut<RunUnlockState>,
    mut skill_queue: ResMut<HeirloomChoiceQueue>,
    mut next_ui_state: ResMut<NextState<UIState>>,
    skill_ui: Query<Entity, With<SkillChoiceUI>>,
) {
    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);

    for (e, mut interactable, banish) in banish_buttons.iter_mut() {
        let banishes_available = run_unlocks.banishes_remaining > 0;
        match hit_test {
            Some(hit_ent) if hit_ent.0 == e => match interactable.current() {
                Interaction::None => {
                    if !banishes_available {
                        continue;
                    }
                    interactable.change(Interaction::Hovering);
                    let ui_element = UIElement::BackButtonHover;
                    commands
                        .entity(e)
                        .insert(ui_element.clone())
                        .insert(graphics.get_ui_element_texture(ui_element));
                }
                Interaction::Hovering => {
                    if left_mouse_pressed && banishes_available {
                        run_unlocks.banishes_remaining =
                            run_unlocks.banishes_remaining.saturating_sub(1);
                        commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonClick, 0.35));

                        if skill_queue.banish_slot(banish.0).is_some() {
                            // Clear the entire queue to prevent the pending levelup check
                            // from reopening the UI - banishing is a deliberate choice to
                            // forfeit the current level-up reward
                            skill_queue.queue.clear();

                            for entity in skill_ui.iter() {
                                commands.entity(entity).despawn_recursive();
                            }
                            next_ui_state.set(UIState::Closed);
                        }
                    }
                }
                _ => (),
            },
            _ => {
                if matches!(interactable.current(), Interaction::Hovering) {
                    interactable.change(Interaction::None);
                    commands
                        .entity(e)
                        .insert(UIElement::BackButton)
                        .insert(graphics.get_ui_element_texture(UIElement::BackButton));
                }
            }
        }
    }
}
pub fn handle_cursor_inventory_upgrade_button(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut upgrade_button: Query<
        (Entity, &mut Interactable, &UpgradeButton),
        Without<InventorySlotState>,
    >,
    mut commands: Commands,
    mut inv_state: ResMut<InventoryState>,
    mut inv: Query<&mut Inventory>,
    graphics: Res<Graphics>,
) {
    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);

    for (e, mut interactable, _) in upgrade_button.iter_mut() {
        match hit_test {
            Some(hit_ent) if hit_ent.0 == e => match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                    let ui_element = UIElement::UpgradeButtonHover;
                    commands
                        .entity(e)
                        .insert(ui_element.clone())
                        .insert(graphics.get_ui_element_texture(ui_element));
                }
                Interaction::Hovering => {
                    if left_mouse_pressed {
                        let mut inv = inv.single_mut();

                        if !inv_state.furnace_state.ready_to_upgrade {
                            if let Some(fuel) = inv.furnace_items.items[0].as_mut() {
                                inv_state.furnace_state.ready_to_upgrade = true;
                                inv_state.furnace_state.current_fuel_type =
                                    fuel.item_stack.obj_type;
                                let updated_fuel = fuel.modify_count(-1);
                                inv.furnace_items.items[0] = updated_fuel;
                            }
                        }
                    }
                }
                _ => (),
            },
            _ => {
                let Interaction::Hovering = interactable.current() else {
                    continue;
                };
                let ui_element = UIElement::UpgradeButton;

                interactable.change(Interaction::None);
                commands
                    .entity(e)
                    .insert(ui_element.clone())
                    .insert(graphics.get_ui_element_texture(ui_element));
            }
        }
    }
}

/// Handles item chest button interactions (for ChestType::Item)
pub fn handle_cursor_item_chest_button(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut item_chest_button: Query<
        (Entity, &mut Interactable, &ItemChestButton),
        Without<InventorySlotState>,
    >,
    mut commands: Commands,
    graphics: Res<Graphics>,
    mut item_chest_state: ResMut<ItemChestState>,
    mut chest_event: EventWriter<ItemChestAnimChangeEvent>,
    mut game: GameParam,
    mut next_ui_state: ResMut<NextState<UIState>>,
) {
    // Only handle Item chests in this system
    if item_chest_state.chest_type != ChestType::Item {
        return;
    }

    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);

    for (e, mut interactable, _) in item_chest_button.iter_mut() {
        match hit_test {
            Some(hit_ent) if hit_ent.0 == e => match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                    let ui_element = UIElement::UpgradeButtonHover;
                    commands
                        .entity(e)
                        .insert(ui_element.clone())
                        .insert(graphics.get_ui_element_texture(ui_element));
                }
                Interaction::Hovering => {
                    if left_mouse_pressed {
                        let state = item_chest_state.state.clone();
                        match state {
                            ItemChestAnimState::Closed => {
                                chest_event.send(ItemChestAnimChangeEvent {
                                    state: ItemChestAnimState::Opening,
                                    set_ui_rarity: None,
                                });
                            }
                            ItemChestAnimState::Opening => {
                                handle_chest_opening_click(&mut item_chest_state, &mut chest_event);
                            }
                            ItemChestAnimState::Done => {
                                let pos = game.player().position.truncate();
                                item_chest_state.picked_item.clone().unwrap().spawn_as_drop(
                                    &mut commands,
                                    &mut game,
                                    pos,
                                );
                                next_ui_state.set(UIState::Closed);
                                commands.remove_resource::<ItemChestState>();
                            }
                        }
                    }
                }
                _ => (),
            },
            _ => {
                let Interaction::Hovering = interactable.current() else {
                    continue;
                };
                let ui_element = UIElement::UpgradeButton;

                interactable.change(Interaction::None);
                commands
                    .entity(e)
                    .insert(ui_element.clone())
                    .insert(graphics.get_ui_element_texture(ui_element));
            }
        }
    }
}

/// Handles heirloom chest button interactions (for ChestType::Heirloom)
pub fn handle_cursor_heirloom_chest_button(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut item_chest_button: Query<
        (Entity, &mut Interactable, &ItemChestButton),
        Without<InventorySlotState>,
    >,
    mut commands: Commands,
    graphics: Res<Graphics>,
    mut item_chest_state: ResMut<ItemChestState>,
    mut chest_event: EventWriter<ItemChestAnimChangeEvent>,
    mut next_ui_state: ResMut<NextState<UIState>>,
    mut skill_queue: ResMut<HeirloomChoiceQueue>,
    mut player_query: Query<(Entity, &Transform, &mut PlayerSkills, &PlayerLevel), With<Player>>,
    proto: ProtoParam,
    mut proto_commands: ProtoCommands,
    mut att_event: EventWriter<AttributeChangeEvent>,
) {
    // Only handle Heirloom chests in this system
    if item_chest_state.chest_type != ChestType::Heirloom {
        return;
    }

    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);

    for (e, mut interactable, _) in item_chest_button.iter_mut() {
        match hit_test {
            Some(hit_ent) if hit_ent.0 == e => match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                    let ui_element = UIElement::UpgradeButtonHover;
                    commands
                        .entity(e)
                        .insert(ui_element.clone())
                        .insert(graphics.get_ui_element_texture(ui_element));
                }
                Interaction::Hovering => {
                    if left_mouse_pressed {
                        let state = item_chest_state.state.clone();
                        match state {
                            ItemChestAnimState::Closed => {
                                chest_event.send(ItemChestAnimChangeEvent {
                                    state: ItemChestAnimState::Opening,
                                    set_ui_rarity: None,
                                });
                            }
                            ItemChestAnimState::Opening => {
                                handle_chest_opening_click(&mut item_chest_state, &mut chest_event);
                            }
                            ItemChestAnimState::Done => {
                                if let Ok((player_entity, transform, mut skills, level)) =
                                    player_query.get_single_mut()
                                {
                                    let picked_heirloom =
                                        item_chest_state.picked_heirloom.clone().unwrap();
                                    // Use grant_heirloom_from_pool instead of handle_pick_skill
                                    // since chest heirlooms come from the pool, not the queue
                                    skill_queue.grant_heirloom_from_pool(
                                        picked_heirloom.clone(),
                                        &mut proto_commands,
                                        &proto,
                                        transform.translation.truncate(),
                                        &mut skills,
                                        level.level,
                                    );
                                    picked_heirloom.heirloom.add_skill_components(
                                        player_entity,
                                        &mut commands,
                                        skills.clone(),
                                    );
                                    att_event.send(AttributeChangeEvent);
                                }
                                next_ui_state.set(UIState::Closed);
                                commands.remove_resource::<ItemChestState>();
                            }
                        }
                    }
                }
                _ => (),
            },
            _ => {
                let Interaction::Hovering = interactable.current() else {
                    continue;
                };
                let ui_element = UIElement::UpgradeButton;

                interactable.change(Interaction::None);
                commands
                    .entity(e)
                    .insert(ui_element.clone())
                    .insert(graphics.get_ui_element_texture(ui_element));
            }
        }
    }
}

/// Helper to handle the "Opening" state click - speeds up the animation
fn handle_chest_opening_click(
    item_chest_state: &mut ResMut<ItemChestState>,
    chest_event: &mut EventWriter<ItemChestAnimChangeEvent>,
) {
    let elapsed = item_chest_state
        .shuffle_duration_timer
        .elapsed()
        .as_secs_f32();
    let duration = item_chest_state
        .shuffle_duration_timer
        .duration()
        .as_secs_f32();
    match item_chest_state.shuffle_duration_timer.percent() {
        0.0..=0.25 => {
            item_chest_state
                .shuffle_duration_timer
                .tick(Duration::from_secs_f32(duration * 0.25 - elapsed));
        }
        0.25..=0.48 => {
            item_chest_state
                .shuffle_duration_timer
                .tick(Duration::from_secs_f32(duration * 0.48 - elapsed));
        }
        0.5..=0.7 => {
            item_chest_state
                .shuffle_duration_timer
                .tick(Duration::from_secs_f32(duration * 0.70 - elapsed));
        }
        _ => {
            chest_event.send(ItemChestAnimChangeEvent {
                state: ItemChestAnimState::Done,
                set_ui_rarity: None,
            });
        }
    }
}

pub fn handle_cursor_main_menu_buttons(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut menu_buttons: Query<
        (Entity, &mut Interactable, &MenuButton, &UIElement),
        Without<InventorySlotState>,
    >,
    mut text: Query<&mut Text, With<MenuButton>>,
    mut send_menu_button_event: EventWriter<MenuButtonClickEvent>,
    mut commands: Commands,
    graphics: Res<Graphics>,
    info_check: Query<&InfoModal>,
    curr_ui_state: Res<State<UIState>>,
) {
    let menu_open = curr_ui_state.0 == UIState::ClassSelection
        || curr_ui_state.0 == UIState::Unlocks
        || curr_ui_state.0 == UIState::Options
        || curr_ui_state.0 == UIState::Achievements;
    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    let left_mouse_pressed = mouse_input.just_released(MouseButton::Left);

    for (e, mut interactable, menu_button, ui_element) in menu_buttons.iter_mut() {
        if !info_check.is_empty() && menu_button != &MenuButton::InfoOK {
            continue;
        }
        if menu_open
            && matches!(
                menu_button,
                MenuButton::Start
                    | MenuButton::Achievements
                    | MenuButton::Unlocks
                    | MenuButton::Options
                    | MenuButton::Quit
            )
        {
            continue;
        }
        let uses_sprite = matches!(
            menu_button,
            MenuButton::Start
                | MenuButton::Unlocks
                | MenuButton::Options
                | MenuButton::Achievements
                | MenuButton::AchievementsPrev
                | MenuButton::AchievementsNext
                | MenuButton::Quit
                | MenuButton::Back
                | MenuButton::Begin
                | MenuButton::ClassUnlockYes
                | MenuButton::ClassUnlockNo
                | MenuButton::GameOverOK
                | MenuButton::OptionsRestart
                | MenuButton::OptionsExit
        );
        match hit_test {
            Some(hit_ent) if hit_ent.0 == e => match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonHover, 0.05));
                    if uses_sprite {
                        let Some(ui_element_hover) = ui_element.get_hover_state() else {
                            continue;
                        };
                        commands
                            .entity(e)
                            .insert(ui_element_hover.clone())
                            .insert(graphics.get_ui_element_texture(ui_element_hover));
                    } else {
                        let color = if menu_button == &MenuButton::GameOverOK
                            || menu_button == &MenuButton::InfoOK
                            || menu_button == &MenuButton::Scrapper
                        {
                            RED
                        } else {
                            DARK_GREEN
                        };
                        if let Ok(mut text_comp) = text.get_mut(e) {
                            text_comp.sections[0].style.color = color;
                        }
                    }
                }
                Interaction::Hovering => {
                    if left_mouse_pressed {
                        send_menu_button_event.send(MenuButtonClickEvent {
                            button: menu_button.clone(),
                        });
                        commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonClick, 0.2));
                    }
                }
                _ => (),
            },
            _ => {
                // reset hovering states if we stop hovering
                let Interaction::Hovering = interactable.current() else {
                    continue;
                };
                interactable.change(Interaction::None);

                if uses_sprite {
                    commands
                        .entity(e)
                        .insert(ui_element.clone().get_normal_state().unwrap())
                        .insert(graphics.get_ui_element_texture(
                            ui_element.clone().get_normal_state().unwrap(),
                        ));
                } else {
                    if let Ok(mut text_comp) = text.get_mut(e) {
                        text_comp.sections[0].style.color = Color::WHITE;
                    }
                    commands.entity(e).remove::<UIElement>();
                    commands.entity(e).remove::<Handle<Image>>();
                }
            }
        }
    }
}

pub fn handle_cursor_essence_buttons(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut essence_buttons: Query<(Entity, &mut Interactable, &EssenceOption)>,
    mut essence_event: EventWriter<SubmitEssenceChoice>,
) {
    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);

    for (e, mut interactable, essence_option) in essence_buttons.iter_mut() {
        match hit_test {
            Some(hit_ent) if hit_ent.0 == e => match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                }
                Interaction::Hovering => {
                    if left_mouse_pressed {
                        essence_event.send(SubmitEssenceChoice {
                            choice: essence_option.clone(),
                        });
                    }
                }
                _ => (),
            },
            _ => {
                // reset hovering states if we stop hovering ?
                let Interaction::Hovering = interactable.current() else {
                    continue;
                };

                interactable.change(Interaction::None);
            }
        }
    }
}

pub fn handle_hotbar_slot_clicks_when_inv_closed(
    mut param_set: ParamSet<(
        Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
        Query<(Entity, &InventorySlotState)>,
        Query<&mut InventorySlotState>,
    )>,
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    mut inv_state: ResMut<InventoryState>,
    ui_state: Res<State<UIState>>,
    mut pet_weapon_events: EventWriter<UpdatePetWeaponEvent>,
) {
    // Only handle clicks when inventory is closed
    if ui_state.0.is_inv_open() {
        return;
    }

    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);
    if !left_mouse_pressed {
        return;
    }

    // Extract slot_index first using read queries
    let clicked_slot_index = {
        // First, find which entity was clicked
        let ui_sprites = param_set.p0();
        let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);

        let clicked_entity = if let Some((hit_entity, _, _)) = hit_test {
            hit_entity
        } else {
            return;
        };

        // Then, check if it's a hotbar slot and get its slot_index
        let slot_slots = param_set.p1();
        slot_slots.iter().find_map(|(entity, slot_state)| {
            if entity == clicked_entity && slot_state.r#type.is_hotbar() {
                Some(slot_state.slot_index)
            } else {
                None
            }
        })
    };

    // If we found a clicked hotbar slot, change to it using mutable query
    if let Some(slot_index) = clicked_slot_index {
        if slot_index != inv_state.active_hotbar_slot {
            let mut slot_query = param_set.p2();
            change_hotbar_slot(slot_index, &mut inv_state, &mut slot_query);
            pet_weapon_events.send(UpdatePetWeaponEvent);
        }
    }
}
