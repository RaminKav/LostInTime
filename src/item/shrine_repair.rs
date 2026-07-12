use bevy::prelude::*;
use bevy_aseprite::{anim::AsepriteAnimation, aseprite, AsepriteBundle};
use rand::Rng;

use crate::{
    assets::{Graphics, SpriteAnchor},
    client::ClientState,
    gamepad_input::GamepadAction,
    inventory::{Inventory, ItemStack},
    item::{
        object_actions::ObjectAction,
        shrine_visuals::{ShrineEye, ShrineEyeDoneVisual, ShrineEyeMarker, ShrineNeedsRepair},
        WorldObject,
    },
    keybinds::InputMappings,
    player::Player,
    proto::proto_param::ProtoParam,
    ui::key_input_guide::{InteractionGuideTrigger, SHRINE_INTERACT_GUIDE_DISTANCE},
    world::{
        chunk::Chunk,
        dimension::{Era, EraManager},
        generation::WorldObjectCache,
        world_helpers::world_pos_to_tile_pos,
        TileMapPosition,
    },
    GameParam,
};
use leafwing_input_manager::prelude::ActionState;

use super::item_actions::ItemActionParam;

aseprite!(pub ShrineRepairRingAnim, "textures/shrines/ring.aseprite");

/// Average material requirement for repairing a shrine. Actual cost rolls ±25%.
#[derive(Clone, Copy, Debug)]
pub struct ShrineRepairMaterial {
    pub item: WorldObject,
    pub average: u32,
}

/// Rolled repair requirements attached while a shrine is broken.
#[derive(Component, Clone, Debug)]
pub struct ShrineRepairCosts {
    pub materials: Vec<(WorldObject, u32)>,
}

/// Active channelled repair: Startup eye anim + looping ring; completes or cancels based on
/// whether the player stays inside the ring for the full Startup clip.
/// Active channelled repair: Startup eye anim + looping ring; completes or cancels based on
/// whether the player stays inside the ring for the full Startup clip.
#[derive(Component)]
pub struct ShrineRepairChannel {
    pub shrine_feet_pos: Vec2,
    pub ring_entity: Entity,
    /// Tracks eye frame progress so we can detect Startup finishing even when
    /// `just_finished` is skipped (last frame shorter than one tick — see bevy_aseprite).
    pub eye_prev_frame: usize,
    pub eye_seen_progress: bool,
}

/// Marker on the looping repair ring visual.
#[derive(Component)]
pub struct ShrineRepairRing;

/// After FlashRed / FlashGreen, restore the eye to this tag when the one-shot finishes.
#[derive(Component)]
pub struct ShrineEyeOneShot {
    pub return_to: &'static str,
    pub prev_frame: usize,
    pub seen_progress: bool,
}

impl ShrineEyeOneShot {
    fn new(return_to: &'static str) -> Self {
        Self {
            return_to,
            prev_frame: 0,
            seen_progress: false,
        }
    }
}

/// True when a Forward aseprite tag has completed one play-through.
/// `just_finished` alone can be missed if the last frame is shorter than a game tick
/// (animation wraps before cleanup runs) — same fix as player one-shot anims.
fn aseprite_tag_finished(
    anim: &AsepriteAnimation,
    prev_frame: &mut usize,
    seen_progress: &mut bool,
) -> bool {
    let current = anim.current_frame();
    let looped = *seen_progress && current < *prev_frame;
    if current != *prev_frame {
        *seen_progress = true;
    }
    *prev_frame = current;
    anim.just_finished() || looped
}

/// Paid + FlashGreen playing; activate the shrine only after FlashGreen finishes
/// (eye settles on Done for one-shots, Idle for repeatables).
#[derive(Component)]
pub struct PendingShrineRepairFinish {
    pub feet_pos: Vec2,
    pub obj: WorldObject,
}

/// True while channelled repair is in progress (Startup ring or FlashGreen → Done wait).
pub fn is_shrine_repairing(
    channel: Option<&ShrineRepairChannel>,
    pending: Option<&PendingShrineRepairFinish>,
) -> bool {
    channel.is_some() || pending.is_some()
}

/// Merchant / cauldron / well stay usable after repair — eye returns to Idle, not Done.
fn is_repeatable_shrine(obj: WorldObject) -> bool {
    matches!(
        obj,
        WorldObject::BlacksmithMerchant
            | WorldObject::CauldronShrine
            | WorldObject::WellShrine
    )
}

fn eye_tag_after_successful_repair(obj: WorldObject) -> &'static str {
    if is_repeatable_shrine(obj) {
        ShrineEye::tags::IDLE
    } else {
        ShrineEye::tags::DONE
    }
}

/// Diameter of the repair ring (player must stay inside this circle).
pub const SHRINE_REPAIR_RING_DIAMETER: f32 = 96.;
const SHRINE_REPAIR_RING_RADIUS: f32 = SHRINE_REPAIR_RING_DIAMETER * 0.5;
/// Parent-local Z: under the shrine sprite (local 0) / eye (+1), above grass patches (~-12).
/// Do not use [`YSort`] on the child — absolute depth stacked on the parent's Z draws above the shrine.
const SHRINE_REPAIR_RING_LOCAL_Z: f32 = -1.0;

pub const SHRINE_BROKEN_CHANCE: f64 = 0.30;
const REPAIR_COST_VARIANCE: f32 = 0.25;

pub fn repair_materials_for(obj: WorldObject) -> &'static [ShrineRepairMaterial] {
    match obj {
        WorldObject::GambleShrine => &[
            ShrineRepairMaterial {
                item: WorldObject::StoneChunk,
                average: 8,
            },
            ShrineRepairMaterial {
                item: WorldObject::Log,
                average: 20,
            },
        ],
        WorldObject::MicrowaveShrine => &[
            ShrineRepairMaterial {
                item: WorldObject::StoneChunk,
                average: 10,
            },
            ShrineRepairMaterial {
                item: WorldObject::Log,
                average: 18,
            },
        ],
        WorldObject::ActiveSkillShrine => &[
            ShrineRepairMaterial {
                item: WorldObject::Log,
                average: 24,
            },
            ShrineRepairMaterial {
                item: WorldObject::Stick,
                average: 6,
            },
            ShrineRepairMaterial {
                item: WorldObject::PlantFibre,
                average: 16,
            },
        ],
        WorldObject::BlacksmithMerchant => &[
            ShrineRepairMaterial {
                item: WorldObject::Log,
                average: 22,
            },
            ShrineRepairMaterial {
                item: WorldObject::Stick,
                average: 6,
            },
            ShrineRepairMaterial {
                item: WorldObject::PlantFibre,
                average: 16,
            },
        ],
        WorldObject::HeirloomShrine => &[
            ShrineRepairMaterial {
                item: WorldObject::StoneChunk,
                average: 12,
            },
            ShrineRepairMaterial {
                item: WorldObject::Log,
                average: 22,
            },
            ShrineRepairMaterial {
                item: WorldObject::Stick,
                average: 4,
            },
        ],
        WorldObject::CombatShrine => &[ShrineRepairMaterial {
            item: WorldObject::StoneChunk,
            average: 20,
        }],
        WorldObject::ChaosTotem => &[ShrineRepairMaterial {
            item: WorldObject::StoneChunk,
            average: 16,
        }],
        WorldObject::CauldronShrine => &[ShrineRepairMaterial {
            item: WorldObject::StoneChunk,
            average: 20,
        }],
        WorldObject::WellShrine => &[
            ShrineRepairMaterial {
                item: WorldObject::StoneChunk,
                average: 12,
            },
            ShrineRepairMaterial {
                item: WorldObject::PlantFibre,
                average: 8,
            },
            ShrineRepairMaterial {
                item: WorldObject::Stick,
                average: 10,
            },
        ],
        _ => &[],
    }
}

pub fn can_be_broken(obj: WorldObject) -> bool {
    !repair_materials_for(obj).is_empty()
}

/// Multiplier on rolled repair material amounts by era (Era1 / Main = 1x).
pub fn repair_cost_era_multiplier(era: &Era) -> f32 {
    match era {
        Era::Main | Era::DungeonMain => 1.0,
        Era::Second => 1.3,
        Era::Third => 2.0,
    }
}

pub fn roll_repair_costs(
    obj: WorldObject,
    era: &Era,
    rng: &mut impl Rng,
) -> Vec<(WorldObject, u32)> {
    let era_mult = repair_cost_era_multiplier(era);
    repair_materials_for(obj)
        .iter()
        .map(|entry| {
            let variance = 1.0 + rng.gen_range(-REPAIR_COST_VARIANCE..=REPAIR_COST_VARIANCE);
            let amount = ((entry.average as f32) * variance * era_mult)
                .round()
                .max(1.0) as u32;
            (entry.item, amount)
        })
        .collect()
}

/// Resolve absolute tile for a world object.
///
/// Placed objects use **chunk-local** [`Transform`]s (parented under [`Chunk`]). Treating that
/// local translation as a world position maps every chunk onto the same keys near chunk (0,0),
/// so shrines that share a tile offset steal each other's `broken_shrine_costs` entries
/// (e.g. Heirloom showing Combat's single StoneChunk cost).
pub fn tile_pos_for_placed_object(
    transform: &Transform,
    anchor: Option<&SpriteAnchor>,
    parent: Option<&Parent>,
    chunks: &Query<&Chunk>,
) -> TileMapPosition {
    let feet = transform.translation.truncate() - anchor.map(|a| a.0).unwrap_or(Vec2::ZERO);
    if let Some(parent) = parent {
        if let Ok(chunk) = chunks.get(parent.get()) {
            let local = world_pos_to_tile_pos(feet);
            return TileMapPosition::new(chunk.chunk_pos, local.tile_pos);
        }
    }
    world_pos_to_tile_pos(feet)
}

/// True when rolled costs use the same material items (and count) as this shrine's recipe.
fn costs_match_recipe(obj: WorldObject, costs: &[(WorldObject, u32)]) -> bool {
    let recipe = repair_materials_for(obj);
    costs.len() == recipe.len()
        && costs
            .iter()
            .zip(recipe.iter())
            .all(|((item, _), entry)| *item == entry.item)
}

/// Roll broken state for a newly placed shrine tile (pre-roll or first procedural spawn).
/// Empty vec in the cache means "rolled healthy" so chunk reloads do not re-roll.
pub fn maybe_mark_shrine_broken(
    cache: &mut WorldObjectCache,
    tile_pos: TileMapPosition,
    obj: WorldObject,
    era: &Era,
    rng: &mut impl Rng,
) {
    if !can_be_broken(obj) {
        return;
    }
    if cache.broken_shrine_costs.contains_key(&tile_pos) {
        return;
    }
    if rng.gen_bool(SHRINE_BROKEN_CHANCE) {
        cache
            .broken_shrine_costs
            .insert(tile_pos, roll_repair_costs(obj, era, rng));
    } else {
        cache.broken_shrine_costs.insert(tile_pos, Vec::new());
    }
}

fn repair_guide_trigger(costs: &[(WorldObject, u32)]) -> InteractionGuideTrigger {
    let icon = costs
        .first()
        .map(|(item, amount)| ItemStack::crate_icon_stack(*item).copy_with_count(*amount as usize));
    InteractionGuideTrigger {
        text: Some("Repair".to_string()),
        activation_distance: SHRINE_INTERACT_GUIDE_DISTANCE,
        icon_stack: icon,
    }
}

/// When a shrine entity spawns, attach broken state from the world cache (rolling for chaos if needed).
pub fn apply_broken_shrine_state_on_spawn(
    mut commands: Commands,
    new_shrines: Query<
        (
            Entity,
            &WorldObject,
            &Transform,
            Option<&SpriteAnchor>,
            Option<&Parent>,
        ),
        Added<WorldObject>,
    >,
    chunks: Query<&Chunk>,
    mut cache: ResMut<WorldObjectCache>,
    era: Res<EraManager>,
) {
    let mut rng = rand::thread_rng();
    for (entity, obj, transform, anchor, parent) in new_shrines.iter() {
        if !can_be_broken(*obj) {
            continue;
        }
        let tile_pos = tile_pos_for_placed_object(transform, anchor, parent, &chunks);

        // Drop polluted entries left by chunk-local tile lookups (wrong shrine recipe).
        if let Some(existing) = cache.broken_shrine_costs.get(&tile_pos) {
            if !existing.is_empty() && !costs_match_recipe(*obj, existing) {
                cache.broken_shrine_costs.remove(&tile_pos);
            }
        }

        if !cache.broken_shrine_costs.contains_key(&tile_pos) {
            maybe_mark_shrine_broken(&mut cache, tile_pos, *obj, &era.current_era, &mut rng);
        }

        let Some(costs) = cache.broken_shrine_costs.get(&tile_pos) else {
            continue;
        };
        if costs.is_empty() {
            continue;
        }

        commands.entity(entity).insert((
            ShrineNeedsRepair,
            ShrineRepairCosts {
                materials: costs.clone(),
            },
            repair_guide_trigger(costs),
        ));
    }
}

fn player_can_afford(inv: &Inventory, costs: &ShrineRepairCosts) -> bool {
    costs
        .materials
        .iter()
        .all(|(item, amount)| inv.items.get_item_count_in_container(*item) >= *amount as usize)
}

/// Pay costs and clear broken state. Returns false if the player cannot afford (no materials removed).
fn try_repair_shrine(
    commands: &mut Commands,
    inv: &mut Inventory,
    cache: &mut WorldObjectCache,
    shrine_entity: Entity,
    tile_pos: TileMapPosition,
    costs: &ShrineRepairCosts,
) -> bool {
    if !player_can_afford(inv, costs) {
        return false;
    }
    for (item, amount) in costs.materials.iter() {
        if let Err(err) = inv.items.remove_from_inventory(*amount as usize, *item) {
            error!("Failed to remove repair material {:?}: {:?}", item, err);
            return false;
        }
    }

    cache.broken_shrine_costs.insert(tile_pos, Vec::new());
    commands
        .entity(shrine_entity)
        .remove::<ShrineNeedsRepair>()
        .remove::<ShrineRepairCosts>()
        .remove::<ShrineRepairChannel>();

    true
}

fn set_eye_animation(
    commands: &mut Commands,
    children: Option<&Children>,
    eye_markers: &Query<(), With<ShrineEyeMarker>>,
    tag: &'static str,
    one_shot_return: Option<&'static str>,
) {
    let Some(children) = children else {
        return;
    };
    for child in children.iter() {
        if eye_markers.get(*child).is_err() {
            continue;
        }
        let Some(mut eye_commands) = commands.get_entity(*child) else {
            continue;
        };
        eye_commands.insert(AsepriteAnimation::from(tag));
        if let Some(return_to) = one_shot_return {
            eye_commands.insert(ShrineEyeOneShot::new(return_to));
        } else {
            eye_commands.remove::<ShrineEyeOneShot>();
        }
        if tag == ShrineEye::tags::DONE {
            eye_commands.insert(ShrineEyeDoneVisual);
        } else {
            eye_commands.remove::<ShrineEyeDoneVisual>();
        }
        break;
    }
}

fn despawn_repair_ring(commands: &mut Commands, ring_entity: Entity) {
    if let Some(entity_commands) = commands.get_entity(ring_entity) {
        entity_commands.despawn_recursive();
    }
}

/// Intercept interact on broken shrines: start a channelled repair if the player can afford it.
pub fn handle_broken_shrine_interact(
    mut commands: Commands,
    broken: Query<
        (
            Entity,
            &GlobalTransform,
            &WorldObject,
            &SpriteAnchor,
            &ShrineRepairCosts,
            Option<&Children>,
            Option<&ShrineRepairChannel>,
        ),
        With<ShrineNeedsRepair>,
    >,
    player_query: Query<(&GlobalTransform, &Inventory), With<Player>>,
    eye_markers: Query<(), With<ShrineEyeMarker>>,
    graphics: Res<Graphics>,
    key_input: Res<Input<KeyCode>>,
    mouse_input: Res<Input<MouseButton>>,
    keybinds: Res<InputMappings>,
    gamepad_action_q: Query<&ActionState<GamepadAction>, With<Player>>,
) {
    let gamepad_pressed = gamepad_action_q
        .get_single()
        .map(|a| a.just_pressed(GamepadAction::Interact))
        .unwrap_or(false);
    if !keybinds.check_interact_input(&key_input, &mouse_input) && !gamepad_pressed {
        return;
    }

    let Ok((player_t, inv)) = player_query.get_single() else {
        return;
    };
    let player_pos = player_t.translation().truncate();
    let Some(ring_handle) = graphics.shrine_repair_ring.as_ref() else {
        return;
    };

    for (entity, transform, _obj, anchor, costs, children, existing_channel) in broken.iter() {
        let feet_pos = transform.translation().truncate() - anchor.0;
        if feet_pos.distance(player_pos) > SHRINE_INTERACT_GUIDE_DISTANCE {
            continue;
        }
        // Consume interact even if already channeling / cannot afford.
        if existing_channel.is_some() || !player_can_afford(inv, costs) {
            return;
        }

        set_eye_animation(
            &mut commands,
            children,
            &eye_markers,
            ShrineEye::tags::STARTUP,
            None,
        );

        let ring_entity = commands
            .spawn((
                ShrineRepairRing,
                AsepriteBundle {
                    aseprite: ring_handle.clone(),
                    animation: AsepriteAnimation::from(ShrineRepairRingAnim::tags::RING),
                    transform: Transform::from_translation(Vec3::new(
                        -anchor.0.x,
                        -anchor.0.y,
                        SHRINE_REPAIR_RING_LOCAL_Z,
                    )),
                    ..default()
                },
                Name::new("ShrineRepairRing"),
            ))
            .id();
        commands.entity(entity).add_child(ring_entity);
        commands.entity(entity).insert(ShrineRepairChannel {
            shrine_feet_pos: feet_pos,
            ring_entity,
            eye_prev_frame: 0,
            eye_seen_progress: false,
        });
        return;
    }
}

/// Tick active repairs: leave the ring → FlashRed fail; Startup finishes while inside → pay + FlashGreen.
pub fn tick_shrine_repair_channel(
    mut commands: Commands,
    mut channels: Query<(
        Entity,
        &mut ShrineRepairChannel,
        &ShrineRepairCosts,
        &WorldObject,
        Option<&Children>,
    )>,
    eye_anims: Query<&AsepriteAnimation, With<ShrineEyeMarker>>,
    eye_markers: Query<(), With<ShrineEyeMarker>>,
    mut player_query: Query<(&GlobalTransform, &mut Inventory), With<Player>>,
    mut game: GameParam,
) {
    let Ok((player_t, mut inv)) = player_query.get_single_mut() else {
        return;
    };
    let player_pos = player_t.translation().truncate();

    for (shrine_e, mut channel, costs, obj, children) in channels.iter_mut() {
        let left_circle = player_pos.distance(channel.shrine_feet_pos) > SHRINE_REPAIR_RING_RADIUS;

        let mut startup_finished = false;
        if let Some(children) = children {
            for child in children.iter() {
                if eye_markers.get(*child).is_err() {
                    continue;
                }
                if let Ok(anim) = eye_anims.get(*child) {
                    let mut prev = channel.eye_prev_frame;
                    let mut seen = channel.eye_seen_progress;
                    startup_finished = aseprite_tag_finished(anim, &mut prev, &mut seen);
                    channel.eye_prev_frame = prev;
                    channel.eye_seen_progress = seen;
                }
                break;
            }
        }

        if left_circle {
            despawn_repair_ring(&mut commands, channel.ring_entity);
            commands.entity(shrine_e).remove::<ShrineRepairChannel>();
            set_eye_animation(
                &mut commands,
                children,
                &eye_markers,
                ShrineEye::tags::FLASH_RED,
                Some(ShrineEye::tags::BROKEN),
            );
            continue;
        }

        if !startup_finished {
            continue;
        }

        let tile_pos = world_pos_to_tile_pos(channel.shrine_feet_pos);
        if !try_repair_shrine(
            &mut commands,
            &mut inv,
            &mut game.world_obj_cache,
            shrine_e,
            tile_pos,
            costs,
        ) {
            // Could not pay (inventory changed mid-channel) — treat as fail.
            despawn_repair_ring(&mut commands, channel.ring_entity);
            commands.entity(shrine_e).remove::<ShrineRepairChannel>();
            set_eye_animation(
                &mut commands,
                children,
                &eye_markers,
                ShrineEye::tags::FLASH_RED,
                Some(ShrineEye::tags::BROKEN),
            );
            continue;
        }

        despawn_repair_ring(&mut commands, channel.ring_entity);
        set_eye_animation(
            &mut commands,
            children,
            &eye_markers,
            ShrineEye::tags::FLASH_GREEN,
            Some(eye_tag_after_successful_repair(*obj)),
        );
        // Activate only after FlashGreen completes (Done for one-shots, Idle for repeatables).
        commands.entity(shrine_e).insert(PendingShrineRepairFinish {
            feet_pos: channel.shrine_feet_pos,
            obj: *obj,
        });
    }
}

/// Freeze Startup / FlashGreen / ring clips while the client is paused so repair
/// does not advance on wall-clock time (bevy_aseprite still ticks otherwise).
pub fn sync_shrine_repair_anim_pause(
    client_state: Res<State<ClientState>>,
    repairing: Query<&Children, Or<(With<ShrineRepairChannel>, With<PendingShrineRepairFinish>)>>,
    mut anims: Query<&mut AsepriteAnimation>,
    rings: Query<(), With<ShrineRepairRing>>,
    eyes: Query<(), With<ShrineEyeMarker>>,
) {
    let playing = client_state.0 == ClientState::Unpaused;
    for children in repairing.iter() {
        for child in children.iter() {
            if rings.get(*child).is_err() && eyes.get(*child).is_err() {
                continue;
            }
            let Ok(mut anim) = anims.get_mut(*child) else {
                continue;
            };
            if playing {
                anim.play();
            } else {
                anim.pause();
            }
        }
    }
}

/// Opens the blacksmith shop after a successful channelled repair (merchant has no ObjectAction).
#[derive(Component)]
pub struct PendingMerchantOpenAfterRepair;

pub fn open_merchant_after_repair(
    mut commands: Commands,
    pending: Query<(Entity, &crate::ui::EssenceShopChoices), With<PendingMerchantOpenAfterRepair>>,
    mut next_inv_state: ResMut<NextState<crate::ui::UIState>>,
) {
    for (entity, choices) in pending.iter() {
        commands.insert_resource(choices.clone());
        commands.insert_resource(crate::ui::MerchantShopOpenLock(Timer::from_seconds(
            0.45,
            TimerMode::Once,
        )));
        next_inv_state.set(crate::ui::UIState::Essence);
        commands
            .entity(entity)
            .remove::<PendingMerchantOpenAfterRepair>();
    }
}

/// After FlashRed / FlashGreen finishes, restore the eye to Broken / Done.
/// On successful repair (PendingShrineRepairFinish), activate the shrine after Done is set.
pub fn finish_shrine_eye_one_shots(
    mut commands: Commands,
    mut eyes: Query<
        (
            Entity,
            &Parent,
            &mut AsepriteAnimation,
            &mut ShrineEyeOneShot,
        ),
        With<ShrineEyeMarker>,
    >,
) {
    for (entity, _parent, mut anim, mut one_shot) in eyes.iter_mut() {
        let mut prev = one_shot.prev_frame;
        let mut seen = one_shot.seen_progress;
        if !aseprite_tag_finished(&anim, &mut prev, &mut seen) {
            one_shot.prev_frame = prev;
            one_shot.seen_progress = seen;
            continue;
        }
        let return_to = one_shot.return_to;
        *anim = AsepriteAnimation::from(return_to);
        if return_to == ShrineEye::tags::DONE {
            commands.entity(entity).insert(ShrineEyeDoneVisual);
        } else {
            commands.entity(entity).remove::<ShrineEyeDoneVisual>();
        }
        commands.entity(entity).remove::<ShrineEyeOneShot>();
    }
}

/// After FlashGreen → Done, run the shrine's interact action (chaos, combat activate, etc.).
pub fn activate_pending_shrine_after_repair(
    mut commands: Commands,
    pending: Query<(Entity, &PendingShrineRepairFinish)>,
    eye_one_shots: Query<(), With<ShrineEyeOneShot>>,
    children_q: Query<&Children>,
    object_actions: Query<&ObjectAction>,
    mut player_query: Query<&mut Inventory, With<Player>>,
    mut game: GameParam,
    mut proto_param: ProtoParam,
    mut item_action_param: ItemActionParam,
) {
    let Ok(mut inv) = player_query.get_single_mut() else {
        return;
    };

    for (shrine_e, pending_finish) in pending.iter() {
        // Wait until FlashGreen one-shot has completed (eye no longer has ShrineEyeOneShot).
        if let Ok(children) = children_q.get(shrine_e) {
            if children
                .iter()
                .any(|child| eye_one_shots.get(*child).is_ok())
            {
                continue;
            }
        }

        let feet = pending_finish.feet_pos;
        let obj = pending_finish.obj;
        commands
            .entity(shrine_e)
            .remove::<PendingShrineRepairFinish>();
        restore_shrine_guide_after_repair(&mut commands, shrine_e, obj);

        let action = object_actions
            .get(shrine_e)
            .ok()
            .cloned()
            .or_else(|| proto_param.get_component::<ObjectAction, _>(obj).cloned());
        if let Some(action) = action {
            action.run_action(
                shrine_e,
                world_pos_to_tile_pos(feet),
                obj,
                &mut game,
                &mut item_action_param,
                &mut commands,
                &mut proto_param,
                &mut inv,
            );
        } else if obj == WorldObject::BlacksmithMerchant {
            commands
                .entity(shrine_e)
                .insert(PendingMerchantOpenAfterRepair);
        }
    }
}

fn restore_shrine_guide_after_repair(commands: &mut Commands, entity: Entity, obj: WorldObject) {
    let guide = match obj {
        WorldObject::CombatShrine => InteractionGuideTrigger {
            text: Some("Activate Shrine".to_string()),
            activation_distance: SHRINE_INTERACT_GUIDE_DISTANCE,
            icon_stack: Some(ItemStack::crate_icon_stack(WorldObject::ChestBlock)),
        },
        WorldObject::GambleShrine => InteractionGuideTrigger {
            text: Some("Activate Shrine".to_string()),
            activation_distance: SHRINE_INTERACT_GUIDE_DISTANCE,
            icon_stack: None,
        },
        WorldObject::BlacksmithMerchant => InteractionGuideTrigger {
            text: Some("Activate Shrine".to_string()),
            activation_distance: SHRINE_INTERACT_GUIDE_DISTANCE,
            icon_stack: Some(ItemStack::crate_icon_stack(WorldObject::Coin)),
        },
        WorldObject::ActiveSkillShrine => InteractionGuideTrigger {
            text: Some("Activate Shrine".to_string()),
            activation_distance: SHRINE_INTERACT_GUIDE_DISTANCE,
            icon_stack: None,
        },
        WorldObject::HeirloomShrine => InteractionGuideTrigger {
            text: Some("Activate Shrine".to_string()),
            activation_distance: SHRINE_INTERACT_GUIDE_DISTANCE,
            icon_stack: None,
        },
        WorldObject::MicrowaveShrine => InteractionGuideTrigger {
            text: Some("Activate Shrine".to_string()),
            activation_distance: SHRINE_INTERACT_GUIDE_DISTANCE,
            icon_stack: None,
        },
        WorldObject::WellShrine => InteractionGuideTrigger {
            text: Some("Salvage Equipment".to_string()),
            activation_distance: SHRINE_INTERACT_GUIDE_DISTANCE,
            icon_stack: None,
        },
        WorldObject::CauldronShrine => InteractionGuideTrigger {
            text: Some("Brew".to_string()),
            activation_distance: SHRINE_INTERACT_GUIDE_DISTANCE,
            icon_stack: None,
        },
        WorldObject::ChaosTotem => InteractionGuideTrigger {
            text: Some("Activate Shrine".to_string()),
            activation_distance: SHRINE_INTERACT_GUIDE_DISTANCE,
            icon_stack: None,
        },
        _ => {
            commands.entity(entity).remove::<InteractionGuideTrigger>();
            return;
        }
    };
    commands.entity(entity).insert(guide);
}
