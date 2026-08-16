use crate::aseprite_assets::Echo;
use bevy::prelude::*;
use bevy_aseprite_ultra::prelude::Aseprite;
use bevy_rapier2d::prelude::{Collider, KinematicCharacterController};
use rand::Rng;

use crate::{
    animations::player_sprite::PlayerAnimation,
    assets::Graphics,
    attributes::{
        attribute_helpers::skill_power_multiplier, modifiers::ModifyHealthEvent, Attack,
        CurrentHealth, CurrentMana, HealthRegen, ProjectileSize, SkillPower,
    },
    audio::{AudioSoundEffect, SoundSpawner},
    blessings::{
        HeirloomManaOverclock, MajorBlessing, OwnedBlessings, OwnedMajorBlessings,
        overclock_mana_cost,
    },
    colors::LIGHT_RED,
    combat_helpers::{spawn_deferred_aseprite_collider, spawn_temp_collider},
    cursor::CursorPos,
    enemy::Mob,
    inputs::MovementVector,
    item::{
        projectile::{Projectile, RangedAttackEvent},
        WorldObject,
    },
    player::{
        mage_skills::spawn_ice_explosion_hitbox,
        skills::{
            active_skill_scaling::{attack_damage_multiplier, PARRY_SPEAR},
            parry_spear_pull_radius_px,
        },
    },
    status_effects::MobStatusEffects,
    ui::{
        damage_numbers::{spawn_floating_text_with_shadow, PreviousHealth},
        game_fonts::FLOATING_TEXT,
    },
    world::TILE_SIZE,
    GameParam, HitEvent,
};

use super::combat_heirlooms::{
    random_cherry_bomb_target, spawn_cherry_bomb_flight, TriggerSummonsEvent,
};
use super::{ActiveSkill, ActiveSkillUsedEvent, Heirloom, Player, PlayerSkills};

/// Brief marker on a mob that was just hit and is queued for a follow-up
/// split-damage hit a few frames later. Removed as soon as the follow-up
/// fires — stored `SparseSet` so split attacks don't move mobs through extra
/// archetypes every swing.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct SecondHitDelay {
    pub delay: Timer,
    pub dir: Vec2,
    pub weapon_obj: WorldObject,
}

// OnHitEcho is now triggered in handle_modify_health_event (src/attributes/modifiers.rs)
// so that all sources of HP loss trigger the echo, not just combat hits.

pub fn handle_second_split_attack(
    mobs: Query<Option<&MobStatusEffects>, With<Mob>>,
    game: GameParam,
    mut second_hit_query: Query<(Entity, &mut SecondHitDelay)>,
    mut hit_event: MessageWriter<HitEvent>,
    time: Res<Time>,
    mut commands: Commands,
) {
    for (e, mut second_hit) in second_hit_query.iter_mut() {
        if !second_hit.delay.tick(time.delta()).just_finished() {
            continue;
        }
        let Ok(status_option) = mobs.get(e) else {
            continue;
        };

        let frail_stacks = status_option.map(|s| s.frail_stacks()).unwrap_or(0);
        let (damage, was_crit, was_overcrit) = game.calculate_player_damage(
            (frail_stacks * 5) as u32,
            None,
            0,
            None,
            frail_stacks,
            0,
            false,
        );

        let split_damage = f32::floor(damage as f32 / 2.) as i32;

        hit_event.write(HitEvent {
            hit_by_pet: None,
            hit_entity: e,
            damage: split_damage,
            dir: second_hit.dir,
            hit_with_melee: Some(second_hit.weapon_obj),
            hit_with_projectile: None,
            was_crit,
            was_overcrit,
            hit_by_mob: None,
            ignore_tool: false,
            from_heirloom_effect: None,
            from_active_skill: false,
        });
        commands.entity(e).remove::<SecondHitDelay>();
    }
}

/// Per-heirloom trigger rate limits (e.g. Chalice echo, Bob's Bell, Summoning Wand).
#[derive(Component)]
pub struct HeirloomTriggerCooldowns {
    pub chalice_echo: Option<Timer>,
    pub bobs_bell: Option<Timer>,
    pub heal_summons: Option<Timer>,
}

/// Shared cooldown duration for heirloom trigger effects (Chalice, Bob's Bell).
pub const HEIRLOOM_TRIGGER_COOLDOWN_SECS: f32 = 0.1;

/// Internal cooldown for Summoning Wand (HealSummons) triggers.
pub const HEAL_SUMMONS_TRIGGER_COOLDOWN_SECS: f32 = 0.2;

/// Ticks all heirloom trigger cooldowns so they can trigger again after their duration.
pub fn tick_heirloom_trigger_cooldowns(
    time: Res<Time>,
    mut query: Query<&mut HeirloomTriggerCooldowns, With<Player>>,
) {
    for mut cooldowns in query.iter_mut() {
        if let Some(ref mut t) = cooldowns.chalice_echo {
            t.tick(time.delta());
        }
        if let Some(ref mut t) = cooldowns.bobs_bell {
            t.tick(time.delta());
        }
        if let Some(ref mut t) = cooldowns.heal_summons {
            t.tick(time.delta());
        }
    }
}

fn new_heirloom_trigger_timer() -> Timer {
    Timer::from_seconds(HEIRLOOM_TRIGGER_COOLDOWN_SECS, TimerMode::Once)
}

fn new_heal_summons_trigger_timer() -> Timer {
    Timer::from_seconds(HEAL_SUMMONS_TRIGGER_COOLDOWN_SECS, TimerMode::Once)
}

pub fn handle_echo_after_heal(
    mut commands: Commands,
    mut changed_health: Query<
        (
            Entity,
            &CurrentHealth,
            &PreviousHealth,
            &PlayerSkills,
            &Attack,
            &ProjectileSize,
            &GlobalTransform,
            Option<&OwnedMajorBlessings>,
            &mut CurrentMana,
            Option<&mut HeirloomTriggerCooldowns>,
            Option<&mut HeirloomManaOverclock>,
        ),
        Changed<CurrentHealth>,
    >,
    asset_server: Res<AssetServer>,
    mut trigger_summons_events: MessageWriter<TriggerSummonsEvent>,
    mut trigger_counts: ResMut<crate::player::skills::HeirloomTriggerCounts>,
    mut blessing_triggers: ResMut<crate::blessings::BlessingTriggerCounts>,
) {
    for (
        e,
        changed_health,
        prev_health,
        skills,
        attack,
        projectile_size,
        player_txfm,
        majors,
        mut current_mana,
        mut cooldowns,
        mut overclock,
    ) in changed_health.iter_mut()
    {
        let delta = changed_health.0 - prev_health.0;
        if delta <= 0 {
            continue;
        }
        let rng = &mut rand::thread_rng();
        let count = skills.get_count(Heirloom::HealEcho);
        if count > 0 && rng.gen_bool((count as f64 * 0.1).clamp(0.0, 1.0)) {
            let chalice_ready = cooldowns.as_ref().map_or(true, |c| {
                c.chalice_echo.as_ref().map_or(true, |t| t.is_finished())
            });
            if chalice_ready {
                let base_mana_cost = (Heirloom::HealEcho.get_mana_cost() as f32
                    * if skills.has(Heirloom::DiscountMP) {
                        0.75
                    } else {
                        1.
                    }) as i32;
                let mana_cost = majors
                    .map(|m| {
                        overclock_mana_cost(
                            m,
                            overclock.as_deref_mut(),
                            base_mana_cost,
                            Some(&mut blessing_triggers),
                        )
                    })
                    .unwrap_or(base_mana_cost);
                if mana_cost == 0 || current_mana.0 >= mana_cost {
                    if mana_cost > 0 {
                        current_mana.0 -= mana_cost;
                        trigger_counts.record_mana(Heirloom::HealEcho, mana_cost);
                    }
                    let aftershock = majors
                        .map(|m| m.has(MajorBlessing::EchoAftershock))
                        .unwrap_or(false);
                    if aftershock {
                        blessing_triggers.increment(MajorBlessing::EchoAftershock);
                    }
                    spawn_echo_hitbox(
                        &mut commands,
                        &asset_server,
                        e,
                        player_txfm.translation(),
                        attack.0,
                        projectile_size.get_multiplier(),
                        aftershock,
                    );
                    trigger_counts.increment(Heirloom::HealEcho);
                    if let Some(ref mut cooldowns) = cooldowns {
                        cooldowns.chalice_echo = Some(new_heirloom_trigger_timer());
                    } else {
                        commands.entity(e).insert(HeirloomTriggerCooldowns {
                            chalice_echo: Some(new_heirloom_trigger_timer()),
                            bobs_bell: None,
                            heal_summons: None,
                        });
                    }
                }
            }
        }
        // HealSummons: 10% chance to trigger all summons once (Ant Farm, Boulder, Piercing Ring).
        // Trigger costs mana; summons are free. Rate-limited to once per 0.2s.
        let count = skills.get_count(Heirloom::HealSummons);
        if count > 0 && rng.gen_bool((0.1 * count as f64).clamp(0.0, 1.0)) {
            let summons_ready = cooldowns.as_ref().map_or(true, |c| {
                c.heal_summons.as_ref().map_or(true, |t| t.is_finished())
            });
            if summons_ready {
                let mana_cost = Heirloom::HealSummons.get_mana_cost();
                if current_mana.0 >= mana_cost {
                    current_mana.0 -= mana_cost;
                    trigger_counts.record_mana(Heirloom::HealSummons, mana_cost);
                    trigger_summons_events.write(TriggerSummonsEvent(e));
                    if let Some(ref mut cooldowns) = cooldowns {
                        cooldowns.heal_summons = Some(new_heal_summons_trigger_timer());
                    } else {
                        commands.entity(e).insert(HeirloomTriggerCooldowns {
                            chalice_echo: None,
                            bobs_bell: None,
                            heal_summons: Some(new_heal_summons_trigger_timer()),
                        });
                    }
                }
            }
        }
    }
}
/// Component on spear projectiles that are being pulled toward a target; it
/// is added on spear cast and removed when the pull is released. `SparseSet`
/// so spears don't occupy a new archetype variant per active pull.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct SpearGravity {
    pub target: Vec2,
    pub timer: Timer,
}
/// Parry-skill timers held on the player only while the Parry skill is
/// equipped. Stored `SparseSet` so the player stays in one archetype when
/// the skill is (un)equipped.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct ParryState {
    pub parry_timer: Timer,
    pub cooldown_timer: Timer,
    pub success: bool,
    pub active: bool,
}

/// Spear-skill cooldown timer on the player. Same churn profile as
/// `ParryState` — stored `SparseSet`.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct SpearState {
    pub spear_timer: Timer,
}

/// Short delay timer on the player before the spear pull actually resolves.
/// Added on spear cast and removed when the delay elapses — `SparseSet` so
/// the player stays in one archetype during spear casts.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct SpearPullDelay {
    pub delay_timer: Timer,
    pub epicenter: Vec2,
    /// Pull radius captured at cast time. Scales with the HP drained by the
    /// 5% max-HP cast cost so a higher max-HP build pulls from further away.
    pub pull_radius: f32,
}

/// Brief knockback/stun state applied to a mob that just got parried.
/// Removed as soon as its timer elapses (a few hundred ms) — `SparseSet` so
/// parrying does not move the mob between archetypes on every parry.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct Parried {
    pub timer: Timer,
    pub kb_applied: bool,
}

#[derive(Debug, Message)]
pub struct ParrySuccessEvent(pub Entity);

#[derive(Component)]
pub struct SpearAttack;

pub fn handle_parry(
    mut player: Query<(Entity, &PlayerSkills, &PlayerAnimation, &mut ParryState), (With<Player>,)>,
    key_input: Res<ButtonInput<KeyCode>>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    mut commands: Commands,
    time: Res<Time>,
    keybinds: Res<crate::keybinds::InputMappings>,
) {
    let Ok((e, skills, curr_anim, mut parry_state)) = player.single_mut() else {
        return;
    };

    if let Some(parry_slot) = skills.has_active_skill(ActiveSkill::Parry) {
        if keybinds.check_skill_input(parry_slot, &key_input, &mouse_input)
            && parry_state.cooldown_timer.is_finished()
            && !curr_anim.is_parrying()
        {
            commands.entity(e).insert(PlayerAnimation::Parry);
            parry_state.cooldown_timer.reset();
            parry_state.parry_timer.reset();
            parry_state.active = true;
            parry_state.parry_timer.tick(time.delta());
            // ActiveSkillUsedEvent dispatched centrally
        }
    }
    parry_state.cooldown_timer.tick(time.delta());

    if parry_state.parry_timer.fraction() != 0. {
        parry_state.parry_timer.tick(time.delta());
        if parry_state.parry_timer.just_finished() {
            parry_state.success = false;
            parry_state.active = false;
        }
    }
}
pub fn handle_spear(
    mut active_skill_events: MessageReader<ActiveSkillUsedEvent>,
    mut player: Query<
        (
            Entity,
            &GlobalTransform,
            &PlayerSkills,
            &Attack,
            &ProjectileSize,
            &mut SpearState,
            &mut KinematicCharacterController,
            &mut MovementVector,
        ),
        With<Player>,
    >,
    mut commands: Commands,
    time: Res<Time>,
    cursor_pos: Res<CursorPos>,
    mut ranged_attack_events: MessageWriter<RangedAttackEvent>,
) {
    let Ok((e, player_pos, skills, _dmg, projectile_size, mut spear_state, mut kcc, mut mv)) =
        player.single_mut()
    else {
        return;
    };

    // Collect activated slots from events this frame (event-driven, no direct input check)
    let spear_slot = skills.has_active_skill(ActiveSkill::ParrySpear);
    let should_activate = spear_slot
        .map(|slot| active_skill_events.read().any(|ev| ev.slot == slot))
        .unwrap_or(false);

    if should_activate {
        // Cooldown and gating are fully handled by dispatch_active_skill_events.
        // Fire spear mechanics here without resetting the cooldown timer.
        spear_state.spear_timer.tick(time.delta());
        commands.entity(e).insert(PlayerAnimation::Spear);
        commands.spawn(SoundSpawner::new(AudioSoundEffect::Spear, 0.2));
        commands.spawn(SoundSpawner::new(AudioSoundEffect::SpearPull, 0.3).with_delay(0.32));

        let player_pos_2d = player_pos.translation().truncate();
        let direction = (cursor_pos.world_coords.truncate() - player_pos_2d).normalize_or_zero();
        let epicenter = player_pos_2d + direction * 1.7 * TILE_SIZE.x;

        let pull_radius = parry_spear_pull_radius_px(projectile_size.0);

        commands.entity(e).insert(SpearPullDelay {
            delay_timer: Timer::from_seconds(0.45, TimerMode::Once),
            epicenter,
            pull_radius,
        });
        ranged_attack_events.write(RangedAttackEvent {
            projectile: Projectile::SpearGravity,
            direction: Vec2::ZERO,
            mana_cost: None,
            mana_cost_heirloom: None,
            from_enemy: false,
            from_entity: None,
            is_followup_proj: false,
            dmg_override: None,
            pos_override: Some(epicenter),
            spawn_delay: 0.2,
        });
    }

    if spear_state.spear_timer.fraction() != 0. {
        spear_state.spear_timer.tick(time.delta());
        if spear_state.spear_timer.just_finished() {
            spear_state.spear_timer.reset();
        }
        // Prevent player movement during spear throw
        mv.0 = mv.0 * 0.;
        kcc.translation = Some(Vec2::new(mv.0.x, mv.0.y));
    }
}

pub fn handle_parry_success(
    player: Query<
        (
            Entity,
            &Attack,
            &HealthRegen,
            &GlobalTransform,
            &PlayerSkills,
            &ProjectileSize,
            Option<&OwnedMajorBlessings>,
        ),
        With<Player>,
    >,
    mut parry_success_event: MessageReader<ParrySuccessEvent>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut modify_health_event: MessageWriter<ModifyHealthEvent>,
    mut blessing_triggers: ResMut<crate::blessings::BlessingTriggerCounts>,
) {
    for _ in parry_success_event.read() {
        let Ok((player_e, attack, health_regen, player_txfm, skills, projectile_size, majors)) =
            player.single()
        else {
            continue;
        };
        spawn_floating_text_with_shadow(
            &mut commands,
            &asset_server,
            player_txfm.translation() + Vec3::new(0., 16., 0.),
            LIGHT_RED,
            "Parry!".to_string(),
            FLOATING_TEXT,
        );
        if skills.has(Heirloom::ParryHPRegen) {
            modify_health_event.write(ModifyHealthEvent(health_regen.0));
        }
        if skills.has(Heirloom::ParryEcho) {
            let aftershock = majors
                .map(|m| m.has(MajorBlessing::EchoAftershock))
                .unwrap_or(false);
            if aftershock {
                blessing_triggers.increment(MajorBlessing::EchoAftershock);
            }
            spawn_echo_hitbox(
                &mut commands,
                &asset_server,
                player_e,
                player_txfm.translation(),
                attack.0,
                projectile_size.get_multiplier(),
                aftershock,
            );
        }
    }
}

pub fn tick_parried_timer(
    mut parried_query: Query<(Entity, &mut Parried)>,
    time: Res<Time>,
    mut commands: Commands,
) {
    for (e, mut parried) in parried_query.iter_mut() {
        parried.timer.tick(time.delta());
        if parried.timer.just_finished() {
            commands.entity(e).remove::<Parried>();
        }
    }
}

pub fn spawn_echo_hitbox(
    commands: &mut Commands,
    asset_server: &AssetServer,
    player: Entity,
    player_world_pos: Vec3,
    dmg: i32,
    size_multiplier: f32,
    echo_aftershock: bool,
) {
    spawn_echo_hitbox_scaled(
        commands,
        asset_server,
        player,
        player_world_pos,
        dmg,
        size_multiplier,
        1.0,
        echo_aftershock,
    );
}

/// Stationary echo at a world position (not parented to the player).
pub fn spawn_world_echo_hitbox(
    commands: &mut Commands,
    asset_server: &AssetServer,
    world_pos: Vec3,
    dmg: i32,
    size_multiplier: f32,
    major_echo_size_mult: f32,
) {
    let base_radius = 24.0;
    let size_multiplier = size_multiplier * major_echo_size_mult;
    spawn_deferred_aseprite_collider(
        commands,
        Transform::from_translation(world_pos).with_scale(Vec3::splat(size_multiplier)),
        10.5,
        dmg,
        Collider::capsule(Vec2::ZERO, Vec2::ZERO, base_radius),
        asset_server.load::<Aseprite>(Echo::PATH),
        "",
        false,
        Projectile::Echo,
        vec![],
        None,
    );
}

/// Like [`spawn_echo_hitbox`], with an extra multiplicative size from major blessings.
/// When `echo_aftershock` is true, also spawns a stationary copy at `player_world_pos`.
pub fn spawn_echo_hitbox_scaled(
    commands: &mut Commands,
    asset_server: &AssetServer,
    player: Entity,
    player_world_pos: Vec3,
    dmg: i32,
    size_multiplier: f32,
    major_echo_size_mult: f32,
    echo_aftershock: bool,
) {
    // NOTE: do NOT pre-scale the collider radius by `size_multiplier`. The entity's
    // Transform scale below is applied to the collider by Rapier, so multiplying the
    // radius here as well would scale the hitbox twice (it would grow ~size^2).
    let base_radius = 24.0;
    let combined_size = size_multiplier * major_echo_size_mult;

    // Queue deferred spawn with parent - actual entity will be created in PreUpdate and parented
    spawn_deferred_aseprite_collider(
        commands,
        Transform::from_translation(Vec3::ZERO).with_scale(Vec3::splat(combined_size)),
        10.5,
        dmg,
        Collider::capsule(Vec2::ZERO, Vec2::ZERO, base_radius),
        asset_server.load::<Aseprite>(Echo::PATH),
        "",
        false,
        Projectile::Echo,
        vec![],       // No extra components needed
        Some(player), // Parent to player entity
    );

    if echo_aftershock {
        spawn_world_echo_hitbox(
            commands,
            asset_server,
            player_world_pos,
            dmg,
            size_multiplier,
            major_echo_size_mult,
        );
    }
}

pub fn handle_spear_pull_delay(
    mut player_query: Query<
        (
            Entity,
            &mut SpearPullDelay,
            &Attack,
            &SkillPower,
            &OwnedBlessings,
            &crate::blessings::OwnedMajorBlessings,
        ),
        With<Player>,
    >,
    mobs: Query<(Entity, &GlobalTransform), With<Mob>>,
    time: Res<Time>,
    mut commands: Commands,
) {
    for (player_e, mut pull_delay, attack, skill_power, blessings, majors) in player_query.iter_mut()
    {
        pull_delay.delay_timer.tick(time.delta());
        if !pull_delay.delay_timer.just_finished() {
            continue;
        }

        // Pull all nearby enemies to the epicenter
        let epicenter = pull_delay.epicenter;
        // Captured at cast time; scales with Size at cast.
        let pull_radius = pull_delay.pull_radius;

        for (mob_e, mob_transform) in mobs.iter() {
            let mob_pos = mob_transform.translation().truncate();
            let distance = (mob_pos - epicenter).length();

            if distance <= pull_radius {
                commands.entity(mob_e).insert(SpearGravity {
                    target: epicenter,
                    timer: Timer::from_seconds(0.5, TimerMode::Once),
                });
            }
        }
        let skill_power_mult = skill_power_multiplier(
            skill_power,
            blessings.get_skill_power_bonus_with_majors(majors),
        );
        // Spawn 20px damage hitbox at epicenter
        let hitbox = spawn_temp_collider(
            &mut commands,
            Transform::from_translation(Vec3::new(epicenter.x, epicenter.y, 1.0)),
            0.5, // Very short duration, just for the hit
            (attack.0 as f32 * skill_power_mult * attack_damage_multiplier(PARRY_SPEAR)) as i32,
            Collider::ball(18.0), // 20px radius
            Projectile::SpearGravity,
        );
        commands.entity(hitbox).insert(SpearAttack);

        // Remove the delay component
        commands.entity(player_e).remove::<SpearPullDelay>();
    }
}

pub fn handle_spear_gravity(
    mut spear_query: Query<(
        Entity,
        &mut SpearGravity,
        &mut GlobalTransform,
        &mut KinematicCharacterController,
    )>,
    time: Res<Time>,
    mut commands: Commands,
) {
    for (e, mut spear_gravity, txfm, mut kcc) in spear_query.iter_mut() {
        spear_gravity.timer.tick(time.delta());
        if spear_gravity.timer.just_finished() {
            commands.entity(e).remove::<SpearGravity>();
            continue;
        }
        let distance = spear_gravity.target - txfm.translation().truncate();
        let delta = (distance).normalize_or_zero();
        if distance.length() < 16. {
            commands.entity(e).remove::<SpearGravity>();
            continue;
        }

        kcc.translation = Some(delta * 300. * time.delta_secs());
    }
}

#[derive(Clone)]
pub enum DelayedCastType {
    IceExplosion {
        pos: Vec3,
        dmg: i32,
        size_multiplier: f32,
    },
    Echo {
        player: Entity,
        world_pos: Vec3,
        dmg: i32,
        size_multiplier: f32,
        aftershock: bool,
    },
    /// Twin Strike Relics: second cherry bomb with a freshly rolled target.
    CherryBomb {
        start_pos: Vec2,
        dmg: i32,
    },
}

#[derive(Component)]
pub struct DelayedHeirloomCast {
    pub delay: Timer,
    pub cast_type: DelayedCastType,
}

pub const HEIRLOOM_EXTRA_CAST_DELAY: f32 = 0.3;

pub fn handle_delayed_heirloom_casts(
    mut commands: Commands,
    mut query: Query<(Entity, &mut DelayedHeirloomCast)>,
    time: Res<Time>,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    majors: Query<&crate::blessings::OwnedMajorBlessings, With<crate::player::Player>>,
) {
    let echo_size_mult = majors
        .single()
        .map(|m| m.echo_size_multiplier())
        .unwrap_or(1.0);
    for (entity, mut cast) in query.iter_mut() {
        cast.delay.tick(time.delta());
        if !cast.delay.just_finished() {
            continue;
        }
        match &cast.cast_type {
            DelayedCastType::IceExplosion {
                pos,
                dmg,
                size_multiplier,
            } => {
                // Collector's Fury applied in `handle_hits` for heirloom projectiles.
                spawn_ice_explosion_hitbox(&mut commands, &graphics, *pos, *dmg, *size_multiplier);
            }
            DelayedCastType::Echo {
                player,
                world_pos,
                dmg,
                size_multiplier,
                aftershock,
            } => {
                spawn_echo_hitbox_scaled(
                    &mut commands,
                    &asset_server,
                    *player,
                    *world_pos,
                    *dmg,
                    *size_multiplier,
                    echo_size_mult,
                    *aftershock,
                );
            }
            DelayedCastType::CherryBomb { start_pos, dmg } => {
                let mut rng = rand::thread_rng();
                let target_pos = random_cherry_bomb_target(*start_pos, &mut rng);
                spawn_cherry_bomb_flight(&mut commands, &graphics, *start_pos, target_pos, *dmg);
            }
        }
        commands.entity(entity).despawn();
    }
}

pub fn spawn_delayed_heirloom_cast(
    commands: &mut Commands,
    delay_secs: f32,
    cast_type: DelayedCastType,
) {
    commands.spawn(DelayedHeirloomCast {
        delay: Timer::from_seconds(delay_secs, TimerMode::Once),
        cast_type,
    });
}
