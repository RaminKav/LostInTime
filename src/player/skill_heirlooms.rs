use bevy::prelude::*;
use bevy::utils::Duration;
use bevy_proto::prelude::ProtoCommands;
use bevy_rapier2d::prelude::Collider;
use rand::Rng;

use crate::{
    ai::FollowState,
    attributes::{Attack, CurrentHealth, MaxHealth},
    audio::{AudioSoundEffect, SoundSpawner},
    custom_commands::CommandsExt,
    enemy::Mob,
    inputs::{CursorPos, FacingDirection},
    item::{
        potion_buffs::AttackSpeedBuff,
        projectile::{Projectile, RangedAttackEvent},
        WorldObject,
    },
    player::{
        mage_skills::TeleportState,
        melee_skills::SpearState,
        rogue_skills::{LungeState, SprintState},
        skills::{
            ActiveSkill, ActiveSkillUsedEvent, BuckshotSkillState, DruidTreeSkillState,
            FirePillarState, HealSkillState, Heirloom, IceWallSkillState, PlayerSkills,
            RapidfireState, SkillChargeTracker, StealthState,
        },
        Player,
    },
    proto::proto_param::ProtoParam,
    GameParam,
};

// Temporary marker components for active effects
#[derive(Component)]
pub struct Stealthed;
#[derive(Component)]
pub struct RapidfireBuff {
    pub attack_speed_bonus: f32,
    pub timer: Timer,
}

pub fn handle_active_skill_event(
    mut events: EventReader<ActiveSkillUsedEvent>,
    mut commands: Commands,
    mut players: Query<
        (
            Entity,
            &mut PlayerSkills,
            &GlobalTransform,
            Option<&StealthState>,
            Option<&RapidfireState>,
            Option<&FirePillarState>,
            Option<&HealSkillState>,
            Option<&BuckshotSkillState>,
            Option<&IceWallSkillState>,
            Option<&DruidTreeSkillState>,
            Option<&Attack>,
            &mut CurrentHealth,
            &MaxHealth,
            &FacingDirection,
        ),
        With<Player>,
    >,
    sprint_states: Query<&SprintState, With<Player>>,
    spear_states: Query<&SpearState, With<Player>>,
    lunge_states: Query<&LungeState, With<Player>>,
    mut teleport_states: Query<&mut TeleportState, With<Player>>,
    mut charge_tracker: Query<&mut SkillChargeTracker, With<Player>>,
    time: Res<Time>,
    cursor: Res<CursorPos>,
    asset_server: Res<AssetServer>,
    mut ranged_attack_events: EventWriter<RangedAttackEvent>,
    mut proto_commands: ProtoCommands,
    proto_param: ProtoParam,
) {
    for ev in events.iter() {
        for (
            player_e,
            skills,
            player_txfm,
            stealth_state,
            rapid_state,
            pillar_state,
            heal_state,
            buckshot_state,
            icewall_state,
            druidtree_state,
            attack_opt,
            mut health,
            max_health,
            facing_dir,
        ) in players.iter_mut()
        {
            // Credit Card
            let credit_card_count = skills.get_count(Heirloom::CreditCard);
            for _ in 0..credit_card_count {
                if ev.slot != 1 {
                    continue; // only trigger on class skill
                }
                //spawn a coin for each
                let mut rng = rand::thread_rng();
                let d = 15.0;
                let drop_offset = Vec2::new(rng.gen_range(-d..d), rng.gen_range(-d..d));
                proto_commands.spawn_item_from_proto(
                    WorldObject::Coin,
                    &proto_param,
                    player_txfm.translation().truncate() + drop_offset,
                    1,
                    None,
                );
            }

            // Get legacy skill states from separate queries
            let sprint_state = sprint_states.get(player_e).ok();
            let spear_state = spear_states.get(player_e).ok();
            let lunge_state = lunge_states.get(player_e).ok();
            let teleport_state = teleport_states.get_mut(player_e).ok();
            // Apply multiplicative cooldown logic is handled in skills when inserted
            let slot_skill = if ev.slot == 0 {
                skills.active_skill_slot_1.as_ref()
            } else {
                skills.active_skill_slot_2.as_ref()
            };
            if let Some(active) = slot_skill {
                // For slot 1 (class skill), handle charge consumption
                let mut should_start_cooldown = true;
                if ev.slot == 1 {
                    if let Ok(mut tracker) = charge_tracker.get_mut(player_e) {
                        if tracker.current_charges > 0 {
                            // Consume a charge
                            tracker.current_charges -= 1;
                            // Only start cooldown when we have 0 charges (not when below max)
                            should_start_cooldown = tracker.current_charges == 0;

                            // If we still have charges remaining, start the charge regeneration cooldown
                            if tracker.current_charges < tracker.max_charges {
                                // Reset the charge regeneration cooldown timer
                                tracker.cooldown_timer = Timer::from_seconds(
                                    tracker.base_cooldown * skills.skill_cooldown_multiplier(),
                                    TimerMode::Once,
                                );
                            }
                        }
                    }
                }

                match active.active_skill {
                    ActiveSkill::Stealth => {
                        // respect cooldown if state exists and we're not using a charge
                        if should_start_cooldown {
                            if let Some(s) = stealth_state {
                                if !s.cooldown_timer.finished() {
                                    continue;
                                }
                            }
                        } else {
                            // If using a charge, remove any existing skill state to prevent blocking
                            if stealth_state.is_some() {
                                commands.entity(player_e).remove::<StealthState>();
                            }
                        }
                        let mut cd = Timer::from_seconds(
                            ev.cooldown * skills.skill_cooldown_multiplier(),
                            TimerMode::Once,
                        );
                        if !should_start_cooldown {
                            // If using a charge, don't start cooldown yet - set timer to finished
                            cd.tick(Duration::from_secs_f32(cd.duration().as_secs_f32()));
                        }
                        // Don't tick the timer here - let tick_skill_cooldowns handle it
                        let power_mult = skills.skill_power_multiplier();
                        let mut dur = Timer::from_seconds(2.0 * power_mult, TimerMode::Once);
                        dur.tick(time.delta());
                        commands
                            .entity(player_e)
                            .insert(StealthState {
                                duration: dur,
                                cooldown_timer: cd,
                            })
                            .insert(Stealthed);

                        // Spawn cosmetic smoke effect on top of player
                        let player_pos = player_txfm.translation().truncate();
                        ranged_attack_events.send(RangedAttackEvent {
                            projectile: Projectile::Smoke,
                            direction: Vec2::ZERO, // Smoke doesn't move
                            mana_cost: None,
                            from_enemy: false,
                            from_entity: Some(player_e),
                            is_followup_proj: false,
                            dmg_override: Some(0), // Cosmetic only, no damage
                            pos_override: Some(player_pos),
                            spawn_delay: 0.0,
                        });

                        commands.spawn(SoundSpawner::new(AudioSoundEffect::GainExp, 0.12));
                    }
                    ActiveSkill::Rapidfire => {
                        if should_start_cooldown {
                            if let Some(r) = rapid_state {
                                if !r.cooldown_timer.finished() {
                                    continue;
                                }
                            }
                        } else {
                            // If using a charge, remove any existing skill state to prevent blocking
                            if rapid_state.is_some() {
                                commands.entity(player_e).remove::<RapidfireState>();
                            }
                        }
                        let mut cd = Timer::from_seconds(
                            ev.cooldown * skills.skill_cooldown_multiplier(),
                            TimerMode::Once,
                        );
                        if !should_start_cooldown {
                            // If using a charge, don't start cooldown yet - set timer to finished
                            cd.tick(Duration::from_secs_f32(cd.duration().as_secs_f32()));
                        }
                        // Don't tick the timer here - let tick_skill_cooldowns handle it
                        let power_mult = skills.skill_power_multiplier();
                        let mut dur = Timer::from_seconds(3.0 * power_mult, TimerMode::Once);
                        dur.tick(time.delta());
                        commands
                            .entity(player_e)
                            .insert(RapidfireState {
                                duration: dur,
                                cooldown_timer: cd,
                                attack_speed_bonus: 1.6,
                            })
                            .insert(AttackSpeedBuff::new(3.0 * power_mult, 1.6));
                        commands.spawn(SoundSpawner::new(AudioSoundEffect::GainExp, 0.12));
                    }
                    ActiveSkill::FirePillar => {
                        if should_start_cooldown {
                            if let Some(p) = pillar_state {
                                if !p.cooldown_timer.finished() {
                                    continue;
                                }
                            }
                        } else {
                            // If using a charge, remove any existing skill state to prevent blocking
                            if pillar_state.is_some() {
                                commands.entity(player_e).remove::<FirePillarState>();
                            }
                        }
                        let mut cd = Timer::from_seconds(
                            ev.cooldown * skills.skill_cooldown_multiplier(),
                            TimerMode::Once,
                        );
                        if !should_start_cooldown {
                            // If using a charge, don't start cooldown yet - set timer to finished
                            cd.tick(Duration::from_secs_f32(cd.duration().as_secs_f32()));
                        }
                        // Don't tick the timer here - let tick_skill_cooldowns handle it
                        commands
                            .entity(player_e)
                            .insert(FirePillarState { cooldown_timer: cd });
                        // spawn fire ring projectile at cursor world position with player's attack as damage
                        let power_mult = skills.skill_power_multiplier();
                        let base_dmg: i32 = attack_opt.map(|a| a.0).unwrap_or(10);
                        let dmg = (base_dmg as f32 * power_mult) as i32;
                        let pos = cursor.world_coords.truncate();
                        ranged_attack_events.send(RangedAttackEvent {
                            projectile: Projectile::FireRing,
                            direction: Vec2::ZERO, // Fire ring doesn't move
                            mana_cost: None,
                            from_enemy: false,
                            from_entity: None,
                            is_followup_proj: false,
                            dmg_override: Some(dmg),
                            pos_override: Some(pos),
                            spawn_delay: 0.0,
                        });
                        commands.spawn(SoundSpawner::new(AudioSoundEffect::IceExplosion, 0.4));
                    }
                    ActiveSkill::Heal => {
                        if should_start_cooldown {
                            if let Some(h) = heal_state {
                                if !h.cooldown_timer.finished() {
                                    continue;
                                }
                            }
                        } else {
                            // If using a charge, remove any existing skill state to prevent blocking
                            if heal_state.is_some() {
                                commands.entity(player_e).remove::<HealSkillState>();
                            }
                        }
                        let mut cd = Timer::from_seconds(
                            ev.cooldown * skills.skill_cooldown_multiplier(),
                            TimerMode::Once,
                        );
                        if !should_start_cooldown {
                            // If using a charge, don't start cooldown yet - set timer to finished
                            cd.tick(Duration::from_secs_f32(cd.duration().as_secs_f32()));
                        }
                        // Don't tick the timer here - let tick_skill_cooldowns handle it
                        commands
                            .entity(player_e)
                            .insert(HealSkillState { cooldown_timer: cd });
                        // Heal for 30% of max health (placeholder value), increased by skill power
                        let power_mult = skills.skill_power_multiplier();
                        let heal_amount = (max_health.0 as f32 * 0.3 * power_mult) as i32;
                        health.0 = (health.0 + heal_amount).min(max_health.0);
                        commands.spawn(SoundSpawner::new(AudioSoundEffect::GainExp, 0.12));
                    }
                    ActiveSkill::Buckshot => {
                        if should_start_cooldown {
                            if let Some(b) = buckshot_state {
                                if !b.cooldown_timer.finished() {
                                    continue;
                                }
                            }
                        } else {
                            // If using a charge, remove any existing skill state to prevent blocking
                            if buckshot_state.is_some() {
                                commands.entity(player_e).remove::<BuckshotSkillState>();
                            }
                        }
                        let mut cd = Timer::from_seconds(
                            ev.cooldown * skills.skill_cooldown_multiplier(),
                            TimerMode::Once,
                        );
                        if !should_start_cooldown {
                            // If using a charge, don't start cooldown yet - set timer to finished
                            cd.tick(Duration::from_secs_f32(cd.duration().as_secs_f32()));
                        }
                        // Don't tick the timer here - let tick_skill_cooldowns handle it
                        commands
                            .entity(player_e)
                            .insert(BuckshotSkillState { cooldown_timer: cd });

                        // Calculate direction to cursor
                        let player_pos = player_txfm.translation().truncate();
                        let cursor_pos = cursor.world_coords.truncate();
                        let direction_to_cursor = (cursor_pos - player_pos).normalize_or_zero();

                        // Spawn cosmetic buckshot animation in the direction of the cursor
                        // The projectile system will handle rotation based on direction
                        let spawn_offset = -30.0; // pixels in front of player
                        let animation_pos = player_pos + direction_to_cursor * spawn_offset;
                        ranged_attack_events.send(RangedAttackEvent {
                            projectile: Projectile::Buckshot,
                            direction: direction_to_cursor,
                            mana_cost: None,
                            from_enemy: false,
                            from_entity: Some(player_e),
                            is_followup_proj: false,
                            dmg_override: Some(0), // Cosmetic only, no damage
                            pos_override: Some(animation_pos),
                            spawn_delay: 0.0,
                        });

                        // Spawn 5 bullets in a spread pattern (placeholder: 5 bullets, 10 degree spread)
                        let bullet_count = 6;
                        let spread_angle = 7.0_f32.to_radians();
                        let base_angle = direction_to_cursor.y.atan2(direction_to_cursor.x);
                        for i in 0..bullet_count {
                            let angle_offset =
                                (i as f32 - (bullet_count - 1) as f32 / 2.0) * spread_angle;
                            let bullet_dir = Vec2::from_angle(base_angle + angle_offset);

                            let power_mult = skills.skill_power_multiplier();
                            let bullet_dmg = attack_opt.map(|a| (a.0 as f32 * power_mult) as i32);
                            ranged_attack_events.send(RangedAttackEvent {
                                projectile: Projectile::Bullet,
                                direction: bullet_dir,
                                mana_cost: None,
                                from_enemy: false,
                                from_entity: Some(player_e),
                                is_followup_proj: true,
                                dmg_override: bullet_dmg,
                                pos_override: None,
                                spawn_delay: 0.0,
                            });
                        }

                        // Bounce player back in opposite direction to cursor
                        // Insert BounceEffect directly with opposite direction
                        use crate::bounce::BounceEffect;
                        let bounce_direction = -direction_to_cursor;
                        let bounce_speed = 300.0; // placeholder speed
                        commands.entity(player_e).insert(BounceEffect::new(
                            player_pos,
                            bounce_direction,
                            bounce_speed,
                            false, // dash_boost
                            0.35,  // duration
                            20.0,  // max_height
                        ));
                        commands.spawn(SoundSpawner::new(AudioSoundEffect::Bow, 0.4));
                    }
                    ActiveSkill::IceWall => {
                        if should_start_cooldown {
                            if let Some(i) = icewall_state {
                                if !i.cooldown_timer.finished() {
                                    continue;
                                }
                            }
                        } else {
                            // If using a charge, remove any existing skill state to prevent blocking
                            if icewall_state.is_some() {
                                commands.entity(player_e).remove::<IceWallSkillState>();
                            }
                        }
                        let mut cd = Timer::from_seconds(
                            ev.cooldown * skills.skill_cooldown_multiplier(),
                            TimerMode::Once,
                        );
                        if !should_start_cooldown {
                            // If using a charge, don't start cooldown yet - set timer to finished
                            cd.tick(Duration::from_secs_f32(cd.duration().as_secs_f32()));
                        }
                        // Don't tick the timer here - let tick_skill_cooldowns handle it
                        commands
                            .entity(player_e)
                            .insert(IceWallSkillState { cooldown_timer: cd });
                        // Placeholder: spawn ice explosion at cursor for now
                        let power_mult = skills.skill_power_multiplier();
                        let base_dmg: i32 = attack_opt.map(|a| a.0).unwrap_or(10);
                        let dmg = (base_dmg as f32 * power_mult) as i32;
                        let pos = cursor.world_coords.truncate() + Vec2::new(0., 32.); // slight offset so it appears below cursor

                        ranged_attack_events.send(RangedAttackEvent {
                            projectile: Projectile::IceWall,
                            direction: Vec2::ZERO,
                            mana_cost: None,
                            from_enemy: false,
                            from_entity: None,
                            is_followup_proj: false,
                            dmg_override: Some(dmg),
                            pos_override: Some(pos),
                            spawn_delay: 0.0,
                        });
                        commands.spawn(SoundSpawner::new(AudioSoundEffect::IceExplosion, 0.4));
                    }
                    ActiveSkill::DruidTree => {
                        if should_start_cooldown {
                            if let Some(d) = druidtree_state {
                                if !d.cooldown_timer.finished() {
                                    continue;
                                }
                            }
                        } else {
                            // If using a charge, remove any existing skill state to prevent blocking
                            if druidtree_state.is_some() {
                                commands.entity(player_e).remove::<DruidTreeSkillState>();
                            }
                        }
                        let mut cd = Timer::from_seconds(
                            ev.cooldown * skills.skill_cooldown_multiplier(),
                            TimerMode::Once,
                        );
                        if !should_start_cooldown {
                            // If using a charge, don't start cooldown yet - set timer to finished
                            cd.tick(Duration::from_secs_f32(cd.duration().as_secs_f32()));
                        }
                        // Don't tick the timer here - let tick_skill_cooldowns handle it
                        commands
                            .entity(player_e)
                            .insert(DruidTreeSkillState { cooldown_timer: cd });

                        // Spawn a non-damageable dummy tree at cursor position
                        let dummy_pos = cursor.world_coords + Vec3::new(0., 0., 990.);
                        commands.spawn((
                            SpriteBundle {
                                sprite: Sprite {
                                    color: Color::rgba(0.4, 0.6, 0.2, 1.0),
                                    custom_size: Some(Vec2::new(24.0, 32.0)),
                                    ..default()
                                },
                                transform: Transform::from_translation(dummy_pos),
                                ..default()
                            },
                            Collider::cuboid(12.0, 16.0),
                            Name::new("DruidTreeDummy"),
                            DruidTreeDummy {
                                timer: Timer::from_seconds(
                                    2.0 * skills.skill_power_multiplier(),
                                    TimerMode::Once,
                                ),
                            },
                        ));

                        commands.spawn(SoundSpawner::new(AudioSoundEffect::GainExp, 0.12));
                    }
                    ActiveSkill::Sprint => {
                        if should_start_cooldown {
                            if let Some(s) = sprint_state {
                                if !s.sprint_cooldown_timer.finished() {
                                    continue;
                                }
                            }
                        } else {
                            // If using a charge, remove any existing skill state to prevent blocking
                            if sprint_state.is_some() {
                                commands.entity(player_e).remove::<SprintState>();
                            }
                        }
                        // Sprint is activated by inserting Sprinting component, handled in rogue_skills.rs
                        // Just update cooldown here
                        let mut cd = Timer::from_seconds(
                            ev.cooldown * skills.skill_cooldown_multiplier(),
                            TimerMode::Once,
                        );
                        if !should_start_cooldown {
                            // If using a charge, don't start cooldown yet - set timer to finished
                            cd.tick(Duration::from_secs_f32(cd.duration().as_secs_f32()));
                        }
                        // Don't tick the timer here - let tick_skill_cooldowns handle it
                        // Update or create sprint state cooldown
                        if sprint_state.is_some() {
                            commands.entity(player_e).remove::<SprintState>();
                        }
                        commands.entity(player_e).insert(SprintState {
                            startup_timer: Timer::from_seconds(0.17, TimerMode::Once),
                            sprint_duration_timer: Timer::from_seconds(2.5, TimerMode::Once),
                            sprint_cooldown_timer: cd,
                            speed_bonus: 1.6,
                        });
                        // Insert Sprinting component to activate sprint
                        use crate::player::rogue_skills::Sprinting;
                        commands.entity(player_e).insert(Sprinting);
                    }
                    ActiveSkill::Teleport => {
                        // For teleport, we need to handle charge-based cooldown reset
                        // The actual teleport activation happens in handle_teleport
                        if let Some(mut teleport) = teleport_state {
                            if should_start_cooldown {
                                // No charges left, start full cooldown
                                let cooldown = ev.cooldown * skills.skill_cooldown_multiplier();
                                teleport.cooldown_timer =
                                    Timer::from_seconds(cooldown, TimerMode::Once);
                            } else {
                                // We have charges remaining, reset cooldown to start charge regeneration
                                if let Ok(tracker) = charge_tracker.get(player_e) {
                                    if tracker.current_charges < tracker.max_charges {
                                        teleport.cooldown_timer = Timer::from_seconds(
                                            tracker.base_cooldown
                                                * skills.skill_cooldown_multiplier(),
                                            TimerMode::Once,
                                        );
                                    }
                                }
                            }
                        }
                    }
                    ActiveSkill::ParrySpear => {
                        if should_start_cooldown {
                            if let Some(s) = spear_state {
                                if !s.cooldown_timer.finished() {
                                    continue;
                                }
                            }
                        } else {
                            if spear_state.is_some() {
                                commands.entity(player_e).remove::<SpearState>();
                            }
                        }
                        let mut cd = Timer::from_seconds(
                            ev.cooldown * skills.skill_cooldown_multiplier(),
                            TimerMode::Once,
                        );
                        if !should_start_cooldown {
                            // If using a charge, don't start cooldown yet - set timer to finished
                            cd.tick(Duration::from_secs_f32(cd.duration().as_secs_f32()));
                        }
                        // Don't tick the timer here - let tick_skill_cooldowns handle it
                        if spear_state.is_some() {
                            commands.entity(player_e).remove::<SpearState>();
                        }
                        commands.entity(player_e).insert(SpearState {
                            cooldown_timer: cd,
                            spear_timer: Timer::from_seconds(0.5, TimerMode::Once),
                        });
                    }
                    ActiveSkill::SprintLunge => {
                        if should_start_cooldown {
                            if let Some(l) = lunge_state {
                                if !l.lunge_cooldown_timer.finished() {
                                    continue;
                                }
                            }
                        } else {
                            if lunge_state.is_some() {
                                commands.entity(player_e).remove::<LungeState>();
                            }
                        }
                        let mut cd = Timer::from_seconds(
                            ev.cooldown * skills.skill_cooldown_multiplier(),
                            TimerMode::Once,
                        );
                        if !should_start_cooldown {
                            // If using a charge, don't start cooldown yet - set timer to finished
                            cd.tick(Duration::from_secs_f32(cd.duration().as_secs_f32()));
                        }
                        // Don't tick the timer here - let tick_skill_cooldowns handle it
                        if lunge_state.is_some() {
                            commands.entity(player_e).remove::<LungeState>();
                        }
                        commands.entity(player_e).insert(LungeState {
                            lunge_cooldown_timer: cd,
                            lunge_duration: Timer::from_seconds(0.69, TimerMode::Once),
                            lunge_speed: 3.9,
                        });
                    }
                    _ => {}
                }
                // Skill Echo trigger: spawn an echo AoE at player position when using any skill
                if skills.has(crate::player::skills::Heirloom::SkillEcho) {
                    let echo_dmg = attack_opt.map(|a| (a.0 as f32 * 1.) as i32).unwrap_or(15);
                    crate::player::melee_skills::spawn_echo_hitbox(
                        &mut commands,
                        &asset_server,
                        player_e,
                        echo_dmg,
                    );
                }
            }
        }
    }
}

pub fn tick_stealth_and_buffs(
    mut commands: Commands,
    time: Res<Time>,
    mut stealth: Query<(Entity, &mut StealthState), With<Stealthed>>,
    mut rapid: Query<(Entity, &mut RapidfireBuff)>,
) {
    for (e, mut s) in stealth.iter_mut() {
        s.duration.tick(time.delta());
        if s.duration.finished() {
            commands.entity(e).remove::<Stealthed>();
        }
    }
    for (e, mut r) in rapid.iter_mut() {
        r.timer.tick(time.delta());
        if r.timer.finished() {
            commands.entity(e).remove::<RapidfireBuff>();
        }
    }
}

// Component for druid tree dummy
#[derive(Component)]
pub struct DruidTreeDummy {
    pub timer: Timer,
}

pub fn tick_skill_cooldowns(
    mut commands: Commands,
    time: Res<Time>,
    mut stealth_cd: Query<(Entity, &mut StealthState)>,
    mut rapid_cd: Query<(Entity, &mut RapidfireState)>,
    mut pillar_cd: Query<(Entity, &mut FirePillarState)>,
    mut heal_cd: Query<(Entity, &mut HealSkillState)>,
    mut buckshot_cd: Query<(Entity, &mut BuckshotSkillState)>,
    mut icewall_cd: Query<(Entity, &mut IceWallSkillState)>,
    mut druidtree_cd: Query<(Entity, &mut DruidTreeSkillState)>,
    mut dummy_query: Query<(Entity, &mut DruidTreeDummy)>,
) {
    for (e, mut s) in stealth_cd.iter_mut() {
        s.cooldown_timer.tick(time.delta());
        if s.cooldown_timer.finished() && s.duration.percent() == 0.0 {
            commands.entity(e).remove::<StealthState>();
        }
    }
    for (e, mut r) in rapid_cd.iter_mut() {
        r.cooldown_timer.tick(time.delta());
        if r.cooldown_timer.finished() && r.duration.percent() == 0.0 {
            commands.entity(e).remove::<RapidfireState>();
        }
    }
    for (e, mut p) in pillar_cd.iter_mut() {
        p.cooldown_timer.tick(time.delta());
        if p.cooldown_timer.finished() {
            commands.entity(e).remove::<FirePillarState>();
        }
    }
    for (e, mut h) in heal_cd.iter_mut() {
        h.cooldown_timer.tick(time.delta());
        if h.cooldown_timer.finished() {
            commands.entity(e).remove::<HealSkillState>();
        }
    }
    for (e, mut b) in buckshot_cd.iter_mut() {
        b.cooldown_timer.tick(time.delta());
        if b.cooldown_timer.finished() {
            commands.entity(e).remove::<BuckshotSkillState>();
        }
    }
    for (e, mut i) in icewall_cd.iter_mut() {
        i.cooldown_timer.tick(time.delta());
        if i.cooldown_timer.finished() {
            commands.entity(e).remove::<IceWallSkillState>();
        }
    }
    for (e, mut d) in druidtree_cd.iter_mut() {
        d.cooldown_timer.tick(time.delta());
        if d.cooldown_timer.finished() {
            commands.entity(e).remove::<DruidTreeSkillState>();
        }
    }
    // Despawn druid tree dummy after duration
    for (e, mut dummy) in dummy_query.iter_mut() {
        dummy.timer.tick(time.delta());
        if dummy.timer.finished() {
            commands.entity(e).despawn_recursive();
        }
    }
}

/// System to make enemies target DruidTree dummies instead of the player
/// This MUST run before the follow system to prevent crashes when dummies despawn
pub fn handle_druid_tree_taunt(
    dummies: Query<(Entity, &GlobalTransform), With<DruidTreeDummy>>,
    mut enemies: Query<(&GlobalTransform, &mut FollowState), (With<Mob>, Without<DruidTreeDummy>)>,
    game: GameParam,
    transforms: Query<&GlobalTransform>,
    dummy_timers: Query<&DruidTreeDummy>,
) {
    // Collect all active dummy entities
    let active_dummy_entities: std::collections::HashSet<Entity> =
        dummies.iter().map(|(e, _)| e).collect();

    // Find all active dummies and taunt nearby enemies
    for (dummy_entity, dummy_txfm) in dummies.iter() {
        let dummy_pos = dummy_txfm.translation().truncate();
        let taunt_range = 200.0; // placeholder: 200 pixel taunt range

        // Check if dummy is about to despawn (timer finished)
        let is_about_to_despawn = dummy_timers
            .get(dummy_entity)
            .map(|d| d.timer.finished())
            .unwrap_or(false);

        // Find all enemies within taunt range
        for (enemy_txfm, mut follow_state) in enemies.iter_mut() {
            let enemy_pos = enemy_txfm.translation().truncate();
            let distance = (dummy_pos - enemy_pos).length();

            // If enemy is targeting this dummy and it's about to despawn, redirect immediately
            if follow_state.target == dummy_entity && is_about_to_despawn {
                follow_state.target = game.game.player;
                continue;
            }

            // If enemy is within range and not already targeting this dummy, redirect them
            if distance <= taunt_range {
                // Only update if they're currently targeting the player
                if follow_state.target == game.game.player {
                    follow_state.target = dummy_entity;
                }
            }
        }
    }

    // For enemies that were targeting a dummy that no longer exists, redirect back to player
    for (_enemy_txfm, mut follow_state) in enemies.iter_mut() {
        // Check if the target entity still exists
        if let Ok(_) = transforms.get(follow_state.target) {
            // If target exists, check if it's still a valid dummy
            if !active_dummy_entities.contains(&follow_state.target)
                && follow_state.target != game.game.player
            {
                // Target is not an active dummy and not the player, redirect to player
                follow_state.target = game.game.player;
            }
        } else {
            // Target entity doesn't exist, redirect to player immediately
            follow_state.target = game.game.player;
        }
    }
}

/// Darken the player's atlas sprite when stealthed, restore when not.
pub fn update_stealth_color(
    mut sprites: Query<(&mut TextureAtlasSprite, Option<&Stealthed>), With<Player>>,
) {
    for (mut sprite, stealth) in sprites.iter_mut() {
        if stealth.is_some() {
            // Darken but keep visible
            sprite.color = Color::rgba(0.2, 0.2, 0.2, 0.3);
        } else {
            // Restore default
            sprite.color = Color::WHITE;
        }
    }
}

/// Regenerates skill charges when cooldown finishes
/// Only regenerates if charges are below max
pub fn regenerate_skill_charges(
    time: Res<Time>,
    mut charge_trackers: Query<&mut crate::player::skills::SkillChargeTracker, With<Player>>,
) {
    for mut tracker in charge_trackers.iter_mut() {
        // Only tick cooldown if we're below max charges
        if tracker.current_charges < tracker.max_charges {
            tracker.cooldown_timer.tick(time.delta());
            if tracker.cooldown_timer.finished() {
                // Regenerate a charge
                tracker.current_charges += 1;
                // Reset cooldown timer for next charge
                tracker.cooldown_timer =
                    Timer::from_seconds(tracker.base_cooldown, TimerMode::Once);
            }
        }
    }
}

/// Initializes or updates the skill charge tracker when PlayerSkills changes
/// Only applies to slot 1 (class skill, not Roll)
pub fn initialize_skill_charge_tracker(
    mut commands: Commands,
    players: Query<
        (
            Entity,
            &PlayerSkills,
            Option<&crate::player::skills::SkillChargeTracker>,
        ),
        (With<Player>, Changed<PlayerSkills>),
    >,
) {
    for (player_e, skills, existing_tracker) in players.iter() {
        // Only track charges for slot 1 (class skill)
        if let Some(slot_2_skill) = &skills.active_skill_slot_2 {
            let max_charges = 1 + skills.skill_extra_charges();
            let base_cooldown = slot_2_skill.active_skill.get_base_cooldown();

            if let Some(tracker) = existing_tracker {
                // Update existing tracker - preserve timer state
                let elapsed = tracker.cooldown_timer.elapsed();
                let duration = tracker.cooldown_timer.duration();
                let mut new_timer = Timer::from_seconds(duration.as_secs_f32(), TimerMode::Once);
                new_timer.tick(elapsed);
                let new_tracker = crate::player::skills::SkillChargeTracker {
                    current_charges: tracker.current_charges.min(max_charges), // Cap at new max
                    max_charges,
                    cooldown_timer: new_timer,
                    base_cooldown,
                };
                commands.entity(player_e).insert(new_tracker);
            } else {
                // Initialize new tracker with max charges
                let mut init_timer = Timer::from_seconds(base_cooldown, TimerMode::Once);
                init_timer.tick(Duration::from_secs_f32(base_cooldown)); // Start finished
                commands
                    .entity(player_e)
                    .insert(crate::player::skills::SkillChargeTracker {
                        current_charges: max_charges,
                        max_charges,
                        cooldown_timer: init_timer,
                        base_cooldown,
                    });
            }
        } else {
            // No skill in slot 2, remove tracker if it exists
            if existing_tracker.is_some() {
                commands
                    .entity(player_e)
                    .remove::<crate::player::skills::SkillChargeTracker>();
            }
        }
    }
}
