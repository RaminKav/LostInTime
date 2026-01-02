use bevy::prelude::*;

use bevy_proto::prelude::ProtoCommands;
use combat_helpers::{handle_deferred_aseprite_spawns, tick_despawn_timer};
use rand::Rng;
pub mod status_effects;
use status_effects::*;

pub mod collisions;
use crate::attributes::{add_item_glows, CurrentMana, ProjectileSize};

pub mod combat_helpers;
use crate::blessings::OwnedBlessings;
use crate::chaos::ChaosTracker;
use crate::night::InfiniteMode;
use crate::player::melee_skills::spawn_echo_hitbox;
use crate::{
    ai::{FollowState, LeapAttackState},
    animations::{AttackEvent, HitAnimationTracker},
    assets::{Graphics, SpriteAnchor},
    attributes::{
        modifiers::ModifyManaEvent, Attack, AttackCooldown, AttributeChangeEvent, CurrentHealth,
        CurrentShield, InvincibilityCooldown, ManaRegen, MaxHealth, ShieldRegen,
    },
    audio::{AudioSoundEffect, SoundSpawner},
    client::{
        analytics::{AnalyticsTrigger, AnalyticsUpdateEvent},
        is_not_paused,
    },
    custom_commands::CommandsExt,
    enemy::{
        red_mushking::{AoEAttackState, DeathState, ReturnToShrineState, SummonAttackState},
        stone_golem::SpikeAttackState,
        Mob, MobLevel,
    },
    item::{
        combat_shrine::{CombatShrineMob, CombatShrineMobDeathEvent},
        dungeon_shrine::{DungeonShrineMob, DungeonShrineMobDeathEvent},
        projectile::Projectile,
        EquipmentType, LootTable, LootTablePlugin, MainHand, RequiredEquipmentType, WorldObject,
    },
    juice::{bounce::BounceOnHit, spawn_xp_particles},
    player::{
        combat_heirlooms::{HallucinationStatType, HallucinationStats},
        levels::{ExperienceReward, PlayerLevel},
        mage_skills::spawn_ice_explosion_hitbox,
        skills::{Heirloom, PlayerSkills},
    },
    proto::proto_param::ProtoParam,
    ui::damage_numbers::spawn_floating_text_with_shadow,
    world::{world_helpers::world_pos_to_tile_pos, TileMapPosition, TILE_SIZE},
    AppExt, CustomFlush, GameParam, GameState, Player, SlimeTempShield, SlimeTempShieldSprite,
    DEBUG,
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
    /// If true, this damage came from a heirloom effect (explosion, etc) and should not trigger other heirloom effects
    pub from_heirloom_effect: bool,
}

#[derive(Component, Debug, Clone)]
pub struct MarkedForDeath;

#[derive(Component, Debug, Clone)]
pub struct KilledByHeirloomEffect;
#[derive(Debug, Clone)]

pub struct EnemyDeathEvent {
    pub entity: Entity,
    pub enemy_pos: Vec2,
    pub killed_by_crit: bool,
}
#[derive(Debug, Clone)]

pub struct ObjBreakEvent {
    pub entity: Entity,
    pub obj: WorldObject,
    pub pos: TileMapPosition,
    pub give_drops_and_xp: bool,
}

#[derive(Component)]
pub struct WasHitWithCrit;

#[derive(Component)]
pub struct WasHitWithOvercrit;

#[derive(Component, Debug, Clone)]
pub struct AttackTimer(pub Timer);

#[derive(Component, Debug, Clone)]
pub struct InvincibilityTimer(pub Timer);
#[derive(Component, Debug, Clone)]

pub struct HitMarker;

#[derive(Component, Debug)]
pub struct JustGotHit;
pub struct CombatPlugin;
impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.with_default_schedule(CoreSchedule::FixedUpdate, |app| {
            app.add_event::<HitEvent>()
                .add_event::<EnemyDeathEvent>()
                .add_event::<StatusEffectEvent>();
        })
        .add_event::<ObjBreakEvent>()
        .add_plugin(CollisionPlugion)
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
            )
                .in_set(OnUpdate(GameState::Main)),
        )
        .add_system(apply_system_buffers.in_set(CustomFlush));
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
    mob_data: Query<(&Mob, &ExperienceReward, &MobLevel)>,
    mut player_xp: Query<(&mut PlayerLevel, &PlayerSkills, &OwnedBlessings)>,
    mut proto_commands: ProtoCommands,
    mut commands: Commands,
    graphics: Res<Graphics>,
    infinite_mode: Res<InfiniteMode>,
    mut chaos_tracker: ResMut<ChaosTracker>,
) {
    for death_event in death_events.iter() {
        let Ok((mob, mob_xp, mob_lvl)) = mob_data.get(death_event.entity) else {
            continue;
        };
        let (mut player_level, player_skills, blessings) = player_xp.single_mut();
        let is_infinite_mode = infinite_mode.active;

        let has_double_gold = blessings.has_double_gold_drops();

        // drop loot
        if let Ok(loot_table) = loot_tables.get(death_event.entity) {
            for drop in LootTablePlugin::get_drops(
                loot_table,
                &proto_param,
                // loot_bonus.single().0,
                0,
                Some(mob_lvl.0),
                is_infinite_mode,
            ) {
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
                                60.0,
                                TimerMode::Once,
                            )));
                    }
                }
            }
        }

        let double_xp_chance = blessings.get_double_xp_chance();

        let xp_multiplier =
            if double_xp_chance > 0.0 && rand::thread_rng().gen_bool(double_xp_chance as f64) {
                2
            } else {
                1
            };

        //give player xp
        let did_level =
            player_level.add_xp(mob_xp.0 * xp_multiplier, &player_skills, &mut chaos_tracker);
        // Only spawn XP particles if not in endless mode (to reduce lag)
        if !is_infinite_mode {
            spawn_xp_particles(
                death_event.enemy_pos,
                &mut commands,
                mob_xp.0 * xp_multiplier,
                did_level,
            );
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
    mut enemy_death_events: EventWriter<EnemyDeathEvent>,
    mut shrine_mob_death_event: EventWriter<CombatShrineMobDeathEvent>,
    mut dungeon_shrine_mob_death_event: EventWriter<DungeonShrineMobDeathEvent>,
    mut obj_death_events: EventWriter<ObjBreakEvent>,
    in_i_frame: Query<&InvincibilityTimer>,
    proto_param: ProtoParam,
    mut analytics_events: EventWriter<AnalyticsUpdateEvent>,
    slime_shields: Query<Entity, With<SlimeTempShieldSprite>>,
    mut hallucination_query: Query<&mut HallucinationStats, With<Player>>,
    mut attribute_events: EventWriter<AttributeChangeEvent>,
    asset_server: Res<AssetServer>,
    mut player_blessing_mana_query: Query<(&OwnedBlessings, &mut CurrentMana), With<Player>>,
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
            if let Some(obj) = obj_option {
                // Allow projectile damage only on breakable objects
                if hit.hit_with_projectile.is_some() && hit.hit_by_mob.is_none() {
                    if !obj.is_breakable_by_projectile() {
                        continue;
                    }
                }
                let anchor = proto_param
                    .get_component::<SpriteAnchor, _>(*obj)
                    .unwrap_or(&SpriteAnchor(Vec2::ZERO));
                let pos = world_pos_to_tile_pos(t.translation().truncate() - anchor.0);

                //TODO: create breaks with tool component, instead of using properties
                if let Some(item_type_req) = hit_req_option {
                    if let Some(hit_item_type) = hit.hit_with_melee {
                        if proto_param
                            .get_component::<EquipmentType, _>(hit_item_type)
                            .unwrap_or(&EquipmentType::None)
                            != &item_type_req.0
                            && !hit.ignore_tool
                        {
                            continue;
                        }
                    } else if !hit.ignore_tool {
                        continue;
                    }
                }
                hit_health.0 -= dmg;
                if *DEBUG {
                    debug!("HP {:?} {:?}", e, hit_health.0);
                }
                if hit_health.0 <= 0 {
                    obj_death_events.send(ObjBreakEvent {
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
                        // 2% flat chance to execute (works on all weapons)
                        let mut rng = rand::thread_rng();
                        if rng.gen_bool(0.02 * lethal_blow_count as f64) {
                            hit_health.0 = 0;

                            // Hallucination effect: grant random stat buff
                            if let Ok(mut hallucination_stats) =
                                hallucination_query.get_single_mut()
                            {
                                let stat_type = HallucinationStatType::random();
                                let amount = rng.gen_range(1..=5);
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
                                );

                                // Trigger attribute recalculation
                                attribute_events.send(AttributeChangeEvent);
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
                        .map(|(b, _)| b.get_mana_guard_percentage())
                        .unwrap_or(0.0);

                    if is_player && mana_guard_percentage > 0.0 {
                        let mana_damage = (damage_to_apply as f32 * mana_guard_percentage) as i32;
                        let health_damage = damage_to_apply - mana_damage;

                        // Apply mana damage first
                        if let Ok((_, mut current_mana)) =
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

                    if is_player && game.has_skill(Heirloom::OnHitEcho) {
                        if let Ok((_, mut current_mana)) =
                            player_blessing_mana_query.get_single_mut()
                        {
                            let mana_cost = Heirloom::OnHitEcho.get_mana_cost();
                            if current_mana.0 >= mana_cost {
                                current_mana.0 -= mana_cost;
                                spawn_echo_hitbox(
                                    &mut commands,
                                    &asset_server,
                                    e,
                                    attack.unwrap_or(&Attack(0)).0,
                                    proj_size.unwrap_or(&ProjectileSize(0)).get_multiplier(),
                                );
                            }
                        };
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
                            100.
                        } else {
                            0.
                        }
                } else {
                    0.
                };

                // Shout projectile has much higher knockback
                let shout_knockback_bonus = if hit.hit_with_projectile == Some(Projectile::Shout) {
                    800.
                } else {
                    0.
                };

                commands.entity(hit.hit_entity).insert(HitAnimationTracker {
                    timer: Timer::from_seconds(
                        //TODO: once we create builders for creatures, add this as a default to all creatures that can be hit
                        0.2,
                        TimerMode::Once,
                    ),
                    knockback: if shielded_hit {
                        0.
                    } else {
                        if is_player {
                            400.
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
                    if hit.from_heirloom_effect {
                        commands.entity(e).insert(KilledByHeirloomEffect);
                    }

                    enemy_death_events.send(EnemyDeathEvent {
                        entity: e,
                        enemy_pos: t.translation().truncate(),
                        killed_by_crit: hit.was_crit,
                    });

                    if let Some(parent_shrine) = shrine_option {
                        shrine_mob_death_event
                            .send(CombatShrineMobDeathEvent(parent_shrine.parent_shrine));
                    }

                    if let Some(parent_shrine) = dungeon_shrine_option {
                        dungeon_shrine_mob_death_event
                            .send(DungeonShrineMobDeathEvent(parent_shrine.parent_shrine));
                    }
                }

                if is_player {
                    analytics_events.send(AnalyticsUpdateEvent {
                        update_type: AnalyticsTrigger::DamageTaken(
                            hit.hit_by_mob.clone().unwrap_or(Mob::default()),
                            final_dmg as u32,
                        ),
                    });
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::PlayerHit, 0.35));
                } else if let Some(mob) = mob_option {
                    game.player_mut().next_hit_crit = false;
                    analytics_events.send(AnalyticsUpdateEvent {
                        update_type: AnalyticsTrigger::DamageDealt(mob.clone(), final_dmg as u32),
                    });
                }
            }

            if let Some(mut hit_e) = commands.get_entity(hit.hit_entity) {
                hit_e.insert(JustGotHit).insert(BounceOnHit::new());
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
            Option<&Slow>,
            Option<&Burning>,
            &GlobalTransform,
            Option<&KilledByHeirloomEffect>,
        ),
        With<MarkedForDeath>,
    >,
    mut analytics: EventWriter<AnalyticsUpdateEvent>,
    mut player: Query<(
        &PlayerSkills,
        &Attack,
        &ManaRegen,
        &mut CurrentMana,
        &ProjectileSize,
    )>,
    graphics: Res<Graphics>,
    mut modify_mana_event: EventWriter<ModifyManaEvent>,
    neaby_mobs: Query<(Entity, &GlobalTransform), (With<Mob>, Without<MarkedForDeath>)>,
    mut status_event: EventWriter<StatusEffectEvent>,
    spike_attack_states: Query<&SpikeAttackState>,
    aoe_attack_states: Query<&AoEAttackState>,
) {
    for (e, mob, slow_option, poison_option, mob_pos, killed_by_heirloom) in dead_query.iter() {
        if mob.is_boss() {
            // Clean up preview entities before removing attack states
            // StoneGolem spike attack preview
            if let Ok(spike_state) = spike_attack_states.get(e) {
                if let Some(preview_entity) = spike_state.preview_entity {
                    commands.entity(preview_entity).despawn_recursive();
                }
            }

            // RedMushking AoE attack previews
            if let Ok(aoe_state) = aoe_attack_states.get(e) {
                if let Some(preview_entity) = aoe_state.preview_entity {
                    commands.entity(preview_entity).despawn_recursive();
                }
                if let Some(second_preview_entity) = aoe_state.second_preview_entity {
                    commands.entity(second_preview_entity).despawn_recursive();
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
                if let Some(_) = slow_option {
                    if skills.has(Heirloom::FrozenAoE) {
                        let mana_cost = Heirloom::FrozenAoE.get_mana_cost();
                        if current_mana.0 >= mana_cost {
                            current_mana.0 -= mana_cost;
                            spawn_ice_explosion_hitbox(
                                &mut commands,
                                &graphics,
                                mob_pos.translation(),
                                attack.0 / 4,
                                projectile_size.get_multiplier(),
                            );
                        }
                    }
                    if skills.has(Heirloom::FrozenMPRegen) {
                        modify_mana_event.send(ModifyManaEvent(
                            mana_regen.0 + skills.get_count(Heirloom::MPRegen) * 5,
                        ));
                    }
                }
                if let Some(p) = poison_option {
                    if skills.has(Heirloom::ViralVenum) {
                        let mana_cost = Heirloom::ViralVenum.get_mana_cost();
                        if current_mana.0 >= mana_cost {
                            current_mana.0 -= mana_cost;
                            for (mob_e, txfm) in neaby_mobs.iter() {
                                if mob_pos.translation().distance(txfm.translation())
                                    < 3. * TILE_SIZE.x
                                {
                                    commands.entity(mob_e).insert(Burning {
                                        stacks: p.stacks,
                                        duration_timer: Timer::from_seconds(
                                            p.duration_timer.duration().as_secs_f32(),
                                            TimerMode::Once,
                                        ),
                                        tick_timer: p.tick_timer.clone(),
                                    });
                                    status_event.send(StatusEffectEvent {
                                        entity: mob_e,
                                        effect: StatusEffect::Poison,
                                        num_stacks: p.stacks as i32,
                                    });
                                }
                            }
                        }
                    }
                }
            }
            commands.entity(e).despawn_recursive();
        }
        analytics.send(AnalyticsUpdateEvent {
            update_type: AnalyticsTrigger::MobKilled(mob.clone()),
        });
    }
}
