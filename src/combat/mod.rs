use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use bevy_aseprite::anim::AsepriteAnimation;
use bevy_aseprite::Aseprite;
use bevy_proto::prelude::ProtoCommands;
use bevy_rapier2d::prelude::Collider;
use combat_helpers::{
    handle_deferred_aseprite_spawns, spawn_one_time_aseprite_collider, tick_despawn_timer,
};
use rand::{seq::SliceRandom, Rng};
pub mod status_effects;
use status_effects::*;

pub mod collisions;
pub mod damage_tracker;
pub mod pickup_radius;
use crate::attributes::{add_item_glows, CurrentMana, Lifesteal, ProjectileSize};
use crate::NO_DROPS;

pub mod combat_helpers;
use crate::blessings::OwnedBlessings;
use crate::enemy::EliteMob;
use crate::night::InfiniteMode;
use crate::player::melee_skills::{
    spawn_delayed_heirloom_cast, spawn_echo_hitbox, DelayedCastType, HEIRLOOM_EXTRA_CAST_DELAY,
};
use crate::{
    ai::{FollowState, LeapAttackState},
    animations::{AttackEvent, HitAnimationTracker},
    assets::{Graphics, SpriteAnchor},
    attributes::{
        modifiers::{ModifyHealthEvent, ModifyManaEvent},
        Attack, AttackCooldown, AttributeChangeEvent, CurrentHealth, CurrentShield,
        InvincibilityCooldown, ManaRegen, MaxHealth, ShieldRegen,
    },
    audio::{AudioSoundEffect, SoundSpawner},
    client::{
        analytics::{AnalyticsTrigger, AnalyticsUpdateEvent},
        is_not_paused,
    },
    custom_commands::CommandsExt,
    enemy::{
        red_mushking::{AoEAttackState, DeathState, ReturnToShrineState, SummonAttackState},
        stone_golem::{SpikeAttackState, SpikeWarning},
        Mob, MobLevel,
    },
    inventory::Inventory,
    item::projectile::RangedAttackEvent,
    item::{
        combat_shrine::{CombatShrineMob, CombatShrineMobDeathEvent},
        dungeon_shrine::{DungeonShrineMob, DungeonShrineMobDeathEvent},
        projectile::{AnimVisualCategory, Projectile},
        EquipmentType, LootTable, LootTablePlugin, MainHand, RequiredEquipmentType, WorldObject,
    },
    juice::bounce::BounceOnHit,
    player::{
        combat_heirlooms::{HallucinationStatType, HallucinationStats},
        levels::PlayerLevel,
        mage_skills::{spawn_ice_explosion_hitbox, IceExplosionDmg, IceFloor},
        skills::{Heirloom, HeirloomTriggerCounts, PlayerSkills},
    },
    proto::proto_param::ProtoParam,
    ui::{
        damage_numbers::spawn_floating_text_with_shadow, game_fonts::FLOATING_TEXT, CheatSettings,
    },
    world::{world_helpers::world_pos_to_tile_pos, y_sort::YSort, TileMapPosition, TILE_SIZE},
    CustomFlush, GameParam, GameState, Player, SlimeTempShield, SlimeTempShieldSprite, DEBUG,
};

use self::collisions::CollisionPlugion;

#[derive(Debug, Clone)]
pub struct HitEvent {
    pub hit_entity: Entity,
    pub damage: i32,
    pub dir: Vec2,
    pub hit_with_melee: Option<WorldObject>,
    pub hit_with_projectile: Option<Projectile>,
    pub hit_by_mob: Option<Mob>,
    pub hit_by_pet: Option<Entity>,
    pub was_crit: bool,
    /// True if crit chance was >100% and second crit roll succeeded (overcrit does 30% extra damage)
    pub was_overcrit: bool,
    pub ignore_tool: bool,
    /// If Some, this damage came from a specific heirloom effect and should not trigger other heirloom effects
    pub from_heirloom_effect: Option<Heirloom>,
}

/// Brief marker set on a mob at the moment of death so follow-up systems can
/// react before the entity is despawned. Stored as `SparseSet` because it is
/// inserted on every kill and removed/despawned immediately after — archetype
/// moves here would otherwise churn every combat archetype every kill.
#[derive(Component, Debug, Clone)]
#[component(storage = "SparseSet")]
pub struct MarkedForDeath;

/// Transient marker set on an enemy killed by an heirloom-effect damage source
/// (used to suppress on-kill heirloom loops). Same churn profile as
/// `MarkedForDeath`, so also stored as `SparseSet`.
#[derive(Component, Debug, Clone)]
#[component(storage = "SparseSet")]
pub struct KilledByHeirloomEffect;
#[derive(Debug, Clone)]

pub struct EnemyDeathEvent {
    pub entity: Entity,
    pub enemy_pos: Vec2,
    pub killed_by_crit: bool,
    pub mob: Mob,
}
#[derive(Debug, Clone)]

pub struct ObjBreakEvent {
    pub entity: Entity,
    pub obj: WorldObject,
    pub pos: TileMapPosition,
    pub give_drops_and_xp: bool,
}

/// Bundles `EventWriter`s for [`handle_hits`] so the system stays within Bevy's `SystemParam` tuple limit.
#[derive(SystemParam)]
pub struct HitOutcomeEvents<'w> {
    pub enemy_death: EventWriter<'w, EnemyDeathEvent>,
    pub combat_shrine_mob_death: EventWriter<'w, CombatShrineMobDeathEvent>,
    pub dungeon_shrine_mob_death: EventWriter<'w, DungeonShrineMobDeathEvent>,
    pub obj_break: EventWriter<'w, ObjBreakEvent>,
    pub analytics: EventWriter<'w, AnalyticsUpdateEvent>,
    pub attribute_change: EventWriter<'w, AttributeChangeEvent>,
}

/// Event to trigger lifesteal calculation and healing
/// `thorns_lifesteal_stacks` should be the number of ThornsLifesteal heirloom stacks (0 if not thorns damage)
#[derive(Debug, Clone)]
pub struct LifestealEvent {
    pub thorns_lifesteal_stacks: i32,
    /// True only for direct player→mob hits from [`crate::item::item_upgrades::handle_on_hit_upgrades`]
    /// (non-[`HitEvent::from_heirloom_effect`]). False for thorns-only lifesteal procs.
    pub is_direct_player_damage: bool,
}

/// Flags set on an entity by `calculate_player_damage` so that
/// `handle_add_damage_numbers_after_hit` can render yellow/orange crit
/// numbers on the next `Changed<CurrentHealth>` tick.
///
/// Always-present on mobs (see `ensure_mob_components`). Inserting
/// `WasHitWithCrit(true)` on a mob is a value update and causes no archetype
/// change. After the damage-number system reads the flag it sets it back to
/// `false` (rather than removing the component), so the entity never leaves
/// its current archetype. Non-mob entities (world objects the player crits
/// on) may still have the component inserted on demand — it stays thereafter.
#[derive(Component, Debug, Default)]
pub struct WasHitWithCrit(pub bool);

#[derive(Component, Debug, Default)]
pub struct WasHitWithOvercrit(pub bool);

/// Per-attack cooldown timer on the player. Inserted when an attack fires and
/// removed the frame the timer finishes — this cycle happens several times a
/// second in combat, so `SparseSet` storage avoids moving the player entity
/// through two archetype variants per swing.
#[derive(Component, Debug, Clone)]
#[component(storage = "SparseSet")]
pub struct AttackTimer(pub Timer);

/// Per-hit i-frame timer. Inserted on any entity that takes damage and removed
/// when the timer finishes — very high churn across mobs and the player, so
/// `SparseSet` storage keeps the entity in its original archetype.
#[derive(Component, Debug, Clone)]
#[component(storage = "SparseSet")]
pub struct InvincibilityTimer(pub Timer);

/// Marker set on the currently-swinging tool entity; removed on the next
/// attack cooldown reset. `SparseSet` because it toggles every attack.
#[derive(Component, Debug, Clone)]
#[component(storage = "SparseSet")]
pub struct HitMarker;
pub struct CombatPlugin;
impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        // NOTE: These events are intentionally registered on the default (Main/Update) schedule
        // rather than FixedUpdate. Their writers (e.g. `handle_hits`) and all current readers
        // (`handle_enemy_death`, `track_mob_kills`, currency/portal/particles/heirloom hooks, etc.)
        // run on Update. Registering the events on FixedUpdate would attach
        // `Events::<T>::update_system` (the double-buffer rotation) to FixedUpdate; when FPS drops
        // below ~30 the FixedUpdate catch-up loop runs that rotation multiple times per render
        // frame and silently drops events that Update-side readers hadn't observed yet. This
        // caused mob-kill scoring (and other on-kill effects that weren't strictly ordered
        // `.after(handle_hits)`) to be lost during late-game slowdown.
        app.add_event::<HitEvent>()
            .add_event::<EnemyDeathEvent>()
            .add_event::<StatusEffectEvent>()
            .add_event::<LifestealEvent>()
            .add_event::<ObjBreakEvent>()
            .init_resource::<damage_tracker::DamageTracker>()
            .init_resource::<damage_tracker::MobStatTracker>()
            .init_resource::<damage_tracker::PetAbilityStats>()
            .add_plugin(CollisionPlugion)
            .add_systems(
                (
                    pickup_radius::update_pickup_radius.run_if(is_not_paused),
                    pickup_radius::mark_items_in_pickup_range.run_if(is_not_paused),
                    pickup_radius::handle_item_pickup_radius.run_if(is_not_paused),
                    pickup_radius::handle_magnet_pull.run_if(is_not_paused),
                )
                    .in_set(OnUpdate(GameState::Main))
                    .chain()
                    .before(collisions::check_item_drop_collisions),
            )
            // Process deferred Aseprite spawns in PreUpdate to ensure frame 0 initialization
            .add_system(
                handle_deferred_aseprite_spawns
                    .in_base_set(CoreSet::PreUpdate)
                    .run_if(in_state(GameState::Main)),
            )
            .add_systems(
                (
                    handle_hits,
                    tick_despawn_timer,
                    cleanup_marked_for_death_entities.after(handle_enemy_death),
                    handle_attack_cooldowns
                        .before(CustomFlush)
                        .run_if(is_not_paused),
                    update_status_effect_icons,
                    handle_new_status_effect_event,
                    // spawn_hit_spark_effect.after(handle_hits),
                    handle_invincibility_frames.after(handle_hits),
                    handle_enemy_death.after(handle_hits),
                    handle_lifesteal,
                    handle_thorns_on_damage_tracker.after(handle_hits),
                    damage_tracker::track_player_damage,
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(apply_system_buffers.in_set(CustomFlush))
            .add_system(update_anim_visibility.in_set(OnUpdate(GameState::Main)));
    }
}

pub fn update_anim_visibility(
    settings: Res<CheatSettings>,
    mut all: Query<(&AnimVisualCategory, &mut Visibility)>,
    added: Query<Entity, Added<AnimVisualCategory>>,
) {
    if settings.is_changed() {
        for (category, mut vis) in all.iter_mut() {
            let should_hide = match category {
                AnimVisualCategory::Attack => settings.hide_attack_anims,
                AnimVisualCategory::Skill => settings.hide_skill_anims,
                AnimVisualCategory::Heirloom => settings.hide_heirloom_anims,
            };
            *vis = if should_hide {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            };
        }
    } else if !added.is_empty() {
        for e in added.iter() {
            if let Ok((category, mut vis)) = all.get_mut(e) {
                let should_hide = match category {
                    AnimVisualCategory::Attack => settings.hide_attack_anims,
                    AnimVisualCategory::Skill => settings.hide_skill_anims,
                    AnimVisualCategory::Heirloom => settings.hide_heirloom_anims,
                };
                *vis = if should_hide {
                    Visibility::Hidden
                } else {
                    Visibility::Inherited
                };
            }
        }
    }
}

pub fn handle_attack_cooldowns(
    mut commands: Commands,
    time: Res<Time>,
    tool_query: Query<Entity, With<MainHand>>,
    mut attack_event: EventReader<AttackEvent>,
    mut player: Query<(Entity, &AttackCooldown, Option<&mut AttackTimer>), With<Player>>,
) {
    let (player_e, cooldown, timer_option) = player.single_mut();

    if !attack_event.is_empty() && timer_option.is_none() {
        if !attack_event.iter().next().unwrap().ignore_cooldown {
            let mut attack_cd_timer = AttackTimer(Timer::from_seconds(cooldown.0, TimerMode::Once));
            attack_cd_timer.0.tick(time.delta());
            commands.entity(player_e).insert(attack_cd_timer);
        }
        if let Ok(tool) = tool_query.get_single() {
            commands.entity(tool).remove::<HitMarker>();
        }
    }
    if let Some(mut t) = timer_option {
        t.0.tick(time.delta());
        if t.0.finished() {
            commands.entity(player_e).remove::<AttackTimer>();
        }
    }
}
fn handle_enemy_death(
    proto_param: ProtoParam,
    mut death_events: EventReader<EnemyDeathEvent>,
    loot_tables: Query<&LootTable>,
    mob_data: Query<(&Mob, &MobLevel, Option<&EliteMob>)>,
    mut player_xp: Query<(&PlayerLevel, &PlayerSkills, &OwnedBlessings)>,
    mut proto_commands: ProtoCommands,
    mut commands: Commands,
    graphics: Res<Graphics>,
    infinite_mode: Res<InfiniteMode>,
    enemies: Query<(Entity, &GlobalTransform), (With<Mob>, Without<Player>)>,
    mut player_query: Query<(&GlobalTransform, &Attack, &mut CurrentMana), With<Player>>,
    mut ranged_attack_event: EventWriter<RangedAttackEvent>,
    mut trigger_counts: ResMut<HeirloomTriggerCounts>,
    asset_server: Res<AssetServer>,
) {
    for death_event in death_events.iter() {
        let Ok((mob, mob_lvl, elite_option)) = mob_data.get(death_event.entity) else {
            continue;
        };
        let (player_level, player_skills, blessings) = player_xp.single_mut();
        let is_infinite_mode = infinite_mode.active;

        let has_double_gold = blessings.has_double_gold_drops();

        // drop loot
        if !*NO_DROPS {
            if let Ok(loot_table) = loot_tables.get(death_event.entity) {
                for drop in LootTablePlugin::get_drops(
                    loot_table,
                    &proto_param,
                    // loot_bonus.single().0,
                    0,
                    Some(mob_lvl.0),
                    is_infinite_mode,
                    mob.is_boss(),
                )
                .iter()
                .filter(|d| {
                    if elite_option.is_some() && d.obj_type == WorldObject::XPShard {
                        return false;
                    }
                    if is_infinite_mode {
                        return matches!(
                            d.obj_type,
                            WorldObject::ChestBlock
                                | WorldObject::Coin
                                | WorldObject::XPShard
                                | WorldObject::XPShardMedium
                                | WorldObject::XPShardLarge
                        );
                    }
                    true
                })
                .collect::<Vec<_>>()
                {
                    let count = if drop.obj_type == WorldObject::Coin && has_double_gold {
                        2
                    } else {
                        1
                    };
                    for _ in 0..count {
                        let mut rng = rand::thread_rng();
                        let d = if mob.is_boss() { 30. } else { 10. };
                        let drop_offset = Vec2::new(rng.gen_range(-d..d), rng.gen_range(-d..d));
                        let drop_e = proto_commands.spawn_item_from_proto(
                            drop.obj_type,
                            &proto_param,
                            death_event.enemy_pos + drop_offset,
                            drop.count,
                            Some(player_level.level),
                        );

                        if let Some(drop_e) = drop_e {
                            add_item_glows(&mut commands, &graphics, drop_e, drop.rarity.clone());
                            commands
                                .entity(drop_e)
                                .insert(crate::item::ItemDropDespawnTimer(Timer::from_seconds(
                                    300.0,
                                    TimerMode::Once,
                                )));
                        }
                    }
                }
            }
        }

        if elite_option.is_some() {
            let mut rng = rand::thread_rng();
            let d = if mob.is_boss() { 30. } else { 10. };
            let drop_offset = Vec2::new(rng.gen_range(-d..d), rng.gen_range(-d..d));
            let _xp_shard_e = proto_commands.spawn_item_from_proto(
                WorldObject::XPShardMedium,
                &proto_param,
                death_event.enemy_pos + drop_offset,
                1,
                Some(player_level.level),
            );
        }

        // On-kill heirloom effects (skip when kill was from another heirloom to prevent chaining)
        let (_, attack, mut current_mana) = player_query.single_mut();
        // KillLightning: 15% chance per stack to spawn lightning on a random nearby enemy; over 100% = guaranteed 1 + (chance-100)% for a second strike on a different enemy
        let kill_lightning_stacks =
            player_skills.get_count(crate::player::skills::Heirloom::KillLightning);
        if kill_lightning_stacks > 0 {
            let mut rng = rand::thread_rng();
            let chance_pct = (kill_lightning_stacks as u32 * 15).min(200); // cap at 200% (1 guaranteed + 100% second)
            let death_pos = death_event.enemy_pos;
            let nearby_enemies: Vec<(Entity, Vec2)> = enemies
                .iter()
                .filter_map(|(e, txfm)| {
                    let enemy_pos = txfm.translation().truncate();
                    let distance = death_pos.distance(enemy_pos);
                    if distance <= 200.0 && distance > 0.0 {
                        Some((e, enemy_pos))
                    } else {
                        None
                    }
                })
                .collect();

            if !nearby_enemies.is_empty() {
                let mut num_procs = 0u32;
                if chance_pct >= 100 {
                    num_procs = 1;
                    if chance_pct > 100 {
                        let extra_pct = (chance_pct - 100).min(100);
                        if rng.gen_ratio(extra_pct, 100) {
                            num_procs = 2;
                        }
                    }
                } else if rng.gen_ratio(chance_pct, 100) {
                    num_procs = 1;
                }

                let lightning_damage = attack.0;
                let mut chosen: Vec<usize> = Vec::new();
                for _ in 0..num_procs {
                    // Prefer a different enemy for subsequent procs
                    let candidates: Vec<usize> = (0..nearby_enemies.len())
                        .filter(|&i| !chosen.contains(&i))
                        .collect();
                    let idx = if candidates.is_empty() {
                        (0..nearby_enemies.len())
                            .collect::<Vec<_>>()
                            .choose(&mut rng)
                            .copied()
                            .unwrap_or(0)
                    } else {
                        *candidates.choose(&mut rng).unwrap_or(&0)
                    };
                    chosen.push(idx);
                    let target_pos = nearby_enemies[idx].1;
                    trigger_counts.increment(Heirloom::KillLightning);
                    ranged_attack_event.send(RangedAttackEvent {
                        projectile: crate::item::projectile::Projectile::Lightning,
                        direction: Vec2::ZERO,
                        mana_cost: Some(5),
                        from_enemy: false,
                        from_entity: None,
                        is_followup_proj: false,
                        dmg_override: Some(lightning_damage),
                        pos_override: Some(target_pos + Vec2::new(0., 48.)),
                        spawn_delay: 0.0,
                    });
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::LightningStaffCast, 0.2));
                }
            }
        }

        // IceStaffFloor: 20% chance per stack on kill to spawn ice floor at death position
        let ice_floor_stacks = player_skills.get_count(Heirloom::IceStaffFloor);
        if ice_floor_stacks > 0 {
            let mut rng = rand::thread_rng();
            let chance = (ice_floor_stacks as f64 * 0.2).min(1.0);
            if rng.gen_bool(chance) {
                let mana_cost = Heirloom::IceStaffFloor.get_mana_cost();
                if current_mana.0 >= mana_cost {
                    current_mana.0 -= mana_cost;
                    trigger_counts.increment(Heirloom::IceStaffFloor);
                    let pos = death_event.enemy_pos.extend(0.0);
                    let ice = spawn_one_time_aseprite_collider(
                        &mut commands,
                        Transform::from_translation(pos),
                        6.5,
                        attack.0,
                        Collider::capsule(Vec2::ZERO, Vec2::ZERO, 14.),
                        asset_server.load::<Aseprite, _>(IceFloor::PATH),
                        AsepriteAnimation::from(IceFloor::tags::ICE_FLOOR),
                        true,
                        Projectile::IceFloor,
                    );
                    commands
                        .entity(ice)
                        .insert(YSort(-0.1))
                        .insert(IceExplosionDmg);
                }
            }
        }
    }
}
fn handle_invincibility_frames(
    mut commands: Commands,
    mut i_frames: Query<(Entity, &mut InvincibilityTimer)>,
    time: Res<Time>,
) {
    for mut i_frame in i_frames.iter_mut() {
        i_frame.1 .0.tick(time.delta());
        if i_frame.1 .0.just_finished() {
            commands.entity(i_frame.0).remove::<InvincibilityTimer>();
        }
    }
}

pub fn handle_hits(
    mut commands: Commands,
    mut mob_stat_tracker: ResMut<damage_tracker::MobStatTracker>,
    mut game: GameParam,
    mut health: Query<(
        Entity,
        &mut CurrentHealth,
        &MaxHealth,
        Option<&Attack>,
        Option<&ProjectileSize>,
        Option<&mut CurrentShield>,
        Option<&mut ShieldRegen>,
        Option<&SlimeTempShield>,
        &GlobalTransform,
        Option<&WorldObject>,
        Option<&Mob>,
        Option<&RequiredEquipmentType>,
        Option<&InvincibilityCooldown>,
        Option<&CombatShrineMob>,
        Option<&DungeonShrineMob>,
    )>,
    mut hit_events: EventReader<HitEvent>,
    mut hit_outcome: HitOutcomeEvents,
    in_i_frame: Query<&InvincibilityTimer>,
    proto_param: ProtoParam,
    slime_shields: Query<Entity, With<SlimeTempShieldSprite>>,
    mut hallucination_query: Query<&mut HallucinationStats, With<Player>>,
    asset_server: Res<AssetServer>,
    mut player_blessing_mana_query: Query<
        (&OwnedBlessings, &mut CurrentMana, &Inventory),
        With<Player>,
    >,
    mut run_beastiary: ResMut<crate::player::beastiary::RunBeastiary>,
    mut last_attacker: ResMut<crate::player::beastiary::LastPlayerAttackerMob>,
) {
    for hit in hit_events.iter() {
        // is in invincibility frames from a previous hit
        if in_i_frame.get(hit.hit_entity).is_ok() {
            continue;
        }

        if let Ok((
            e,
            mut hit_health,
            max_health,
            attack,
            proj_size,
            mut shields_option,
            mut shield_regen_option,
            slime_shield_option,
            t,
            obj_option,
            mob_option,
            hit_req_option,
            i_frame_option,
            shrine_option,
            dungeon_shrine_option,
        )) = health.get_mut(hit.hit_entity)
        {
            // don't shoot a dead horse...
            if hit_health.0 <= 0 {
                continue;
            }
            let dmg = if hit.damage == 0 && hit.hit_entity != game.game.player {
                1
            } else {
                hit.damage
            };
            // Propagate crit flags from the HitEvent onto the target so the
            // damage-numbers UI can render yellow/orange numbers. Skip the
            // player (they don't take crits) and skip non-crit hits to keep
            // the "consume by setting back to false" pattern from churning
            // archetypes — see WasHitWithCrit doc comment.
            if hit.was_crit && hit.hit_entity != game.game.player {
                commands.entity(e).insert(WasHitWithCrit(true));
                if hit.was_overcrit {
                    commands.entity(e).insert(WasHitWithOvercrit(true));
                }
            }
            if let Some(obj) = obj_option {
                // Skill projectiles (AoE, shouts, explosions, etc.) never satisfy tool gating -
                // only regular attack projectiles can chop/mine when the player carries the tool.
                let is_skill_projectile = hit
                    .hit_with_projectile
                    .as_ref()
                    .map_or(false, |p| p.is_skill_projectile());

                // Does the player have the tool required to break this object in any inventory slot?
                let player_has_required_tool = if let Some(item_type_req) = hit_req_option {
                    !is_skill_projectile
                        && player_blessing_mana_query
                            .get_single()
                            .ok()
                            .map_or(false, |(_, _, inv)| {
                                inv.has_equipment_type(&item_type_req.0, &proto_param)
                            })
                } else {
                    false
                };

                // Allow projectile damage on breakable objects, OR on tool-gated objects when the
                // player carries the matching tool (e.g. axe in inventory lets arrows chop trees).
                if hit.hit_with_projectile.is_some() && hit.hit_by_mob.is_none() {
                    if !obj.is_breakable_by_projectile() && !player_has_required_tool {
                        continue;
                    }
                }
                let anchor = proto_param
                    .get_component::<SpriteAnchor, _>(*obj)
                    .unwrap_or(&SpriteAnchor(Vec2::ZERO));
                let pos = world_pos_to_tile_pos(t.translation().truncate() - anchor.0);

                // Tool requirement: the player just needs the appropriate tool somewhere in their
                // inventory for attacks to break the object. Fall back to the old main-hand check
                // for compatibility (e.g. mob `ignore_tool: true` hits, or legacy hits where the
                // tool was used directly as the melee weapon).
                if let Some(item_type_req) = hit_req_option {
                    if !hit.ignore_tool && !player_has_required_tool {
                        if let Some(hit_item_type) = hit.hit_with_melee {
                            if proto_param
                                .get_component::<EquipmentType, _>(hit_item_type)
                                .unwrap_or(&EquipmentType::None)
                                != &item_type_req.0
                            {
                                continue;
                            }
                        } else {
                            continue;
                        }
                    }
                }
                hit_health.0 -= dmg;
                if *DEBUG {
                    debug!("HP {:?} {:?}", e, hit_health.0);
                }
                if hit_health.0 <= 0 {
                    hit_outcome.obj_break.send(ObjBreakEvent {
                        entity: e,
                        obj: *obj,
                        pos,
                        give_drops_and_xp: true,
                    });
                }
            } else {
                let is_player = game.game.player == e;
                if let Some(mob) = mob_option {
                    let lethal_blow_count =
                        game.get_player_skills().get_count(Heirloom::LethalBlow);
                    if lethal_blow_count > 0 && !mob.is_boss() {
                        let mut rng = rand::thread_rng();
                        if rng.gen_bool(0.005 * lethal_blow_count as f64) {
                            hit_health.0 = 0;

                            // Hallucination effect: grant random stat buff
                            if let Ok(mut hallucination_stats) =
                                hallucination_query.get_single_mut()
                            {
                                let stat_type = HallucinationStatType::random();
                                let amount = rng.gen_range(1..=4);
                                hallucination_stats.add_stat(stat_type, amount);

                                // Show floating text with stat gain at player position (like item pickups)
                                let player_pos = game.player().position;
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
                                    FLOATING_TEXT,
                                );

                                // Trigger attribute recalculation
                                hit_outcome.attribute_change.send(AttributeChangeEvent);
                            }
                        }
                    }
                }
                let final_dmg = dmg
                    - if game.has_skill(Heirloom::MinusOneDamageOnHit) && is_player {
                        1
                    } else {
                        0
                    };
                let mut shielded_hit = false;
                if slime_shield_option.is_some() {
                    // if we have a slime temp shield, it only has 1 HP
                    if final_dmg >= 1 {
                        // shield breaks
                        commands.entity(e).remove::<SlimeTempShield>();
                        // Safely get the shield entity - might not exist if already despawned
                        if let Ok(shield_entity) = slime_shields.get_single() {
                            commands.entity(shield_entity).despawn_recursive();
                        }
                        shielded_hit = true;
                        if *DEBUG {
                            info!("Slime Temp Shield broken!");
                        }
                    }
                } else if let Some(shields) = shields_option.as_deref_mut() {
                    if shields.0 > 0 {
                        let shield_damage = final_dmg.min(shields.0);
                        shields.0 -= shield_damage;
                        shielded_hit = true;
                        if *DEBUG {
                            info!("Shield HP {:?}", shields.0);
                        }
                    }
                }

                if !shielded_hit {
                    let minus_one_reduction =
                        if game.has_skill(Heirloom::MinusOneDamageOnHit) && is_player {
                            1
                        } else {
                            0
                        };
                    let damage_to_apply = final_dmg - minus_one_reduction;

                    // ManaGuard blessing: 80% of damage comes from mana instead of health
                    let mana_guard_percentage = player_blessing_mana_query
                        .get_single()
                        .map(|(b, _, _)| b.get_mana_guard_percentage())
                        .unwrap_or(0.0);

                    if is_player && mana_guard_percentage > 0.0 {
                        let mana_damage = (damage_to_apply as f32 * mana_guard_percentage) as i32;
                        let health_damage = damage_to_apply - mana_damage;

                        // Apply mana damage first
                        if let Ok((_, mut current_mana, _)) =
                            player_blessing_mana_query.get_single_mut()
                        {
                            let actual_mana_damage = mana_damage.min(current_mana.0);
                            current_mana.0 -= actual_mana_damage;
                            // Any overflow goes to health
                            let overflow = mana_damage - actual_mana_damage;
                            hit_health.0 -= health_damage + overflow;
                        } else {
                            // No mana query result, apply full damage to health
                            hit_health.0 -= damage_to_apply;
                        }
                    } else {
                        hit_health.0 -= damage_to_apply;
                    }

                    if is_player {
                        if let Some(attacker_mob) = hit.hit_by_mob.as_ref() {
                            if *attacker_mob != Mob::None && damage_to_apply > 0 {
                                mob_stat_tracker
                                    .record_damage_taken(attacker_mob.clone(), damage_to_apply);
                            }
                        }

                        let echo_count = game.skill_count(Heirloom::OnHitEcho);
                        if echo_count > 0 {
                            if let Ok((_, mut current_mana, _)) =
                                player_blessing_mana_query.get_single_mut()
                            {
                                let mana_cost = Heirloom::OnHitEcho.get_mana_cost();
                                let dmg = attack.unwrap_or(&Attack(0)).0;
                                let size_mult =
                                    proj_size.unwrap_or(&ProjectileSize(0)).get_multiplier();

                                for i in 0..echo_count {
                                    if current_mana.0 < mana_cost {
                                        break;
                                    }
                                    current_mana.0 -= mana_cost;
                                    game.heirloom_trigger_counts.increment(Heirloom::OnHitEcho);

                                    if i == 0 {
                                        spawn_echo_hitbox(
                                            &mut commands,
                                            &asset_server,
                                            e,
                                            dmg,
                                            size_mult,
                                        );
                                    } else {
                                        spawn_delayed_heirloom_cast(
                                            &mut commands,
                                            HEIRLOOM_EXTRA_CAST_DELAY * i as f32,
                                            DelayedCastType::Echo {
                                                player: e,
                                                dmg,
                                                size_multiplier: size_mult,
                                            },
                                        );
                                    }
                                }
                            };
                        }
                    }
                    if *DEBUG {
                        info!("HP {:?}", hit_health.0);
                    }
                }
                if let Some(shield_regen) = shield_regen_option.as_deref_mut() {
                    shield_regen.delay_timer.reset();
                    shield_regen.regen_timer.reset();
                }

                let mob_kb = if let Some(mob) = mob_option {
                    mob.get_base_kb()
                        + if game.has_skill(Heirloom::Knockback) {
                            200.
                        } else {
                            0.
                        }
                } else {
                    0.
                };

                // Shout projectile has much higher knockback
                let shout_knockback_bonus = if hit.hit_with_projectile == Some(Projectile::Shout) {
                    2000.
                } else {
                    0.
                };

                commands.entity(hit.hit_entity).insert(HitAnimationTracker {
                    is_active: true,
                    timer: Timer::from_seconds(
                        //TODO: once we create builders for creatures, add this as a default to all creatures that can be hit
                        0.2,
                        TimerMode::Once,
                    ),
                    knockback: if shielded_hit {
                        0.
                    } else {
                        if is_player {
                            200.
                        } else {
                            mob_kb + shout_knockback_bonus
                        }
                    },
                    dir: hit.dir,
                });
                if let Some(i_frames) = i_frame_option {
                    commands.entity(hit.hit_entity).insert(InvincibilityTimer(
                        Timer::from_seconds(i_frames.0, TimerMode::Once),
                    ));
                }
                if hit_health.0 <= 0 && game.player_query.single().0 != e {
                    commands.entity(e).insert(MarkedForDeath);

                    // Mark if killed by heirloom effect to prevent chaining
                    if hit.from_heirloom_effect.is_some() {
                        commands.entity(e).insert(KilledByHeirloomEffect);
                    }

                    hit_outcome.enemy_death.send(EnemyDeathEvent {
                        entity: e,
                        enemy_pos: t.translation().truncate(),
                        killed_by_crit: hit.was_crit,
                        mob: mob_option.cloned().unwrap_or(Mob::None),
                    });

                    if let Some(parent_shrine) = shrine_option {
                        hit_outcome
                            .combat_shrine_mob_death
                            .send(CombatShrineMobDeathEvent(parent_shrine.parent_shrine));
                    }

                    if let Some(parent_shrine) = dungeon_shrine_option {
                        hit_outcome
                            .dungeon_shrine_mob_death
                            .send(DungeonShrineMobDeathEvent(parent_shrine.parent_shrine));
                    }
                }

                if is_player {
                    let attacker_mob = hit.hit_by_mob.clone().unwrap_or(Mob::default());
                    hit_outcome.analytics.send(AnalyticsUpdateEvent {
                        update_type: AnalyticsTrigger::DamageTaken(
                            attacker_mob.clone(),
                            final_dmg as u32,
                        ),
                    });
                    if attacker_mob != Mob::None && final_dmg > 0 {
                        run_beastiary.record_damage_taken(attacker_mob.clone(), final_dmg as u32);
                        // Memo for `clamp_health` to attribute death to this mob.
                        last_attacker.0 = Some(attacker_mob);
                    }
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::PlayerHit, 0.35));
                } else if let Some(mob) = mob_option {
                    game.player_mut().next_hit_crit = false;
                    hit_outcome.analytics.send(AnalyticsUpdateEvent {
                        update_type: AnalyticsTrigger::DamageDealt(mob.clone(), final_dmg as u32),
                    });
                    if final_dmg > 0 {
                        run_beastiary.record_damage_dealt(mob.clone(), final_dmg as u32);
                    }
                }
            }

            // Only insert hit reaction when we applied the hit and the entity is still alive
            // (avoids queuing commands for entities that will be despawned by cleanup this frame)
            if hit_health.0 > 0 {
                if let Some(mut hit_e) = commands.get_entity(hit.hit_entity) {
                    hit_e.insert(BounceOnHit::new());
                }
            }
        }
    }
}
pub fn cleanup_marked_for_death_entities(
    mut commands: Commands,
    dead_query: Query<
        (
            Entity,
            &Mob,
            Option<&crate::combat::status_effects::MobStatusEffects>,
            &GlobalTransform,
            Option<&KilledByHeirloomEffect>,
        ),
        With<MarkedForDeath>,
    >,
    mut analytics: EventWriter<AnalyticsUpdateEvent>,
    mut run_beastiary: ResMut<crate::player::beastiary::RunBeastiary>,
    mut player: Query<(
        &PlayerSkills,
        &Attack,
        &ManaRegen,
        &mut CurrentMana,
        &ProjectileSize,
    )>,
    graphics: Res<Graphics>,
    mut modify_mana_event: EventWriter<ModifyManaEvent>,
    mut neaby_mobs: Query<
        (
            Entity,
            &GlobalTransform,
            &mut crate::combat::status_effects::MobStatusEffects,
        ),
        (With<Mob>, Without<MarkedForDeath>),
    >,
    mut status_event: EventWriter<StatusEffectEvent>,
    spike_attack_states: Query<&SpikeAttackState>,
    aoe_attack_states: Query<&AoEAttackState>,
    spike_warnings: Query<(Entity, &SpikeWarning)>,
    mut trigger_counts: ResMut<HeirloomTriggerCounts>,
) {
    // Collect boss/non-boss deaths first to avoid overlapping mutable borrows
    // of `neaby_mobs` from inside the loop.
    let mut nearby_venom_targets: Vec<(Entity, Vec2, crate::combat::status_effects::Burning)> =
        Vec::new();

    for (e, mob, status_option, mob_pos, killed_by_heirloom) in dead_query.iter() {
        if mob.is_boss() {
            // Clean up preview entities before removing attack states
            // StoneGolem spike attack warnings - despawn all warnings for this golem
            if mob == &Mob::StoneGolem {
                for (warning_entity, warning) in spike_warnings.iter() {
                    if warning.golem_entity == e {
                        commands.entity(warning_entity).despawn_recursive();
                    }
                }
            }

            // RedMushking AoE attack previews
            if let Ok(aoe_state) = aoe_attack_states.get(e) {
                for preview_e in &aoe_state.preview_entities {
                    if let Some(entity_commands) = commands.get_entity(*preview_e) {
                        entity_commands.despawn_recursive();
                    }
                }
            }

            commands
                .entity(e)
                .insert(DeathState)
                .remove::<FollowState>()
                .remove::<SummonAttackState>()
                .remove::<LeapAttackState>()
                .remove::<ReturnToShrineState>()
                .remove::<AoEAttackState>() // Remove RedMushking's AoE attack state
                .remove::<SpikeAttackState>() // Remove StoneGolem's attack state
                .remove::<MarkedForDeath>();
        } else {
            let (skills, attack, mana_regen, mut current_mana, projectile_size) =
                player.single_mut();

            // Only trigger heirloom on-kill effects if the kill wasn't from a heirloom effect
            // This prevents chaining (e.g., ice explosion killing enemies that trigger more ice explosions)
            let can_trigger_heirloom_effects = killed_by_heirloom.is_none();

            if can_trigger_heirloom_effects {
                let was_slowed = status_option.map(|s| s.is_slowed()).unwrap_or(false);
                if was_slowed {
                    let heirloomc_count = skills.get_count(Heirloom::FrozenAoE) as f64;
                    if heirloomc_count > 0. {
                        let trigger_chance = heirloomc_count * 0.25;
                        let guaranteed_casts = trigger_chance as u32;
                        let remainder = trigger_chance - guaranteed_casts as f64;
                        let mut rng = rand::thread_rng();
                        let bonus = if remainder > 0. && rng.gen_bool(remainder.clamp(0., 1.)) {
                            1u32
                        } else {
                            0
                        };
                        let total_casts = guaranteed_casts + bonus;
                        let mana_cost = Heirloom::FrozenAoE.get_mana_cost();
                        let pos = mob_pos.translation();
                        let dmg = attack.0 / 4;
                        let size_mult = projectile_size.get_multiplier();

                        for i in 0..total_casts {
                            if current_mana.0 < mana_cost {
                                break;
                            }
                            current_mana.0 -= mana_cost;
                            trigger_counts.increment(Heirloom::FrozenAoE);

                            if i == 0 {
                                spawn_ice_explosion_hitbox(
                                    &mut commands,
                                    &graphics,
                                    pos,
                                    dmg,
                                    size_mult,
                                );
                            } else {
                                spawn_delayed_heirloom_cast(
                                    &mut commands,
                                    HEIRLOOM_EXTRA_CAST_DELAY * i as f32,
                                    DelayedCastType::IceExplosion {
                                        pos,
                                        dmg,
                                        size_multiplier: size_mult,
                                    },
                                );
                            }
                        }
                    }
                    let rng = &mut rand::thread_rng();
                    let mirror_count = skills.get_count(Heirloom::FrozenMPRegen);
                    if mirror_count > 0 && rng.gen_bool((0.2 * mirror_count as f64).min(1.0)) {
                        modify_mana_event.send(ModifyManaEvent(mana_regen.0));
                        trigger_counts.increment(Heirloom::FrozenMPRegen);
                    }
                }
                if let Some(p) = status_option.and_then(|s| s.burning.as_ref()) {
                    if skills.has(Heirloom::ViralVenum) {
                        let mana_cost = Heirloom::ViralVenum.get_mana_cost();
                        if current_mana.0 >= mana_cost {
                            current_mana.0 -= mana_cost;
                            trigger_counts.increment(Heirloom::ViralVenum);
                            // Defer nearby-mob status mutation out of this loop
                            // so we don't take overlapping borrows of `neaby_mobs`.
                            nearby_venom_targets.push((
                                e,
                                mob_pos.translation().truncate(),
                                p.clone(),
                            ));
                        }
                    }
                }
            }
            commands.entity(e).despawn_recursive();
        }
        analytics.send(AnalyticsUpdateEvent {
            update_type: AnalyticsTrigger::MobKilled(mob.clone()),
        });
        run_beastiary.record_kill(mob.clone());
    }

    // Apply deferred ViralVenum spreads after we've released read access above.
    if !nearby_venom_targets.is_empty() {
        for (_source_e, source_pos, source_burning) in nearby_venom_targets.into_iter() {
            for (mob_e, txfm, mut status) in neaby_mobs.iter_mut() {
                if source_pos.distance(txfm.translation().truncate()) < 3. * TILE_SIZE.x {
                    status.burning = Some(crate::combat::status_effects::Burning {
                        stacks: source_burning.stacks,
                        duration_timer: Timer::from_seconds(
                            source_burning.duration_timer.duration().as_secs_f32(),
                            TimerMode::Once,
                        ),
                        tick_timer: source_burning.tick_timer.clone(),
                    });
                    status_event.send(StatusEffectEvent {
                        entity: mob_e,
                        effect: StatusEffect::Poison,
                        num_stacks: source_burning.stacks as i32,
                    });
                }
            }
        }
    }
}

/// Handles lifesteal calculation and healing based on LifestealEvent
/// This centralizes all lifesteal logic to avoid duplication
pub fn handle_lifesteal(
    mut lifesteal_events: EventReader<LifestealEvent>,
    player_query: Query<(&PlayerSkills, &Lifesteal, &GlobalTransform), With<Player>>,
    mut modify_health_events: EventWriter<ModifyHealthEvent>,
    mut modify_mana_events: EventWriter<ModifyManaEvent>,
    mut proto_commands: ProtoCommands,
    proto: ProtoParam,
    mut trigger_counts: ResMut<HeirloomTriggerCounts>,
) {
    let Ok((skills, lifesteal, player_txfm)) = player_query.get_single() else {
        return;
    };
    let player_pos = player_txfm.translation().truncate();

    for event in lifesteal_events.iter() {
        let mut rng = rand::thread_rng();

        if event.is_direct_player_damage {
            let stacks = skills.get_count(Heirloom::DamageDealtMp);
            if stacks > 0 {
                let chance = (0.03_f64 * stacks as f64).clamp(0.0, 1.0);
                if rng.gen_bool(chance) {
                    modify_mana_events.send(ModifyManaEvent(1));
                    trigger_counts.increment(Heirloom::DamageDealtMp);
                }
            }
        }

        // ThornsLifesteal bonus: +25% per stack for thorns damage specifically
        let thorns_bonus = event.thorns_lifesteal_stacks * 25;
        let total_lifesteal = lifesteal.0 + thorns_bonus;

        if total_lifesteal > 0 {
            // Lifesteal can go over 100%
            // >= 100% = guaranteed 1 HP heal
            // For each additional 100% over 100%, guaranteed another HP
            // Remainder is a random chance for +1 more HP
            let mut heal_amount = 0;
            let mut remaining_lifesteal = total_lifesteal;

            // Process full 100% chunks
            while remaining_lifesteal >= 100 {
                heal_amount += 1;
                remaining_lifesteal -= 100;
            }

            // Random roll for remainder
            if remaining_lifesteal > 0
                && rng.gen_bool((remaining_lifesteal as f64 / 100.0).clamp(0.0, 1.0))
            {
                heal_amount += 1;
            }

            if heal_amount > 0 {
                modify_health_events.send(ModifyHealthEvent(heal_amount));

                // LifestealCoins: Spawn a coin for each lifesteal proc
                let count = skills.get_count(Heirloom::LifestealCoins) as f64;
                if count > 0. && rng.gen_bool((count * 0.1).min(1.)) {
                    let d = 32.0;
                    let drop_offset = Vec2::new(rng.gen_range(-d..d), rng.gen_range(-d..d));
                    proto_commands.spawn_item_from_proto(
                        WorldObject::Coin,
                        &proto,
                        player_pos + drop_offset,
                        1,
                        None,
                    );
                    trigger_counts.increment(Heirloom::LifestealCoins);
                }
            }
        }
    }
}

/// Handles ThornsOnDamage tracker increment when player takes damage
/// This runs after handle_hits to ensure we only increment when damage is actually applied
/// Uses a Local HashSet to track which mobs have already triggered the increment this frame
pub fn handle_thorns_on_damage_tracker(
    mut hit_events: EventReader<HitEvent>,
    mut thorns_tracker: Query<
        &mut crate::player::combat_heirlooms::ThornsOnDamageTracker,
        With<Player>,
    >,
    player_skills: Query<&PlayerSkills, With<Player>>,
    health: Query<(Entity, &CurrentHealth), With<Player>>,
    mut attribute_events: EventWriter<AttributeChangeEvent>,
    in_i_frame: Query<&InvincibilityTimer>,
    mut trigger_counts: ResMut<HeirloomTriggerCounts>,
) {
    let Ok((player_entity, _)) = health.get_single() else {
        return;
    };

    for hit in hit_events.iter() {
        if in_i_frame.get(hit.hit_entity).is_ok() {
            continue;
        }
        if hit.hit_entity == player_entity {
            if hit.hit_by_mob.is_some() {
                if let Ok(mut tracker) = thorns_tracker.get_single_mut() {
                    if let Ok(skills) = player_skills.get_single() {
                        let stacks =
                            skills.get_count(crate::player::skills::Heirloom::ThornsOnDamage);
                        if stacks > 0 {
                            tracker.thorns_gained += stacks;
                            attribute_events.send(AttributeChangeEvent);
                            trigger_counts.increment(Heirloom::ThornsOnDamage);
                        }
                    }
                }
            }
        }
    }
}
