use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::utils::Duration;
use bevy_proto::prelude::ProtoCommands;
use bevy_rapier2d::prelude::Collider;
use rand::Rng;

use crate::{
    ai::FollowState,
    attributes::{Attack, AttackCooldown, CurrentHealth, MaxHealth},
    audio::{AudioSoundEffect, SoundSpawner},
    combat::HitEvent,
    custom_commands::CommandsExt,
    enemy::Mob,
    inputs::{CursorPos, FacingDirection},
    item::{
        projectile::{Projectile, RangedAttackEvent},
        WorldObject,
    },
    player::{
        mage_skills::TeleportState,
        melee_skills::SpearState,
        rogue_skills::{LungeState, SprintState},
        skills::{
            ActiveSkill, ActiveSkillUsedEvent, BuckshotSkillState, DruidTreeSkillState,
            FirePillarState, HealSkillState, Heirloom, IceWallSkillState, PiercingStarSkillState,
            PlayerSkills, RapidfireState, ShoutSkillState, Slot1ChargeTracker, Slot2ChargeTracker,
            StealthState,
        },
        Player,
    },
    proto::proto_param::ProtoParam,
    GameParam,
};

// Temporary marker components for active effects
#[derive(Component)]
pub struct Stealthed;

#[derive(SystemParam)]
pub struct SkillStateQueries<'w, 's> {
    pub stealth_states: Query<'w, 's, &'static StealthState, With<Player>>,
    pub rapidfire_states: Query<'w, 's, &'static RapidfireState, With<Player>>,
    pub fire_pillar_states: Query<'w, 's, &'static FirePillarState, With<Player>>,
    pub heal_states: Query<'w, 's, &'static HealSkillState, With<Player>>,
    pub buckshot_states: Query<'w, 's, &'static BuckshotSkillState, With<Player>>,
    pub icewall_states: Query<'w, 's, &'static IceWallSkillState, With<Player>>,
    pub druidtree_states: Query<'w, 's, &'static DruidTreeSkillState, With<Player>>,
    pub shout_states: Query<'w, 's, &'static ShoutSkillState, With<Player>>,
    pub piercing_star_states: Query<'w, 's, &'static PiercingStarSkillState, With<Player>>,
    pub sprint_states: Query<'w, 's, &'static SprintState, With<Player>>,
    pub spear_states: Query<'w, 's, &'static SpearState, With<Player>>,
    pub lunge_states: Query<'w, 's, &'static LungeState, With<Player>>,
    pub teleport_states: Query<'w, 's, &'static mut TeleportState, With<Player>>,
    pub slot1_trackers: Query<'w, 's, &'static mut Slot1ChargeTracker, With<Player>>,
    pub slot2_trackers: Query<'w, 's, &'static mut Slot2ChargeTracker, With<Player>>,
    pub player_projectile_size:
        Query<'w, 's, &'static crate::attributes::ProjectileSize, With<Player>>,
}

pub fn handle_active_skill_event(
    mut events: EventReader<ActiveSkillUsedEvent>,
    mut commands: Commands,
    mut players: Query<
        (
            Entity,
            &mut PlayerSkills,
            &GlobalTransform,
            Option<&Attack>,
            &mut CurrentHealth,
            &MaxHealth,
            &FacingDirection,
        ),
        With<Player>,
    >,
    mut skill_states: SkillStateQueries,
    time: Res<Time>,
    cursor: Res<CursorPos>,
    asset_server: Res<AssetServer>,
    mut ranged_attack_events: EventWriter<RangedAttackEvent>,
    mut proto_commands: ProtoCommands,
    proto_param: ProtoParam,
) {
    for ev in events.iter() {
        for (player_e, skills, player_txfm, attack_opt, mut health, max_health, facing_dir) in
            players.iter_mut()
        {
            // Get optional states from separate queries
            let stealth_state = skill_states.stealth_states.get(player_e).ok();
            let rapid_state = skill_states.rapidfire_states.get(player_e).ok();
            let pillar_state = skill_states.fire_pillar_states.get(player_e).ok();
            let heal_state = skill_states.heal_states.get(player_e).ok();
            let buckshot_state = skill_states.buckshot_states.get(player_e).ok();
            let icewall_state = skill_states.icewall_states.get(player_e).ok();
            let druidtree_state = skill_states.druidtree_states.get(player_e).ok();
            let shout_state = skill_states.shout_states.get(player_e).ok();
            // Credit Card
            let credit_card_count = skills.get_count(Heirloom::CreditCard);
            for _ in 0..credit_card_count {
                if ev.slot != 1 && ev.slot != 2 {
                    continue; // only trigger on class skills (slots 1 and 2)
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
            let sprint_state = skill_states.sprint_states.get(player_e).ok();
            let spear_state = skill_states.spear_states.get(player_e).ok();
            let lunge_state = skill_states.lunge_states.get(player_e).ok();
            let teleport_state = skill_states.teleport_states.get_mut(player_e).ok();
            // Apply multiplicative cooldown logic is handled in skills when inserted
            let slot_skill = match ev.slot {
                0 => skills.active_skill_slot_1.as_ref(),
                1 => skills.active_skill_slot_2.as_ref(),
                2 => skills.active_skill_slot_3.as_ref(),
                _ => None,
            };
            if let Some(active) = slot_skill {
                // For slot 1 and 2 (class skills), handle charge consumption from their independent trackers
                let mut should_start_cooldown = true;
                if ev.slot == 1 {
                    // Use slot 1's independent tracker
                    if let Ok(mut tracker) = skill_states.slot1_trackers.get_mut(player_e) {
                        if tracker.0.current_charges > 0 {
                            tracker.0.current_charges -= 1;
                            should_start_cooldown = tracker.0.current_charges == 0;
                            if tracker.0.current_charges < tracker.0.max_charges {
                                tracker.0.cooldown_timer = Timer::from_seconds(
                                    tracker.0.base_cooldown * skills.skill_cooldown_multiplier(),
                                    TimerMode::Once,
                                );
                            }
                        }
                    }
                } else if ev.slot == 2 {
                    // Use slot 2's independent tracker
                    if let Ok(mut tracker) = skill_states.slot2_trackers.get_mut(player_e) {
                        if tracker.0.current_charges > 0 {
                            tracker.0.current_charges -= 1;
                            should_start_cooldown = tracker.0.current_charges == 0;
                            if tracker.0.current_charges < tracker.0.max_charges {
                                tracker.0.cooldown_timer = Timer::from_seconds(
                                    tracker.0.base_cooldown * skills.skill_cooldown_multiplier(),
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
                        let _player_pos = player_txfm.translation().truncate();
                        ranged_attack_events.send(RangedAttackEvent {
                            projectile: Projectile::Smoke,
                            direction: Vec2::ZERO, // Smoke doesn't move
                            mana_cost: None,
                            from_enemy: false,
                            from_entity: Some(player_e),
                            is_followup_proj: false,
                            dmg_override: Some(0), // Cosmetic only, no damage
                            pos_override: None,
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
                        let dur = Timer::from_seconds(3.0 * power_mult, TimerMode::Once);
                        info!("dur: {:?}", dur);
                        commands.entity(player_e).insert(RapidfireState {
                            duration: dur,
                            cooldown_timer: cd,
                            attack_speed_bonus: 1.6,
                        });

                        // Spawn cosmetic attack speed effect on top of player
                        let player_pos = player_txfm.translation().truncate();
                        ranged_attack_events.send(RangedAttackEvent {
                            projectile: Projectile::AttackSpeed,
                            direction: Vec2::ZERO, // Doesn't move
                            mana_cost: None,
                            from_enemy: false,
                            from_entity: Some(player_e),
                            is_followup_proj: false,
                            dmg_override: Some(0), // Cosmetic only, no damage
                            pos_override: Some(player_pos + Vec2::new(0., 32.)),
                            spawn_delay: 0.0,
                        });

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
                        commands.entity(player_e).insert(FirePillarState {
                            cooldown_timer: cd,
                            hit_clear_timer: Timer::from_seconds(0.75, TimerMode::Repeating),
                        });
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

                        // Spawn cosmetic heal hearts effect on top of player
                        let player_pos = player_txfm.translation().truncate();
                        ranged_attack_events.send(RangedAttackEvent {
                            projectile: Projectile::HealHearts,
                            direction: Vec2::ZERO, // Doesn't move
                            mana_cost: None,
                            from_enemy: false,
                            from_entity: Some(player_e),
                            is_followup_proj: false,
                            dmg_override: Some(0), // Cosmetic only, no damage
                            pos_override: Some(player_pos + Vec2::new(0., 16.)),
                            spawn_delay: 0.0,
                        });

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
                        let dmg = (base_dmg as f32 * power_mult * 2.) as i32; // ice wall does double base dmg
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
                    ActiveSkill::Shout => {
                        if should_start_cooldown {
                            if let Some(s) = shout_state {
                                if !s.cooldown_timer.finished() {
                                    continue;
                                }
                            }
                        } else {
                            // If using a charge, remove any existing skill state to prevent blocking
                            if shout_state.is_some() {
                                commands.entity(player_e).remove::<ShoutSkillState>();
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
                            .insert(ShoutSkillState { cooldown_timer: cd });

                        // Spawn Shout projectile at player position (AoE burst around player)
                        let player_pos = player_txfm.translation().truncate();
                        let power_mult = skills.skill_power_multiplier();
                        let base_dmg: i32 = attack_opt.map(|a| a.0).unwrap_or(10);
                        let dmg = (base_dmg as f32 * power_mult) as i32;

                        ranged_attack_events.send(RangedAttackEvent {
                            projectile: Projectile::Shout,
                            direction: Vec2::ZERO, // AoE doesn't need direction
                            mana_cost: None,
                            from_enemy: false,
                            from_entity: Some(player_e),
                            is_followup_proj: false,
                            dmg_override: Some(dmg),
                            pos_override: None,
                            spawn_delay: 0.0,
                        });

                        commands.spawn(SoundSpawner::new(AudioSoundEffect::GainExp, 0.12));
                    }
                    ActiveSkill::PiercingStar => {
                        if should_start_cooldown {
                            if let Ok(p) = skill_states.piercing_star_states.get(player_e) {
                                if !p.cooldown_timer.finished() {
                                    continue;
                                }
                            }
                        } else {
                            // If using a charge, remove any existing skill state to prevent blocking
                            if skill_states.piercing_star_states.get(player_e).is_ok() {
                                commands.entity(player_e).remove::<PiercingStarSkillState>();
                            }
                        }

                        // Calculate direction to cursor first - validate before setting cooldown
                        let player_pos = player_txfm.translation().truncate();
                        let cursor_pos = cursor.world_coords.truncate();
                        let direction_to_cursor = (cursor_pos - player_pos).normalize_or_zero();

                        // Only proceed if we have a valid direction (not zero)
                        if direction_to_cursor.length_squared() < 0.01 {
                            // Cursor is too close to player, skip spawning but don't set cooldown
                            continue;
                        }

                        // Now set cooldown since we're about to spawn the projectile
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
                            .insert(PiercingStarSkillState { cooldown_timer: cd });

                        // Spawn ThrowingStarLarge projectile
                        let power_mult = skills.skill_power_multiplier();
                        let base_dmg: i32 = attack_opt.map(|a| a.0).unwrap_or(10);
                        let dmg = (base_dmg as f32 * power_mult) as i32;

                        ranged_attack_events.send(RangedAttackEvent {
                            projectile: Projectile::ThrowingStarLarge,
                            direction: direction_to_cursor,
                            mana_cost: None,
                            from_enemy: false,
                            from_entity: Some(player_e),
                            is_followup_proj: false,
                            dmg_override: Some(dmg),
                            pos_override: None,
                            spawn_delay: 0.0,
                        });
                        commands.spawn(SoundSpawner::new(AudioSoundEffect::Claw, 0.4));
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
                            startup_timer: Timer::from_seconds(0.0, TimerMode::Once),
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
                                // Get the appropriate tracker based on which slot teleport is in
                                if ev.slot == 1 {
                                    if let Ok(tracker) = skill_states.slot1_trackers.get(player_e) {
                                        if tracker.0.current_charges < tracker.0.max_charges {
                                            teleport.cooldown_timer = Timer::from_seconds(
                                                tracker.0.base_cooldown
                                                    * skills.skill_cooldown_multiplier(),
                                                TimerMode::Once,
                                            );
                                        }
                                    }
                                } else if ev.slot == 2 {
                                    if let Ok(tracker) = skill_states.slot2_trackers.get(player_e) {
                                        if tracker.0.current_charges < tracker.0.max_charges {
                                            teleport.cooldown_timer = Timer::from_seconds(
                                                tracker.0.base_cooldown
                                                    * skills.skill_cooldown_multiplier(),
                                                TimerMode::Once,
                                            );
                                        }
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
                            lunge_duration: Timer::from_seconds(0.42, TimerMode::Once),
                            lunge_speed: 9.5,
                        });
                    }
                    _ => {}
                }
                // Skill Echo trigger: spawn an echo AoE at player position when using any skill
                if active.active_skill != ActiveSkill::Roll
                    && skills.has(crate::player::skills::Heirloom::SkillEcho)
                {
                    let echo_dmg = attack_opt.map(|a| (a.0 as f32 * 1.) as i32).unwrap_or(15);
                    let size_mult = skill_states
                        .player_projectile_size
                        .get_single()
                        .map(|s| s.get_multiplier())
                        .unwrap_or(1.0);
                    crate::player::melee_skills::spawn_echo_hitbox(
                        &mut commands,
                        &asset_server,
                        player_e,
                        echo_dmg,
                        size_mult,
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
    mut rapid: Query<(Entity, &mut RapidfireState)>,
    mut attribute_event: EventWriter<crate::attributes::AttributeChangeEvent>,
) {
    for (e, mut s) in stealth.iter_mut() {
        s.duration.tick(time.delta());
        if s.duration.finished() {
            commands.entity(e).remove::<Stealthed>();
        }
    }
    // Tick RapidfireState duration timer
    for (e, mut r) in rapid.iter_mut() {
        r.duration.tick(time.delta());
        if r.duration.finished() {
            info!("Removing RapidfireState from entity {:?}", e);
            commands.entity(e).remove::<RapidfireState>();
            // Trigger attribute recalculation to reset AttackCooldown
            attribute_event.send_default();
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
    mut shout_cd: Query<(Entity, &mut ShoutSkillState)>,
    mut piercing_star_cd: Query<(Entity, &mut PiercingStarSkillState)>,
    mut sprint_cd: Query<(Entity, &mut SprintState)>,
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
        // Remove RapidfireState when both cooldown is finished AND duration has expired
        if r.cooldown_timer.finished() && r.duration.finished() {
            commands.entity(e).remove::<RapidfireState>();
        }
    }
    for (e, mut p) in pillar_cd.iter_mut() {
        p.cooldown_timer.tick(time.delta());
        p.hit_clear_timer.tick(time.delta());
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
    for (e, mut s) in shout_cd.iter_mut() {
        s.cooldown_timer.tick(time.delta());
        if s.cooldown_timer.finished() {
            commands.entity(e).remove::<ShoutSkillState>();
        }
    }
    for (e, mut p) in piercing_star_cd.iter_mut() {
        p.cooldown_timer.tick(time.delta());
        if p.cooldown_timer.finished() {
            commands.entity(e).remove::<PiercingStarSkillState>();
        }
    }
    // Tick Sprint cooldown - this ensures it ticks even while sprinting
    for (_e, mut sprint) in sprint_cd.iter_mut() {
        sprint.sprint_cooldown_timer.tick(time.delta());
    }
    // Despawn druid tree dummy after duration
    for (e, mut dummy) in dummy_query.iter_mut() {
        dummy.timer.tick(time.delta());
        if dummy.timer.finished() {
            commands.entity(e).despawn_recursive();
        }
    }
}

/// Clear hit_entities for FireRing projectiles every 1.0s while FirePillar is active
pub fn handle_fire_pillar_hit_clear(
    mut fire_pillar_states: Query<&mut FirePillarState>,
    mut fire_ring_projectiles: Query<
        (
            &mut crate::item::projectile::ProjectileState,
            &crate::item::projectile::Projectile,
        ),
        With<crate::item::projectile::Projectile>,
    >,
) {
    for mut pillar_state in fire_pillar_states.iter_mut() {
        if pillar_state.hit_clear_timer.just_finished() {
            info!("Clearing hit_entities for FireRing projectiles due to FirePillar effect");
            // Clear hit_entities for all FireRing projectiles
            for (mut proj_state, proj) in fire_ring_projectiles.iter_mut() {
                if matches!(proj, crate::item::projectile::Projectile::FireRing) {
                    info!("Clearing hit_entities for FireRing projectile");
                    proj_state.hit_entities.clear();
                    pillar_state.hit_clear_timer.reset();
                }
            }
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
    mut slot1_trackers: Query<&mut Slot1ChargeTracker, With<Player>>,
    mut slot2_trackers: Query<&mut Slot2ChargeTracker, With<Player>>,
) {
    // Regenerate charges for slot 1
    for mut tracker in slot1_trackers.iter_mut() {
        if tracker.0.current_charges < tracker.0.max_charges {
            tracker.0.cooldown_timer.tick(time.delta());
            if tracker.0.cooldown_timer.finished() {
                tracker.0.current_charges += 1;
                tracker.0.cooldown_timer =
                    Timer::from_seconds(tracker.0.base_cooldown, TimerMode::Once);
            }
        }
    }
    // Regenerate charges for slot 2
    for mut tracker in slot2_trackers.iter_mut() {
        if tracker.0.current_charges < tracker.0.max_charges {
            tracker.0.cooldown_timer.tick(time.delta());
            if tracker.0.cooldown_timer.finished() {
                tracker.0.current_charges += 1;
                tracker.0.cooldown_timer =
                    Timer::from_seconds(tracker.0.base_cooldown, TimerMode::Once);
            }
        }
    }
}

/// Initializes or updates the skill charge trackers when PlayerSkills changes
/// Creates separate trackers for slot 1 and slot 2 class skills
pub fn initialize_skill_charge_tracker(
    mut commands: Commands,
    players: Query<Entity, (With<Player>, Changed<PlayerSkills>)>,
    player_skills: Query<&PlayerSkills, With<Player>>,
    mut slot1_trackers: Query<&mut Slot1ChargeTracker>,
    mut slot2_trackers: Query<&mut Slot2ChargeTracker>,
) {
    for player_e in players.iter() {
        let Ok(skills) = player_skills.get(player_e) else {
            continue;
        };
        let max_charges = 1 + skills.skill_extra_charges();

        // Manage tracker for slot 1 (active_skill_slot_2)
        if let Some(slot_2_skill) = &skills.active_skill_slot_2 {
            let base_cooldown = slot_2_skill.active_skill.get_base_cooldown();

            if let Ok(mut tracker) = slot1_trackers.get_mut(player_e) {
                // Update existing tracker - preserve current charges and timer
                let elapsed = tracker.0.cooldown_timer.elapsed();
                tracker.0.current_charges = tracker.0.current_charges.min(max_charges);
                tracker.0.max_charges = max_charges;
                tracker.0.base_cooldown = base_cooldown;
                // Only update duration if it changed
                if (tracker.0.cooldown_timer.duration().as_secs_f32() - base_cooldown).abs() > 0.01
                {
                    let mut new_timer = Timer::from_seconds(base_cooldown, TimerMode::Once);
                    new_timer.tick(elapsed);
                    tracker.0.cooldown_timer = new_timer;
                }
            } else {
                // Create new tracker with max charges
                let mut init_timer = Timer::from_seconds(base_cooldown, TimerMode::Once);
                init_timer.tick(Duration::from_secs_f32(base_cooldown)); // Start finished
                commands.entity(player_e).insert(Slot1ChargeTracker(
                    crate::player::skills::SkillChargeTracker {
                        current_charges: max_charges,
                        max_charges,
                        cooldown_timer: init_timer,
                        base_cooldown,
                    },
                ));
            }
        } else {
            // Remove tracker if skill no longer exists
            if slot1_trackers.get(player_e).is_ok() {
                commands.entity(player_e).remove::<Slot1ChargeTracker>();
            }
        }

        // Manage tracker for slot 2 (active_skill_slot_3)
        if let Some(slot_3_skill) = &skills.active_skill_slot_3 {
            let base_cooldown = slot_3_skill.active_skill.get_base_cooldown();

            if let Ok(mut tracker) = slot2_trackers.get_mut(player_e) {
                // Update existing tracker - preserve current charges and timer
                let elapsed = tracker.0.cooldown_timer.elapsed();
                tracker.0.current_charges = tracker.0.current_charges.min(max_charges);
                tracker.0.max_charges = max_charges;
                tracker.0.base_cooldown = base_cooldown;
                // Only update duration if it changed
                if (tracker.0.cooldown_timer.duration().as_secs_f32() - base_cooldown).abs() > 0.01
                {
                    let mut new_timer = Timer::from_seconds(base_cooldown, TimerMode::Once);
                    new_timer.tick(elapsed);
                    tracker.0.cooldown_timer = new_timer;
                }
            } else {
                // Create new tracker with max charges
                let mut init_timer = Timer::from_seconds(base_cooldown, TimerMode::Once);
                init_timer.tick(Duration::from_secs_f32(base_cooldown)); // Start finished
                commands.entity(player_e).insert(Slot2ChargeTracker(
                    crate::player::skills::SkillChargeTracker {
                        current_charges: max_charges,
                        max_charges,
                        cooldown_timer: init_timer,
                        base_cooldown,
                    },
                ));
            }
        } else {
            // Remove tracker if skill no longer exists
            if slot2_trackers.get(player_e).is_ok() {
                commands.entity(player_e).remove::<Slot2ChargeTracker>();
            }
        }
    }
}

/// System to trigger attribute recalculation when RapidfireState is added
/// Similar to trigger_attribute_update_on_buff_added for potions
pub fn trigger_attribute_update_on_rapidfire_added(
    added_rapidfire: Query<(), Added<RapidfireState>>,
    mut attribute_event: EventWriter<crate::attributes::AttributeChangeEvent>,
) {
    if !added_rapidfire.is_empty() {
        attribute_event.send_default();
    }
}

/// System to apply attack speed buff to player's attack cooldown
/// This runs in PostUpdate to ensure it runs after attribute recalculation
/// It applies the multiplier whenever attributes are recalculated while Rapidfire is active
pub fn apply_rapid_fire_speed_buff(
    mut player_query: Query<(&mut AttackCooldown, &RapidfireState), With<Player>>,
    mut att_events: EventReader<crate::attributes::AttributeChangeEvent>,
    added_rapidfire: Query<(), Added<RapidfireState>>,
) {
    // Check if attributes were recalculated this frame OR Rapidfire was just added
    let should_apply = !att_events.is_empty() || !added_rapidfire.is_empty();

    // Only apply if attributes were recalculated or Rapidfire was just activated
    if !should_apply {
        return;
    }

    for (mut attack_cooldown, buff) in player_query.iter_mut() {
        // Only apply if duration is active (not finished)
        if buff.duration.finished() {
            continue;
        }
        // Safety check: ensure cooldown is valid before division
        if attack_cooldown.0 > 0.0 && attack_cooldown.0.is_finite() && buff.attack_speed_bonus > 0.0
        {
            // Apply the multiplier (divide by the bonus)
            // This is safe because AttackCooldown is recalculated from scratch each time
            // attributes change, so we're not compounding the multiplier
            attack_cooldown.0 = attack_cooldown.0 / buff.attack_speed_bonus;
        }
    }
}

/// Reduces class skill cooldown when player lands a crit
/// Reduces by 0.1s per stack of CritSkillCooldownReduction heirloom
pub fn reduce_skill_cooldown_on_crit(
    mut hit_events: EventReader<HitEvent>,
    mut players: Query<
        (
            Entity,
            &PlayerSkills,
            Option<&mut StealthState>,
            Option<&mut RapidfireState>,
            Option<&mut FirePillarState>,
            Option<&mut HealSkillState>,
            Option<&mut BuckshotSkillState>,
            Option<&mut IceWallSkillState>,
            Option<&mut DruidTreeSkillState>,
            Option<&mut ShoutSkillState>,
        ),
        With<Player>,
    >,
    mut slot1_trackers: Query<&mut Slot1ChargeTracker>,
    mut slot2_trackers: Query<&mut Slot2ChargeTracker>,
    mut sprint_states: Query<&mut SprintState, With<Player>>,
    mut spear_states: Query<&mut SpearState, With<Player>>,
    mut lunge_states: Query<&mut LungeState, With<Player>>,
    mut teleport_states: Query<&mut TeleportState, With<Player>>,
) {
    for hit in hit_events.iter() {
        // Only process crits from player attacks (not from mobs hitting player)
        if !hit.was_crit || hit.hit_by_mob.is_some() {
            continue;
        }

        for (
            player_e,
            skills,
            stealth_state,
            rapid_state,
            pillar_state,
            heal_state,
            buckshot_state,
            icewall_state,
            druidtree_state,
            shout_state,
        ) in players.iter_mut()
        {
            let heirloom_count = skills.get_count(Heirloom::CritSkillCooldownReduction);
            if heirloom_count == 0 {
                continue;
            }

            let reduction = 0.1 * heirloom_count as f32;

            // Reduce cooldown for skill state components
            // Instead of creating a new timer, tick the existing timer forward by the reduction amount
            // This preserves the original duration for the HUD UI
            if let Some(mut state) = stealth_state {
                if !state.cooldown_timer.finished() {
                    state
                        .cooldown_timer
                        .tick(Duration::from_secs_f32(reduction));
                }
            }
            if let Some(mut state) = rapid_state {
                if !state.cooldown_timer.finished() {
                    state
                        .cooldown_timer
                        .tick(Duration::from_secs_f32(reduction));
                }
            }
            if let Some(mut state) = pillar_state {
                if !state.cooldown_timer.finished() {
                    state
                        .cooldown_timer
                        .tick(Duration::from_secs_f32(reduction));
                }
            }
            if let Some(mut state) = heal_state {
                if !state.cooldown_timer.finished() {
                    state
                        .cooldown_timer
                        .tick(Duration::from_secs_f32(reduction));
                }
            }
            if let Some(mut state) = buckshot_state {
                if !state.cooldown_timer.finished() {
                    state
                        .cooldown_timer
                        .tick(Duration::from_secs_f32(reduction));
                }
            }
            if let Some(mut state) = icewall_state {
                if !state.cooldown_timer.finished() {
                    state
                        .cooldown_timer
                        .tick(Duration::from_secs_f32(reduction));
                }
            }
            if let Some(mut state) = druidtree_state {
                if !state.cooldown_timer.finished() {
                    state
                        .cooldown_timer
                        .tick(Duration::from_secs_f32(reduction));
                }
            }
            if let Some(mut state) = shout_state {
                if !state.cooldown_timer.finished() {
                    state
                        .cooldown_timer
                        .tick(Duration::from_secs_f32(reduction));
                }
            }

            // Reduce cooldown for legacy skills
            if let Ok(mut state) = sprint_states.get_single_mut() {
                if !state.sprint_cooldown_timer.finished() {
                    state
                        .sprint_cooldown_timer
                        .tick(Duration::from_secs_f32(reduction));
                }
            }
            if let Ok(mut state) = spear_states.get_single_mut() {
                if !state.cooldown_timer.finished() {
                    state
                        .cooldown_timer
                        .tick(Duration::from_secs_f32(reduction));
                }
            }
            if let Ok(mut state) = lunge_states.get_single_mut() {
                if !state.lunge_cooldown_timer.finished() {
                    state
                        .lunge_cooldown_timer
                        .tick(Duration::from_secs_f32(reduction));
                }
            }
            if let Ok(mut state) = teleport_states.get_single_mut() {
                if !state.cooldown_timer.finished() {
                    state
                        .cooldown_timer
                        .tick(Duration::from_secs_f32(reduction));
                }
            }

            // Reduce cooldown for SkillChargeTracker (slot 1)
            // Reduce charge regeneration cooldown for slot 1
            if let Ok(mut tracker) = slot1_trackers.get_mut(player_e) {
                if tracker.0.current_charges < tracker.0.max_charges {
                    if !tracker.0.cooldown_timer.finished() {
                        tracker
                            .0
                            .cooldown_timer
                            .tick(Duration::from_secs_f32(reduction));
                    }
                }
            }
            // Reduce charge regeneration cooldown for slot 2
            if let Ok(mut tracker) = slot2_trackers.get_mut(player_e) {
                if tracker.0.current_charges < tracker.0.max_charges {
                    if !tracker.0.cooldown_timer.finished() {
                        tracker
                            .0
                            .cooldown_timer
                            .tick(Duration::from_secs_f32(reduction));
                    }
                }
            }
        }
    }
}

/// Handle CritHeal - crits have a chance to heal
pub fn handle_crit_heal(
    mut hit_events: EventReader<crate::combat::HitEvent>,
    player_query: Query<&PlayerSkills, With<crate::player::Player>>,
    mut modify_health_event: EventWriter<crate::attributes::modifiers::ModifyHealthEvent>,
) {
    let Ok(skills) = player_query.get_single() else {
        return;
    };

    let stacks = skills.get_count(Heirloom::CritHeal);
    if stacks <= 0 {
        return;
    }

    let mut rng = rand::thread_rng();

    for hit in hit_events.iter() {
        // Only process crits from player attacks (not from mobs hitting player)
        // Treat crit and overcrit the same
        if (!hit.was_crit && !hit.was_overcrit)
            || hit.hit_by_mob.is_some()
            || hit.from_heirloom_effect
        {
            continue;
        }

        let chance = 25 * stacks; // 25% per stack
        let heal_amount = if chance > 100 {
            // Past 100%, chance for 2 HP
            let extra_chance = chance - 100;
            if rng.gen_ratio(extra_chance.clamp(1, 100) as u32, 100) {
                2
            } else {
                1
            }
        } else if rng.gen_ratio(chance.clamp(1, 100) as u32, 100) {
            1
        } else {
            0
        };

        if heal_amount > 0 {
            modify_health_event.send(crate::attributes::modifiers::ModifyHealthEvent(heal_amount));
        }
    }
}
