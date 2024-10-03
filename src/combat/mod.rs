use bevy::prelude::*;

use bevy_proto::prelude::ProtoCommands;
use combat_helpers::tick_despawn_timer;
use rand::Rng;
pub mod status_effects;
use status_effects::*;

pub mod collisions;

pub mod combat_helpers;
use crate::{
    ai::{FollowState, LeapAttackState},
    animations::{AnimationTimer, AttackEvent, DoneAnimation, HitAnimationTracker},
    assets::SpriteAnchor,
    attributes::{
        modifiers::ModifyManaEvent, Attack, AttackCooldown, CurrentHealth, InvincibilityCooldown,
        LootRateBonus, ManaRegen,
    },
    client::{
        analytics::{AnalyticsTrigger, AnalyticsUpdateEvent},
        is_not_paused,
    },
    custom_commands::CommandsExt,
    enemy::{
        red_mushking::{DeathState, ReturnToShrineState, SummonAttackState},
        Mob, MobLevel,
    },
    item::{
        combat_shrine::{CombatShrineMob, CombatShrineMobDeathEvent},
        projectile::Projectile,
        EquipmentType, LootTable, LootTablePlugin, MainHand, RequiredEquipmentType, WorldObject,
    },
    juice::bounce::BounceOnHit,
    player::{
        levels::{ExperienceReward, PlayerLevel},
        mage_skills::spawn_ice_explosion_hitbox,
        skills::{PlayerSkills, Skill},
    },
    proto::proto_param::ProtoParam,
    world::{world_helpers::world_pos_to_tile_pos, TileMapPosition, TILE_SIZE},
    AppExt, CustomFlush, Game, GameParam, GameState, Player, YSort, DEBUG,
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
    pub was_crit: bool,
    pub ignore_tool: bool,
}

#[derive(Component, Debug, Clone)]
pub struct MarkedForDeath;
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
    mut player_xp: Query<(&mut PlayerLevel, &PlayerSkills)>,
    mut proto_commands: ProtoCommands,
    loot_bonus: Query<&LootRateBonus>,
) {
    for death_event in death_events.iter() {
        let Ok((mob, mob_xp, mob_lvl)) = mob_data.get(death_event.entity) else {
            continue;
        };
        let (mut player_level, skills) = player_xp.single_mut();
        let is_crit_bonus = skills.has(Skill::CritLoot) && death_event.killed_by_crit;
        // drop loot
        if let Ok(loot_table) = loot_tables.get(death_event.entity) {
            for drop in LootTablePlugin::get_drops(
                loot_table,
                &proto_param,
                loot_bonus.single().0 + if is_crit_bonus { 25 } else { 0 },
                Some(mob_lvl.0),
            ) {
                let mut rng = rand::thread_rng();
                let d = if mob.is_boss() { 30. } else { 10. };
                let drop_offset = Vec2::new(rng.gen_range(-d..d), rng.gen_range(-d..d));
                proto_commands.spawn_item_from_proto(
                    drop.obj_type,
                    &proto_param,
                    death_event.enemy_pos + drop_offset,
                    drop.count,
                    Some(player_level.level),
                );
            }
        }
        //give player xp
        player_level.add_xp(mob_xp.0);
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
fn _spawn_hit_spark_effect(
    mut commands: Commands,
    mut hit_events: EventReader<HitEvent>,
    game: Res<Game>,
    transforms: Query<&GlobalTransform>,
    asset_server: Res<AssetServer>,
    mut texture_atlases: ResMut<Assets<TextureAtlas>>,
    is_mob: Query<&Mob>,
) {
    // add spark animation entity as child, will animate once and remove itself.
    for hit in hit_events.iter() {
        if hit.hit_entity == game.player {
            continue;
        }
        if is_mob.get(hit.hit_entity).is_ok() {
            continue;
        }
        let texture_handle = asset_server.load("textures/effects/hit-particles.png");
        let texture_atlas =
            TextureAtlas::from_grid(texture_handle, Vec2::new(32.0, 32.0), 7, 1, None, None);
        let texture_atlas_handle = texture_atlases.add(texture_atlas);
        let mut rng = rand::thread_rng();
        let s = 5;
        let x_rng = rng.gen_range(-s..s);
        let y_rng = rng.gen_range(-s..s);
        let hit_pos = transforms.get(hit.hit_entity).unwrap().translation();

        commands.spawn((
            SpriteSheetBundle {
                texture_atlas: texture_atlas_handle,
                transform: Transform::from_translation(Vec3::new(
                    hit_pos.x + x_rng as f32,
                    hit_pos.y - 5. + y_rng as f32,
                    1.,
                )),
                ..default()
            },
            AnimationTimer(Timer::from_seconds(0.075, TimerMode::Repeating)),
            YSort(1.),
            DoneAnimation,
            Name::new("Hit Spark"),
        ));

        commands.entity(hit.hit_entity).remove::<JustGotHit>();
    }
}

pub fn handle_hits(
    mut commands: Commands,
    mut game: GameParam,
    mut health: Query<(
        Entity,
        &mut CurrentHealth,
        &GlobalTransform,
        Option<&WorldObject>,
        Option<&Mob>,
        Option<&RequiredEquipmentType>,
        Option<&InvincibilityCooldown>,
        Option<&CombatShrineMob>,
    )>,
    mut hit_events: EventReader<HitEvent>,
    mut enemy_death_events: EventWriter<EnemyDeathEvent>,
    mut shrine_mob_death_event: EventWriter<CombatShrineMobDeathEvent>,
    mut obj_death_events: EventWriter<ObjBreakEvent>,
    in_i_frame: Query<&InvincibilityTimer>,
    proto_param: ProtoParam,
    mut analytics_events: EventWriter<AnalyticsUpdateEvent>,
) {
    for hit in hit_events.iter() {
        // is in invincibility frames from a previous hit
        if in_i_frame.get(hit.hit_entity).is_ok() {
            continue;
        }

        if let Ok((
            e,
            mut hit_health,
            t,
            obj_option,
            mob_option,
            hit_req_option,
            i_frame_option,
            shrine_option,
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
                hit_health.0 -= dmg
                    - if game.has_skill(Skill::MinusOneDamageOnHit) {
                        1
                    } else {
                        0
                    };
                if *DEBUG {
                    debug!("HP {:?}", hit_health.0);
                }

                let mob_kb = if let Some(mob) = mob_option {
                    mob.get_base_kb()
                        + if game.has_skill(Skill::Knockback) {
                            100.
                        } else {
                            0.
                        }
                } else {
                    0.
                };
                commands.entity(hit.hit_entity).insert(HitAnimationTracker {
                    timer: Timer::from_seconds(
                        //TODO: once we create builders for creatures, add this as a default to all creatures that can be hit
                        0.2,
                        TimerMode::Once,
                    ),
                    knockback: if is_player { 400. } else { mob_kb },
                    dir: hit.dir,
                });
                if let Some(i_frames) = i_frame_option {
                    commands.entity(hit.hit_entity).insert(InvincibilityTimer(
                        Timer::from_seconds(i_frames.0, TimerMode::Once),
                    ));
                }
                if hit_health.0 <= 0 && game.player_query.single().0 != e {
                    commands.entity(e).insert(MarkedForDeath);
                    enemy_death_events.send(EnemyDeathEvent {
                        entity: e,
                        enemy_pos: t.translation().truncate(),
                        killed_by_crit: hit.was_crit,
                    });

                    if let Some(parent_shrine) = shrine_option {
                        shrine_mob_death_event
                            .send(CombatShrineMobDeathEvent(parent_shrine.parent_shrine));
                    }
                }

                if is_player {
                    analytics_events.send(AnalyticsUpdateEvent {
                        update_type: AnalyticsTrigger::DamageTaken(
                            hit.hit_by_mob.clone().unwrap_or(Mob::default()),
                            dmg as u32,
                        ),
                    });
                } else if let Some(mob) = mob_option {
                    game.player_mut().next_hit_crit = false;
                    analytics_events.send(AnalyticsUpdateEvent {
                        update_type: AnalyticsTrigger::DamageDealt(mob.clone(), dmg as u32),
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
        ),
        With<MarkedForDeath>,
    >,
    mut analytics: EventWriter<AnalyticsUpdateEvent>,
    player: Query<(&PlayerSkills, &Attack, &ManaRegen)>,
    asset_server: Res<AssetServer>,
    mut modify_mana_event: EventWriter<ModifyManaEvent>,
    neaby_mobs: Query<(Entity, &GlobalTransform), (With<Mob>, Without<MarkedForDeath>)>,
    mut status_event: EventWriter<StatusEffectEvent>,
) {
    for (e, mob, slow_option, poison_option, mob_pos) in dead_query.iter() {
        if mob.is_boss() {
            commands
                .entity(e)
                .insert(DeathState)
                .remove::<FollowState>()
                .remove::<SummonAttackState>()
                .remove::<LeapAttackState>()
                .remove::<ReturnToShrineState>()
                .remove::<MarkedForDeath>();
        } else {
            let (skills, attack, mana_regen) = player.single();
            if let Some(_) = slow_option {
                if skills.has(Skill::FrozenAoE) {
                    spawn_ice_explosion_hitbox(
                        &mut commands,
                        &asset_server,
                        mob_pos.translation(),
                        attack.0 / 4,
                    );
                }
                if skills.has(Skill::FrozenMPRegen) {
                    modify_mana_event.send(ModifyManaEvent(
                        mana_regen.0 + skills.get_count(Skill::MPRegen) * 5,
                    ));
                }
            }
            if let Some(p) = poison_option {
                if skills.has(Skill::ViralVenum) {
                    for (mob_e, txfm) in neaby_mobs.iter() {
                        if mob_pos.translation().distance(txfm.translation()) < 3. * TILE_SIZE.x {
                            commands.entity(mob_e).insert(Burning {
                                damage: p.damage,
                                duration_timer: Timer::from_seconds(
                                    p.duration_timer.duration().as_secs_f32(),
                                    TimerMode::Once,
                                ),
                                tick_timer: p.tick_timer.clone(),
                            });
                            status_event.send(StatusEffectEvent {
                                entity: mob_e,
                                effect: StatusEffect::Poison,
                                num_stacks: 1,
                            });
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
