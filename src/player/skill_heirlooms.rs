use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::utils::Duration;
use bevy_proto::prelude::{ProtoCommands, Prototypes};
use bevy_rapier2d::prelude::Collider;
use rand::{seq::SliceRandom, Rng};

use crate::{
    ai::FollowState,
    animations::{player_sprite::PlayerAnimation, AttackEvent},
    attributes::{
        attribute_helpers::skill_power_multiplier, Attack, AttackCooldown, BonusAttackSpeed,
        CurrentHealth, CurrentMana, MaxHealth, SkillPower,
    },
    audio::{AudioSoundEffect, SoundSpawner},
    blessings::{Blessing, OwnedBlessings},
    combat::{
        status_effects::{RapidfireSlow, StatusEffect, StatusEffectEvent},
        EnemyDeathEvent, HitEvent,
    },
    cursor::CursorPos,
    custom_commands::CommandsExt,
    enemy::Mob,
    item::{
        projectile::{BombTarget, Projectile, ProjectileState, RangedAttackEvent},
        WorldObject,
    },
    player::{
        melee_skills::{
            spawn_echo_hitbox, HeirloomTriggerCooldowns, ParryState, SpearState,
            HEIRLOOM_TRIGGER_COOLDOWN_SECS,
        },
        rogue_skills::{LungeState, SprintState},
        skills::{
            grant_skill_charge_after_cooldown_complete, ActiveSkill,
            ActiveSkillUsedEvent, BombState, BuckshotSkillState, ClassSkillSlots,
            DaggerThrowKillTracker, DaggerThrowState, DruidTreeSkillState, FirePillarState,
            FuryState, HealSkillState, Heirloom, IceWallSkillState, LaserBeamState,
            LastHitProjectile, LightningState, PhasingThroughEnemies, PiercingStarSkillState,
            PlayerSkills, RapidfireState, ShoutSkillState, SlashState, SpinAttackState,
            StealthState, TripleThrowState,
        },
        Player,
    },
    proto::proto_param::ProtoParam,
    status_effects::Frail,
    world::TILE_SIZE,
    GameParam,
};

fn start_slot_cooldown_for_cast(
    slots: &mut ClassSkillSlots,
    slot: usize,
    cd_secs: f32,
    should_start: bool,
) {
    if slot < 4 {
        slots.0[slot].start_cooldown_seconds(cd_secs, should_start);
    }
}

// Temporary marker components for active effects
#[derive(Component)]
pub struct Stealthed;

/// If Stealth still needs charges back (extra heirloom charges), start the next regen on the slot.
fn try_queue_stealth_charge_regen_after_grant(
    skills: &PlayerSkills,
    blessings: &OwnedBlessings,
    slots: &mut ClassSkillSlots,
) {
    for slot in &mut slots.0 {
        if slot.tracked_skill == ActiveSkill::Stealth && slot.current_charges < slot.max_charges {
            let cd = skills
                .effective_skill_cooldown(&ActiveSkill::Stealth, blessings)
                .max(0.0);
            slot.start_cooldown_seconds(cd, true);
            return;
        }
    }
}

/// Ends stealth visuals and starts the skill cooldown (call when attack/skill breaks stealth).
pub fn break_stealth(commands: &mut Commands, player_e: Entity, _stealth: &mut StealthState) {
    commands.entity(player_e).remove::<Stealthed>();
    commands.entity(player_e).remove::<StealthState>();
}

pub fn break_stealth_on_player_attack(
    mut attack_events: EventReader<AttackEvent>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut StealthState), (With<Stealthed>, With<Player>)>,
) {
    let mut any = false;
    for _ in attack_events.iter() {
        any = true;
    }
    if !any {
        return;
    }
    for (e, mut stealth) in q.iter_mut() {
        break_stealth(&mut commands, e, &mut stealth);
    }
}

#[derive(SystemParam)]
pub struct SkillStateQueries<'w, 's> {
    pub stealth_states: Query<'w, 's, &'static mut StealthState, With<Player>>,
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
    pub class_skill_slots: Query<'w, 's, &'static mut ClassSkillSlots, With<Player>>,
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
            Option<&Stealthed>,
        ),
        With<Player>,
    >,
    mut skill_states: SkillStateQueries,
    mut kill_trackers: Query<&mut DaggerThrowKillTracker, With<Player>>,
    time: Res<Time>,
    cursor: Res<CursorPos>,
    asset_server: Res<AssetServer>,
    mut ranged_attack_events: EventWriter<RangedAttackEvent>,
    mut proto_commands: ProtoCommands,
    proto_param: ProtoParam,
    prototypes: Prototypes,
    enemies: Query<(Entity, &GlobalTransform), With<Mob>>,
    mut trigger_counts: ResMut<crate::player::skills::HeirloomTriggerCounts>,
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
            stealthed_option,
        ) in players.iter_mut()
        {
            let Ok(mut class_slots) = skill_states.class_skill_slots.get_mut(player_e) else {
                continue;
            };
            // Get optional states from separate queries
            let stealth_state = skill_states.stealth_states.get_mut(player_e).ok();
            let has_stealth = stealth_state.is_some();
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
                if active.active_skill != ActiveSkill::Stealth {
                    if stealthed_option.is_some() {
                        if let Some(mut stealth_state) = stealth_state {
                            break_stealth(&mut commands, player_e, &mut stealth_state);
                        }
                    }
                }
                if blessings.has_blessing(Blessing::SkillAttackSpeed) {
                    commands
                        .entity(player_e)
                        .insert(crate::item::potion_buffs::AttackSpeedBuff::new(2.0, 0.3));
                }
                // ev.cooldown is already the effective cooldown (base * heirloom reduction * blessing mult)
                let skill_cd = ev.cooldown;

                // Slots 0–3: charge consumption lives on ClassSkillSlots only.
                let mut should_start_cooldown = true;
                if ev.slot < 4 {
                    let s = &mut class_slots.0[ev.slot];
                    if s.current_charges > 0 {
                        s.current_charges -= 1;
                        should_start_cooldown = s.current_charges < s.max_charges;
                    }
                }
                let power_mult =
                    skill_power_multiplier(skill_power, blessings.get_skill_power_bonus());
                match active.active_skill {
                    ActiveSkill::Stealth => {
                        if !should_start_cooldown {
                            if has_stealth {
                                commands.entity(player_e).remove::<StealthState>();
                            }
                        }
                        let mut dur =
                            Timer::from_seconds((2.0 * power_mult).max(0.0), TimerMode::Once);
                        dur.tick(time.delta());
                        commands
                            .entity(player_e)
                            .insert(StealthState {
                                duration: dur,
                            })
                            .insert(Stealthed);
                        start_slot_cooldown_for_cast(
                            &mut class_slots,
                            ev.slot,
                            skill_cd,
                            should_start_cooldown,
                        );

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
                        if !should_start_cooldown {
                            if rapid_state.is_some() {
                                commands.entity(player_e).remove::<RapidfireState>();
                            }
                        }
                        let dur = Timer::from_seconds(3.0, TimerMode::Once);
                        info!("dur: {:?}", dur);
                        commands.entity(player_e).insert(RapidfireState {
                            duration: dur,
                            attack_speed_bonus: 0.8 * power_mult,
                        });
                        start_slot_cooldown_for_cast(
                            &mut class_slots,
                            ev.slot,
                            skill_cd,
                            should_start_cooldown,
                        );

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
                        if !should_start_cooldown {
                            if pillar_state.is_some() {
                                commands.entity(player_e).remove::<FirePillarState>();
                            }
                        }
                        commands.entity(player_e).insert(FirePillarState {
                            hit_clear_timer: Timer::from_seconds(0.75, TimerMode::Repeating),
                        });
                        start_slot_cooldown_for_cast(
                            &mut class_slots,
                            ev.slot,
                            skill_cd,
                            should_start_cooldown,
                        );
                        // spawn fire ring projectile at cursor world position with player's attack as damage
                        let base_dmg: i32 = attack_opt.map(|a| a.0).unwrap_or(10);
                        let dmg = (base_dmg as f32 * power_mult * 0.95) as i32;
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
                        if !should_start_cooldown {
                            if laser_beam_state.is_some() {
                                commands.entity(player_e).remove::<LaserBeamState>();
                            }
                        }
                        commands.entity(player_e).insert(LaserBeamState {
                            hit_clear_timer: Timer::from_seconds(0.5, TimerMode::Repeating),
                        });
                        start_slot_cooldown_for_cast(
                            &mut class_slots,
                            ev.slot,
                            skill_cd,
                            should_start_cooldown,
                        );
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
                        if !should_start_cooldown {
                            if heal_state.is_some() {
                                commands.entity(player_e).remove::<HealSkillState>();
                            }
                        }
                        commands.entity(player_e).insert(HealSkillState);
                        start_slot_cooldown_for_cast(
                            &mut class_slots,
                            ev.slot,
                            skill_cd,
                            should_start_cooldown,
                        );
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
                        if !should_start_cooldown {
                            if buckshot_state.is_some() {
                                commands.entity(player_e).remove::<BuckshotSkillState>();
                            }
                        }
                        commands.entity(player_e).insert(BuckshotSkillState);
                        start_slot_cooldown_for_cast(
                            &mut class_slots,
                            ev.slot,
                            skill_cd,
                            should_start_cooldown,
                        );

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
                        commands
                            .entity(player_e)
                            .insert(PhasingThroughEnemies::new(0.35));
                        commands.spawn(SoundSpawner::new(AudioSoundEffect::Bow, 0.4));
                    }
                    ActiveSkill::IceWall => {
                        if !should_start_cooldown {
                            if icewall_state.is_some() {
                                commands.entity(player_e).remove::<IceWallSkillState>();
                            }
                        }
                        commands.entity(player_e).insert(IceWallSkillState);
                        start_slot_cooldown_for_cast(
                            &mut class_slots,
                            ev.slot,
                            skill_cd,
                            should_start_cooldown,
                        );
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
                        if !should_start_cooldown {
                            if druidtree_state.is_some() {
                                commands.entity(player_e).remove::<DruidTreeSkillState>();
                            }
                        }
                        commands.entity(player_e).insert(DruidTreeSkillState);
                        start_slot_cooldown_for_cast(
                            &mut class_slots,
                            ev.slot,
                            skill_cd,
                            should_start_cooldown,
                        );

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
                        if !should_start_cooldown {
                            if shout_state.is_some() {
                                commands.entity(player_e).remove::<ShoutSkillState>();
                            }
                        }
                        commands.entity(player_e).insert(ShoutSkillState);
                        start_slot_cooldown_for_cast(
                            &mut class_slots,
                            ev.slot,
                            skill_cd,
                            should_start_cooldown,
                        );

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
                        if !should_start_cooldown {
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

                        commands.entity(player_e).insert(PiercingStarSkillState);
                        start_slot_cooldown_for_cast(
                            &mut class_slots,
                            ev.slot,
                            skill_cd,
                            should_start_cooldown,
                        );

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
                        if !should_start_cooldown {
                            if sprint_state.is_some() {
                                commands.entity(player_e).remove::<SprintState>();
                            }
                        }
                        if sprint_state.is_some() {
                            commands.entity(player_e).remove::<SprintState>();
                        }
                        commands.entity(player_e).insert(SprintState {
                            startup_timer: Timer::from_seconds(0.0, TimerMode::Once),
                            sprint_duration_timer: Timer::from_seconds(2.5, TimerMode::Once),
                            speed_bonus: 1.6,
                        });
                        start_slot_cooldown_for_cast(
                            &mut class_slots,
                            ev.slot,
                            skill_cd,
                            should_start_cooldown,
                        );
                        // Insert Sprinting component to activate sprint
                        use crate::player::rogue_skills::Sprinting;
                        commands.entity(player_e).insert(Sprinting);
                    }
                    ActiveSkill::Teleport => {
                        start_slot_cooldown_for_cast(
                            &mut class_slots,
                            ev.slot,
                            skill_cd,
                            should_start_cooldown,
                        );
                    }
                    ActiveSkill::ParrySpear => {
                        if !should_start_cooldown {
                            if spear_state.is_some() {
                                commands.entity(player_e).remove::<SpearState>();
                            }
                        }
                        if spear_state.is_some() {
                            commands.entity(player_e).remove::<SpearState>();
                        }
                        commands.entity(player_e).insert(SpearState {
                            spear_timer: Timer::from_seconds(0.5, TimerMode::Once),
                        });
                        start_slot_cooldown_for_cast(
                            &mut class_slots,
                            ev.slot,
                            skill_cd,
                            should_start_cooldown,
                        );
                    }
                    ActiveSkill::SprintLunge => {
                        if !should_start_cooldown {
                            if lunge_state.is_some() {
                                commands.entity(player_e).remove::<LungeState>();
                            }
                        }
                        if lunge_state.is_some() {
                            commands.entity(player_e).remove::<LungeState>();
                        }
                        commands.entity(player_e).insert(LungeState {
                            lunge_duration: Timer::from_seconds(0.42, TimerMode::Once)
                                .tick(Duration::from_secs_f32(0.1))
                                .clone(),
                            lunge_speed: 9.5,
                        });
                        start_slot_cooldown_for_cast(
                            &mut class_slots,
                            ev.slot,
                            skill_cd,
                            should_start_cooldown,
                        );
                    }
                    ActiveSkill::Lightning => {
                        if !should_start_cooldown {
                            if lightning_state.is_some() {
                                commands.entity(player_e).remove::<LightningState>();
                            }
                        }
                        commands.entity(player_e).insert(LightningState);
                        start_slot_cooldown_for_cast(
                            &mut class_slots,
                            ev.slot,
                            skill_cd,
                            should_start_cooldown,
                        );

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
                        if !should_start_cooldown {
                            if daggerthrow_state.is_some() {
                                commands.entity(player_e).remove::<DaggerThrowState>();
                            }
                        }
                        commands.entity(player_e).insert(DaggerThrowState);
                        start_slot_cooldown_for_cast(
                            &mut class_slots,
                            ev.slot,
                            skill_cd,
                            should_start_cooldown,
                        );

                        // Get kill tracker and reset it after use
                        let kill_count = if let Ok(mut tracker) = kill_trackers.get_mut(player_e) {
                            let count = tracker.kill_count.min(50);
                            tracker.kill_count = 0; // Reset after use
                            count
                        } else {
                            // Initialize tracker if it doesn't exist
                            commands
                                .entity(player_e)
                                .insert(DaggerThrowKillTracker::default());
                            0
                        };

                        // Find all enemies
                        let player_pos = player_txfm.translation().truncate();
                        let all_enemies: Vec<(Entity, Vec2)> = enemies
                            .iter()
                            .map(|(e, t)| (e, t.translation().truncate()))
                            .collect();

                        if all_enemies.is_empty() {
                            continue;
                        }

                        // Throw 1 dagger at a random enemy, plus extra daggers based on kill count
                        let total_daggers = 1 + kill_count;
                        let base_dmg: i32 = attack_opt.map(|a| a.0).unwrap_or(10);
                        let dmg = (base_dmg as f32 * power_mult * 1.15) as i32;
                        let mut rng = rand::thread_rng();

                        for i in 0..total_daggers {
                            // Pick a random enemy for each dagger
                            if let Some((_, enemy_pos)) = all_enemies.choose(&mut rng) {
                                let direction = (*enemy_pos - player_pos).normalize_or_zero();
                                ranged_attack_events.send(RangedAttackEvent {
                                    projectile: Projectile::DaggerThrow,
                                    direction,
                                    mana_cost: None,
                                    from_enemy: false,
                                    from_entity: Some(player_e),
                                    is_followup_proj: false,
                                    dmg_override: Some(dmg),
                                    pos_override: None,
                                    spawn_delay: i as f32 * 0.02, // Slight delay between daggers
                                });
                            }
                        }
                        commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonClick, 0.2));
                    }
                    ActiveSkill::DaggerSlash => {
                        if !should_start_cooldown {
                            if slash_state.is_some() {
                                commands.entity(player_e).remove::<SlashState>();
                            }
                        }
                        commands.entity(player_e).insert(SlashState);
                        start_slot_cooldown_for_cast(
                            &mut class_slots,
                            ev.slot,
                            skill_cd,
                            should_start_cooldown,
                        );

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
                        if !should_start_cooldown {
                            if triplethrow_state.is_some() {
                                commands.entity(player_e).remove::<TripleThrowState>();
                            }
                        }
                        commands.entity(player_e).insert(TripleThrowState);
                        start_slot_cooldown_for_cast(
                            &mut class_slots,
                            ev.slot,
                            skill_cd,
                            should_start_cooldown,
                        );

                        let player_pos = player_txfm.translation().truncate();
                        let cursor_pos = cursor.world_coords.truncate();
                        let base_direction = (cursor_pos - player_pos).normalize_or_zero();
                        let base_angle = base_direction.y.atan2(base_direction.x);

                        // Throw 3 throwing stars in a cone (15 degree spread)
                        let spread_angle = 15.0_f32.to_radians();
                        let base_dmg: i32 = attack_opt.map(|a| a.0).unwrap_or(10);
                        let dmg = (base_dmg as f32 * power_mult * 1.35) as i32;
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
                        if !should_start_cooldown {
                            if fury_state.is_some() {
                                commands.entity(player_e).remove::<FuryState>();
                            }
                        }
                        commands.entity(player_e).insert(FuryState {
                            duration: Timer::from_seconds(2.5, TimerMode::Once),
                            throw_timer: Timer::from_seconds(0.3, TimerMode::Repeating),
                        });
                        start_slot_cooldown_for_cast(
                            &mut class_slots,
                            ev.slot,
                            skill_cd,
                            should_start_cooldown,
                        );
                        commands.spawn(SoundSpawner::new(AudioSoundEffect::GainExp, 0.2));
                    }
                    ActiveSkill::Bomb => {
                        if !should_start_cooldown {
                            if bomb_state.is_some() {
                                commands.entity(player_e).remove::<BombState>();
                            }
                        }
                        commands.entity(player_e).insert(BombState);
                        start_slot_cooldown_for_cast(
                            &mut class_slots,
                            ev.slot,
                            skill_cd,
                            should_start_cooldown,
                        );

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
                        if !should_start_cooldown {
                            if spinattack_state.is_some() {
                                commands.entity(player_e).remove::<SpinAttackState>();
                            }
                        }
                        commands.entity(player_e).insert(SpinAttackState);
                        start_slot_cooldown_for_cast(
                            &mut class_slots,
                            ev.slot,
                            skill_cd,
                            should_start_cooldown,
                        );

                        commands
                            .entity(player_e)
                            .insert(crate::item::potion_buffs::MovementSpeedBuff::new(0.45, 2.6));
                        commands
                            .entity(player_e)
                            .insert(PhasingThroughEnemies::new(0.45));

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
                        trigger_counts.increment(Heirloom::SkillEcho);
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
            commands.entity(e).remove::<StealthState>();
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


fn remove_skill_state_after_slot_cooldown(commands: &mut Commands, entity: Entity, skill: ActiveSkill) {
    match skill {
        ActiveSkill::Stealth => {
            commands.entity(entity).remove::<StealthState>();
        }
        ActiveSkill::FirePillar => {
            commands.entity(entity).remove::<FirePillarState>();
        }
        ActiveSkill::LaserBeam => {
            commands.entity(entity).remove::<LaserBeamState>();
        }
        ActiveSkill::Heal => {
            commands.entity(entity).remove::<HealSkillState>();
        }
        ActiveSkill::Buckshot => {
            commands.entity(entity).remove::<BuckshotSkillState>();
        }
        ActiveSkill::IceWall => {
            commands.entity(entity).remove::<IceWallSkillState>();
        }
        ActiveSkill::DruidTree => {
            commands.entity(entity).remove::<DruidTreeSkillState>();
        }
        ActiveSkill::Shout => {
            commands.entity(entity).remove::<ShoutSkillState>();
        }
        ActiveSkill::PiercingStar => {
            commands.entity(entity).remove::<PiercingStarSkillState>();
        }
        ActiveSkill::Lightning => {
            commands.entity(entity).remove::<LightningState>();
        }
        ActiveSkill::DaggerThrow => {
            commands.entity(entity).remove::<DaggerThrowState>();
        }
        ActiveSkill::DaggerSlash => {
            commands.entity(entity).remove::<SlashState>();
        }
        ActiveSkill::TripleThrow => {
            commands.entity(entity).remove::<TripleThrowState>();
        }
        ActiveSkill::Bomb => {
            commands.entity(entity).remove::<BombState>();
        }
        ActiveSkill::SpinAttack => {
            commands.entity(entity).remove::<SpinAttackState>();
        }
        ActiveSkill::Rapidfire | ActiveSkill::Fury => {}
        ActiveSkill::Teleport
        | ActiveSkill::Sprint
        | ActiveSkill::SprintLunge
        | ActiveSkill::ParrySpear
        | ActiveSkill::Roll
        | ActiveSkill::Parry => {}
    }
}

pub fn tick_class_skill_slots(
    mut commands: Commands,
    time: Res<Time>,
    player_skills: Query<&PlayerSkills, With<Player>>,
    blessings: Query<&OwnedBlessings, With<Player>>,
    mut q: Query<(Entity, &mut ClassSkillSlots, Option<&Stealthed>), With<Player>>,
) {
    for (entity, mut slots, stealthed) in q.iter_mut() {
        for i in 0..4 {
            let skill = slots.0[i].tracked_skill;
            if skill == ActiveSkill::Stealth && stealthed.is_some() {
                continue;
            }
            slots.0[i].cooldown_timer.tick(time.delta());
            if !slots.0[i].cooldown_timer.just_finished() {
                continue;
            }
            if skill == ActiveSkill::Rapidfire || skill == ActiveSkill::Fury {
                continue;
            }
            grant_skill_charge_after_cooldown_complete(entity, skill, slots.as_mut());
            remove_skill_state_after_slot_cooldown(&mut commands, entity, skill);
            if skill == ActiveSkill::Stealth {
                if let (Ok(sk), Ok(bl)) = (player_skills.get_single(), blessings.get_single()) {
                    try_queue_stealth_charge_regen_after_grant(sk, bl, slots.as_mut());
                }
            }
        }
    }
}

pub fn tick_class_skill_hit_clear_timers(
    time: Res<Time>,
    mut pillar: Query<&mut FirePillarState, With<Player>>,
    mut laser: Query<&mut LaserBeamState, With<Player>>,
) {
    for mut p in pillar.iter_mut() {
        p.hit_clear_timer.tick(time.delta());
    }
    for mut l in laser.iter_mut() {
        l.hit_clear_timer.tick(time.delta());
    }
}

pub fn tick_fury_duration_and_throw(
    time: Res<Time>,
    mut fury: Query<&mut FuryState, With<Player>>,
    attack_cooldown: Query<&AttackCooldown, With<Player>>,
) {
    for mut f in fury.iter_mut() {
        f.duration.tick(time.delta());
        let attack_speed_mult = if let Ok(cooldown) = attack_cooldown.get_single() {
            let reference_base_cooldown = 0.6;
            let denom = 2. * cooldown.0 - reference_base_cooldown;
            let raw = if denom > 0.001 {
                reference_base_cooldown / denom
            } else {
                10.0
            };
            raw.clamp(0.1, 10.0)
        } else {
            1.0
        };
        let scaled_delta = time.delta().mul_f32(attack_speed_mult);
        f.throw_timer.tick(scaled_delta);
    }
}

pub fn finalize_rapidfire_fury_charges(
    mut commands: Commands,
    mut q: Query<(Entity, &mut ClassSkillSlots, Option<&RapidfireState>, Option<&FuryState>), With<Player>>,
) {
    for (e, mut slots, rapid, fury) in q.iter_mut() {
        if let Some(r) = rapid {
            if r.duration.finished() {
                if let Some(si) = slots
                    .0
                    .iter()
                    .position(|s| s.tracked_skill == ActiveSkill::Rapidfire)
                {
                    if slots.0[si].cooldown_timer.finished() {
                        grant_skill_charge_after_cooldown_complete(e, ActiveSkill::Rapidfire, slots.as_mut());
                        commands.entity(e).remove::<RapidfireState>();
                    }
                }
            }
        }
        if let Some(f) = fury {
            if f.duration.finished() {
                if let Some(si) = slots.0.iter().position(|s| s.tracked_skill == ActiveSkill::Fury) {
                    if slots.0[si].cooldown_timer.finished() {
                        grant_skill_charge_after_cooldown_complete(e, ActiveSkill::Fury, slots.as_mut());
                        commands.entity(e).remove::<FuryState>();
                    }
                }
            }
        }
    }
}

pub fn tick_druid_tree_dummy_timers(
    mut commands: Commands,
    time: Res<Time>,
    mut dummy_query: Query<(Entity, &mut DruidTreeDummy)>,
) {
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
    mut fire_ring_projectiles: Query<(&mut ProjectileState, &Projectile), With<Projectile>>,
) {
    for mut pillar_state in fire_pillar_states.iter_mut() {
        if pillar_state.hit_clear_timer.just_finished() {
            info!("Clearing hit_entities for FireRing projectiles due to FirePillar effect");
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
    let active_dummy_entities: std::collections::HashSet<Entity> =
        dummies.iter().map(|(e, _)| e).collect();

    for (dummy_entity, dummy_txfm) in dummies.iter() {
        let dummy_pos = dummy_txfm.translation().truncate();
        let taunt_range = 200.0;

        let is_about_to_despawn = dummy_timers
            .get(dummy_entity)
            .map(|d| d.timer.finished())
            .unwrap_or(false);

        for (enemy_txfm, mut follow_state) in enemies.iter_mut() {
            let enemy_pos = enemy_txfm.translation().truncate();
            let distance = (dummy_pos - enemy_pos).length();

            if follow_state.target == dummy_entity && is_about_to_despawn {
                follow_state.target = game.game.player;
                continue;
            }

            if distance <= taunt_range {
                if follow_state.target == game.game.player {
                    follow_state.target = dummy_entity;
                }
            }
        }
    }

    for (_enemy_txfm, mut follow_state) in enemies.iter_mut() {
        if let Ok(_) = transforms.get(follow_state.target) {
            if !active_dummy_entities.contains(&follow_state.target)
                && follow_state.target != game.game.player
            {
                follow_state.target = game.game.player;
            }
        } else {
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
            sprite.color = Color::rgba(0.2, 0.2, 0.2, 0.3);
        } else {
            sprite.color = Color::WHITE;
        }
    }
}

fn finished_timer_init() -> Timer {
    let mut t = Timer::from_seconds(1.0, TimerMode::Once);
    t.tick(Duration::from_secs_f32(999.0));
    t
}

fn update_one_slot_runtime(
    slot: &mut crate::player::skills::SlotSkillRuntime,
    slot_skill: Option<&crate::player::skills::ActiveSkillChoiceState>,
    max_charges: u32,
) {
    use crate::player::skills::{ActiveSkill, SlotSkillRuntime};
    let empty_roll = || SlotSkillRuntime {
        current_charges: 0,
        max_charges: 0,
        cooldown_timer: finished_timer_init(),
        base_cooldown: 0.0,
        tracked_skill: ActiveSkill::Roll,
    };
    match slot_skill {
        None => {
            *slot = empty_roll();
        }
        Some(choice) => {
            if choice.active_skill == ActiveSkill::Roll {
                *slot = empty_roll();
                return;
            }
            let base_cooldown = choice.active_skill.get_base_cooldown();
            let current_skill = choice.active_skill;
            let skill_changed = slot.tracked_skill != current_skill;
            if skill_changed {
                let base_cd = base_cooldown.max(0.0);
                let mut init_timer = Timer::from_seconds(base_cd, TimerMode::Once);
                init_timer.tick(Duration::from_secs_f32(base_cd));
                slot.current_charges = max_charges;
                slot.max_charges = max_charges;
                slot.base_cooldown = base_cooldown;
                slot.cooldown_timer = init_timer;
                slot.tracked_skill = current_skill;
            } else {
                let old_max = slot.max_charges;
                slot.max_charges = max_charges;
                if max_charges > old_max {
                    let extra_charges = max_charges - old_max;
                    slot.current_charges = (slot.current_charges + extra_charges).min(max_charges);
                } else {
                    slot.current_charges = slot.current_charges.min(max_charges);
                }
                slot.base_cooldown = base_cooldown;
                if (slot.cooldown_timer.duration().as_secs_f32() - base_cooldown).abs() > 0.01 {
                    let elapsed = slot.cooldown_timer.elapsed();
                    let base_cd = base_cooldown.max(0.0);
                    let mut new_timer = Timer::from_seconds(base_cd, TimerMode::Once);
                    new_timer.tick(elapsed);
                    slot.cooldown_timer = new_timer;
                }
            }
        }
    }
}

pub fn initialize_class_skill_slots(
    mut commands: Commands,
    players: Query<Entity, (With<Player>, Or<(Changed<PlayerSkills>, Without<ClassSkillSlots>)>)>,
    player_skills: Query<&PlayerSkills, With<Player>>,
    mut slots_q: Query<&mut ClassSkillSlots, With<Player>>,
) {
    for player_e in players.iter() {
        let Ok(skills) = player_skills.get(player_e) else {
            continue;
        };
        let max_charges = 1 + skills.skill_extra_charges();
        let slot_refs = [
            skills.active_skill_slot_0.as_ref(),
            skills.active_skill_slot_1.as_ref(),
            skills.active_skill_slot_2.as_ref(),
            skills.active_skill_slot_3.as_ref(),
        ];
        if let Ok(mut slots) = slots_q.get_mut(player_e) {
            for i in 0..4 {
                update_one_slot_runtime(&mut slots.0[i], slot_refs[i], max_charges);
            }
        } else {
            let mut slots = ClassSkillSlots::default();
            for i in 0..4 {
                update_one_slot_runtime(&mut slots.0[i], slot_refs[i], max_charges);
            }
            commands.entity(player_e).insert(slots);
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
/// Reduces by 0.1s per stack of CritSkillCooldownReduction heirloom (Bob's Bell).
/// Rate-limited to once per 0.1s via HeirloomTriggerCooldowns.
pub fn reduce_skill_cooldown_on_crit(
    mut hit_events: EventReader<HitEvent>,
    mut commands: Commands,
    mut players: Query<
        (Entity, &PlayerSkills, Option<&mut HeirloomTriggerCooldowns>),
        With<Player>,
    >,
    mut class_slots: Query<&mut ClassSkillSlots, With<Player>>,
) {
    for hit in hit_events.iter() {
        if !hit.was_crit || hit.hit_by_mob.is_some() {
            continue;
        }

        for (player_e, skills, mut heirloom_cooldowns) in players.iter_mut() {
            let heirloom_count = skills.get_count(Heirloom::CritSkillCooldownReduction);
            if heirloom_count == 0 {
                continue;
            }
            if let Some(ref cooldowns) = heirloom_cooldowns {
                if cooldowns
                    .bobs_bell
                    .as_ref()
                    .map_or(false, |t| !t.finished())
                {
                    continue;
                }
            }

            let reduction = (0.1 * heirloom_count as f32).max(0.0);

            if let Ok(mut slots) = class_slots.get_mut(player_e) {
                for slot in &mut slots.0 {
                    if !slot.cooldown_timer.finished() {
                        slot.cooldown_timer
                            .tick(Duration::from_secs_f32(reduction));
                    }
                }
            }

            let bobs_bell_timer =
                Timer::from_seconds(HEIRLOOM_TRIGGER_COOLDOWN_SECS, TimerMode::Once);
            if let Some(ref mut cooldowns) = heirloom_cooldowns {
                cooldowns.bobs_bell = Some(bobs_bell_timer);
            } else {
                commands.entity(player_e).insert(HeirloomTriggerCooldowns {
                    chalice_echo: None,
                    bobs_bell: Some(bobs_bell_timer),
                });
            }
        }
    }
}

pub fn handle_crit_heal(
    mut hit_events: EventReader<crate::combat::HitEvent>,
    player_query: Query<&PlayerSkills, With<crate::player::Player>>,
    mut modify_health_event: EventWriter<crate::attributes::modifiers::ModifyHealthEvent>,
    mut trigger_counts: ResMut<crate::player::skills::HeirloomTriggerCounts>,
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
            || hit.from_heirloom_effect.is_some()
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
            trigger_counts.increment(Heirloom::CritHeal);
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

/// Tracks the last projectile that hit each enemy (for dagger throw kill tracking)
pub fn track_enemy_hit_projectiles(
    mut commands: Commands,
    mut hit_events: EventReader<HitEvent>,
    mut enemies: Query<&mut LastHitProjectile>,
    mobs: Query<Entity, With<Mob>>,
) {
    for hit in hit_events.iter() {
        // Only track hits on enemies
        if !mobs.contains(hit.hit_entity) {
            continue;
        }

        // Get or insert the LastHitProjectile component
        if let Ok(mut last_hit) = enemies.get_mut(hit.hit_entity) {
            last_hit.projectile = hit.hit_with_projectile.clone();
        } else {
            // Insert the component if it doesn't exist
            commands.entity(hit.hit_entity).insert(LastHitProjectile {
                projectile: hit.hit_with_projectile.clone(),
            });
        }
    }
}

/// Tracks enemy deaths and increments dagger throw kill tracker (excluding dagger throw kills)
pub fn track_dagger_throw_kills(
    mut death_events: EventReader<EnemyDeathEvent>,
    mut kill_trackers: Query<&mut DaggerThrowKillTracker, With<Player>>,
    last_hit_projectiles: Query<&LastHitProjectile>,
) {
    let Ok(mut tracker) = kill_trackers.get_single_mut() else {
        return;
    };

    for death_event in death_events.iter() {
        // Check if this enemy was killed by a dagger throw
        let was_killed_by_dagger_throw =
            if let Ok(last_hit) = last_hit_projectiles.get(death_event.entity) {
                last_hit.projectile == Some(Projectile::DaggerThrow)
            } else {
                false
            };

        // Only count kills that weren't from dagger throw
        if !was_killed_by_dagger_throw {
            tracker.kill_count = (tracker.kill_count + 1).min(10);
        }
    }
}
