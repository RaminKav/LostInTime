use std::marker::PhantomData;

use crate::{
    attributes::{
        hunger::Hunger,
        modifiers::{ModifyHealthEvent, ModifyManaEvent},
    },
    chaos::IncreaseChaosEvent,
    client::analytics::{AnalyticsTrigger, AnalyticsUpdateEvent},
    cursor::CursorPos,
    inventory::Inventory,
    juice::UseItemEvent,
    night::NightTracker,
    player::{stats::SkillPoints, ModifyCurencyEvent, MovePlayerEvent},
    proto::proto_param::ProtoParam,
    ui::{
        item_chest::ItemChestState, minimap::UpdateMiniMapEvent, scrapper_ui::ScrapperContainer,
        ChestContainer, FurnaceContainer, InventorySlotState, InventorySlotType, UIState,
    },
    world::{
        dimension::DimensionSpawnEvent,
        portal::BossKillTracker,
        world_helpers::{can_object_be_placed_here, world_pos_to_tile_pos},
        TileMapPosition,
    },
    BounceEvent, GameParam, GameState, TextureCamera,
};
use bevy::{ecs::system::SystemParam, prelude::*};
use bevy_ecs_tilemap::tiles::TilePos;
use bevy_proto::prelude::{ReflectSchematic, Schematic};

use super::{
    potion_buffs::{AttackSpeedBuff, MovementSpeedBuff},
    CraftingTracker, PlaceItemEvent, Recipes, WorldObject,
};

#[derive(Component, Reflect, FromReflect, Clone, Schematic, Default, PartialEq)]
#[reflect(Component, Schematic)]
pub enum ItemAction {
    #[default]
    None,
    ModifyHealth(i32),
    ModifyMana(i32),
    ApplyAttackSpeedBuff(f32, f32),   // (duration, speed_multiplier)
    ApplyMovementSpeedBuff(f32, f32), // (duration, speed_multiplier)
    TeleportHome,
    PlacesInto(WorldObject),
    Eat(i8),
    Essence,
    DungeonKey,
    GrantSkillPoint(u8),
    ItemChest,
    HeirloomChest,
    BeaconPortal,
    BeaconDungeonEntrance,
    BeaconBossShrine,
}
impl ItemAction {
    pub fn get_tooltip(&self) -> Option<String> {
        match self {
            ItemAction::ModifyHealth(delta) => {
                Some(format!("{}{} HP", if delta > &0 { "+" } else { "" }, delta))
            }
            ItemAction::ModifyMana(delta) => Some(format!(
                "{}{} Mana",
                if delta > &0 { "+" } else { "" },
                delta
            )),
            ItemAction::ApplyAttackSpeedBuff(duration, multiplier) => Some(format!(
                "+{:.0}% Att Speed for {:.0}s",
                (multiplier - 1.0) * 100.0,
                duration
            )),
            ItemAction::ApplyMovementSpeedBuff(duration, multiplier) => Some(format!(
                "+{:.0}% Speed for {:.0}s",
                (multiplier - 1.0) * 100.0,
                duration
            )),
            ItemAction::Eat(delta) => Some(format!(
                "{}{} Food",
                if delta > &0 { "+" } else { "" },
                delta
            )),
            _ => None,
        }
    }
}

#[derive(Component, Reflect, FromReflect, Schematic, Default)]
#[reflect(Component, Schematic)]
pub struct ItemActions {
    pub actions: Vec<ItemAction>,
}

impl ItemActions {
    pub fn get_action_type(&self) -> String {
        let mut has_eat = false;
        let mut has_places_into = false;
        let mut has_consumable = false;

        for action in &self.actions {
            match action {
                ItemAction::Eat(_) => has_eat = true,
                ItemAction::PlacesInto(_) => has_places_into = true,
                ItemAction::ModifyHealth(_) => has_consumable = true,
                ItemAction::ModifyMana(_) => has_consumable = true,
                ItemAction::ApplyAttackSpeedBuff(_, _) => has_consumable = true,
                ItemAction::ApplyMovementSpeedBuff(_, _) => has_consumable = true,
                _ => {}
            }
        }

        if has_eat {
            "Consumable".to_string()
        } else if has_places_into {
            "Placeable".to_string()
        } else if has_consumable {
            "Consumable".to_string()
        } else {
            "Useable".to_string()
        }
    }
}
#[derive(Component, Reflect, FromReflect, Schematic, Default)]
#[reflect(Component, Schematic)]
pub struct ManaCost(pub i32);
#[derive(Component, Reflect, FromReflect, Schematic, Default)]
#[reflect(Component, Schematic)]
pub struct ConsumableItem;

pub struct ActionSuccessEvent {
    pub obj: WorldObject,
    pub item_slot: usize,
}
#[derive(SystemParam)]
pub struct ItemActionParam<'w, 's> {
    pub move_player_event: EventWriter<'w, MovePlayerEvent>,
    pub bounce_event: EventWriter<'w, BounceEvent>,
    pub use_item_event: EventWriter<'w, UseItemEvent>,
    pub currency_event: EventWriter<'w, ModifyCurencyEvent>,
    pub modify_health_event: EventWriter<'w, ModifyHealthEvent>,
    pub increase_chaos_event: EventWriter<'w, IncreaseChaosEvent>,
    pub dim_event: EventWriter<'w, DimensionSpawnEvent>,
    pub analytics_event: EventWriter<'w, AnalyticsUpdateEvent>,
    pub next_inv_state: ResMut<'w, NextState<UIState>>,
    pub next_game_state: ResMut<'w, NextState<GameState>>,
    pub modify_mana_event: EventWriter<'w, ModifyManaEvent>,
    pub place_item_event: EventWriter<'w, PlaceItemEvent>,
    pub action_success_event: EventWriter<'w, ActionSuccessEvent>,
    pub minimap_event: EventWriter<'w, UpdateMiniMapEvent>,
    pub cursor_pos: Res<'w, CursorPos>,
    pub hunger_query: Query<'w, 's, &'static mut Hunger>,
    pub chest_query: Query<'w, 's, &'static ChestContainer>,
    pub scrapper_query: Query<'w, 's, &'static ScrapperContainer>,
    pub furnace_query: Query<'w, 's, &'static FurnaceContainer>,
    pub crafting_tracker: ResMut<'w, CraftingTracker>,
    pub recipes: Res<'w, Recipes>,
    pub night_tracker: Res<'w, NightTracker>,
    pub skill_points_query: Query<'w, 's, &'static mut SkillPoints>,
    pub game_camera: Query<'w, 's, Entity, With<TextureCamera>>,
    pub asset_server: Res<'w, AssetServer>,
    pub beacon_guidance: ResMut<'w, crate::ui::damage_numbers::BeaconGuidanceRegistry>,
    pub boss_kill_tracker: Option<Res<'w, BossKillTracker>>,
    pub achievements: Option<ResMut<'w, crate::player::achievements::Achievements>>,
    pub player_class: Option<ResMut<'w, crate::player::skills::PlayerClass>>,
    pub achievement_events: EventWriter<'w, crate::player::achievements::AchievementUnlockedEvent>,
    pub player_skills:
        Query<'w, 's, &'static crate::player::skills::PlayerSkills, With<crate::player::Player>>,
    pub infinite_mode: Res<'w, crate::night::InfiniteMode>,

    #[system_param(ignore)]
    marker: PhantomData<&'s ()>,
}

impl ItemActions {
    pub fn run_action(
        &self,
        obj: WorldObject,
        item_slot: usize,
        item_action_param: &mut ItemActionParam,
        game: &mut GameParam,
        proto_param: &ProtoParam,
        commands: &mut Commands,
    ) {
        for action in &self.actions {
            match action {
                ItemAction::ModifyHealth(delta) => {
                    item_action_param
                        .modify_health_event
                        .send(ModifyHealthEvent(*delta));
                    item_action_param.use_item_event.send(UseItemEvent(obj));
                }
                ItemAction::ModifyMana(delta) => {
                    item_action_param
                        .modify_mana_event
                        .send(ModifyManaEvent(*delta));
                    item_action_param.use_item_event.send(UseItemEvent(obj));
                }
                ItemAction::ApplyAttackSpeedBuff(duration, multiplier) => {
                    // Apply attack speed buff to player
                    if let Ok((player_entity, _, _)) = game.player_query.get_single() {
                        commands
                            .entity(player_entity)
                            .insert(AttackSpeedBuff::new(*duration, *multiplier));
                    }
                    item_action_param.use_item_event.send(UseItemEvent(obj));
                }
                ItemAction::ApplyMovementSpeedBuff(duration, multiplier) => {
                    // Apply movement speed buff to player
                    if let Ok((player_entity, _, _)) = game.player_query.get_single() {
                        commands
                            .entity(player_entity)
                            .insert(MovementSpeedBuff::new(*duration, *multiplier));
                    }
                    item_action_param.use_item_event.send(UseItemEvent(obj));
                }
                ItemAction::TeleportHome => {
                    item_action_param.move_player_event.send(MovePlayerEvent {
                        pos: TileMapPosition::new(IVec2::new(0, 0), TilePos::new(0, 0)),
                    });
                    item_action_param.use_item_event.send(UseItemEvent(obj));
                }
                ItemAction::PlacesInto(obj) => {
                    let pos = item_action_param.cursor_pos.world_coords.truncate();
                    if game.player().position.truncate().distance(pos)
                        > game.player().reach_distance * 32.
                    {
                        return;
                    }
                    if !can_object_be_placed_here(
                        world_pos_to_tile_pos(pos),
                        game,
                        *obj,
                        proto_param,
                    ) {
                        return;
                    }
                    item_action_param.place_item_event.send(PlaceItemEvent {
                        obj: *obj,
                        pos,
                        placed_by_player: true,
                        override_existing_obj: false,
                    });
                    item_action_param
                        .analytics_event
                        .send(AnalyticsUpdateEvent {
                            update_type: AnalyticsTrigger::ObjectPlaced(*obj),
                        });
                }
                ItemAction::Eat(delta) => {
                    for mut hunger in item_action_param.hunger_query.iter_mut() {
                        hunger.modify_hunger(*delta);
                    }
                    item_action_param.use_item_event.send(UseItemEvent(obj));
                }
                ItemAction::Essence => {
                    item_action_param.next_inv_state.set(UIState::Essence);
                }
                ItemAction::DungeonKey => {
                    // spawn_new_dungeon_dimension(
                    //     game,
                    //     commands,
                    //     &mut proto_param.proto_commands,
                    //     &mut item_action_param.move_player_event,
                    // );
                }
                ItemAction::GrantSkillPoint(amount) => {
                    let mut sp = item_action_param.skill_points_query.single_mut();
                    sp.count += *amount;

                    item_action_param.use_item_event.send(UseItemEvent(obj));
                }
                ItemAction::ItemChest => {
                    commands.insert_resource(ItemChestState::new_item_chest());
                }
                ItemAction::HeirloomChest => {
                    commands.insert_resource(ItemChestState::new_heirloom_chest());
                }
                ItemAction::BeaconPortal => {
                    if let Some(e) = item_action_param.beacon_guidance.portal.take() {
                        // Safely despawn - check if entity exists first
                        if let Some(entity_commands) = commands.get_entity(e) {
                            entity_commands.despawn_recursive();
                        }
                    } else {
                        let icon_e =
                            crate::ui::damage_numbers::spawn_screen_locked_icon_to_world_pos(
                                commands,
                                &game.graphics,
                                &item_action_param.asset_server,
                                obj,
                                Vec2::ZERO,
                            );
                        item_action_param.beacon_guidance.portal = Some(icon_e);
                    }
                }
                ItemAction::BeaconDungeonEntrance => {
                    if let Some(e) = item_action_param.beacon_guidance.dungeon.take() {
                        // Safely despawn - check if entity exists first
                        if let Some(entity_commands) = commands.get_entity(e) {
                            entity_commands.despawn_recursive();
                        }
                    } else {
                        let pos = game
                            .world_obj_cache
                            .unique_objs
                            .get(&WorldObject::DungeonEntrance)
                            .map(|tp| {
                                crate::world::world_helpers::tile_pos_to_world_pos(*tp, false)
                            })
                            .unwrap_or(Vec2::ZERO);
                        // Only spawn if we have a valid position (not Vec2::ZERO)
                        if pos != Vec2::ZERO {
                            let icon_e =
                                crate::ui::damage_numbers::spawn_screen_locked_icon_to_world_pos(
                                    commands,
                                    &game.graphics,
                                    &item_action_param.asset_server,
                                    obj,
                                    pos,
                                );
                            item_action_param.beacon_guidance.dungeon = Some(icon_e);
                        }
                    }
                }
                ItemAction::BeaconBossShrine => {
                    if let Some(e) = item_action_param.beacon_guidance.boss.take() {
                        // Safely despawn - check if entity exists first
                        if let Some(entity_commands) = commands.get_entity(e) {
                            entity_commands.despawn_recursive();
                        }
                    } else {
                        let pos = game
                            .world_obj_cache
                            .unique_objs
                            .get(&WorldObject::BossShrine)
                            .map(|tp| {
                                crate::world::world_helpers::tile_pos_to_world_pos(*tp, false)
                            })
                            .unwrap_or(Vec2::ZERO);
                        // Only spawn if we have a valid position (not Vec2::ZERO)
                        if pos != Vec2::ZERO {
                            let icon_e =
                                crate::ui::damage_numbers::spawn_screen_locked_icon_to_world_pos(
                                    commands,
                                    &game.graphics,
                                    &item_action_param.asset_server,
                                    obj,
                                    pos,
                                );
                            item_action_param.beacon_guidance.boss = Some(icon_e);
                        }
                    }
                }
                _ => {}
            }
        }

        item_action_param
            .action_success_event
            .send(ActionSuccessEvent { obj, item_slot });
    }
}

pub fn handle_item_action_success(
    mut success_events: EventReader<ActionSuccessEvent>,
    mut inv: Query<&mut Inventory>,
    proto_param: ProtoParam,
    mut analytics_event: EventWriter<AnalyticsUpdateEvent>,
    mut inv_slots: Query<&mut InventorySlotState>,
) {
    for e in success_events.iter() {
        if proto_param
            .get_component::<ConsumableItem, _>(e.obj)
            .is_some()
        {
            let mut item_action_item = inv.single().items.items[e.item_slot].clone().unwrap();
            inv.single_mut().items.items[e.item_slot] = item_action_item.modify_count(-1);
            analytics_event.send(AnalyticsUpdateEvent {
                update_type: AnalyticsTrigger::ItemConsumed(e.obj),
            });

            let was_last_consumable_in_slot = item_action_item.item_stack.count == 0;
            if was_last_consumable_in_slot {
                let FOOD = vec![
                    WorldObject::Apple,
                    WorldObject::BrownMushroomBlock,
                    WorldObject::RedMushroomBlock,
                    WorldObject::RedStew,
                    WorldObject::Berries,
                    WorldObject::CookedMeat,
                ];
                let HEALING = vec![
                    WorldObject::SmallPotion,
                    WorldObject::LargePotion,
                    WorldObject::Bandage,
                    WorldObject::Apple,
                    WorldObject::CookedMeat,
                    WorldObject::RedStew,
                    WorldObject::RedMushroomBlock,
                    WorldObject::Berries,
                ];
                let consumable_slot = item_action_item.slot;
                let mut was_food = false;
                let mut was_healing = false;
                let item_actions = proto_param
                    .get_component::<ItemActions, _>(e.obj)
                    .expect("response to an item without an action");
                item_actions.actions.iter().for_each(|a| match a {
                    &ItemAction::Eat(_) => was_food = true,
                    &ItemAction::ModifyHealth(_) => was_healing = true,
                    _ => {}
                });
                let mut inv = inv.single_mut();
                if was_food {
                    // find another food item in inv and place it in this slot
                    for food in FOOD.iter() {
                        if let Some(matching_slot) = inv.items.get_slot_for_item_in_container(food)
                        {
                            let next_consumable_item_stack = inv.items.items[matching_slot]
                                .as_ref()
                                .unwrap()
                                .modify_slot(consumable_slot);
                            next_consumable_item_stack.add_to_container(
                                &mut inv.items,
                                InventorySlotType::Normal,
                                &mut inv_slots,
                            );
                            inv.items.items[matching_slot] = None;
                            return;
                        }
                    }
                }
                if was_healing {
                    // find another food item in inv and place it in this slot
                    for healing_item in HEALING.iter() {
                        if let Some(matching_slot) =
                            inv.items.get_slot_for_item_in_container(healing_item)
                        {
                            let next_consumable_item_stack = inv.items.items[matching_slot]
                                .as_ref()
                                .unwrap()
                                .modify_slot(consumable_slot);
                            next_consumable_item_stack.add_to_container(
                                &mut inv.items,
                                InventorySlotType::Normal,
                                &mut inv_slots,
                            );
                            inv.items.items[matching_slot] = None;
                            return;
                        }
                    }
                }
            }
        }
    }
}
