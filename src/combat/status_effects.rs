use bevy::prelude::*;
use serde::Deserialize;
use strum_macros::{Display, EnumIter};

use crate::assets::Graphics;
use crate::ecs_helpers::SafeHierarchyExt;
use crate::ui::game_fonts as gf;
use crate::attributes::{BonusDamage, CritChance, CritDamage};
use crate::enemy::red_mushking::DeathState;
use crate::player::skills::{Heirloom, PlayerSkills};
use crate::Player;
use rand::Rng;

use super::{HitEvent, MarkedForDeath};

#[derive(Deserialize, Debug, EnumIter, Display, Hash, Clone, Reflect, FromReflect, Eq, PartialEq)]
pub enum StatusEffect {
    Slow,
    Frail,
    Poison,
    Frozen,
}

#[derive(Deserialize, Debug, Clone, Reflect, FromReflect)]
pub struct StatusEffectState {
    pub effect: StatusEffect,
    pub num_stacks: i32,
    pub index: usize,
}

#[derive(Component, Deserialize, Debug, Clone, Reflect, FromReflect)]
pub struct StatusEffectTracker {
    pub effects: Vec<StatusEffectState>,
}
#[derive(Component)]
pub struct StatusEffectIcon;

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

/// Shared blue tint for freeze-style status effects (Freeze blessing, Death Defiance, Rapidfire).
pub const STATUS_EFFECT_BLUE_TINT: Color = Color::rgba(0.5, 0.7, 1.0, 1.0);

/// Stores the mob's pre-tint color while Rapidfire is slowing all enemies.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct RapidfireSlowTint {
    pub original_color: Color,
}

pub fn apply_status_blue_tint(sprite: &mut TextureAtlasSprite) -> Color {
    let original = sprite.color;
    sprite.color = STATUS_EFFECT_BLUE_TINT;
    original
}

/// Frozen status effect from Freeze blessing - mob is completely frozen when at 3 stacks
#[derive(Debug, Clone)]
pub struct Frozen {
    pub timer: Timer,
    pub original_color: Color,
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
        let slow_mult = 1.0 - self.slow_stacks() as f32 * 0.15;
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
    mut events: EventReader<StatusEffectEvent>,
) {
    for event in events.iter() {
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
                if children.iter().any(|c| c == &prev_icon.0) {
                    commands.entity(prev_icon.0).despawn_recursive();
                }
            }
        }
        for (height, effect) in tracker.effects.iter().enumerate() {
            let total_stacks = effect.num_stacks as f32;
            let h = height as f32;

            // For poison stacks > 5, show 1 icon + text count
            if effect.effect == StatusEffect::Poison && effect.num_stacks > 5 {
                let icon = graphics.get_status_effect_icon(effect.effect.clone());
                let s = 5.;
                let translation = Vec3::new(-s / 2., 7. * h + 12., 1.);
                let icon_entity = commands
                    .spawn(SpriteBundle {
                        texture: icon,
                        sprite: Sprite {
                            custom_size: Some(Vec2::new(5., 5.)),
                            ..Default::default()
                        },
                        transform: Transform {
                            translation,
                            scale: Vec3::new(1., 1., 1.),
                            ..Default::default()
                        },
                        ..Default::default()
                    })
                    .insert(StatusEffectIcon)
                    .safe_set_parent(entity)
                    .id();

                // Add text count next to the icon
                commands
                    .spawn(Text2dBundle {
                        text: Text::from_section(
                            effect.num_stacks.to_string(),
                            gf::MICRO.text_style(&asset_server, Color::WHITE),
                        ),
                        transform: Transform {
                            translation: Vec3::new(3.0, 0., 1.),
                            scale: gf::MICRO.transform_scale(),
                            ..Default::default()
                        },
                        text_anchor: bevy::sprite::Anchor::CenterLeft,
                        ..Default::default()
                    })
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
                        .spawn(SpriteBundle {
                            texture: icon,
                            sprite: Sprite {
                                custom_size: Some(Vec2::new(5., 5.)),
                                ..Default::default()
                            },
                            transform: Transform {
                                translation,
                                scale: Vec3::new(1., 1., 1.),
                                ..Default::default()
                            },
                            ..Default::default()
                        })
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
    mut status_event: EventWriter<StatusEffectEvent>,
    mut hit_event: EventWriter<HitEvent>,
    player_skills: Query<(&PlayerSkills, &BonusDamage, &CritChance, &CritDamage), With<Player>>,
) {
    // Get poison strength and crit from player (if player exists)
    let Ok((skills, bonus_damage, crit_chance, crit_damage)) = player_skills.get_single() else {
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
                    damage = (damage as f32 * 1.1_f32.powi(frail_stacks as i32)).round() as i32;
                }

                // Poison can crit internally (extra damage) but we don't set was_crit so other systems don't react
                if effective_crit_chance > 0 && rng.gen_ratio(effective_crit_chance.min(100), 100) {
                    damage = (damage as f32 * crit_multiplier).round().max(1.) as i32;
                }
                damage = damage.max(1);

                hit_event.send(HitEvent {
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
                status_event.send(StatusEffectEvent {
                    entity: e,
                    effect: StatusEffect::Poison,
                    num_stacks: 0,
                });
            } else {
                let stacks = burning.stacks as i32;
                status_event.send(StatusEffectEvent {
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
    mut status_event: EventWriter<StatusEffectEvent>,
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
            status_event.send(StatusEffectEvent {
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
    mut status_event: EventWriter<StatusEffectEvent>,
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
            status_event.send(StatusEffectEvent {
                entity: e,
                effect: StatusEffect::Slow,
                num_stacks: remaining,
            });
        }
    }
}

/// Adds a slow stack (up to 3) via mutating [`MobStatusEffects`] in-place.
/// Replaces the previous insert-component path; no archetype transitions.
pub fn try_add_slow_stacks(
    hit_e: Entity,
    status: &mut MobStatusEffects,
    status_event: &mut EventWriter<StatusEffectEvent>,
) {
    if let Some(slow) = status.slow.as_mut() {
        if slow.num_stacks < 3 {
            slow.num_stacks += 1;
            slow.timer.reset();
            status_event.send(StatusEffectEvent {
                entity: hit_e,
                effect: StatusEffect::Slow,
                num_stacks: slow.num_stacks as i32,
            });
        }
    } else {
        status.slow = Some(Slow {
            num_stacks: 1,
            timer: Timer::from_seconds(1.7, TimerMode::Repeating),
        });
        status_event.send(StatusEffectEvent {
            entity: hit_e,
            effect: StatusEffect::Slow,
            num_stacks: 1,
        });
    }
}

/// Handle frozen status effect ticks - mobs are frozen with blue tint
pub fn handle_frozen_ticks(
    time: Res<Time>,
    mut frozen_mobs: Query<
        (&mut MobStatusEffects, &mut TextureAtlasSprite),
        (Without<DeathState>, Without<MarkedForDeath>),
    >,
) {
    for (mut status, mut sprite) in frozen_mobs.iter_mut() {
        let Some(frozen) = status.frozen.as_mut() else {
            continue;
        };
        frozen.timer.tick(time.delta());

        if frozen.timer.just_finished() {
            // Restore original color and clear freeze
            sprite.color = frozen.original_color;
            status.frozen = None;
        }
    }
}

/// Check if a mob should become frozen when reaching 3 slow stacks (Freeze blessing)
pub fn check_freeze_on_slow_stacks(
    blessings: Query<&crate::blessings::OwnedBlessings>,
    mut slow_query: Query<
        (Entity, &mut MobStatusEffects, &mut TextureAtlasSprite),
        (
            Changed<MobStatusEffects>,
            Without<DeathState>,
            Without<MarkedForDeath>,
        ),
    >,
    mut status_event: EventWriter<StatusEffectEvent>,
) {
    let has_freeze_blessing = blessings
        .get_single()
        .map(|b| b.has_blessing(crate::blessings::Blessing::Freeze))
        .unwrap_or(false);

    if !has_freeze_blessing {
        return;
    }

    for (entity, mut status, mut sprite) in slow_query.iter_mut() {
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
        let original_color = sprite.color;
        status.frozen = Some(Frozen {
            timer: Timer::from_seconds(2.0, TimerMode::Once),
            original_color,
        });
        // Apply blue tint directly to the sprite
        sprite.color = STATUS_EFFECT_BLUE_TINT;
        status_event.send(StatusEffectEvent {
            entity,
            effect: StatusEffect::Frozen,
            num_stacks: 1,
        });
    }
}
