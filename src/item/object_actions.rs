use super::active_skill_shrine::{
    refresh_active_skill_shrine_offer_skills, skill_choices_from_offer_skills,
    ActiveSkillShrineSelection, ActiveSkillShrineState,
};
use super::combat_shrine::{CombatShrine, CombatShrineAnim};
use super::dungeon_shrine::{DungeonShrine, DungeonShrineType};
use super::gamble_shrine::{GambleShrine, GambleShrineAnim};
use super::heirloom_shrine::HeirloomShrineState;
use super::item_actions::ItemActionParam;
use super::microwave_shrine::MicrowaveShrineState;
use super::{get_crafting_inventory_item_stacks, PlaceItemEvent, WorldObject};

use crate::assets::SpriteAnchor;
use crate::chaos::IncreaseChaosEvent;
use crate::colors::{RED, WHITE};
use crate::container::Container;
use crate::custom_commands::CommandsExt;
use crate::inventory::Inventory;
use crate::item::dungeon_shrine::NUM_DUNGEON_SHRINE_MOBS;
use crate::juice::ShakeEffect;
use crate::player::ModifyCurencyEvent;
use crate::proto::proto_param::ProtoParam;
use crate::ui::crafting_ui::{CraftingContainer, CraftingContainerType};
use crate::ui::damage_numbers::{
    spawn_floating_text_with_shadow, spawn_screen_locked_icon_to_world_pos, BeaconGuidance,
    BeaconTarget,
};
use crate::ui::game_fonts::FLOATING_TEXT;
use crate::ui::item_chest::ItemChestState;
use crate::ui::key_input_guide::InteractionGuideTrigger;
use crate::ui::minimap::UpdateMiniMapEvent;
use crate::ui::tips::{Tip, TipEvent};
use crate::ui::UIState;
use crate::world::dimension::{DimensionSpawnEvent, Era};
use crate::world::world_helpers;
use crate::world::world_helpers::tile_pos_to_world_pos;
use itertools::Itertools;

use crate::world::TileMapPosition;
use crate::{
    attributes::modifiers::ModifyHealthEvent, player::MovePlayerEvent,
    world::world_helpers::world_pos_to_tile_pos,
};
use crate::{BounceEvent, GameParam, DEBUG};
use bevy::prelude::*;
use bevy_aseprite::anim::AsepriteAnimation;
use bevy_proto::prelude::{ReflectSchematic, Schematic};
use rand::Rng;

#[derive(Component, Reflect, FromReflect, Schematic, Clone, Default)]
#[reflect(Component, Schematic)]
pub enum ObjectAction {
    #[default]
    None,
    ModifyHealth(i32),
    Teleport(Vec2),
    DungeonTeleport,
    DungeonExit,
    Chest,
    Scrapper,
    Crafting(CraftingContainerType),
    Furnace,
    ChangeObject(WorldObject),
    SetHome,
    CombatShrine,
    GambleShrine,
    ActiveSkillShrine,
    HeirloomShrine,
    MicrowaveShrine,
    WeaponShrine,
    ArmorShrine,
    AccessoryShrine,
    ToggleBeacon(WorldObject),
    IncreaseChaos(f32),
    TimePortal,
}

#[derive(Component, Reflect, FromReflect, Schematic, Default)]
#[reflect(Component, Schematic)]
pub enum ObjectActionCost {
    #[default]
    None,
    CoinCost(i32),
    TimeFragmentCost(i32),
    Item(WorldObject, usize),
}

#[derive(Component, Reflect, FromReflect, Schematic, Default)]
#[reflect(Component, Schematic)]
pub enum TouchTriggerObjectAction {
    #[default]
    None,
    Bounce,
    ItemChest,
    HeirloomChest,
}

impl ObjectAction {
    pub fn run_action(
        &self,
        e: Entity,
        obj_pos: TileMapPosition,
        obj: WorldObject,
        game: &mut GameParam,
        item_action_param: &mut ItemActionParam,
        commands: &mut Commands,
        proto_param: &mut ProtoParam,
        inv: &mut Inventory,
    ) {
        let maybe_cost: Option<&ObjectActionCost> =
            proto_param.get_component::<ObjectActionCost, _>(obj);
        if let Some(cost) = maybe_cost {
            match cost {
                ObjectActionCost::CoinCost(cost) => {
                    if game.get_coins() as i32 >= *cost {
                        item_action_param.currency_event.send(ModifyCurencyEvent {
                            delta: -cost,
                            obj: WorldObject::Coin,
                        });
                    } else {
                        //TODO: SCREEN SHAKE
                        return;
                    }
                }
                ObjectActionCost::TimeFragmentCost(cost) => {
                    if game.get_time_fragments() as i32 >= *cost {
                        item_action_param.currency_event.send(ModifyCurencyEvent {
                            delta: -cost,
                            obj: WorldObject::TimeFragment,
                        });
                    } else {
                        //TODO: SCREEN SHAKE
                        return;
                    }
                }
                ObjectActionCost::Item(obj, count) => {
                    if inv.items.get_item_count_in_container(*obj) >= *count {
                        if let Err(err) = inv.items.remove_from_inventory(*count, *obj) {
                            error!("Error removing item from inventory: {:?}", err);
                            return;
                        }
                    } else {
                        //TODO: SCREEN SHAKE
                        return;
                    }
                }
                _ => {}
            }
        }
        match self {
            ObjectAction::ModifyHealth(delta) => {
                item_action_param
                    .modify_health_event
                    .send(ModifyHealthEvent(*delta));
            }
            ObjectAction::Teleport(pos) => {
                let pos = world_pos_to_tile_pos(*pos);
                item_action_param
                    .move_player_event
                    .send(MovePlayerEvent { pos });
            }
            ObjectAction::DungeonTeleport => {
                // Prevent dungeon entry if endless mode is active
                if item_action_param.infinite_mode.active {
                    info!("Cannot enter dungeon while endless mode is active");
                    return;
                }
                item_action_param.dim_event.send(DimensionSpawnEvent {
                    swap_to_dim_now: true,
                    new_era: Some(Era::DungeonMain),
                });
            }
            ObjectAction::DungeonExit => {
                // Unlock dungeon completion achievement
                if let Some(ref mut achievements) = item_action_param.achievements {
                    use crate::player::achievements::{persist_achievements_state, Achievement};
                    if achievements.complete(Achievement::DungeonCrawler) {
                        persist_achievements_state(achievements);
                        item_action_param.achievement_events.send(
                            crate::player::achievements::AchievementUnlockedEvent {
                                achievement: Achievement::DungeonCrawler,
                                reward_currency: Achievement::DungeonCrawler.reward_currency(),
                            },
                        );
                        info!("Achievement completed: DungeonCrawler");
                    }
                }

                // Find the most recent non-dungeon era, defaulting to Era::Main if none found
                let current_era: usize = game
                    .era
                    .visited_eras
                    .iter()
                    .filter(|e| e != &&Era::DungeonMain)
                    .map(|e| e.index())
                    .sorted()
                    .last()
                    .unwrap_or(0); // Default to Era::Main (index 0) if no previous era
                item_action_param.dim_event.send(DimensionSpawnEvent {
                    swap_to_dim_now: true,
                    new_era: Some(Era::from_index(current_era)),
                });
            }
            ObjectAction::Chest => {
                let chest_inv = item_action_param.chest_query.get(e).unwrap();
                commands.insert_resource(chest_inv.clone());
            }
            ObjectAction::Scrapper => {
                let scrapper_inv = item_action_param.scrapper_query.get(e).unwrap();
                commands.insert_resource(scrapper_inv.clone());
            }
            ObjectAction::ChangeObject(new_obj) => {
                commands.entity(e).despawn_recursive();
                let pos = item_action_param.cursor_pos.world_coords.truncate();
                game.remove_object_from_chunk_cache(world_pos_to_tile_pos(pos));

                item_action_param.place_item_event.send(PlaceItemEvent {
                    obj: *new_obj,
                    pos,
                    placed_by_player: true,
                    override_existing_obj: false,
                });
            }
            ObjectAction::Crafting(crafting_type) => {
                let items = if let Some(crafting_items) = item_action_param
                    .crafting_tracker
                    .crafting_type_map
                    .get(crafting_type)
                {
                    get_crafting_inventory_item_stacks(
                        crafting_items.to_vec(),
                        &item_action_param.recipes,
                        proto_param,
                        game.get_player_level() as u8,
                    )
                } else {
                    vec![]
                };
                let crafting_container_res = CraftingContainer {
                    items: Container { items },
                };
                commands.insert_resource(crafting_container_res.clone());

                item_action_param
                    .crafting_tracker
                    .discovered_crafting_types
                    .push(crafting_type.clone());
            }
            ObjectAction::Furnace => {
                let furnace_res = item_action_param.furnace_query.get(e).unwrap();
                commands.insert_resource(furnace_res.clone());
            }
            ObjectAction::SetHome => {
                let pos =
                    world_pos_to_tile_pos(item_action_param.cursor_pos.world_coords.truncate());
                game.game.home_pos = Some(pos);
            }
            ObjectAction::ToggleBeacon(obj) => {
                // Map beacon color to target
                let (target, world_pos) = match obj {
                    WorldObject::YellowBeacon | WorldObject::YellowBeaconBlock => {
                        (BeaconTarget::Portal, Vec2::ZERO)
                    }
                    WorldObject::RedBeacon | WorldObject::RedBeaconBlock => {
                        if let Some(tp) = game
                            .world_obj_cache
                            .unique_objs
                            .get(&WorldObject::DungeonEntrance)
                        {
                            (
                                BeaconTarget::DungeonEntrance,
                                world_helpers::tile_pos_to_world_pos(*tp, false),
                            )
                        } else {
                            (BeaconTarget::DungeonEntrance, Vec2::ZERO)
                        }
                    }
                    WorldObject::PinkBeacon | WorldObject::PinkBeaconBlock => {
                        if let Some(tp) = game
                            .world_obj_cache
                            .unique_objs
                            .get(&WorldObject::BossShrine)
                        {
                            (
                                BeaconTarget::BossShrine,
                                world_helpers::tile_pos_to_world_pos(*tp, false),
                            )
                        } else {
                            (BeaconTarget::BossShrine, Vec2::ZERO)
                        }
                    }
                    _ => (BeaconTarget::Portal, Vec2::ZERO),
                };

                // Ensure only one exists per target
                match target {
                    BeaconTarget::Portal => {
                        if let Some(e) = item_action_param.beacon_guidance.portal.take() {
                            commands.entity(e).despawn_recursive();
                        }
                    }
                    BeaconTarget::DungeonEntrance => {
                        if let Some(e) = item_action_param.beacon_guidance.dungeon.take() {
                            commands.entity(e).despawn_recursive();
                        }
                    }
                    BeaconTarget::BossShrine => {
                        if let Some(e) = item_action_param.beacon_guidance.boss.take() {
                            commands.entity(e).despawn_recursive();
                        }
                    }
                }

                let icon_e = spawn_screen_locked_icon_to_world_pos(
                    commands,
                    &game.graphics,
                    &item_action_param.asset_server,
                    obj.clone(),
                    world_pos,
                );
                commands.entity(icon_e).insert(BeaconGuidance(target));

                match target {
                    BeaconTarget::Portal => item_action_param.beacon_guidance.portal = Some(icon_e),
                    BeaconTarget::DungeonEntrance => {
                        item_action_param.beacon_guidance.dungeon = Some(icon_e)
                    }
                    BeaconTarget::BossShrine => {
                        item_action_param.beacon_guidance.boss = Some(icon_e)
                    }
                }
            }
            ObjectAction::CombatShrine => {
                // Screen Shake
                let mut rng = rand::thread_rng();
                let seed = rng.gen_range(0..100000);
                let speed = 10.;
                let max_mag = 120.;
                let noise = 0.5;
                let dir = Vec2::new(1., 1.);
                for e in item_action_param.game_camera.iter_mut() {
                    commands.entity(e).insert(ShakeEffect {
                        timer: Timer::from_seconds(4., TimerMode::Once),
                        speed,
                        seed,
                        max_mag,
                        noise,
                        dir,
                    });
                }
                let mut rng = rand::thread_rng();
                let num_days = 2 + item_action_param.night_tracker.days;
                let num_spawns_left = rng.gen_range(num_days..=(num_days + 2)) as usize;
                commands
                    .entity(e)
                    .insert(CombatShrine {
                        num_mobs_left: num_spawns_left,
                        tile_pos: obj_pos,
                    })
                    .insert(AsepriteAnimation::from(CombatShrineAnim::tags::ACTIVATE))
                    .remove::<InteractionGuideTrigger>()
                    .remove::<ObjectAction>();
            }
            ObjectAction::GambleShrine => {
                // Screen Shake
                let mut rng = rand::thread_rng();
                let seed = rng.gen_range(0..100000);
                let speed = 10.;
                let max_mag = 120.;
                let noise = 0.5;
                let dir = Vec2::new(1., 1.);
                for e in item_action_param.game_camera.iter_mut() {
                    commands.entity(e).insert(ShakeEffect {
                        timer: Timer::from_seconds(1.5, TimerMode::Once),
                        speed,
                        seed,
                        max_mag,
                        noise,
                        dir,
                    });
                }

                // Instantly go to ACTIVATE_SUCCESS animation
                commands
                    .entity(e)
                    .remove::<InteractionGuideTrigger>()
                    .remove::<ObjectAction>()
                    .insert(GambleShrine {
                        success: true,
                        tile_pos: obj_pos,
                    })
                    .insert(AsepriteAnimation::from(
                        GambleShrineAnim::tags::ACTIVATE_SUCCESS,
                    ));
            }
            ObjectAction::ActiveSkillShrine => {
                // Always re-validate the cached offer against the player's current
                // active skills so anything they picked up since the world rolled
                // this shrine (e.g. via another shrine) is filtered out, and any
                // gaps are topped up with fresh rolls.
                let cached_offer = game
                    .world_obj_cache
                    .active_skill_shrine_offers
                    .get(&obj_pos)
                    .cloned()
                    .unwrap_or_default();
                let offer_skills = refresh_active_skill_shrine_offer_skills(
                    &cached_offer,
                    item_action_param.player_skills.get_single().ok(),
                );

                if offer_skills != cached_offer {
                    if offer_skills.is_empty() {
                        game.world_obj_cache
                            .active_skill_shrine_offers
                            .remove(&obj_pos);
                    } else {
                        game.world_obj_cache
                            .active_skill_shrine_offers
                            .insert(obj_pos, offer_skills.clone());
                    }
                }

                let skill_choices = skill_choices_from_offer_skills(&offer_skills);

                commands.insert_resource(ActiveSkillShrineSelection {
                    skill_choices: skill_choices.clone(),
                    shrine_entity: e,
                });

                // Mark shrine as activated
                commands
                    .entity(e)
                    .remove::<ObjectAction>()
                    .remove::<InteractionGuideTrigger>()
                    .insert(ActiveSkillShrineState {
                        is_used: false,
                        tile_pos: obj_pos,
                    });

                // Open active skill shrine selection UI
                item_action_param
                    .next_inv_state
                    .set(UIState::ActiveSkillShrine);
            }
            ObjectAction::HeirloomShrine => {
                // Mark shrine as activated
                commands
                    .entity(e)
                    .remove::<ObjectAction>()
                    .remove::<InteractionGuideTrigger>()
                    .insert(HeirloomShrineState {
                        is_used: false,
                        tile_pos: obj_pos,
                    });

                // Open skills choice UI - will be populated by handle_heirloom_shrine_interaction
                item_action_param.next_inv_state.set(UIState::Skills);
            }
            ObjectAction::MicrowaveShrine => {
                commands.entity(e).insert(MicrowaveShrineState {
                    is_used: false,
                    tile_pos: obj_pos,
                });

                item_action_param
                    .next_inv_state
                    .set(UIState::MicrowaveShrine);
            }
            ObjectAction::WeaponShrine => {
                // Screen Shake
                let mut rng = rand::thread_rng();
                let seed = rng.gen_range(0..100000);
                let speed = 10.;
                let max_mag = 120.;
                let noise = 0.5;
                let dir = Vec2::new(1., 1.);
                for e in item_action_param.game_camera.iter_mut() {
                    commands.entity(e).insert(ShakeEffect {
                        timer: Timer::from_seconds(4., TimerMode::Once),
                        speed,
                        seed,
                        max_mag,
                        noise,
                        dir,
                    });
                }
                commands
                    .entity(e)
                    .insert(DungeonShrine {
                        shrine_type: DungeonShrineType::Weapon,
                        num_mobs_left: NUM_DUNGEON_SHRINE_MOBS,
                        is_cleared: false,
                        is_activated: false,
                        tile_pos: obj_pos,
                    })
                    .insert(AsepriteAnimation::from(CombatShrineAnim::tags::ACTIVATE))
                    .remove::<InteractionGuideTrigger>()
                    .remove::<ObjectAction>();

                // let proto_ref: &ProtoParam =
                //     unsafe { &*(proto_param as *mut ProtoParam as *const ProtoParam) };
                mark_other_dungeon_shrines_completed(commands, game, proto_param, e);
            }
            ObjectAction::ArmorShrine => {
                // Screen Shake
                let mut rng = rand::thread_rng();
                let seed = rng.gen_range(0..100000);
                let speed = 10.;
                let max_mag = 120.;
                let noise = 0.5;
                let dir = Vec2::new(1., 1.);
                for e in item_action_param.game_camera.iter_mut() {
                    commands.entity(e).insert(ShakeEffect {
                        timer: Timer::from_seconds(4., TimerMode::Once),
                        speed,
                        seed,
                        max_mag,
                        noise,
                        dir,
                    });
                }

                commands
                    .entity(e)
                    .insert(DungeonShrine {
                        shrine_type: DungeonShrineType::Armor,
                        num_mobs_left: NUM_DUNGEON_SHRINE_MOBS,
                        is_cleared: false,
                        is_activated: false,
                        tile_pos: obj_pos,
                    })
                    .insert(AsepriteAnimation::from(CombatShrineAnim::tags::ACTIVATE))
                    .remove::<InteractionGuideTrigger>()
                    .remove::<ObjectAction>();

                mark_other_dungeon_shrines_completed(commands, game, proto_param, e);
            }
            ObjectAction::AccessoryShrine => {
                // Screen Shake
                let mut rng = rand::thread_rng();
                let seed = rng.gen_range(0..100000);
                let speed = 10.;
                let max_mag = 120.;
                let noise = 0.5;
                let dir = Vec2::new(1., 1.);
                for e in item_action_param.game_camera.iter_mut() {
                    commands.entity(e).insert(ShakeEffect {
                        timer: Timer::from_seconds(4., TimerMode::Once),
                        speed,
                        seed,
                        max_mag,
                        noise,
                        dir,
                    });
                }

                commands
                    .entity(e)
                    .insert(DungeonShrine {
                        shrine_type: DungeonShrineType::Accessory,
                        num_mobs_left: NUM_DUNGEON_SHRINE_MOBS,
                        is_cleared: false,
                        is_activated: false,
                        tile_pos: obj_pos,
                    })
                    .insert(AsepriteAnimation::from(CombatShrineAnim::tags::ACTIVATE))
                    .remove::<InteractionGuideTrigger>()
                    .remove::<ObjectAction>();

                mark_other_dungeon_shrines_completed(commands, game, proto_param, e);
            }
            ObjectAction::IncreaseChaos(amount) => {
                item_action_param
                    .increase_chaos_event
                    .send(IncreaseChaosEvent { amount: *amount });
                let pos = tile_pos_to_world_pos(obj_pos, true);
                info!("{pos:?} {obj_pos:?}");

                // Update sprite to the "Done" variant
                let done_sprite = game
                    .graphics
                    .spritesheet_map
                    .as_ref()
                    .unwrap()
                    .get(&WorldObject::ChaosTotemDone)
                    .unwrap()
                    .clone();

                commands
                    .entity(e)
                    .insert(WorldObject::ChaosTotemDone)
                    .insert(done_sprite)
                    .remove::<InteractionGuideTrigger>()
                    .remove::<ObjectAction>();

                // Update the world object cache so the totem stays "Done" when chunk respawns
                game.add_object_to_chunk_cache(obj_pos, WorldObject::ChaosTotemDone);

                // Update minimap to reflect the totem is now "Done"
                item_action_param.minimap_event.send(UpdateMiniMapEvent {
                    pos: Some(obj_pos),
                    new_tile: Some(WorldObject::ChaosTotemDone),
                });

                if let Some(seen_tips) = item_action_param.seen_tips.as_ref() {
                    if !seen_tips.has_seen(&Tip::Chaos) {
                        item_action_param.tip_event.send(TipEvent {
                            tip: Tip::Chaos,
                            pos: Vec3::new(-184., -116., 55.),
                        });
                    }
                }

                let spawn_pos = pos + Vec2::new(0., -18.);
                // We need both mutable proto_commands and immutable proto_param.
                // Since spawn_item_from_proto only reads from proto_param, we can safely
                // create an immutable reference using a raw pointer cast.
                let proto_ref: &ProtoParam =
                    unsafe { &*(proto_param as *mut ProtoParam as *const ProtoParam) };
                proto_param.proto_commands.spawn_item_from_proto(
                    WorldObject::Coin,
                    proto_ref,
                    spawn_pos,
                    10,
                    None,
                );

                spawn_floating_text_with_shadow(
                    commands,
                    &item_action_param.asset_server,
                    pos.extend(game.player().position.z) + Vec3::new(0., 20., 0.),
                    RED,
                    format!("+{} Chaos", amount),
                    FLOATING_TEXT,
                );
            }
            ObjectAction::TimePortal => {
                if !*DEBUG {
                    let current_era = game.era.current_era.clone();
                    if current_era == Era::Third {
                        // In endless mode, entering the portal in era 3 kills the player (no next era)
                        if item_action_param.infinite_mode.active {
                            item_action_param
                                .modify_health_event
                                .send(ModifyHealthEvent(-9999999));
                        }
                        return;
                    }

                    if let Some(boss_kill_tracker) = item_action_param.boss_kill_tracker.as_ref() {
                        if !boss_kill_tracker.is_boss_killed(&current_era) {
                            let pos = tile_pos_to_world_pos(obj_pos, true);
                            spawn_floating_text_with_shadow(
                                commands,
                                &item_action_param.asset_server,
                                pos.extend(game.player().position.z) + Vec3::new(0., 28., 0.),
                                WHITE,
                                "A strong force prevents you...".to_string(),
                                FLOATING_TEXT,
                            );
                            return;
                        }
                    } else {
                        let pos = tile_pos_to_world_pos(obj_pos, true);
                        spawn_floating_text_with_shadow(
                            commands,
                            &item_action_param.asset_server,
                            pos.extend(game.player().position.z) + Vec3::new(0., 28., 0.),
                            WHITE,
                            "A strong force prevents you...".to_string(),
                            FLOATING_TEXT,
                        );
                        return;
                    }
                }

                // Determine next era based on current era
                let current_era = game.era.current_era.clone();
                let next_era = match current_era {
                    Era::Main => Some(Era::Second),
                    Era::Second => Some(Era::Third),
                    Era::Third => None,
                    Era::DungeonMain => None, // Shouldn't be able to use portal in dungeon
                };

                if let Some(era) = next_era {
                    item_action_param.dim_event.send(DimensionSpawnEvent {
                        swap_to_dim_now: true,
                        new_era: Some(era),
                    });
                }
            }
            _ => {}
        }
    }
}
impl TouchTriggerObjectAction {
    pub fn run_action(
        &self,
        entity: Entity,
        commands: &mut Commands,
        item_action_param: &mut ItemActionParam,
    ) {
        match self {
            TouchTriggerObjectAction::Bounce => {
                item_action_param.bounce_event.send(BounceEvent);
            }
            TouchTriggerObjectAction::ItemChest => {
                commands.insert_resource(ItemChestState::new_item_chest());
                commands.entity(entity).despawn_recursive();
            }
            TouchTriggerObjectAction::HeirloomChest => {
                commands.insert_resource(ItemChestState::new_heirloom_chest());
                commands.entity(entity).despawn_recursive();
            }
            _ => {}
        }
    }
}

fn mark_other_dungeon_shrines_completed(
    commands: &mut Commands,
    game: &mut GameParam,
    proto_param: &ProtoParam,
    activated_entity: Entity,
) {
    let mut shrine_updates: Vec<(Entity, WorldObject, TileMapPosition)> = Vec::new();

    for (entity, transform, _size, obj) in game.world_object_query.iter() {
        if entity == activated_entity {
            continue;
        }

        let done_object = match obj {
            WorldObject::WeaponShrine => Some(WorldObject::WeaponShrineDone),
            WorldObject::ArmorShrine => Some(WorldObject::ArmorShrineDone),
            WorldObject::AccessoryShrine => Some(WorldObject::AccessoryShrineDone),
            _ => None,
        };

        if let Some(done_obj) = done_object {
            let anchor = proto_param
                .get_component::<SpriteAnchor, _>(done_obj)
                .unwrap_or(&SpriteAnchor(Vec2::ZERO));
            let tile_pos = world_pos_to_tile_pos(transform.translation().truncate() - anchor.0);

            shrine_updates.push((entity, done_obj, tile_pos));
        }
    }

    for (entity, done_obj, tile_pos) in shrine_updates {
        commands
            .entity(entity)
            .insert(done_obj)
            .insert(AsepriteAnimation::from(CombatShrineAnim::tags::DONE))
            .remove::<ObjectAction>()
            .remove::<InteractionGuideTrigger>();

        game.add_object_to_chunk_cache(tile_pos, done_obj);
    }
}
