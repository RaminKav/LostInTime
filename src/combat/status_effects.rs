use bevy::ecs::lifecycle::HookContext;
use bevy::ecs::world::DeferredWorld;
use bevy::prelude::*;
use serde::Deserialize;
use strum_macros::{Display, EnumIter};

use crate::assets::Graphics;
use crate::attributes::{BonusDamage, CritChance, CritDamage};
use crate::ecs_helpers::SafeHierarchyExt;
use crate::enemy::red_mushking::DeathState;
use crate::player::skills::{Heirloom, PlayerSkills};
use crate::ui::game_fonts as gf;
use crate::Player;
use rand::Rng;

use super::{HitEvent, MarkedForDeath};

#[derive(Deserialize, Debug, EnumIter, Display, Hash, Clone, Reflect, Eq, PartialEq)]
pub enum StatusEffect {
    Slow,
    Frail,
    Poison,
    Frozen,
}

#[derive(Deserialize, Debug, Clone, Reflect)]
pub struct StatusEffectState {
    pub effect: StatusEffect,
    pub num_stacks: i32,
    pub index: usize,
}

#[derive(Component, Deserialize, Debug, Clone, Reflect)]
pub struct StatusEffectTracker {
    pub effects: Vec<StatusEffectState>,
}
#[derive(Component)]
pub struct StatusEffectIcon;

#[derive(Message)]
pub struct StatusEffectEvent {
    pub effect: StatusEffect,
    pub num_stacks: i32,
    pub entity: Entity,
}

// -----------------------------------------------------------------------------
// Per-mob status state.
//
// These USED to each be separate `#[derive(Component)]` types that were
// inserted/removed on mobs as effects applied and expired. That insert/remove
// churn was the primary driver of archetype fragmentation: with 5+ independent
// toggleable components, mobs could traverse 2^5 = 32 archetype variants per
// mob type, and every empty variant still costs per-query iteration time.
//
// They are now plain `Debug + Clone` state structs that live as `Option`
// fields on a single always-present [`MobStatusEffects`] component. Every mob
// keeps the same ECS archetype for its entire lifetime regardless of which
// status effects are active — adding/removing a status is now a field
// mutation, not a structural change.
#[derive(Debug, Clone)]
pub struct Burning {
    pub tick_timer: Timer,
    pub duration_timer: Timer,
    pub stacks: u128,
}
#[derive(Debug, Clone)]
pub struct Poisoned {
    pub tick_timer: Timer,
    pub duration_timer: Timer,
    pub damage: u8,
}
#[derive(Debug, Clone)]
pub struct Frail {
    pub num_stacks: u8,
    pub timer: Timer,
}
#[derive(Debug, Clone)]
pub struct Slow {
    pub num_stacks: u8,
    pub timer: Timer,
}

/// Movement slow per freeze/slow stack (2%).
pub const SLOW_SPEED_REDUCTION_PER_STACK: f32 = 0.02;
/// Floor so extreme stacks cannot fully stop movement on their own.
pub const SLOW_SPEED_MULTIPLIER_MIN: f32 = 0.05;
/// Bonus damage per frail stack (3%, additive).
pub const FRAIL_DAMAGE_BONUS_PER_STACK: f32 = 0.03;

#[inline]
pub fn frail_damage_multiplier(stacks: u8) -> f32 {
    1.0 + stacks as f32 * FRAIL_DAMAGE_BONUS_PER_STACK
}

/// Shared blue tint for freeze-style status effects (Freeze blessing, Death Defiance, Rapidfire).
pub const STATUS_EFFECT_BLUE_TINT: Color = Color::srgba(0.5, 0.7, 1.0, 1.0);

/// Untinted sprite color captured while any status blue tint marker is present.
#[derive(Component, Clone, Copy)]
pub struct BaseSpriteColor(pub Color);

/// Freeze-blessing tint marker. Nesting-safe with other blue-tint markers below.
#[derive(Component)]
#[component(
    storage = "SparseSet",
    on_insert = apply_status_sprite_tint,
    on_remove = clear_status_sprite_tint
)]
pub struct FrozenTint;

/// Present while Rapidfire is slowing all enemies (drives shared blue tint).
#[derive(Component)]
#[component(
    storage = "SparseSet",
    on_insert = apply_status_sprite_tint,
    on_remove = clear_status_sprite_tint
)]
pub struct RapidfireSlowTint;

/// Death Defiance proc tint. Separate from [`DeathDefianceFrozen`] so this module
/// does not depend on combat heirlooms.
#[derive(Component)]
#[component(
    storage = "SparseSet",
    on_insert = apply_status_sprite_tint,
    on_remove = clear_status_sprite_tint
)]
pub struct DeathDefianceTint;

fn status_tint_marker_count(world: &DeferredWorld, entity: Entity) -> u8 {
    let mut count = 0;
    if world.get::<FrozenTint>(entity).is_some() {
        count += 1;
    }
    if world.get::<RapidfireSlowTint>(entity).is_some() {
        count += 1;
    }
    if world.get::<DeathDefianceTint>(entity).is_some() {
        count += 1;
    }
    count
}

fn apply_status_sprite_tint(mut world: DeferredWorld, context: HookContext) {
    let entity = context.entity;
    let Some(current) = world.get::<Sprite>(entity).map(|s| s.color) else {
        return;
    };
    if world.get::<BaseSpriteColor>(entity).is_none() {
        world.commands().entity(entity).insert(BaseSpriteColor(current));
    }
    if let Some(mut sprite) = world.get_mut::<Sprite>(entity) {
        sprite.color = STATUS_EFFECT_BLUE_TINT;
    }
}

/// `on_remove` runs while the marker is still present, so a shared insert/remove
/// hook would keep seeing a tint and never restore the original color.
fn clear_status_sprite_tint(mut world: DeferredWorld, context: HookContext) {
    let entity = context.entity;
    if status_tint_marker_count(&world, entity) > 1 {
        return;
    }
    if let Some(base) = world.get::<BaseSpriteColor>(entity).map(|b| b.0) {
        if let Some(mut sprite) = world.get_mut::<Sprite>(entity) {
            sprite.color = base;
        }
        world.commands().entity(entity).remove::<BaseSpriteColor>();
    }
}

/// Frozen status effect from Freeze blessing - mob is completely frozen when at 3 stacks
#[derive(Debug, Clone)]
pub struct Frozen {
    pub timer: Timer,
}

/// Consolidated per-mob status state. Always present on mobs; mutate the
/// `Option` fields instead of inserting/removing status-effect components to
/// avoid archetype fragmentation. See the module-level comment above.
#[derive(Component, Default, Debug)]
pub struct MobStatusEffects {
    pub burning: Option<Burning>,
    pub poisoned: Option<Poisoned>,
    pub frail: Option<Frail>,
    pub slow: Option<Slow>,
    pub frozen: Option<Frozen>,
    /// 50% speed reduction applied by the RapidFire skill. Was a marker
    /// component before consolidation.
    pub rapidfire_slow: bool,
}

impl MobStatusEffects {
    #[inline]
    pub fn is_burning(&self) -> bool {
        self.burning.is_some()
    }
    #[inline]
    pub fn is_slowed(&self) -> bool {
        self.slow.is_some()
    }
    #[inline]
    pub fn is_frozen(&self) -> bool {
        self.frozen.is_some()
    }
    #[inline]
    pub fn frail_stacks(&self) -> u8 {
        self.frail.as_ref().map(|f| f.num_stacks).unwrap_or(0)
    }
    #[inline]
    pub fn slow_stacks(&self) -> u8 {
        self.slow.as_ref().map(|s| s.num_stacks).unwrap_or(0)
    }
    /// Multiplicative movement-speed modifier from all slow-type effects
    /// (Slow stacks, RapidfireSlow). Returns 1.0 when nothing is slowing the
    /// mob.
    #[inline]
    pub fn movement_speed_multiplier(&self) -> f32 {
        let slow_mult = (1.0 - self.slow_stacks() as f32 * SLOW_SPEED_REDUCTION_PER_STACK)
            .max(SLOW_SPEED_MULTIPLIER_MIN);
        let rapidfire_mult = if self.rapidfire_slow { 0.5 } else { 1.0 };
        slow_mult * rapidfire_mult
    }
}

/// Ensures every mob entity has the bundle of "always-present" mob state
/// components that we use to avoid archetype fragmentation from transient
/// on-hit/status components.
///
/// Components added:
///   * [`MobStatusEffects`] — consolidated Burning/Frail/Slow/Frozen/...
///   * [`HitAnimationTracker`] — hit-react timer+knockback (was transient)
///   * [`BounceOnHit`] — bounce animation state (was transient)
///   * [`WasHitWithCrit`] / [`WasHitWithOvercrit`] — crit flags consumed by
///     the damage-numbers system (were marker components)
///
/// Protos don't attach these directly because several contain `Timer`s that
/// are awkward to reflect. This system runs every frame and fills them in
/// for any newly-spawned mob that doesn't have them yet. That causes
/// exactly one archetype transition per mob at the point of first spawn,
/// then zero thereafter — insertions from combat become pure value updates.
pub fn ensure_mob_status_effects(
    mut commands: Commands,
    mobs: Query<Entity, (With<crate::enemy::Mob>, Without<MobStatusEffects>)>,
) {
    for e in mobs.iter() {
        commands.entity(e).insert((
            MobStatusEffects::default(),
            crate::animations::HitAnimationTracker::default(),
            crate::juice::bounce::BounceOnHit::default(),
            crate::combat::WasHitWithCrit::default(),
            crate::combat::WasHitWithOvercrit::default(),
        ));
    }
}

pub fn handle_new_status_effect_event(
    mut query: Query<&mut StatusEffectTracker>,
    mut events: MessageReader<StatusEffectEvent>,
) {
    for event in events.read() {
        let Ok(mut tracker) = query.get_mut(event.entity) else {
            continue;
        };

        if event.num_stacks == 0 {
            tracker.effects.retain(|e| e.effect != event.effect);
            continue;
        }
        if let Some(prev_status_tracker) = tracker
            .effects
            .iter_mut()
            .find(|e| e.effect == event.effect)
        {
            prev_status_tracker.num_stacks = event.num_stacks;
        } else {
            let index = tracker.effects.len();
            tracker.effects.push(StatusEffectState {
                effect: event.effect.clone(),
                num_stacks: event.num_stacks,
                index,
            });
        }
    }
}

pub fn update_status_effect_icons(
    mut query: Query<
        (Entity, Option<&Children>, &StatusEffectTracker),
        (
            Changed<StatusEffectTracker>,
            Without<DeathState>,
            Without<MarkedForDeath>,
        ),
    >,
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    prev_status_icons: Query<(Entity, &StatusEffectIcon)>,
) {
    for (entity, maybe_children, tracker) in query.iter_mut() {
        // remove old icons
        if let Some(children) = maybe_children {
            for prev_icon in prev_status_icons.iter() {
                if children.iter().any(|c| c == prev_icon.0) {
                    commands.entity(prev_icon.0).despawn();
                }
            }
        }
        for (height, effect) in tracker.effects.iter().enumerate() {
            let total_stacks = effect.num_stacks as f32;
            let h = height as f32;

            // High stack counts: show 1 icon + text instead of one icon per stack.
            let use_count_badge = matches!(
                effect.effect,
                StatusEffect::Poison | StatusEffect::Frail | StatusEffect::Slow
            ) && effect.num_stacks > 5;
            if use_count_badge {
                let icon = graphics.get_status_effect_icon(effect.effect.clone());
                let s = 5.;
                let translation = Vec3::new(-s / 2., 7. * h + 12., 1.);
                let icon_entity = commands
                    .spawn((
                        Sprite {
                            image: icon,
                            custom_size: Some(Vec2::new(5., 5.)),
                            ..default()
                        },
                        Transform {
                            translation,
                            scale: Vec3::new(1., 1., 1.),
                            ..Default::default()
                        },
                    ))
                    .insert(StatusEffectIcon)
                    .safe_set_parent(entity)
                    .id();

                // Add text count next to the icon
                commands
                    .spawn(
                        gf::MICRO
                            .text(&asset_server, effect.num_stacks.to_string(), Color::WHITE)
                            .anchor(bevy::sprite::Anchor::CENTER_LEFT)
                            .with_transform(Transform {
                                translation: Vec3::new(3.0, 0., 1.),
                                scale: gf::MICRO.transform_scale(),
                                ..Default::default()
                            }),
                    )
                    // .insert(RenderLayers::from_layers(&[1]))
                    .safe_set_parent(icon_entity);
            } else {
                // Original behavior: show one icon per stack
                for i in 0..effect.num_stacks {
                    let icon = graphics.get_status_effect_icon(effect.effect.clone());
                    let d = 1.;
                    let s = 5.;
                    let i = i as f32;
                    let translation = Vec3::new(
                        i * (s + d) - (total_stacks - 1.) * (s / 2.) - d,
                        7. * h + 12.,
                        1.,
                    );
                    commands
                        .spawn((
                            Sprite {
                                image: icon,
                                custom_size: Some(Vec2::new(5., 5.)),
                                ..default()
                            },
                            Transform {
                                translation,
                                scale: Vec3::new(1., 1., 1.),
                                ..Default::default()
                            },
                        ))
                        .insert(StatusEffectIcon)
                        .safe_set_parent(entity);
                }
            }
        }
    }
}

pub fn handle_burning_ticks(
    mut burning: Query<
        (Entity, &mut MobStatusEffects),
        (Without<DeathState>, Without<MarkedForDeath>),
    >,
    time: Res<Time>,
    mut status_event: MessageWriter<StatusEffectEvent>,
    mut hit_event: MessageWriter<HitEvent>,
    player_skills: Query<(&PlayerSkills, &BonusDamage, &CritChance, &CritDamage), With<Player>>,
) {
    // Get poison strength and crit from player (if player exists)
    let Ok((skills, bonus_damage, crit_chance, crit_damage)) = player_skills.single() else {
        return;
    };

    let poison_strength_bonus =
        1. + skills.get_count(Heirloom::PoisonStrength) as f32 + bonus_damage.0 as f32 / 100.;

    // Cap crit chance at 200%; excess converts to crit damage at 1:1
    let total_crit_chance_raw = crit_chance.0.try_into().unwrap_or(0_u32);
    let effective_crit_chance = total_crit_chance_raw.min(200);
    let overflow_crit_damage = total_crit_chance_raw.saturating_sub(200) as i32;
    let crit_multiplier = f32::abs((crit_damage.0 + overflow_crit_damage) as f32) / 100.0;

    let mut rng = rand::thread_rng();

    for (e, mut status) in burning.iter_mut() {
        let frail_stacks = status.frail_stacks();
        let Some(burning) = status.burning.as_mut() else {
            continue;
        };
        burning.duration_timer.tick(time.delta());
        if !burning.duration_timer.just_finished() {
            burning.tick_timer.tick(time.delta());
            if burning.tick_timer.just_finished() {
                // Damage = stacks + bonus damage from PoisonStrength heirloom
                let base_damage = burning.stacks as i32;
                let mut damage = base_damage * poison_strength_bonus.round() as i32;

                // Frail multiplier: 1.1x per stack (same as other damage)
                if frail_stacks > 0 {
                    damage = (damage as f32 * frail_damage_multiplier(frail_stacks)).round() as i32;
                }

                // Poison can crit internally (extra damage) but we don't set was_crit so other systems don't react
                if effective_crit_chance > 0 && rng.gen_ratio(effective_crit_chance.min(100), 100) {
                    damage = (damage as f32 * crit_multiplier).round().max(1.) as i32;
                }
                damage = damage.max(1);

                hit_event.write(HitEvent {
                    hit_by_pet: None,
                    hit_entity: e,
                    damage,
                    dir: Vec2::new(0.5, 0.5),
                    hit_with_melee: None,
                    hit_with_projectile: None,
                    was_crit: false,
                    was_overcrit: false,
                    hit_by_mob: None,
                    ignore_tool: false,
                    from_heirloom_effect: Some(Heirloom::PoisonStacks),
                    from_active_skill: false,
                });
                burning.tick_timer.reset();
            }
        } else {
            // Reduce half the stacks and reset the timer instead of clearing all
            burning.stacks = burning.stacks / 2;
            burning.duration_timer.reset();

            if burning.stacks == 0 {
                status.burning = None;
                status_event.write(StatusEffectEvent {
                    entity: e,
                    effect: StatusEffect::Poison,
                    num_stacks: 0,
                });
            } else {
                let stacks = burning.stacks as i32;
                status_event.write(StatusEffectEvent {
                    entity: e,
                    effect: StatusEffect::Poison,
                    num_stacks: stacks,
                });
            }
        }
    }
}
pub fn handle_frail_stack_ticks(
    mut frailed: Query<
        (Entity, &mut MobStatusEffects),
        (Without<DeathState>, Without<MarkedForDeath>),
    >,
    time: Res<Time>,
    mut status_event: MessageWriter<StatusEffectEvent>,
) {
    for (e, mut status) in frailed.iter_mut() {
        let Some(frail) = status.frail.as_mut() else {
            continue;
        };
        frail.timer.tick(time.delta());
        if frail.timer.just_finished() {
            frail.num_stacks = frail.num_stacks.saturating_sub(1);
            let remaining = frail.num_stacks as i32;
            if frail.num_stacks == 0 {
                status.frail = None;
            }
            status_event.write(StatusEffectEvent {
                entity: e,
                effect: StatusEffect::Frail,
                num_stacks: remaining,
            });
        }
    }
}
pub fn handle_slow_stack_ticks(
    mut slowed: Query<
        (Entity, &mut MobStatusEffects),
        (Without<DeathState>, Without<MarkedForDeath>),
    >,
    time: Res<Time>,
    mut status_event: MessageWriter<StatusEffectEvent>,
) {
    for (e, mut status) in slowed.iter_mut() {
        let Some(slow) = status.slow.as_mut() else {
            continue;
        };
        slow.timer.tick(time.delta());
        if slow.timer.just_finished() {
            slow.num_stacks = slow.num_stacks.saturating_sub(1);
            let remaining = slow.num_stacks as i32;
            if slow.num_stacks == 0 {
                status.slow = None;
            }
            status_event.write(StatusEffectEvent {
                entity: e,
                effect: StatusEffect::Slow,
                num_stacks: remaining,
            });
        }
    }
}

/// Compounding Malady: each status application gains one extra stack.
fn with_status_apply_extra(stacks_to_add: u32, apply_extra_stack: bool) -> u32 {
    if stacks_to_add == 0 {
        0
    } else if apply_extra_stack {
        stacks_to_add.saturating_add(1)
    } else {
        stacks_to_add
    }
}

/// Adds `stacks_to_add` freeze/slow stacks (no cap; stacks like poison).
/// When `shared_affliction_poison_tick_secs` is `Some`, also applies the same number of poison stacks.
/// When `apply_extra_stack` is true (Compounding Malady), adds one more stack.
pub fn try_add_slow_stacks(
    hit_e: Entity,
    status: &mut MobStatusEffects,
    status_event: &mut MessageWriter<StatusEffectEvent>,
    stacks_to_add: u32,
    shared_affliction_poison_tick_secs: Option<f32>,
    apply_extra_stack: bool,
) -> bool {
    let stacks_to_add = with_status_apply_extra(stacks_to_add, apply_extra_stack);
    if stacks_to_add == 0 {
        return false;
    }
    let add = stacks_to_add.min(u8::MAX as u32) as u8;
    if let Some(slow) = status.slow.as_mut() {
        slow.num_stacks = slow.num_stacks.saturating_add(add);
        slow.timer.reset();
        status_event.write(StatusEffectEvent {
            entity: hit_e,
            effect: StatusEffect::Slow,
            num_stacks: slow.num_stacks as i32,
        });
    } else {
        status.slow = Some(Slow {
            num_stacks: add,
            timer: Timer::from_seconds(2.0, TimerMode::Repeating),
        });
        status_event.write(StatusEffectEvent {
            entity: hit_e,
            effect: StatusEffect::Slow,
            num_stacks: add as i32,
        });
    }

    if let Some(tick_secs) = shared_affliction_poison_tick_secs {
        if let Some(burning) = status.burning.as_mut() {
            burning.stacks = burning.stacks.saturating_add(add as u128);
            burning.duration_timer.reset();
            status_event.write(StatusEffectEvent {
                entity: hit_e,
                effect: StatusEffect::Poison,
                num_stacks: burning.stacks as i32,
            });
        } else {
            status.burning = Some(Burning {
                tick_timer: Timer::from_seconds(tick_secs, TimerMode::Repeating),
                duration_timer: Timer::from_seconds(3.0, TimerMode::Once),
                stacks: add as u128,
            });
            status_event.write(StatusEffectEvent {
                entity: hit_e,
                effect: StatusEffect::Poison,
                num_stacks: add as i32,
            });
        }
    }
    true
}

/// Adds `stacks_to_add` frail stacks (no cap; stacks like poison).
/// When `apply_extra_stack` is true (Compounding Malady), adds one more stack.
pub fn try_add_frail_stacks(
    hit_e: Entity,
    status: &mut MobStatusEffects,
    status_event: &mut MessageWriter<StatusEffectEvent>,
    stacks_to_add: u32,
    apply_extra_stack: bool,
) -> bool {
    let stacks_to_add = with_status_apply_extra(stacks_to_add, apply_extra_stack);
    if stacks_to_add == 0 {
        return false;
    }
    let add = stacks_to_add.min(u8::MAX as u32) as u8;
    if let Some(frail) = status.frail.as_mut() {
        frail.num_stacks = frail.num_stacks.saturating_add(add);
        frail.timer.reset();
        status_event.write(StatusEffectEvent {
            entity: hit_e,
            effect: StatusEffect::Frail,
            num_stacks: frail.num_stacks as i32,
        });
    } else {
        status.frail = Some(Frail {
            num_stacks: add,
            timer: Timer::from_seconds(2.0, TimerMode::Repeating),
        });
        status_event.write(StatusEffectEvent {
            entity: hit_e,
            effect: StatusEffect::Frail,
            num_stacks: add as i32,
        });
    }
    true
}

/// Adds `stacks_to_add` poison stacks.
/// When `apply_extra_stack` is true (Compounding Malady), adds one more stack.
pub fn try_add_poison_stacks(
    hit_e: Entity,
    status: &mut MobStatusEffects,
    status_event: &mut MessageWriter<StatusEffectEvent>,
    stacks_to_add: u32,
    poison_tick_secs: f32,
    duration_secs: f32,
    apply_extra_stack: bool,
) -> bool {
    let stacks_to_add = with_status_apply_extra(stacks_to_add, apply_extra_stack);
    if stacks_to_add == 0 {
        return false;
    }
    let add = stacks_to_add as u128;
    if let Some(burning) = status.burning.as_mut() {
        burning.stacks = burning.stacks.saturating_add(add);
        burning.duration_timer.reset();
        status_event.write(StatusEffectEvent {
            entity: hit_e,
            effect: StatusEffect::Poison,
            num_stacks: burning.stacks as i32,
        });
    } else {
        status.burning = Some(Burning {
            tick_timer: Timer::from_seconds(poison_tick_secs, TimerMode::Repeating),
            duration_timer: Timer::from_seconds(duration_secs, TimerMode::Once),
            stacks: add,
        });
        status_event.write(StatusEffectEvent {
            entity: hit_e,
            effect: StatusEffect::Poison,
            num_stacks: add as i32,
        });
    }
    true
}

/// Handle frozen status effect ticks - mobs are frozen with blue tint
pub fn handle_frozen_ticks(
    mut commands: Commands,
    time: Res<Time>,
    mut frozen_mobs: Query<
        (Entity, &mut MobStatusEffects),
        (Without<DeathState>, Without<MarkedForDeath>),
    >,
) {
    for (entity, mut status) in frozen_mobs.iter_mut() {
        let Some(frozen) = status.frozen.as_mut() else {
            continue;
        };
        frozen.timer.tick(time.delta());

        if frozen.timer.just_finished() {
            status.frozen = None;
            commands.entity(entity).remove::<FrozenTint>();
        }
    }
}

/// Check if a mob should become frozen when reaching 3 slow stacks (Freeze blessing)
pub fn check_freeze_on_slow_stacks(
    mut commands: Commands,
    blessings: Query<&crate::blessings::OwnedBlessings>,
    mut slow_query: Query<
        (Entity, &mut MobStatusEffects),
        (
            Changed<MobStatusEffects>,
            Without<DeathState>,
            Without<MarkedForDeath>,
        ),
    >,
    mut status_event: MessageWriter<StatusEffectEvent>,
) {
    let has_freeze_blessing = blessings
        .single()
        .map(|b| b.has_blessing(crate::blessings::Blessing::Freeze))
        .unwrap_or(false);

    if !has_freeze_blessing {
        return;
    }

    for (entity, mut status) in slow_query.iter_mut() {
        if status.frozen.is_some() {
            continue;
        }
        let should_freeze = status
            .slow
            .as_ref()
            .map(|s| s.num_stacks >= 3)
            .unwrap_or(false);
        if !should_freeze {
            continue;
        }
        status.frozen = Some(Frozen {
            timer: Timer::from_seconds(2.0, TimerMode::Once),
        });
        commands.entity(entity).insert(FrozenTint);
        status_event.write(StatusEffectEvent {
            entity,
            effect: StatusEffect::Frozen,
            num_stacks: 1,
        });
    }
}
