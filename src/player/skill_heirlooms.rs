use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::utils::Duration;
use bevy_proto::prelude::{ProtoCommands, Prototypes};
use bevy_rapier2d::prelude::Collider;
use rand::{seq::SliceRandom, Rng};

use crate::{
    ai::FollowState,
    animations::player_sprite::PlayerAnimation,
    attributes::{
        attribute_helpers::skill_power_multiplier, Attack, AttackCooldown, BonusAttackSpeed,
        CurrentHealth, CurrentMana, MaxHealth, SkillPower,
    },
    audio::{AudioSoundEffect, SoundSpawner},
    blessings::{Blessing, OwnedBlessings},
    combat::{
        status_effects::{RapidfireSlow, StatusEffect, StatusEffectEvent},
        HitEvent,
    },
    cursor::CursorPos,
    custom_commands::CommandsExt,
    enemy::Mob,
    item::{
        projectile::{BombTarget, Projectile, ProjectileState, RangedAttackEvent},
        WorldObject,
    },
    player::{
        mage_skills::TeleportState,
        melee_skills::{spawn_echo_hitbox, SpearState},
        rogue_skills::{LungeState, SprintState},
        skills::{
            ActiveSkill, ActiveSkillUsedEvent, BombState, BuckshotSkillState, DaggerThrowState,
            DruidTreeSkillState, FirePillarState, FuryState, HealSkillState, Heirloom,
            IceWallSkillState, LaserBeamState, LightningState, PiercingStarSkillState,
            PlayerSkills, RapidfireState, ShoutSkillState, SlashState, Slot1ChargeTracker,
            Slot2ChargeTracker, Slot3ChargeTracker, Slot4ChargeTracker, SpinAttackState,
            StealthState, TripleThrowState,
        },
        Player,
    },
    proto::proto_param::ProtoParam,
    status_effects::Frail,
    world::TILE_SIZE,
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
    pub laser_beam_states: Query<'w, 's, &'static LaserBeamState, With<Player>>,
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
    pub slot3_trackers: Query<'w, 's, &'static mut Slot3ChargeTracker, With<Player>>,
    pub slot4_trackers: Query<'w, 's, &'static mut Slot4ChargeTracker, With<Player>>,
    pub player_projectile_size:
        Query<'w, 's, &'static crate::attributes::ProjectileSize, With<Player>>,
    pub lightning_states: Query<'w, 's, &'static LightningState, With<Player>>,
    pub daggerthrow_states: Query<'w, 's, &'static DaggerThrowState, With<Player>>,
    pub slash_states: Query<'w, 's, &'static SlashState, With<Player>>,
    pub triplethrow_states: Query<'w, 's, &'static TripleThrowState, With<Player>>,
    pub fury_states: Query<'w, 's, &'static FuryState, With<Player>>,
    pub bomb_states: Query<'w, 's, &'static BombState, With<Player>>,
    pub spinattack_states: Query<'w, 's, &'static SpinAttackState, With<Player>>,
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
            &OwnedBlessings,
            &mut CurrentMana,
            &SkillPower,
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
    prototypes: Prototypes,
    enemies: Query<(Entity, &GlobalTransform), With<Mob>>,
) {
    for ev in events.iter() {
        for (
            player_e,
            skills,
            player_txfm,
            attack_opt,
            mut health,
            max_health,
            blessings,
            mut current_mana,
            skill_power,
        ) in players.iter_mut()
        {
            // Get optional states from separate queries
            let stealth_state = skill_states.stealth_states.get(player_e).ok();
            let rapid_state = skill_states.rapidfire_states.get(player_e).ok();
            let pillar_state = skill_states.fire_pillar_states.get(player_e).ok();
            let laser_beam_state = skill_states.laser_beam_states.get(player_e).ok();
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
            let lightning_state = skill_states.lightning_states.get(player_e).ok();
            let daggerthrow_state = skill_states.daggerthrow_states.get(player_e).ok();
            let slash_state = skill_states.slash_states.get(player_e).ok();
            let triplethrow_state = skill_states.triplethrow_states.get(player_e).ok();
            let fury_state = skill_states.fury_states.get(player_e).ok();
            let bomb_state = skill_states.bomb_states.get(player_e).ok();
            let spinattack_state = skill_states.spinattack_states.get(player_e).ok();
            // Apply multiplicative cooldown logic is handled in skills when inserted
            let slot_skill = match ev.slot {
                0 => skills.active_skill_slot_0.as_ref(),
                1 => skills.active_skill_slot_1.as_ref(),
                2 => skills.active_skill_slot_2.as_ref(),
                3 => skills.active_skill_slot_3.as_ref(),
                4 => skills.active_skill_slot_4.as_ref(),
                _ => None,
            };
            if let Some(active) = slot_skill {
                info!("USED SLOT {}: {:?}", ev.slot, active.active_skill);
                let blessing_cd_mult = blessings.get_skill_cooldown_increase();
                if blessings.has_blessing(Blessing::SkillAttackSpeed) {
                    commands
                        .entity(player_e)
                        .insert(crate::item::potion_buffs::AttackSpeedBuff::new(2.0, 0.3));
                }
                // For slots 1-4 (class skills), handle charge consumption from their independent trackers
                let mut should_start_cooldown = true;
                if ev.slot == 0 {
                    // Use slot 1's independent tracker
                    if let Ok(mut tracker) = skill_states.slot1_trackers.get_mut(player_e) {
                        if tracker.0.current_charges > 0 {
                            tracker.0.current_charges -= 1;
                            should_start_cooldown = tracker.0.current_charges == 0;
                            if tracker.0.current_charges < tracker.0.max_charges {
                                tracker.0.cooldown_timer = Timer::from_seconds(
                                    tracker.0.base_cooldown
                                        * skills.skill_cooldown_multiplier()
                                        * blessing_cd_mult,
                                    TimerMode::Once,
                                );
                            }
                        }
                    }
                } else if ev.slot == 1 {
                    // Use slot 2's independent tracker
                    if let Ok(mut tracker) = skill_states.slot2_trackers.get_mut(player_e) {
                        if tracker.0.current_charges > 0 {
                            tracker.0.current_charges -= 1;
                            should_start_cooldown = tracker.0.current_charges == 0;
                            if tracker.0.current_charges < tracker.0.max_charges {
                                tracker.0.cooldown_timer = Timer::from_seconds(
                                    tracker.0.base_cooldown
                                        * skills.skill_cooldown_multiplier()
                                        * blessing_cd_mult,
                                    TimerMode::Once,
                                );
                            }
                        }
                    }
                } else if ev.slot == 2 {
                    // Use slot 3's independent tracker
                    if let Ok(mut tracker) = skill_states.slot3_trackers.get_mut(player_e) {
                        if tracker.0.current_charges > 0 {
                            tracker.0.current_charges -= 1;
                            should_start_cooldown = tracker.0.current_charges == 0;
                            if tracker.0.current_charges < tracker.0.max_charges {
                                tracker.0.cooldown_timer = Timer::from_seconds(
                                    tracker.0.base_cooldown
                                        * skills.skill_cooldown_multiplier()
                                        * blessing_cd_mult,
                                    TimerMode::Once,
                                );
                            }
                        }
                    }
                } else if ev.slot == 3 {
                    // Use slot 4's independent tracker
                    if let Ok(mut tracker) = skill_states.slot4_trackers.get_mut(player_e) {
                        if tracker.0.current_charges > 0 {
                            tracker.0.current_charges -= 1;
                            should_start_cooldown = tracker.0.current_charges == 0;
                            if tracker.0.current_charges < tracker.0.max_charges {
                                tracker.0.cooldown_timer = Timer::from_seconds(
                                    tracker.0.base_cooldown
                                        * skills.skill_cooldown_multiplier()
                                        * blessing_cd_mult,
                                    TimerMode::Once,
                                );
                            }
                        }
                    }
                }
                let power_mult =
                    skill_power_multiplier(skill_power, blessings.get_skill_power_bonus());
                let skill_cd = ev.cooldown * skills.skill_cooldown_multiplier() * blessing_cd_mult;
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
                        let mut cd = Timer::from_seconds(skill_cd, TimerMode::Once);
                        if !should_start_cooldown {
                            // If using a charge, don't start cooldown yet - set timer to finished
                            cd.tick(Duration::from_secs_f32(cd.duration().as_secs_f32()));
                        }
                        // Don't tick the timer here - let tick_skill_cooldowns handle it
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
                        let mut cd = Timer::from_seconds(skill_cd, TimerMode::Once);
                        if !should_start_cooldown {
                            // If using a charge, don't start cooldown yet - set timer to finished
                            cd.tick(Duration::from_secs_f32(cd.duration().as_secs_f32()));
                        }
                        // Don't tick the timer here - let tick_skill_cooldowns handle it
                        let dur = Timer::from_seconds(3.0, TimerMode::Once);
                        info!("dur: {:?}", dur);
                        commands.entity(player_e).insert(RapidfireState {
                            duration: dur,
                            cooldown_timer: cd,
                            attack_speed_bonus: 0.8 * power_mult,
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
                        let mut cd = Timer::from_seconds(skill_cd, TimerMode::Once);
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
                        let base_dmg: i32 = attack_opt.map(|a| a.0).unwrap_or(10);
                        let dmg = (base_dmg as f32 * power_mult * 0.55) as i32;
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
                        commands.spawn(SoundSpawner::new(AudioSoundEffect::IceExplosion, 0.2));
                    }
                    ActiveSkill::LaserBeam => {
                        if should_start_cooldown {
                            if let Some(l) = laser_beam_state {
                                if !l.cooldown_timer.finished() {
                                    continue;
                                }
                            }
                        } else {
                            if laser_beam_state.is_some() {
                                commands.entity(player_e).remove::<LaserBeamState>();
                            }
                        }
                        let mut cd = Timer::from_seconds(skill_cd, TimerMode::Once);
                        if !should_start_cooldown {
                            cd.tick(Duration::from_secs_f32(cd.duration().as_secs_f32()));
                        }
                        commands.entity(player_e).insert(LaserBeamState {
                            cooldown_timer: cd,
                            hit_clear_timer: Timer::from_seconds(0.5, TimerMode::Repeating),
                        });
                        let base_dmg: i32 = attack_opt.map(|a| a.0).unwrap_or(10);
                        let dmg = (base_dmg as f32 * power_mult * 0.6) as i32;
                        let player_pos = player_txfm.translation().truncate();
                        let direction =
                            (cursor.world_coords.truncate() - player_pos).normalize_or_zero();
                        ranged_attack_events.send(RangedAttackEvent {
                            projectile: Projectile::LaserBeam,
                            direction,
                            mana_cost: None,
                            from_enemy: false,
                            from_entity: Some(player_e),
                            is_followup_proj: false,
                            dmg_override: Some(dmg),
                            pos_override: Some(Vec2::ZERO),
                            spawn_delay: 0.0,
                        });
                        commands
                            .spawn(SoundSpawner::new(AudioSoundEffect::LightningStaffCast, 0.2));
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
                        let mut cd = Timer::from_seconds(skill_cd, TimerMode::Once);
                        if !should_start_cooldown {
                            // If using a charge, don't start cooldown yet - set timer to finished
                            cd.tick(Duration::from_secs_f32(cd.duration().as_secs_f32()));
                        }
                        // Don't tick the timer here - let tick_skill_cooldowns handle it
                        commands
                            .entity(player_e)
                            .insert(HealSkillState { cooldown_timer: cd });
                        // Heal for 30% of max health (placeholder value), increased by skill power
                        let heal_amount = (max_health.0 as f32 * 0.3 * power_mult) as i32;
                        health.0 = (health.0 + heal_amount).min(max_health.0);

                        // Spawn cosmetic heal hearts effect on top of player
                        ranged_attack_events.send(RangedAttackEvent {
                            projectile: Projectile::HealHearts,
                            direction: Vec2::ZERO, // Doesn't move
                            mana_cost: None,
                            from_enemy: false,
                            from_entity: Some(player_e),
                            is_followup_proj: false,
                            dmg_override: Some(0), // Cosmetic only, no damage
                            pos_override: Some(Vec2::new(0., 16.)),
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
                        let mut cd = Timer::from_seconds(skill_cd, TimerMode::Once);
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
                        let mut cd = Timer::from_seconds(skill_cd, TimerMode::Once);
                        if !should_start_cooldown {
                            // If using a charge, don't start cooldown yet - set timer to finished
                            cd.tick(Duration::from_secs_f32(cd.duration().as_secs_f32()));
                        }
                        // Don't tick the timer here - let tick_skill_cooldowns handle it
                        commands
                            .entity(player_e)
                            .insert(IceWallSkillState { cooldown_timer: cd });
                        // Placeholder: spawn ice explosion at cursor for now
                        let base_dmg: i32 = attack_opt.map(|a| a.0).unwrap_or(10);
                        let dmg = (base_dmg as f32 * power_mult * 3.) as i32; // ice wall does double base dmg
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
                        commands.spawn(SoundSpawner::new(AudioSoundEffect::IceExplosion, 0.2));
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
                        let mut cd = Timer::from_seconds(skill_cd, TimerMode::Once);
                        if !should_start_cooldown {
                            // If using a charge, don't start cooldown yet - set timer to finished
                            cd.tick(Duration::from_secs_f32(cd.duration().as_secs_f32()));
                        }
                        // Don't tick the timer here - let tick_skill_cooldowns handle it
                        commands
                            .entity(player_e)
                            .insert(DruidTreeSkillState { cooldown_timer: cd });

                        // Spawn a non-damageable dummy tree at cursor position
                        let dummy_pos = cursor.world_coords;
                        if let Some(p) = proto_commands.spawn_from_proto(
                            WorldObject::GreenSaplingStage2,
                            &prototypes,
                            dummy_pos.truncate(),
                        ) {
                            commands
                                .entity(p)
                                .insert(Collider::cuboid(12.0, 16.0))
                                .insert(Name::new("DruidTreeDummy"))
                                .insert(DruidTreeDummy {
                                    timer: Timer::from_seconds(2.0 * power_mult, TimerMode::Once),
                                });
                        }

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
                        let mut cd = Timer::from_seconds(skill_cd, TimerMode::Once);
                        if !should_start_cooldown {
                            // If using a charge, don't start cooldown yet - set timer to finished
                            cd.tick(Duration::from_secs_f32(cd.duration().as_secs_f32()));
                        }
                        // Don't tick the timer here - let tick_skill_cooldowns handle it
                        commands
                            .entity(player_e)
                            .insert(ShoutSkillState { cooldown_timer: cd });

                        let base_dmg: i32 = attack_opt.map(|a| a.0).unwrap_or(10);
                        let dmg = (base_dmg as f32 * power_mult * 1.6) as i32;

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
                        let mut cd = Timer::from_seconds(skill_cd, TimerMode::Once);
                        if !should_start_cooldown {
                            // If using a charge, don't start cooldown yet - set timer to finished
                            cd.tick(Duration::from_secs_f32(cd.duration().as_secs_f32()));
                        }
                        // Don't tick the timer here - let tick_skill_cooldowns handle it
                        commands
                            .entity(player_e)
                            .insert(PiercingStarSkillState { cooldown_timer: cd });

                        let base_dmg: i32 = attack_opt.map(|a| a.0).unwrap_or(10);
                        let dmg = (base_dmg as f32 * power_mult * 1.5) as i32;

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
                        commands.spawn(SoundSpawner::new(AudioSoundEffect::Claw, 0.2));
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
                        let mut cd = Timer::from_seconds(skill_cd, TimerMode::Once);
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
                                let cooldown = skill_cd;
                                teleport.cooldown_timer =
                                    Timer::from_seconds(cooldown, TimerMode::Once);
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
                        let mut cd = Timer::from_seconds(skill_cd, TimerMode::Once);
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
                        let mut cd = Timer::from_seconds(skill_cd, TimerMode::Once);
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
                    ActiveSkill::Lightning => {
                        if should_start_cooldown {
                            if let Some(l) = lightning_state {
                                if !l.cooldown_timer.finished() {
                                    continue;
                                }
                            }
                        } else {
                            if lightning_state.is_some() {
                                commands.entity(player_e).remove::<LightningState>();
                            }
                        }
                        let mut cd = Timer::from_seconds(skill_cd, TimerMode::Once);
                        if !should_start_cooldown {
                            cd.tick(Duration::from_secs_f32(cd.duration().as_secs_f32()));
                        }
                        commands
                            .entity(player_e)
                            .insert(LightningState { cooldown_timer: cd });

                        // Find 3 nearest enemies
                        let player_pos = player_txfm.translation().truncate();
                        let mut enemy_distances: Vec<(Entity, Vec2, f32)> = enemies
                            .iter()
                            .map(|(e, t)| {
                                let pos = t.translation().truncate();
                                let dist = player_pos.distance(pos);
                                (e, pos, dist)
                            })
                            .collect();
                        enemy_distances.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap());
                        enemy_distances.truncate(3);

                        // Spawn lightning at each enemy (using IceExplosionAOE as placeholder)
                        let base_dmg: i32 = attack_opt.map(|a| a.0).unwrap_or(10);
                        let dmg = (base_dmg as f32 * power_mult * 0.85) as i32;
                        for (_, enemy_pos, _) in enemy_distances {
                            ranged_attack_events.send(RangedAttackEvent {
                                projectile: Projectile::Lightning,
                                direction: Vec2::ZERO,
                                mana_cost: None,
                                from_enemy: false,
                                from_entity: None,
                                is_followup_proj: false,
                                dmg_override: Some(dmg),
                                pos_override: Some(enemy_pos + Vec2::new(0., 48.)),
                                spawn_delay: 0.0,
                            });
                        }
                        commands
                            .spawn(SoundSpawner::new(AudioSoundEffect::LightningStaffCast, 0.4));
                    }
                    ActiveSkill::DaggerThrow => {
                        if should_start_cooldown {
                            if let Some(d) = daggerthrow_state {
                                if !d.cooldown_timer.finished() {
                                    continue;
                                }
                            }
                        } else {
                            if daggerthrow_state.is_some() {
                                commands.entity(player_e).remove::<DaggerThrowState>();
                            }
                        }
                        let mut cd = Timer::from_seconds(skill_cd, TimerMode::Once);
                        if !should_start_cooldown {
                            cd.tick(Duration::from_secs_f32(cd.duration().as_secs_f32()));
                        }
                        commands
                            .entity(player_e)
                            .insert(DaggerThrowState { cooldown_timer: cd });

                        // Find 3 nearest enemies
                        let player_pos = player_txfm.translation().truncate();
                        let mut enemy_distances: Vec<(Entity, Vec2, f32)> = enemies
                            .iter()
                            .map(|(e, t)| {
                                let pos = t.translation().truncate();
                                let dist = player_pos.distance(pos);
                                (e, pos, dist)
                            })
                            .collect();
                        enemy_distances.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap());
                        enemy_distances.truncate(3);

                        // Throw 3 throwing stars towards nearest enemies
                        let base_dmg: i32 = attack_opt.map(|a| a.0).unwrap_or(10);
                        let dmg = (base_dmg as f32 * power_mult * 1.75) as i32;
                        for (_, enemy_pos, _) in enemy_distances {
                            let direction = (enemy_pos - player_pos).normalize_or_zero();
                            ranged_attack_events.send(RangedAttackEvent {
                                projectile: Projectile::DaggerThrow,
                                direction,
                                mana_cost: None,
                                from_enemy: false,
                                from_entity: Some(player_e),
                                is_followup_proj: false,
                                dmg_override: Some(dmg),
                                pos_override: None,
                                spawn_delay: 0.0,
                            });
                        }
                        commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonClick, 0.2));
                    }
                    ActiveSkill::DaggerSlash => {
                        if should_start_cooldown {
                            if let Some(s) = slash_state {
                                if !s.cooldown_timer.finished() {
                                    continue;
                                }
                            }
                        } else {
                            if slash_state.is_some() {
                                commands.entity(player_e).remove::<SlashState>();
                            }
                        }
                        let mut cd = Timer::from_seconds(skill_cd, TimerMode::Once);
                        if !should_start_cooldown {
                            cd.tick(Duration::from_secs_f32(cd.duration().as_secs_f32()));
                        }
                        commands
                            .entity(player_e)
                            .insert(SlashState { cooldown_timer: cd });

                        // Calculate direction to cursor (like dagger attack)
                        let player_pos = player_txfm.translation().truncate();
                        let cursor_pos = cursor.world_coords.truncate();
                        let direction = (cursor_pos - player_pos).normalize_or_zero();

                        // Spawn sword projectile in front of player
                        let base_dmg: i32 = attack_opt.map(|a| a.0).unwrap_or(10);
                        let dmg = (base_dmg as f32 * power_mult * 1.75) as i32;
                        ranged_attack_events.send(RangedAttackEvent {
                            projectile: Projectile::DaggerSlash,
                            direction,
                            mana_cost: None,
                            from_enemy: false,
                            from_entity: None,
                            is_followup_proj: false,
                            dmg_override: Some(dmg),
                            pos_override: Some(Vec2::ZERO),
                            spawn_delay: 0.0,
                        });
                        commands.spawn(SoundSpawner::new(AudioSoundEffect::SwordSwing, 0.3));
                    }
                    ActiveSkill::TripleThrow => {
                        if should_start_cooldown {
                            if let Some(t) = triplethrow_state {
                                if !t.cooldown_timer.finished() {
                                    continue;
                                }
                            }
                        } else {
                            if triplethrow_state.is_some() {
                                commands.entity(player_e).remove::<TripleThrowState>();
                            }
                        }
                        let mut cd = Timer::from_seconds(skill_cd, TimerMode::Once);
                        if !should_start_cooldown {
                            cd.tick(Duration::from_secs_f32(cd.duration().as_secs_f32()));
                        }
                        commands
                            .entity(player_e)
                            .insert(TripleThrowState { cooldown_timer: cd });

                        let player_pos = player_txfm.translation().truncate();
                        let cursor_pos = cursor.world_coords.truncate();
                        let base_direction = (cursor_pos - player_pos).normalize_or_zero();
                        let base_angle = base_direction.y.atan2(base_direction.x);

                        // Throw 3 throwing stars in a cone (15 degree spread)
                        let spread_angle = 15.0_f32.to_radians();
                        let base_dmg: i32 = attack_opt.map(|a| a.0).unwrap_or(10);
                        let dmg = (base_dmg as f32 * power_mult * 2.35) as i32;
                        for i in 0..3 {
                            let angle_offset = (i as f32 - 1.0) * spread_angle;
                            let angle = base_angle + angle_offset;
                            let direction = Vec2::new(angle.cos(), angle.sin());
                            ranged_attack_events.send(RangedAttackEvent {
                                projectile: Projectile::ThrowingStar,
                                direction,
                                mana_cost: None,
                                from_enemy: false,
                                from_entity: Some(player_e),
                                is_followup_proj: false,
                                dmg_override: Some(dmg),
                                pos_override: None,
                                spawn_delay: 0.0,
                            });
                        }
                        commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonClick, 0.2));
                        commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonClick, 0.2));
                    }
                    ActiveSkill::Fury => {
                        if should_start_cooldown {
                            if let Some(f) = fury_state {
                                if !f.cooldown_timer.finished() {
                                    continue;
                                }
                            }
                        } else {
                            if fury_state.is_some() {
                                commands.entity(player_e).remove::<FuryState>();
                            }
                        }
                        let mut cd = Timer::from_seconds(skill_cd, TimerMode::Once);
                        if !should_start_cooldown {
                            cd.tick(Duration::from_secs_f32(cd.duration().as_secs_f32()));
                        }
                        commands.entity(player_e).insert(FuryState {
                            cooldown_timer: cd,
                            duration: Timer::from_seconds(2.5, TimerMode::Once),
                            throw_timer: Timer::from_seconds(0.3, TimerMode::Repeating),
                        });
                        commands.spawn(SoundSpawner::new(AudioSoundEffect::GainExp, 0.2));
                    }
                    ActiveSkill::Bomb => {
                        if should_start_cooldown {
                            if let Some(b) = bomb_state {
                                if !b.cooldown_timer.finished() {
                                    continue;
                                }
                            }
                        } else {
                            if bomb_state.is_some() {
                                commands.entity(player_e).remove::<BombState>();
                            }
                        }
                        let mut cd = Timer::from_seconds(skill_cd, TimerMode::Once);
                        if !should_start_cooldown {
                            cd.tick(Duration::from_secs_f32(cd.duration().as_secs_f32()));
                        }
                        commands
                            .entity(player_e)
                            .insert(BombState { cooldown_timer: cd });

                        // Calculate direction to cursor position
                        let player_pos = player_txfm.translation().truncate();
                        let cursor_pos = cursor.world_coords.truncate();
                        let direction = (cursor_pos - player_pos).normalize_or_zero();

                        // Spawn bomb projectile toward cursor position
                        let base_dmg: i32 = attack_opt.map(|a| a.0).unwrap_or(10);
                        let dmg = (base_dmg as f32 * power_mult * 2.2) as i32;

                        // Store the target position for later attachment to the bomb projectile
                        // We'll attach it after the projectile spawns
                        commands.entity(player_e).insert(BombTarget {
                            target_pos: cursor_pos,
                        });

                        ranged_attack_events.send(RangedAttackEvent {
                            projectile: Projectile::Bomb,
                            direction,
                            mana_cost: None,
                            from_enemy: false,
                            from_entity: Some(player_e),
                            is_followup_proj: false,
                            dmg_override: Some(dmg),
                            pos_override: None,
                            spawn_delay: 0.0,
                        });
                    }
                    ActiveSkill::SpinAttack => {
                        if should_start_cooldown {
                            if let Some(s) = spinattack_state {
                                if !s.cooldown_timer.finished() {
                                    continue;
                                }
                            }
                        } else {
                            if spinattack_state.is_some() {
                                commands.entity(player_e).remove::<SpinAttackState>();
                            }
                        }
                        let mut cd = Timer::from_seconds(skill_cd, TimerMode::Once);
                        if !should_start_cooldown {
                            cd.tick(Duration::from_secs_f32(cd.duration().as_secs_f32()));
                        }
                        commands
                            .entity(player_e)
                            .insert(SpinAttackState { cooldown_timer: cd });

                        commands
                            .entity(player_e)
                            .insert(crate::item::potion_buffs::MovementSpeedBuff::new(0.45, 2.6));

                        let base_dmg: i32 = attack_opt.map(|a| a.0).unwrap_or(10);
                        let dmg = (base_dmg as f32 * power_mult * 0.8) as i32;
                        commands
                            .entity(player_e)
                            .insert(PlayerAnimation::SpinAttack);

                        ranged_attack_events.send(RangedAttackEvent {
                            projectile: Projectile::SpinAttack,
                            direction: Vec2::ZERO,
                            mana_cost: None,
                            from_enemy: false,
                            from_entity: Some(player_e),
                            is_followup_proj: false,
                            dmg_override: Some(dmg),
                            pos_override: Some(Vec2::ZERO),
                            spawn_delay: 0.0,
                        });

                        commands.spawn(SoundSpawner::new(AudioSoundEffect::SwordSwing, 0.2));
                    }
                    _ => {}
                }
                // Skill Echo trigger: spawn an echo AoE at player position when using any skill
                if active.active_skill != ActiveSkill::Roll
                    && skills.has(crate::player::skills::Heirloom::SkillEcho)
                {
                    let mana_cost = Heirloom::SkillEcho.get_mana_cost();
                    if current_mana.0 >= mana_cost {
                        current_mana.0 -= mana_cost;
                        let echo_dmg = attack_opt.map(|a| (a.0 as f32 * 1.) as i32).unwrap_or(15);
                        let size_mult = skill_states
                            .player_projectile_size
                            .get_single()
                            .map(|s| s.get_multiplier())
                            .unwrap_or(1.0);
                        spawn_echo_hitbox(
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
}

pub fn tick_stealth_and_buffs(
    mut commands: Commands,
    time: Res<Time>,
    mut stealth: Query<(Entity, &mut StealthState), With<Stealthed>>,
    mut rapid: Query<(Entity, &mut RapidfireState), With<Player>>,
    mut player_query: Query<&mut BonusAttackSpeed, With<Player>>,
    mut attribute_event: EventWriter<crate::attributes::AttributeChangeEvent>,
) {
    for (e, mut s) in stealth.iter_mut() {
        s.duration.tick(time.delta());
        if s.duration.finished() {
            commands.entity(e).remove::<Stealthed>();
        }
    }
    for (_e, mut r) in rapid.iter_mut() {
        let was_finished = r.duration.finished();
        r.duration.tick(time.delta());
        if !was_finished && r.duration.finished() {
            if let Ok(mut bonus_speed) = player_query.get_single_mut() {
                bonus_speed.remove_multiplier(r.attack_speed_bonus);
                attribute_event.send_default();
            }
        }
    }
}

/// Apply RapidfireSlow to all enemies when RapidFire is active
pub fn handle_rapidfire_slow_enemies(
    mut commands: Commands,
    rapidfire_states: Query<&RapidfireState, With<Player>>,
    enemies: Query<Entity, (With<Mob>, Without<RapidfireSlow>)>,
) {
    // Check if RapidFire is active
    if let Ok(state) = rapidfire_states.get_single() {
        if !state.duration.finished() && state.duration.percent() > 0. {
            for enemy_entity in enemies.iter() {
                commands.entity(enemy_entity).insert(RapidfireSlow);
            }
        }
    }
}

/// Remove RapidfireSlow from all enemies when RapidFire ends
pub fn handle_rapidfire_slow_remove(
    mut commands: Commands,
    rapidfire_states: Query<&RapidfireState, With<Player>>,
    enemies_with_slow: Query<Entity, (With<Mob>, With<RapidfireSlow>)>,
) {
    // Check if RapidFire is no longer active
    if let Ok(state) = rapidfire_states.get_single() {
        if state.duration.finished() {
            for enemy_entity in enemies_with_slow.iter() {
                commands.entity(enemy_entity).remove::<RapidfireSlow>();
            }
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
    mut laser_beam_cd: Query<(Entity, &mut LaserBeamState)>,
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
    for (e, mut l) in laser_beam_cd.iter_mut() {
        l.cooldown_timer.tick(time.delta());
        l.hit_clear_timer.tick(time.delta());
        if l.cooldown_timer.finished() {
            commands.entity(e).remove::<LaserBeamState>();
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

pub fn tick_new_skill_cooldowns(
    mut commands: Commands,
    time: Res<Time>,
    mut lightning_cd: Query<(Entity, &mut LightningState)>,
    mut daggerthrow_cd: Query<(Entity, &mut DaggerThrowState)>,
    mut slash_cd: Query<(Entity, &mut SlashState)>,
    mut triplethrow_cd: Query<(Entity, &mut TripleThrowState)>,
    mut fury_cd: Query<(Entity, &mut FuryState), With<Player>>,
    mut bomb_cd: Query<(Entity, &mut BombState)>,
    mut spinattack_cd: Query<(Entity, &mut SpinAttackState)>,
    attack_cooldown: Query<&AttackCooldown, With<Player>>,
) {
    for (e, mut l) in lightning_cd.iter_mut() {
        l.cooldown_timer.tick(time.delta());
        if l.cooldown_timer.finished() {
            commands.entity(e).remove::<LightningState>();
        }
    }
    for (e, mut d) in daggerthrow_cd.iter_mut() {
        d.cooldown_timer.tick(time.delta());
        if d.cooldown_timer.finished() {
            commands.entity(e).remove::<DaggerThrowState>();
        }
    }
    for (e, mut s) in slash_cd.iter_mut() {
        s.cooldown_timer.tick(time.delta());
        if s.cooldown_timer.finished() {
            commands.entity(e).remove::<SlashState>();
        }
    }
    for (e, mut t) in triplethrow_cd.iter_mut() {
        t.cooldown_timer.tick(time.delta());
        if t.cooldown_timer.finished() {
            commands.entity(e).remove::<TripleThrowState>();
        }
    }
    for (e, mut f) in fury_cd.iter_mut() {
        f.cooldown_timer.tick(time.delta());
        f.duration.tick(time.delta());

        // Scale throw timer based on attack speed
        let attack_speed_mult = if let Ok(cooldown) = attack_cooldown.get_single() {
            let reference_base_cooldown = 0.6;
            info!(
                "Current AttackCooldown: {:?} | {:?}",
                cooldown.0,
                (reference_base_cooldown / (2. * cooldown.0 - reference_base_cooldown)).max(0.1)
            );
            (reference_base_cooldown / (2. * cooldown.0 - reference_base_cooldown)).max(0.1)
        } else {
            1.0
        };

        // Higher attack speed = faster throws = timer ticks faster
        let scaled_delta = time.delta().mul_f32(attack_speed_mult);
        f.throw_timer.tick(scaled_delta);

        if f.cooldown_timer.finished() && f.duration.finished() {
            commands.entity(e).remove::<FuryState>();
        }
    }
    for (e, mut b) in bomb_cd.iter_mut() {
        b.cooldown_timer.tick(time.delta());
        if b.cooldown_timer.finished() {
            commands.entity(e).remove::<BombState>();
        }
    }
    for (e, mut s) in spinattack_cd.iter_mut() {
        s.cooldown_timer.tick(time.delta());
        if s.cooldown_timer.finished() {
            commands.entity(e).remove::<SpinAttackState>();
        }
    }
}

/// Clear hit_entities for FireRing projectiles every 1.0s while FirePillar is active
pub fn handle_fire_pillar_hit_clear(
    mut fire_pillar_states: Query<&mut FirePillarState>,
    mut fire_ring_projectiles: Query<(&mut ProjectileState, &Projectile), With<Projectile>>,
) {
    for mut pillar_state in fire_pillar_states.iter_mut() {
        if pillar_state.hit_clear_timer.just_finished() {
            info!("Clearing hit_entities for FireRing projectiles due to FirePillar effect");
            // Clear hit_entities for all FireRing projectiles
            for (mut proj_state, proj) in fire_ring_projectiles.iter_mut() {
                if matches!(proj, Projectile::FireRing) {
                    info!("Clearing hit_entities for FireRing projectile");
                    proj_state.hit_entities.clear();
                    pillar_state.hit_clear_timer.reset();
                }
            }
        }
    }
}

/// Clear hit_entities for LaserBeam projectiles periodically while LaserBeamState is active
pub fn handle_laser_beam_hit_clear(
    mut laser_beam_states: Query<&mut LaserBeamState>,
    mut laser_beam_projectiles: Query<(&mut ProjectileState, &Projectile), With<Projectile>>,
) {
    for mut laser_state in laser_beam_states.iter_mut() {
        if laser_state.hit_clear_timer.just_finished() {
            // Clear hit_entities for all LaserBeam projectiles
            for (mut proj_state, proj) in laser_beam_projectiles.iter_mut() {
                if matches!(proj, Projectile::LaserBeam) {
                    proj_state.hit_entities.clear();
                    laser_state.hit_clear_timer.reset();
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
    mut slot3_trackers: Query<&mut Slot3ChargeTracker, With<Player>>,
    mut slot4_trackers: Query<&mut Slot4ChargeTracker, With<Player>>,
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
    // Regenerate charges for slot 3
    for mut tracker in slot3_trackers.iter_mut() {
        if tracker.0.current_charges < tracker.0.max_charges {
            tracker.0.cooldown_timer.tick(time.delta());
            if tracker.0.cooldown_timer.finished() {
                tracker.0.current_charges += 1;
                tracker.0.cooldown_timer =
                    Timer::from_seconds(tracker.0.base_cooldown, TimerMode::Once);
            }
        }
    }
    // Regenerate charges for slot 4
    for mut tracker in slot4_trackers.iter_mut() {
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
/// Creates separate trackers for slots 1-4 class skills
pub fn initialize_skill_charge_tracker(
    mut commands: Commands,
    players: Query<Entity, (With<Player>, Changed<PlayerSkills>)>,
    player_skills: Query<&PlayerSkills, With<Player>>,
    mut slot1_trackers: Query<&mut Slot1ChargeTracker>,
    mut slot2_trackers: Query<&mut Slot2ChargeTracker>,
    mut slot3_trackers: Query<&mut Slot3ChargeTracker>,
    mut slot4_trackers: Query<&mut Slot4ChargeTracker>,
) {
    for player_e in players.iter() {
        let Ok(skills) = player_skills.get(player_e) else {
            continue;
        };
        let max_charges = 1 + skills.skill_extra_charges();

        // Manage tracker for slot 1 (active_skill_slot_1)
        if let Some(slot_1_skill) = &skills.active_skill_slot_0 {
            let base_cooldown = slot_1_skill.active_skill.get_base_cooldown();
            let current_skill = slot_1_skill.active_skill.clone();

            if let Ok(mut tracker) = slot1_trackers.get_mut(player_e) {
                // Check if the skill changed
                let skill_changed = tracker.0.tracked_skill != current_skill;

                if skill_changed {
                    // Skill changed - reset tracker completely with new skill
                    let mut init_timer = Timer::from_seconds(base_cooldown, TimerMode::Once);
                    init_timer.tick(Duration::from_secs_f32(base_cooldown));
                    tracker.0.current_charges = max_charges;
                    tracker.0.max_charges = max_charges;
                    tracker.0.base_cooldown = base_cooldown;
                    tracker.0.cooldown_timer = init_timer;
                    tracker.0.tracked_skill = current_skill;
                } else {
                    // Same skill - update max_charges and grant extra charges if heirloom was acquired
                    let old_max = tracker.0.max_charges;
                    tracker.0.max_charges = max_charges;

                    // If max_charges increased, grant the extra charges immediately
                    if max_charges > old_max {
                        let extra_charges = max_charges - old_max;
                        tracker.0.current_charges =
                            (tracker.0.current_charges + extra_charges).min(max_charges);
                    } else {
                        // Cap current charges at new max (in case max decreased)
                        tracker.0.current_charges = tracker.0.current_charges.min(max_charges);
                    }

                    tracker.0.base_cooldown = base_cooldown;
                    // Only update duration if it changed (and skill didn't change)
                    if (tracker.0.cooldown_timer.duration().as_secs_f32() - base_cooldown).abs()
                        > 0.01
                    {
                        let elapsed = tracker.0.cooldown_timer.elapsed();
                        let mut new_timer = Timer::from_seconds(base_cooldown, TimerMode::Once);
                        new_timer.tick(elapsed);
                        tracker.0.cooldown_timer = new_timer;
                    }
                }
            } else {
                // Create new tracker with max charges
                let mut init_timer = Timer::from_seconds(base_cooldown, TimerMode::Once);
                init_timer.tick(Duration::from_secs_f32(base_cooldown));
                commands.entity(player_e).insert(Slot1ChargeTracker(
                    crate::player::skills::SkillChargeTracker {
                        current_charges: max_charges,
                        max_charges,
                        cooldown_timer: init_timer,
                        base_cooldown,
                        tracked_skill: current_skill,
                    },
                ));
            }
        } else {
            // Remove tracker if skill no longer exists
            if slot1_trackers.get(player_e).is_ok() {
                commands.entity(player_e).remove::<Slot1ChargeTracker>();
            }
        }

        // Manage tracker for slot 2 (active_skill_slot_2)
        if let Some(slot_2_skill) = &skills.active_skill_slot_1 {
            let base_cooldown = slot_2_skill.active_skill.get_base_cooldown();
            let current_skill = slot_2_skill.active_skill.clone();

            if let Ok(mut tracker) = slot2_trackers.get_mut(player_e) {
                // Check if the skill changed
                let skill_changed = tracker.0.tracked_skill != current_skill;

                if skill_changed {
                    // Skill changed - reset tracker completely with new skill
                    let mut init_timer = Timer::from_seconds(base_cooldown, TimerMode::Once);
                    init_timer.tick(Duration::from_secs_f32(base_cooldown));
                    tracker.0.current_charges = max_charges;
                    tracker.0.max_charges = max_charges;
                    tracker.0.base_cooldown = base_cooldown;
                    tracker.0.cooldown_timer = init_timer;
                    tracker.0.tracked_skill = current_skill;
                } else {
                    // Same skill - update max_charges and grant extra charges if heirloom was acquired
                    let old_max = tracker.0.max_charges;
                    tracker.0.max_charges = max_charges;

                    // If max_charges increased, grant the extra charges immediately
                    if max_charges > old_max {
                        let extra_charges = max_charges - old_max;
                        tracker.0.current_charges =
                            (tracker.0.current_charges + extra_charges).min(max_charges);
                    } else {
                        // Cap current charges at new max (in case max decreased)
                        tracker.0.current_charges = tracker.0.current_charges.min(max_charges);
                    }

                    tracker.0.base_cooldown = base_cooldown;
                    // Only update duration if it changed (and skill didn't change)
                    if (tracker.0.cooldown_timer.duration().as_secs_f32() - base_cooldown).abs()
                        > 0.01
                    {
                        let elapsed = tracker.0.cooldown_timer.elapsed();
                        let mut new_timer = Timer::from_seconds(base_cooldown, TimerMode::Once);
                        new_timer.tick(elapsed);
                        tracker.0.cooldown_timer = new_timer;
                    }
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
                        tracked_skill: current_skill,
                    },
                ));
            }
        } else {
            // Remove tracker if skill no longer exists
            if slot2_trackers.get(player_e).is_ok() {
                commands.entity(player_e).remove::<Slot2ChargeTracker>();
            }
        }

        // Manage tracker for slot 3 (active_skill_slot_3)
        if let Some(slot_3_skill) = &skills.active_skill_slot_2 {
            let base_cooldown = slot_3_skill.active_skill.get_base_cooldown();
            let current_skill = slot_3_skill.active_skill.clone();

            if let Ok(mut tracker) = slot3_trackers.get_mut(player_e) {
                // Check if the skill changed
                let skill_changed = tracker.0.tracked_skill != current_skill;

                if skill_changed {
                    // Skill changed - reset tracker completely with new skill
                    let mut init_timer = Timer::from_seconds(base_cooldown, TimerMode::Once);
                    init_timer.tick(Duration::from_secs_f32(base_cooldown));
                    tracker.0.current_charges = max_charges;
                    tracker.0.max_charges = max_charges;
                    tracker.0.base_cooldown = base_cooldown;
                    tracker.0.cooldown_timer = init_timer;
                    tracker.0.tracked_skill = current_skill;
                } else {
                    // Same skill - update max_charges and grant extra charges if heirloom was acquired
                    let old_max = tracker.0.max_charges;
                    tracker.0.max_charges = max_charges;

                    // If max_charges increased, grant the extra charges immediately
                    if max_charges > old_max {
                        let extra_charges = max_charges - old_max;
                        tracker.0.current_charges =
                            (tracker.0.current_charges + extra_charges).min(max_charges);
                    } else {
                        // Cap current charges at new max (in case max decreased)
                        tracker.0.current_charges = tracker.0.current_charges.min(max_charges);
                    }

                    tracker.0.base_cooldown = base_cooldown;
                    // Only update duration if it changed (and skill didn't change)
                    if (tracker.0.cooldown_timer.duration().as_secs_f32() - base_cooldown).abs()
                        > 0.01
                    {
                        let elapsed = tracker.0.cooldown_timer.elapsed();
                        let mut new_timer = Timer::from_seconds(base_cooldown, TimerMode::Once);
                        new_timer.tick(elapsed);
                        tracker.0.cooldown_timer = new_timer;
                    }
                }
            } else {
                // Create new tracker with max charges
                let mut init_timer = Timer::from_seconds(base_cooldown, TimerMode::Once);
                init_timer.tick(Duration::from_secs_f32(base_cooldown));
                commands.entity(player_e).insert(Slot3ChargeTracker(
                    crate::player::skills::SkillChargeTracker {
                        current_charges: max_charges,
                        max_charges,
                        cooldown_timer: init_timer,
                        base_cooldown,
                        tracked_skill: current_skill,
                    },
                ));
            }
        } else {
            // Remove tracker if skill no longer exists
            if slot3_trackers.get(player_e).is_ok() {
                commands.entity(player_e).remove::<Slot3ChargeTracker>();
            }
        }

        // Manage tracker for slot 4 (active_skill_slot_4)
        if let Some(slot_4_skill) = &skills.active_skill_slot_3 {
            let base_cooldown = slot_4_skill.active_skill.get_base_cooldown();
            let current_skill = slot_4_skill.active_skill.clone();

            if let Ok(mut tracker) = slot4_trackers.get_mut(player_e) {
                // Check if the skill changed
                let skill_changed = tracker.0.tracked_skill != current_skill;

                if skill_changed {
                    // Skill changed - reset tracker completely with new skill
                    let mut init_timer = Timer::from_seconds(base_cooldown, TimerMode::Once);
                    init_timer.tick(Duration::from_secs_f32(base_cooldown));
                    tracker.0.current_charges = max_charges;
                    tracker.0.max_charges = max_charges;
                    tracker.0.base_cooldown = base_cooldown;
                    tracker.0.cooldown_timer = init_timer;
                    tracker.0.tracked_skill = current_skill;
                } else {
                    // Same skill - update max_charges and grant extra charges if heirloom was acquired
                    let old_max = tracker.0.max_charges;
                    tracker.0.max_charges = max_charges;

                    // If max_charges increased, grant the extra charges immediately
                    if max_charges > old_max {
                        let extra_charges = max_charges - old_max;
                        tracker.0.current_charges =
                            (tracker.0.current_charges + extra_charges).min(max_charges);
                    } else {
                        // Cap current charges at new max (in case max decreased)
                        tracker.0.current_charges = tracker.0.current_charges.min(max_charges);
                    }

                    tracker.0.base_cooldown = base_cooldown;
                    // Only update duration if it changed (and skill didn't change)
                    if (tracker.0.cooldown_timer.duration().as_secs_f32() - base_cooldown).abs()
                        > 0.01
                    {
                        let elapsed = tracker.0.cooldown_timer.elapsed();
                        let mut new_timer = Timer::from_seconds(base_cooldown, TimerMode::Once);
                        new_timer.tick(elapsed);
                        tracker.0.cooldown_timer = new_timer;
                    }
                }
            } else {
                // Create new tracker with max charges
                let mut init_timer = Timer::from_seconds(base_cooldown, TimerMode::Once);
                init_timer.tick(Duration::from_secs_f32(base_cooldown));
                commands.entity(player_e).insert(Slot4ChargeTracker(
                    crate::player::skills::SkillChargeTracker {
                        current_charges: max_charges,
                        max_charges,
                        cooldown_timer: init_timer,
                        base_cooldown,
                        tracked_skill: current_skill,
                    },
                ));
            }
        } else {
            // Remove tracker if skill no longer exists
            if slot4_trackers.get(player_e).is_ok() {
                commands.entity(player_e).remove::<Slot4ChargeTracker>();
            }
        }
    }
}

pub fn add_rapidfire_speed_to_bonus(
    added_rapidfire: Query<&RapidfireState, Added<RapidfireState>>,
    mut player_query: Query<&mut BonusAttackSpeed, With<Player>>,
    mut attribute_event: EventWriter<crate::attributes::AttributeChangeEvent>,
) {
    for buff in added_rapidfire.iter() {
        if let Ok(mut bonus_speed) = player_query.get_single_mut() {
            bonus_speed.add_multiplier(buff.attack_speed_bonus);
            attribute_event.send_default();
        }
    }
}

pub fn remove_rapidfire_speed_from_bonus(
    mut removed_rapidfire: RemovedComponents<RapidfireState>,
    mut player_query: Query<&mut BonusAttackSpeed, With<Player>>,
    rapidfire_query: Query<&RapidfireState>,
    mut attribute_event: EventWriter<crate::attributes::AttributeChangeEvent>,
) {
    for entity in removed_rapidfire.iter() {
        if let Ok(buff) = rapidfire_query.get(entity) {
            if let Ok(mut bonus_speed) = player_query.get_single_mut() {
                bonus_speed.remove_multiplier(buff.attack_speed_bonus);
                attribute_event.send_default();
            }
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
    mut slot3_trackers: Query<&mut Slot3ChargeTracker>,
    mut slot4_trackers: Query<&mut Slot4ChargeTracker>,
    mut sprint_states: Query<&mut SprintState, With<Player>>,
    mut spear_states: Query<&mut SpearState, With<Player>>,
    mut lunge_states: Query<&mut LungeState, With<Player>>,
    mut teleport_states: Query<&mut TeleportState, With<Player>>,
    mut laser_beam_states: Query<&mut LaserBeamState, With<Player>>,
    mut spinattack_states: Query<&mut SpinAttackState, With<Player>>,
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
            if let Ok(mut state) = laser_beam_states.get_single_mut() {
                if !state.cooldown_timer.finished() {
                    state
                        .cooldown_timer
                        .tick(Duration::from_secs_f32(reduction));
                }
            }
            if let Ok(mut state) = spinattack_states.get_single_mut() {
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
            // Reduce charge regeneration cooldown for slot 3
            if let Ok(mut tracker) = slot3_trackers.get_mut(player_e) {
                if tracker.0.current_charges < tracker.0.max_charges {
                    if !tracker.0.cooldown_timer.finished() {
                        tracker
                            .0
                            .cooldown_timer
                            .tick(Duration::from_secs_f32(reduction));
                    }
                }
            }
            // Reduce charge regeneration cooldown for slot 4
            if let Ok(mut tracker) = slot4_trackers.get_mut(player_e) {
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

pub fn handle_fury_skill(
    fury_states: Query<(&FuryState, &GlobalTransform), With<Player>>,
    enemies: Query<(Entity, &GlobalTransform), With<Mob>>,
    player_skills: Query<(&SkillPower, &Attack, &OwnedBlessings), With<Player>>,
    mut ranged_attack_events: EventWriter<RangedAttackEvent>,
) {
    for (fury_state, player_transform) in fury_states.iter() {
        if fury_state.duration.finished() {
            continue;
        }

        if fury_state.throw_timer.just_finished() {
            let player_pos = player_transform.translation().truncate();
            let Ok((skill_power, attack, blessings)) = player_skills.get_single() else {
                continue;
            };

            let power_mult = skill_power_multiplier(skill_power, blessings.get_skill_power_bonus());
            let base_dmg: i32 = attack.0;
            let dmg = (base_dmg as f32 * power_mult * 1.65) as i32;

            let range = 10.0 * TILE_SIZE.x;
            let nearby_enemies: Vec<(Entity, Vec2, f32)> = enemies
                .iter()
                .map(|(e, t)| {
                    let pos = t.translation().truncate();
                    let dist = player_pos.distance(pos);
                    (e, pos, dist)
                })
                .filter(|(_, _, dist)| *dist <= range)
                .collect();

            let mut rng = rand::thread_rng();
            let direction = if !nearby_enemies.is_empty() {
                let (_, enemy_pos, _) = nearby_enemies.choose(&mut rng).unwrap();
                (*enemy_pos - player_pos).normalize_or_zero()
            } else {
                let angle = rng.gen_range(0.0..std::f32::consts::TAU);
                Vec2::new(angle.cos(), angle.sin())
            };

            ranged_attack_events.send(RangedAttackEvent {
                projectile: Projectile::FuryKunai,
                direction,
                mana_cost: None,
                from_enemy: false,
                from_entity: None,
                is_followup_proj: false,
                dmg_override: Some(dmg),
                pos_override: None,
                spawn_delay: 0.0,
            });
        }
    }
}

/// Attach BombTarget component to newly spawned Bomb projectiles
pub fn handle_attach_bomb_target(
    mut commands: Commands,
    mut bomb_projectiles: Query<
        (Entity, &Projectile, &GlobalTransform),
        (With<Projectile>, Added<Projectile>),
    >,
    player_bomb_targets: Query<(Entity, &BombTarget), With<Player>>,
    transforms: Query<&GlobalTransform>,
) {
    for (proj_entity, proj, proj_transform) in bomb_projectiles.iter_mut() {
        if *proj != Projectile::Bomb {
            continue;
        }

        let proj_pos = proj_transform.translation().truncate();
        for (player_e, bomb_target) in player_bomb_targets.iter() {
            if let Ok(player_txfm) = transforms.get(player_e) {
                let player_pos = player_txfm.translation().truncate();
                let distance = proj_pos.distance(player_pos);
                if distance < 100.0 {
                    commands.entity(proj_entity).insert(BombTarget {
                        target_pos: bomb_target.target_pos,
                    });
                    commands.entity(player_e).remove::<BombTarget>();
                    break;
                }
            }
        }
    }
}

/// Handle bomb explosions when bomb reaches target or hits something
pub fn handle_bomb_explosion(
    mut commands: Commands,
    mut bomb_projectiles: Query<(Entity, &GlobalTransform, Option<&BombTarget>), With<Projectile>>,
    projectiles: Query<&Projectile>,
    mut ranged_attack_events: EventWriter<RangedAttackEvent>,
    enemies: Query<(Entity, &GlobalTransform), With<Mob>>,
    player_skills: Query<(&SkillPower, &Attack, &OwnedBlessings, &Attack), With<Player>>,
    mut status_event: EventWriter<StatusEffectEvent>,
) {
    let Ok((skill_power, attack, blessings, _)) = player_skills.get_single() else {
        return;
    };
    let power_mult = skill_power_multiplier(skill_power, blessings.get_skill_power_bonus());

    let base_dmg: i32 = attack.0;
    let dmg = (base_dmg as f32 * power_mult) as i32;

    for (bomb_entity, bomb_txfm, bomb_target_opt) in bomb_projectiles.iter_mut() {
        if let Ok(proj) = projectiles.get(bomb_entity) {
            if *proj != Projectile::Bomb {
                continue;
            }
        } else {
            continue;
        }
        if let Some(bomb_target) = bomb_target_opt {
            let bomb_pos = bomb_txfm.translation().truncate();
            let distance_to_target = bomb_pos.distance(bomb_target.target_pos);

            if distance_to_target < 5.0 {
                ranged_attack_events.send(RangedAttackEvent {
                    projectile: Projectile::BombExplosion,
                    direction: Vec2::ZERO,
                    mana_cost: None,
                    from_enemy: false,
                    from_entity: None,
                    is_followup_proj: false,
                    dmg_override: Some(dmg),
                    pos_override: Some(bomb_target.target_pos),
                    spawn_delay: 0.0,
                });

                let explosion_radius = 50.0;
                for (enemy_entity, enemy_transform) in enemies.iter() {
                    let enemy_pos = enemy_transform.translation().truncate();
                    let distance = bomb_target.target_pos.distance(enemy_pos);
                    if distance <= explosion_radius {
                        commands.entity(enemy_entity).insert(Frail {
                            num_stacks: 3,
                            timer: Timer::from_seconds(1.2, TimerMode::Repeating),
                        });

                        status_event.send(StatusEffectEvent {
                            entity: enemy_entity,
                            effect: StatusEffect::Frail,
                            num_stacks: 3,
                        });
                    }
                }

                commands.entity(bomb_entity).despawn_recursive();
            }
        }
    }
}
