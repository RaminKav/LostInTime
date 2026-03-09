use bevy::prelude::*;
use bevy_proto::prelude::{ReflectSchematic, Schematic};
use serde::Deserialize;
use strum_macros::{Display, EnumIter};

use crate::assets::Graphics;
use crate::attributes::{BonusDamage, CritChance, CritDamage};
use crate::enemy::red_mushking::DeathState;
use crate::player::skills::{Heirloom, PlayerSkills};
use crate::Player;
use rand::Rng;

use super::{HitEvent, MarkedForDeath};

#[derive(
    Deserialize, Debug, EnumIter, Display, Hash, Clone, Reflect, FromReflect, Eq, PartialEq,
)]
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

#[derive(Component, Deserialize, Debug, Clone, Schematic, Reflect, FromReflect)]
#[reflect(Schematic)]
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

#[derive(Component, Clone)]
pub struct Burning {
    pub tick_timer: Timer,
    pub duration_timer: Timer,
    pub stacks: u8,
}
#[derive(Component)]
pub struct Poisoned {
    pub tick_timer: Timer,
    pub duration_timer: Timer,
    pub damage: u8,
}
#[derive(Component, Debug)]
pub struct Frail {
    pub num_stacks: u8,
    pub timer: Timer,
}
#[derive(Component, Debug)]
pub struct Slow {
    pub num_stacks: u8,
    pub timer: Timer,
}

/// Frozen status effect from Freeze blessing - mob is completely frozen when at 3 stacks
#[derive(Component, Debug)]
pub struct Frozen {
    pub timer: Timer,
    pub original_color: Color,
}

/// Component to mark enemies slowed by RapidFire skill (50% speed reduction)
#[derive(Component, Debug)]
pub struct RapidfireSlow;

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
        Changed<StatusEffectTracker>,
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
                    .set_parent(entity)
                    .id();

                // Add text count next to the icon
                commands
                    .spawn(Text2dBundle {
                        text: Text::from_section(
                            effect.num_stacks.to_string(),
                            TextStyle {
                                font: asset_server.load("fonts/slkscr.ttf"),
                                font_size: 8.4,
                                color: Color::WHITE,
                            },
                        ),
                        transform: Transform {
                            translation: Vec3::new(3.0, 0., 1.),
                            ..Default::default()
                        },
                        text_anchor: bevy::sprite::Anchor::CenterLeft,
                        ..Default::default()
                    })
                    // .insert(RenderLayers::from_layers(&[1]))
                    .set_parent(icon_entity);
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
                        .set_parent(entity);
                }
            }
        }
    }
}

pub fn handle_burning_ticks(
    mut burning: Query<
        (Entity, &mut Burning, Option<&Frail>),
        (Without<DeathState>, Without<MarkedForDeath>),
    >,
    time: Res<Time>,
    mut commands: Commands,
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

    for (e, mut burning, frail_option) in burning.iter_mut() {
        burning.duration_timer.tick(time.delta());
        if !burning.duration_timer.just_finished() {
            burning.tick_timer.tick(time.delta());
            if burning.tick_timer.just_finished() {
                // Damage = stacks + bonus damage from PoisonStrength heirloom
                let base_damage = burning.stacks as i32;
                let mut damage = base_damage * poison_strength_bonus.round() as i32;

                // Frail multiplier: 1.1x per stack (same as other damage)
                let frail_stacks = frail_option.map(|f| f.num_stacks).unwrap_or(0);
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
                });
                burning.tick_timer.reset();
            }
        } else {
            // Reduce half the stacks and reset the timer instead of clearing all
            burning.stacks = (burning.stacks / 2).max(0);
            burning.duration_timer.reset();

            if burning.stacks == 0 {
                commands.entity(e).remove::<Burning>();
                status_event.send(StatusEffectEvent {
                    entity: e,
                    effect: StatusEffect::Poison,
                    num_stacks: 0,
                });
            } else {
                status_event.send(StatusEffectEvent {
                    entity: e,
                    effect: StatusEffect::Poison,
                    num_stacks: burning.stacks as i32,
                });
            }
        }
    }
}
pub fn handle_frail_stack_ticks(
    mut frailed: Query<(Entity, &mut Frail)>,
    mut commands: Commands,
    time: Res<Time>,
    mut status_event: EventWriter<StatusEffectEvent>,
) {
    for (e, mut frail) in frailed.iter_mut() {
        frail.timer.tick(time.delta());
        if frail.timer.just_finished() {
            frail.num_stacks -= 1;
            if frail.num_stacks == 0 {
                commands.entity(e).remove::<Frail>();
            }
            status_event.send(StatusEffectEvent {
                entity: e,
                effect: StatusEffect::Frail,
                num_stacks: frail.num_stacks as i32,
            });
        }
    }
}
pub fn handle_slow_stack_ticks(
    mut slowed: Query<(Entity, &mut Slow)>,
    mut commands: Commands,
    time: Res<Time>,
    mut status_event: EventWriter<StatusEffectEvent>,
) {
    for (e, mut slow) in slowed.iter_mut() {
        slow.timer.tick(time.delta());
        if slow.timer.just_finished() {
            slow.num_stacks -= 1;
            if slow.num_stacks == 0 {
                commands.entity(e).remove::<Slow>();
            }
            status_event.send(StatusEffectEvent {
                entity: e,
                effect: StatusEffect::Slow,
                num_stacks: slow.num_stacks as i32,
            });
        }
    }
}

pub fn try_add_slow_stacks(
    hit_e: Entity,
    commands: &mut Commands,
    status_event: &mut EventWriter<StatusEffectEvent>,
    slowed_option: Option<&mut Slow>,
) {
    if let Some(slow_stacks) = slowed_option {
        if slow_stacks.num_stacks < 3 {
            slow_stacks.num_stacks += 1;
            slow_stacks.timer.reset();
            status_event.send(StatusEffectEvent {
                entity: hit_e,
                effect: StatusEffect::Slow,
                num_stacks: slow_stacks.num_stacks as i32,
            });
        }
    } else {
        // Check if entity still exists before inserting Slow
        if let Some(mut hit_entity_commands) = commands.get_entity(hit_e) {
            hit_entity_commands.insert(Slow {
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
}

/// Handle frozen status effect ticks - mobs are frozen with blue tint
pub fn handle_frozen_ticks(
    mut commands: Commands,
    time: Res<Time>,
    mut frozen_mobs: Query<(Entity, &mut Frozen, &mut TextureAtlasSprite)>,
) {
    for (entity, mut frozen, mut sprite) in frozen_mobs.iter_mut() {
        frozen.timer.tick(time.delta());

        if frozen.timer.just_finished() {
            // Restore original color and remove freeze
            sprite.color = frozen.original_color;
            commands.entity(entity).remove::<Frozen>();
        }
    }
}

/// Check if a mob should become frozen when reaching 3 slow stacks (Freeze blessing)
pub fn check_freeze_on_slow_stacks(
    mut commands: Commands,
    blessings: Query<&crate::blessings::OwnedBlessings>,
    slow_query: Query<(Entity, &Slow, &TextureAtlasSprite), (Changed<Slow>, Without<Frozen>)>,
    mut status_event: EventWriter<StatusEffectEvent>,
) {
    let has_freeze_blessing = blessings
        .get_single()
        .map(|b| b.has_blessing(crate::blessings::Blessing::Freeze))
        .unwrap_or(false);

    if !has_freeze_blessing {
        return;
    }

    for (entity, slow, sprite) in slow_query.iter() {
        if slow.num_stacks >= 3 {
            // Freeze the mob with a blue tint
            commands.entity(entity).insert(Frozen {
                timer: Timer::from_seconds(2.0, TimerMode::Once),
                original_color: sprite.color,
            });
            // Apply blue tint
            commands.entity(entity).insert(TextureAtlasSprite {
                color: Color::rgba(0.5, 0.7, 1.0, 1.0),
                ..sprite.clone()
            });
            // Send frozen status event
            status_event.send(StatusEffectEvent {
                entity,
                effect: StatusEffect::Frozen,
                num_stacks: 1,
            });
        }
    }
}
