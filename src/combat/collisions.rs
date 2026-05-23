use super::{try_add_slow_stacks, HitEvent, HitMarker, InvincibilityTimer, StatusEffectEvent};
use crate::blessings::OwnedBlessings;
use crate::client::is_not_paused;
use crate::combat::LifestealEvent;
use crate::player::combat_heirlooms::ThornsOnDamageTracker;
use crate::player::skill_heirlooms::{handle_fire_pillar_hit_clear, handle_laser_beam_hit_clear};
use crate::player::skills::{Heirloom, PlayerSkills};
use crate::{
    attributes::ItemRarity,
    player::beastiary::mob_display_name,
    ui::{
        damage_numbers::FloatingTextQueue,
        global_text_message::GlobalTextMessageEvent,
    },
};
use crate::NO_XP;
use crate::{
    animations::{player_sprite::PlayerAnimation, ui_animaitons::UIIconMover},
    attributes::{
        modifiers::ModifyManaEvent, Attack, Defence, Dodge, InvincibilityCooldown, Thorns,
    },
    audio::{AudioSoundEffect, SoundSpawner},
    chaos::ChaosTracker,
    client::analytics::{AnalyticsTrigger, AnalyticsUpdateEvent},
    enemy::{Mob, MobIsAttacking},
    inventory::{
        can_auto_equip_weapon_on_pickup, try_auto_equip_weapon_on_pickup, Inventory, ItemStack,
    },
    item::{
        item_actions::ItemActionParam,
        object_actions::TouchTriggerObjectAction,
        projectile::{
            EnemyProjectile, PetProjectileMarker, Projectile, ProjectileState, RangedAttackEvent,
        },
        Equipment, ItemDrop, MainHand, WorldObject,
    },
    player::rogue_skills::LungeState,
    player::skill_heirlooms::Stealthed,
    player::{
        mage_skills::IceExplosionDmg,
        melee_skills::{Parried, ParryState, ParrySuccessEvent, SpearAttack},
    },
    proto::proto_param::ProtoParam,
    ui::{damage_numbers::DodgeEvent, FlashExpBarEvent},
    CustomFlush, GameParam, GameState, Player, ScreenResolution,
};
use bevy::prelude::*;
use bevy_rapier2d::prelude::{Collider, CollisionEvent, RapierContext};
use rand::Rng;

use crate::pets::state::Pet;
use crate::world::chunk::WaterCollider;

pub struct CollisionPlugion;

impl Plugin for CollisionPlugion {
    fn build(&self, app: &mut App) {
        app.add_systems(
            (
                add_contact_damage_to_cactuses.in_set(OnUpdate(GameState::Main)),
                check_melee_hit_collisions.run_if(is_not_paused),
                check_boss_to_objects_collisions.run_if(is_not_paused),
                check_mob_to_player_collisions.run_if(is_not_paused),
                check_projectile_hit_mob_collisions.run_if(is_not_paused),
                check_multihit_projectile_ongoing_collisions
                    .run_if(is_not_paused)
                    .after(handle_fire_pillar_hit_clear)
                    .after(handle_laser_beam_hit_clear),
                check_projectile_hit_player_collisions.run_if(is_not_paused),
                check_contact_damage_collisions.run_if(is_not_paused),
                check_object_trigger_collisions
                    .run_if(is_not_paused)
                    .after(CustomFlush)
                    .before(check_item_drop_collisions),
            )
                .in_set(OnUpdate(GameState::Main)),
        )
        .add_system(
            check_item_drop_collisions
                .after(CustomFlush)
                .run_if(in_state(GameState::Main).or_else(in_state(GameState::Initializing))),
        );
    }
}

#[derive(Component)]
pub struct DamagesWorldObjects;
#[derive(Component)]
pub struct PlayerAttackCollider;

/// Deals this much damage to the player when they collide with the entity (e.g. desert cactuses).
#[derive(Component)]
pub struct ContactDamage(pub i32);

fn add_contact_damage_to_cactuses(
    mut commands: Commands,
    cactuses: Query<(Entity, &WorldObject), (With<WorldObject>, Without<ContactDamage>)>,
) {
    for (entity, obj) in cactuses.iter() {
        if obj.is_desert_cactus() {
            commands.entity(entity).insert(ContactDamage(5));
        }
    }
}

fn check_contact_damage_collisions(
    player: Query<(Entity, &GlobalTransform, &Defence), With<Player>>,
    hazards: Query<(&ContactDamage, &GlobalTransform)>,
    rapier_context: Res<RapierContext>,
    mut hit_event: EventWriter<HitEvent>,
    in_i_frame: Query<&InvincibilityTimer>,
) {
    let Ok((player_e, player_txfm, defence)) = player.get_single() else {
        return;
    };
    for (e1, e2, _) in rapier_context.intersections_with(player_e) {
        if e1 != player_e {
            continue;
        }
        let Ok((contact_damage, hazard_txfm)) = hazards.get(e2) else {
            continue;
        };
        if in_i_frame.get(player_e).is_ok() {
            continue;
        }
        let player_pos = player_txfm.translation().truncate();
        let hazard_pos = hazard_txfm.translation().truncate();
        let dir = (player_pos - hazard_pos).normalize_or_zero();
        let damage = f32::round(contact_damage.0 as f32 * (0.997_f32.powi(defence.0))) as i32;
        hit_event.send(HitEvent {
            hit_entity: player_e,
            damage,
            dir,
            hit_with_melee: None,
            hit_with_projectile: None,
            hit_by_mob: None,
            hit_by_pet: None,
            was_crit: false,
            was_overcrit: false,
            ignore_tool: false,
            from_heirloom_effect: None,
        });
        break;
    }
}

fn check_melee_hit_collisions(
    mut commands: Commands,
    context: ResMut<RapierContext>,
    weapons: Query<
        (Entity, &Parent, &GlobalTransform, &WorldObject),
        (Without<HitMarker>, With<MainHand>),
    >,
    mut hit_event: EventWriter<HitEvent>,
    game: GameParam,
    world_obj: Query<Entity, (With<WorldObject>, Without<MainHand>)>,
    mobs: Query<
        (
            &GlobalTransform,
            Option<&crate::combat::status_effects::MobStatusEffects>,
        ),
        With<Mob>,
    >,
    anim: Query<&PlayerAnimation>,
    mut hit_tracker: Local<Vec<Entity>>,
) {
    let anim = anim.single();
    if !anim.is_an_attack() {
        hit_tracker.clear();
    }
    if let Ok((weapon_e, weapon_parent, weapon_t, weapon_obj)) = weapons.get_single() {
        let hits_this_frame = context.intersection_pairs().filter(|c| {
            (c.0 == weapon_e && c.1 != weapon_parent.get())
                || (c.1 == weapon_e && c.0 != weapon_parent.get())
        });
        for hit in hits_this_frame {
            let hit_entity = if hit.0 == weapon_e { hit.1 } else { hit.0 };
            if !anim.is_an_attack()
                || world_obj.get(hit_entity).is_ok()
                || hit_tracker.contains(&hit_entity)
                || weapon_obj.is_magic_weapon()
            {
                continue;
            }

            hit_tracker.push(hit_entity);
            let Ok((mob_txfm, status_option)) = mobs.get(hit_entity) else {
                continue;
            };

            let frail_stacks = status_option.map(|s| s.frail_stacks()).unwrap_or(0);
            let (mut damage, was_crit, was_overcrit) =
                game.calculate_player_damage(0, None, 0, None, frail_stacks, 0);

            let is_status_effected = status_option
                .map(|s| s.is_burning() || s.is_slowed() || s.frail.is_some())
                .unwrap_or(false);

            if is_status_effected && game.has_skill(Heirloom::TeleportStatusDMG) {
                damage = f32::ceil(damage as f32 * 1.2) as u32;
            }
            let delta = weapon_t.translation() - mob_txfm.translation();

            hit_event.send(HitEvent {
                hit_entity,
                hit_by_pet: None,
                damage: damage as i32,
                dir: delta.normalize_or_zero().truncate() * -1.,
                hit_with_melee: Some(*weapon_obj),
                hit_with_projectile: None,
                was_crit,
                was_overcrit,
                hit_by_mob: None,
                ignore_tool: false,
                from_heirloom_effect: None,
            });

            commands.spawn(SoundSpawner::new(AudioSoundEffect::DefaultEnemyHit, 0.2));
        }
    }
}
fn check_projectile_hit_mob_collisions(
    mut commands: Commands,
    player_attack: Query<(Entity, &Children), With<Player>>,
    allowed_targets: Query<
        (Entity, &GlobalTransform),
        (
            Without<ItemStack>,
            Without<MainHand>,
            Without<Projectile>,
            Without<Pet>,                      // Don't hit pets
            Without<TouchTriggerObjectAction>, // Don't hit bounce flowers etc.
            Without<WaterCollider>,            // Don't hit water tile colliders
        ),
    >,
    mut hit_event: EventWriter<HitEvent>,
    mut collisions: EventReader<CollisionEvent>,
    mut projectiles: Query<
        (
            Entity,
            &mut ProjectileState,
            &Projectile,
            &Attack,
            Option<&IceExplosionDmg>,
            Option<&SpearAttack>,
        ),
        Without<EnemyProjectile>,
    >,
    proj_transforms: Query<&GlobalTransform, Without<EnemyProjectile>>,
    mut children: Query<&Parent>,
    mut status_check: Query<&mut crate::combat::status_effects::MobStatusEffects>,
    nearby_mobs: Query<(Entity, &GlobalTransform), With<Mob>>,
    mut game: GameParam,
    mut status_event: EventWriter<StatusEffectEvent>,
    pet_check: Query<Entity, With<PetProjectileMarker>>,
    player_skills: Query<&PlayerSkills, With<Player>>,
    mut lifesteal_events: EventWriter<LifestealEvent>,
) {
    for evt in collisions.iter() {
        let CollisionEvent::Started(e1, e2, _) = evt else {
            continue;
        };
        for (e1, e2) in [(e1, e2), (e2, e1)] {
            //TODO: fr gotta refasctor this...
            let (proj_entity, mut state, proj, att, ice_aoe, spear_att) =
                if let Ok(parent_e) = children.get_mut(*e1) {
                    if let Ok((proj_entity, state, proj, att, ice_aoe, spear_att)) =
                        projectiles.get_mut(parent_e.get())
                    {
                        //collider is on the child, proj data on the parent
                        (proj_entity, state, proj, att, ice_aoe, spear_att)
                    } else if let Ok((proj_entity, state, proj, att, ice_aoe, spear_att)) =
                        projectiles.get_mut(*e1)
                    {
                        //collider and proj data are on the same entity
                        (proj_entity, state, proj, att, ice_aoe, spear_att)
                    } else {
                        continue;
                    }
                } else if let Ok((proj_entity, state, proj, att, ice_aoe, spear_att)) =
                    projectiles.get_mut(*e1)
                {
                    //collider and proj data are on the same entity
                    (proj_entity, state, proj, att, ice_aoe, spear_att)
                } else {
                    continue;
                };
            let Ok((player_e, children)) = player_attack.get_single() else {
                continue;
            };
            if player_e == *e2 || children.contains(e2) || !allowed_targets.contains(*e2) {
                continue;
            }
            if state.hit_entities.contains(e2) {
                continue;
            }
            state.hit_entities.push(*e2);
            let status_result = status_check.get_mut(*e2).ok();
            let (is_slowed, is_status_effected, frail_stacks) = match &status_result {
                Some(status) => (
                    status.is_slowed(),
                    status.is_burning() || status.is_slowed() || status.frail.is_some(),
                    status.frail_stacks(),
                ),
                None => (false, false, 0),
            };

            let crit_bonus = if is_slowed && game.has_skill(Heirloom::FrozenCrit) {
                15
            } else {
                0
            } + if state.mana_bar_full && game.has_skill(Heirloom::MPBarCrit) {
                10
            } else {
                0
            };
            // ArrowVolley: each arrow gains bonus crit damage equal to the player's
            // current crit chance (e.g. 50% crit chance => +50% crit damage on volley arrows).
            let bonus_crit_damage = if *proj == Projectile::ArrowVolleyShot {
                game.player_stats
                    .get_single()
                    .map(|stats| stats.3 .0)
                    .unwrap_or(0)
            } else {
                0
            };
            let (mut damage, was_crit, was_overcrit) = game.calculate_player_damage(
                crit_bonus,
                None,
                0,
                Some(att.0),
                frail_stacks,
                bonus_crit_damage,
            );
            if is_status_effected && game.has_skill(Heirloom::TeleportStatusDMG) {
                damage = f32::ceil(damage as f32 * 1.2) as u32;
            }
            let (_e, hit_txfm) = allowed_targets.get(*e2).unwrap();
            let enemy_pos = hit_txfm.translation().truncate();
            // Note: SpearAttack gravity pull is now handled proactively in handle_spear_pull_delay
            // The SpearAttack component is still used to identify the damage source
            if ice_aoe.is_some() {
                if let Some(mut status) = status_result {
                    try_add_slow_stacks(*e2, status.as_mut(), &mut status_event);
                }
            }

            // Calculate knockback direction
            // For Shout (AoE), calculate direction from projectile position to enemy
            // For other projectiles, use the projectile's direction
            let knockback_dir = if *proj == Projectile::Shout {
                // Get projectile position - use the projectile entity we already have
                let proj_pos = proj_transforms
                    .get(proj_entity)
                    .map(|t| t.translation().truncate())
                    .unwrap_or(enemy_pos);

                // Direction from projectile to enemy
                let delta = enemy_pos - proj_pos;
                delta.normalize_or_zero()
            } else {
                state.direction
            };

            // Check if this projectile is from a heirloom on-kill effect
            let heirloom_source = match proj {
                Projectile::IceExplosionAOE => Some(Heirloom::FrozenAoE),
                Projectile::Echo => Some(Heirloom::OnHitEcho),
                Projectile::EnergyBall => Some(Heirloom::EnergyBallBarrage),
                _ => None,
            };

            // ThornsLifesteal: Apply lifesteal for ThornsProjectile hits
            let is_thorns_projectile = *proj == Projectile::ThornsProjectile;
            if is_thorns_projectile {
                if let Ok(skills) = player_skills.get_single() {
                    let thorns_lifesteal_stacks = skills.get_count(Heirloom::ThornsLifesteal);
                    if thorns_lifesteal_stacks > 0 {
                        game.heirloom_trigger_counts
                            .increment(Heirloom::ThornsLifesteal);
                        lifesteal_events.send(LifestealEvent {
                            thorns_lifesteal_stacks,
                            is_direct_player_damage: false,
                        });
                    }
                }
            }

            hit_event.send(HitEvent {
                hit_by_pet: pet_check.get(*e1).ok(),
                hit_entity: *e2,
                damage: damage as i32,
                dir: knockback_dir,
                hit_with_melee: None,
                hit_with_projectile: Some(proj.clone()),
                ignore_tool: false,
                hit_by_mob: None,
                was_crit,
                was_overcrit,
                from_heirloom_effect: heirloom_source,
            });
            if nearby_mobs.get(*e2).is_ok() {
                if proj.clone() == Projectile::IceShard
                    || proj.clone() == Projectile::IceExplosionAOE
                    || proj.clone() == Projectile::FireRing
                {
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::IceStaffHit, 0.2));
                } else if proj.clone() == Projectile::Electricity
                    || proj.clone() == Projectile::EnergyBall
                {
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::LightningStaffHit, 0.2));
                } else {
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::DefaultEnemyHit, 0.2));
                }
            }

            //non-animating sprites are despawned immediately
            if state.despawn_on_hit {
                commands.entity(proj_entity).despawn_recursive();
            }
        }
    }
}

/// Check for ongoing collisions with multi-hit projectiles (FireRing, LaserBeam) after hit_entities are cleared
/// This allows enemies already colliding to take damage again immediately
fn check_multihit_projectile_ongoing_collisions(
    mut commands: Commands,
    player_attack: Query<(Entity, &Children), With<Player>>,
    allowed_targets: Query<
        (Entity, &GlobalTransform),
        (
            Without<ItemStack>,
            Without<MainHand>,
            Without<Projectile>,
            Without<Pet>,
            Without<TouchTriggerObjectAction>,
            Without<WaterCollider>, // Don't hit water tile colliders
        ),
    >,
    mut hit_event: EventWriter<HitEvent>,
    rapier_context: Res<RapierContext>,
    mut projectiles: Query<
        (
            Entity,
            &mut ProjectileState,
            &Projectile,
            &Attack,
            Option<&IceExplosionDmg>,
            Option<&SpearAttack>,
        ),
        Without<EnemyProjectile>,
    >,
    proj_transforms: Query<&GlobalTransform, Without<EnemyProjectile>>,
    children: Query<&Children>,
    mut status_check: Query<&mut crate::combat::status_effects::MobStatusEffects>,
    nearby_mobs: Query<(Entity, &GlobalTransform), With<Mob>>,
    game: GameParam,
    mut status_event: EventWriter<StatusEffectEvent>,
    pet_check: Query<Entity, With<PetProjectileMarker>>,
) {
    // Only process multi-hit projectiles (FireRing, LaserBeam)
    for (proj_entity, mut state, proj, att, ice_aoe, spear_att) in projectiles.iter_mut() {
        if *proj != Projectile::FireRing && *proj != Projectile::LaserBeam {
            continue;
        }

        // Check for ongoing intersections with this multi-hit projectile
        // Try the projectile entity first, then check children if it has any
        let entities_to_check: Vec<Entity> = {
            let mut entities = vec![proj_entity];
            if let Ok(proj_children) = children.get(proj_entity) {
                entities.extend(proj_children.iter());
            }
            entities
        };

        for collider_entity in entities_to_check {
            for (e1, e2, _) in rapier_context.intersections_with(collider_entity) {
                for (e1, e2) in [(e1, e2), (e2, e1)] {
                    // Find which entity is the projectile/collider and which is the target
                    let target_e = if e1 == collider_entity {
                        e2
                    } else if e2 == collider_entity {
                        e1
                    } else {
                        continue;
                    };

                    let Ok((player_e, player_children)) = player_attack.get_single() else {
                        continue;
                    };
                    if player_e == target_e
                        || player_children.contains(&target_e)
                        || !allowed_targets.contains(target_e)
                    {
                        continue;
                    }

                    // Only process if not already in hit_entities (to avoid duplicate hits in same frame)
                    if state.hit_entities.contains(&target_e) {
                        continue;
                    }

                    // Add to hit_entities to prevent duplicate processing
                    state.hit_entities.push(target_e);

                    let status_result = status_check.get_mut(target_e).ok();
                    let (is_slowed, is_status_effected, frail_stacks) = match &status_result {
                        Some(status) => (
                            status.is_slowed(),
                            status.is_burning() || status.is_slowed() || status.frail.is_some(),
                            status.frail_stacks(),
                        ),
                        None => (false, false, 0),
                    };

                    let crit_bonus =
                        if is_slowed && game.has_skill(Heirloom::FrozenCrit) {
                            15
                        } else {
                            0
                        } + if state.mana_bar_full && game.has_skill(Heirloom::MPBarCrit) {
                            10
                        } else {
                            0
                        };

                    let (mut damage, was_crit, was_overcrit) = game.calculate_player_damage(
                        crit_bonus,
                        None,
                        0,
                        Some(att.0),
                        frail_stacks,
                        0,
                    );

                    if is_status_effected && game.has_skill(Heirloom::TeleportStatusDMG) {
                        damage = f32::ceil(damage as f32 * 1.2) as u32;
                    }

                    let (_e, hit_txfm) = allowed_targets.get(target_e).unwrap();
                    let enemy_pos = hit_txfm.translation().truncate();

                    // Note: SpearAttack gravity pull is now handled proactively in handle_spear_pull_delay
                    // The SpearAttack component is still used to identify the damage source

                    if ice_aoe.is_some() {
                        if let Some(mut status) = status_result {
                            try_add_slow_stacks(target_e, status.as_mut(), &mut status_event);
                        }
                    }

                    // For multi-hit projectiles, calculate direction from projectile to enemy
                    let proj_pos = proj_transforms
                        .get(proj_entity)
                        .map(|t| t.translation().truncate())
                        .unwrap_or(enemy_pos);
                    let delta = enemy_pos - proj_pos;
                    let knockback_dir = delta.normalize_or_zero();

                    let heirloom_source = match proj {
                        Projectile::IceExplosionAOE => Some(Heirloom::FrozenAoE),
                        _ => None,
                    };

                    hit_event.send(HitEvent {
                        hit_by_pet: pet_check.get(collider_entity).ok(),
                        hit_entity: target_e,
                        damage: damage as i32,
                        dir: knockback_dir,
                        hit_with_melee: None,
                        hit_with_projectile: Some(proj.clone()),
                        ignore_tool: false,
                        hit_by_mob: None,
                        was_crit,
                        was_overcrit,
                        from_heirloom_effect: heirloom_source,
                    });

                    if nearby_mobs.get(target_e).is_ok() {
                        // Play appropriate sound based on projectile type
                        let sound = match proj {
                            Projectile::LaserBeam => AudioSoundEffect::LightningStaffHit,
                            _ => AudioSoundEffect::IceStaffHit,
                        };
                        commands.spawn(SoundSpawner::new(sound, 0.2));
                    }
                }
            }
        }
    }
}

fn check_projectile_hit_player_collisions(
    mut commands: Commands,
    enemy_attack: Query<(Entity, &Attack), With<Mob>>,
    mut allowed_targets: Query<
        (
            Option<&WorldObject>,
            Option<&mut ParryState>,
            Option<&InvincibilityCooldown>,
            Option<&Attack>,
            Option<&Stealthed>,
            Option<&Defence>,
        ),
        (
            Or<(With<Player>, With<WorldObject>)>,
            (Without<Projectile>, Without<MainHand>, Without<ItemStack>),
        ),
    >,
    lunge_states: Query<&LungeState, With<Player>>,
    mut hit_event: EventWriter<HitEvent>,
    mut collisions: EventReader<CollisionEvent>,
    mut projectiles: Query<
        (
            Entity,
            &mut ProjectileState,
            &Projectile,
            &Attack,
            &EnemyProjectile,
        ),
        With<EnemyProjectile>,
    >,
    mut parry_events: EventWriter<ParrySuccessEvent>,
    mut ranged_attack_event: EventWriter<RangedAttackEvent>,
    mut children: Query<&Parent>,
) {
    for evt in collisions.iter() {
        let CollisionEvent::Started(e1, e2, _) = evt else {
            continue;
        };
        for (e1, e2) in [(e1, e2), (e2, e1)] {
            let (proj_entity, mut state, proj, att, enemy_proj) = if let Ok(e) =
                children.get_mut(*e1)
            {
                if let Ok((proj_entity, state, proj, att, enemy_proj)) =
                    projectiles.get_mut(e.get())
                {
                    (proj_entity, state, proj, att, enemy_proj)
                } else {
                    continue;
                }
            } else if let Ok((proj_entity, state, proj, att, enemy_proj)) = projectiles.get_mut(*e1)
            {
                (proj_entity, state, proj, att, enemy_proj)
            } else {
                continue;
            };
            let Ok((enemy_e, _attack)) = enemy_attack.get(enemy_proj.entity) else {
                continue;
            };
            if enemy_e == *e2 || !allowed_targets.contains(*e2) {
                continue;
            }
            if let Some(obj) = allowed_targets.get(*e2).unwrap().0 {
                if [
                    WorldObject::Grass,
                    WorldObject::Grass2,
                    WorldObject::Grass3,
                    WorldObject::RedFlower,
                    WorldObject::PinkFlower,
                    WorldObject::YellowFlower,
                    WorldObject::RedMushroom,
                    WorldObject::BrownMushroom,
                    WorldObject::Stick,
                ]
                .contains(obj)
                {
                    continue;
                }
            }
            if state.hit_entities.contains(e2) {
                continue;
            }
            state.hit_entities.push(*e2);
            let mut hit_successful = true;
            let (_, mut parry_option, i_frames, p_attack, stealth_opt, defence_opt) =
                allowed_targets.get_mut(*e2).unwrap();
            // Ignore projectile hits if stealthed
            if stealth_opt.is_some() {
                continue;
            }
            // Ignore projectile hits if lunging (lunge duration is active)
            // Check if lunge has started (percent > 0) but not finished (percent < 1.0)
            if let Ok(lunge_state) = lunge_states.get(*e2) {
                let lunge_percent = lunge_state.lunge_duration.percent();
                if lunge_percent > 0.0 && lunge_percent < 1.0 {
                    continue;
                }
            }
            if let Some(ref mut parry) = parry_option {
                if parry.active && !parry.success {
                    parry_events.send(ParrySuccessEvent(*e1));
                    hit_successful = false;
                    parry.success = true;

                    commands
                        .entity(*e2)
                        .insert(InvincibilityTimer(Timer::from_seconds(
                            i_frames.unwrap().0,
                            TimerMode::Once,
                        )))
                        .insert(PlayerAnimation::ParryHit);

                    //deflected proj
                    ranged_attack_event.send(RangedAttackEvent {
                        projectile: proj.clone(),
                        direction: -state.direction,
                        from_enemy: false,
                        from_entity: None,
                        is_followup_proj: false,
                        mana_cost: None,
                        dmg_override: Some(p_attack.unwrap().0),
                        pos_override: None,
                        spawn_delay: 0.,
                    })
                }
            }
            if hit_successful {
                // Apply defense reduction if the target is a player with defense stat
                let final_damage = if let Some(defence) = defence_opt {
                    // Same formula as mob-to-player collisions: damage * (0.99 ^ defense)
                    f32::round(att.0 as f32 * (0.997_f32.powi(defence.0))) as i32
                } else {
                    att.0
                };

                hit_event.send(HitEvent {
                    hit_by_pet: None,
                    hit_entity: *e2,
                    damage: final_damage,
                    dir: state.direction,
                    hit_with_melee: None,
                    hit_with_projectile: Some(proj.clone()),
                    ignore_tool: false,
                    hit_by_mob: Some(enemy_proj.mob.clone()),
                    was_crit: false,
                    was_overcrit: false,
                    from_heirloom_effect: None,
                });
            }
            if state.despawn_on_hit {
                commands.entity(proj_entity).despawn_recursive();
            }
        }
    }
}
pub const ITEM_PICKUP_DISTANCE: f32 = 6.0;

pub fn check_item_drop_collisions(
    mut commands: Commands,
    player: Query<&Transform, With<Player>>,
    pets: Query<(), With<Pet>>,
    item_drops: Query<
        (Entity, &Transform, &ItemStack),
        (
            With<ItemDrop>,
            Without<MainHand>,
            Without<Equipment>,
            Without<TouchTriggerObjectAction>,
            Without<Player>,
        ),
    >,
    mut game: GameParam,
    mut inv: Query<&mut Inventory>,
    mut analytics: EventWriter<AnalyticsUpdateEvent>,
    mut modify_mana_event: EventWriter<ModifyManaEvent>,
    mut text_timer: ResMut<FloatingTextQueue>,
    resolution: Res<ScreenResolution>,
    mut chaos_tracker: ResMut<ChaosTracker>,
    mut flash_event: EventWriter<FlashExpBarEvent>,
    mut global_text_events: EventWriter<GlobalTextMessageEvent>,
    proto: ProtoParam,
    mut beastiary: ResMut<crate::player::beastiary::Beastiary>,
) {
    let player_txfm = player.single();
    let player_pos = player_txfm.translation.truncate();

    for (e2, item_txfm, item_stack) in item_drops.iter() {
        let item_pos = item_txfm.translation.truncate();
        if player_pos.distance_squared(item_pos) > ITEM_PICKUP_DISTANCE * ITEM_PICKUP_DISTANCE {
            continue;
        }
        let item_stack = item_stack.clone();
        let obj = item_stack.obj_type;
        // Bestiary mob cards: never enter inventory, just bump the persistent
        // bestiary count (live + on disk) and despawn. No fly-to-HUD.
        if let Some(mob) = crate::player::beastiary::mob_for_card(obj) {
            beastiary
                .entries
                .entry(mob.clone())
                .or_default()
                .cards_collected += 1;
            crate::client::persist_beastiary_card_pickup(mob.clone());
            commands.entity(e2).despawn_recursive();
            commands.spawn(SoundSpawner::new(AudioSoundEffect::ItemPickup, 0.15));
            let item_rarity = proto
                .get_item_data(obj)
                .map(|data| data.rarity.clone())
                .unwrap_or(ItemRarity::Common);
            let text_color = if item_rarity == ItemRarity::Common {
                crate::colors::WHITE
            } else {
                item_rarity.get_color()
            };
            global_text_events.send(
                GlobalTextMessageEvent::new(
                    format!("{} Card Obtained!", mob_display_name(&mob)),
                    text_color,
                )
                .with_icon(obj),
            );
            continue;
        }
        if obj == WorldObject::TimeFragment || obj == WorldObject::Coin {
            commands.spawn(UIIconMover::new(
                Vec3::new(0., 0., 9.),
                Vec3::new(
                    -resolution.game_width / 2. + 15.,
                    resolution.game_height / 2. - 50.
                        + if obj == WorldObject::Coin { -12. } else { 0. },
                    9.,
                ),
                obj,
                0.,
                800.,
                None,
                false,
                item_stack.clone(),
                true,
            ));
            commands.entity(e2).despawn_recursive();
            analytics.send(AnalyticsUpdateEvent {
                update_type: AnalyticsTrigger::ItemCollected(obj),
            });
            text_timer.add_item(obj);
            continue;
        } else if obj == WorldObject::ManaOrb {
            let player_skills = game.get_player_skills();
            let mana_from_orb = 10 + player_skills.get_count(Heirloom::ManaOrbs) as i32 * 5;
            modify_mana_event.send(ModifyManaEvent(mana_from_orb));
            analytics.send(AnalyticsUpdateEvent {
                update_type: AnalyticsTrigger::ItemCollected(obj),
            });
            commands.entity(e2).despawn_recursive();
            commands.spawn(SoundSpawner::new(AudioSoundEffect::ItemPickup, 0.15));
            continue;
        } else if obj == WorldObject::XPShard
            || obj == WorldObject::XPShardMedium
            || obj == WorldObject::XPShardLarge
        {
            let mut xp_amount = match obj {
                WorldObject::XPShard => 9,
                WorldObject::XPShardMedium => 32,
                WorldObject::XPShardLarge => 500,
                _ => 0,
            };
            if *NO_XP {
                xp_amount = 0;
            }
            let player_skills = game.get_player_skills();
            let mut player_level = game.get_player_level_mut();
            let did_level = player_level.add_xp(xp_amount, &player_skills, &mut chaos_tracker);

            flash_event.send(FlashExpBarEvent {
                amount: xp_amount,
                did_level,
            });

            analytics.send(AnalyticsUpdateEvent {
                update_type: AnalyticsTrigger::ItemCollected(obj),
            });
            commands.entity(e2).despawn_recursive();
            commands.spawn(SoundSpawner::new(AudioSoundEffect::ItemPickup, 0.15));
            continue;
        }
        let player_has_pet = pets.iter().next().is_some();
        if !can_auto_equip_weapon_on_pickup(&item_stack, inv.single(), player_has_pet) {
            let inv_container = inv.single().items.clone();
            if inv_container
                .get_first_empty_player_slot_for_pickup(&item_stack, &proto)
                .is_none()
                && inv_container
                    .get_slot_for_item_in_container_with_space_for_pickup(&item_stack, None, &proto)
                    .is_none()
            {
                return;
            }
        }

        let mut inv_mut = inv.single_mut();
        if !try_auto_equip_weapon_on_pickup(
            item_stack.clone(),
            &mut inv_mut,
            &mut game.inv_slot_query,
            player_has_pet,
        ) {
            item_stack.add_to_inventory(&mut inv_mut.items, &mut game.inv_slot_query, &proto);
        }

        if obj != WorldObject::TimeFragment
            && obj != WorldObject::Coin
            && obj != WorldObject::ManaOrb
            && obj != WorldObject::XPShard
            && obj != WorldObject::XPShardMedium
            && obj != WorldObject::XPShardLarge
        {
            text_timer.add_item(obj);
        }

        commands.entity(e2).despawn_recursive();
        analytics.send(AnalyticsUpdateEvent {
            update_type: AnalyticsTrigger::ItemCollected(obj),
        });
        commands.spawn(SoundSpawner::new(AudioSoundEffect::ItemPickup, 0.15));
    }
}
pub fn check_object_trigger_collisions(
    mut commands: Commands,
    player: Query<Entity, With<Player>>,
    player_txfm: Query<&Transform, With<Player>>,
    allowed_targets_with_collider: Query<
        Entity,
        (
            Without<MainHand>,
            Without<Equipment>,
            With<TouchTriggerObjectAction>,
            With<Collider>,
        ),
    >,
    trigger_objects_no_collider: Query<
        (Entity, &Transform, &TouchTriggerObjectAction),
        (
            Without<MainHand>,
            Without<Equipment>,
            Without<Player>,
            Without<Collider>,
        ),
    >,
    rapier_context: Res<RapierContext>,
    items_query: Query<&TouchTriggerObjectAction>,
    game: GameParam,
    mut item_action_param: ItemActionParam,
    mut flower_anim_query: Query<
        &mut bevy_aseprite::anim::AsepriteAnimation,
        (With<WorldObject>, Without<Player>),
    >,
) {
    if !game.player().is_moving {
        return;
    }
    let player_e = player.single();
    let player_pos = player_txfm.single().translation.truncate();

    // Collider-backed triggers (e.g. PinkFlower): use Rapier overlap so the sensor position matches gameplay.
    for (e1, e2, _) in rapier_context.intersections_with(player_e) {
        for (e1, e2) in [(e1, e2), (e2, e1)] {
            let Ok(_) = player.get(e1) else { continue };
            if !allowed_targets_with_collider.contains(e2) {
                continue;
            }
            let action = items_query.get(e2).unwrap();

            if matches!(action, TouchTriggerObjectAction::Bounce) {
                if let Ok(mut anim) = flower_anim_query.get_mut(e2) {
                    anim.play();
                }
            }

            action.run_action(e2, &mut commands, &mut item_action_param);
        }
    }

    // No collider (e.g. chests using item_drop template): distance check only.
    for (entity, obj_txfm, action) in trigger_objects_no_collider.iter() {
        let obj_pos = obj_txfm.translation.truncate();
        if player_pos.distance_squared(obj_pos) > ITEM_PICKUP_DISTANCE * ITEM_PICKUP_DISTANCE {
            continue;
        }

        if matches!(action, TouchTriggerObjectAction::Bounce) {
            if let Ok(mut anim) = flower_anim_query.get_mut(entity) {
                anim.play();
            }
        }

        action.run_action(entity, &mut commands, &mut item_action_param);
    }
}
fn check_mob_to_player_collisions(
    mut commands: Commands,
    mut player: Query<
        (
            Entity,
            &Transform,
            &Thorns,
            &Defence,
            &Dodge,
            &InvincibilityCooldown,
            Option<&mut ParryState>,
            Option<&Stealthed>,
            Option<&LungeState>,
            &Attack, // Player's attack for thorns calculation
            &OwnedBlessings,
            &PlayerSkills,
            Option<&mut ThornsOnDamageTracker>,
        ),
        With<Player>,
    >,
    dmg_source: Query<
        (&Transform, &Attack, Option<&MobIsAttacking>),
        (Without<Player>, Without<PlayerAttackCollider>),
    >,
    rapier_context: Res<RapierContext>,
    mut hit_event: EventWriter<HitEvent>,
    mut dodge_event: EventWriter<DodgeEvent>,
    in_i_frame: Query<&InvincibilityTimer>,
    mut parry_events: EventWriter<ParrySuccessEvent>,
    mut ranged_attack_event: EventWriter<RangedAttackEvent>,
    mut lifesteal_events: EventWriter<LifestealEvent>,
    mut trigger_counts: ResMut<crate::player::skills::HeirloomTriggerCounts>,
) {
    let (
        player_e,
        player_txfm,
        thorns,
        defence,
        dodge,
        i_frames,
        mut parry_option,
        stealth_opt,
        lunge_opt,
        player_attack,
        owned_blessings,
        player_skills,
        mut thorns_tracker_opt,
    ) = player.single_mut();
    let mut hit_this_frame = false;
    for (e1, e2, _) in rapier_context.intersections_with(player_e) {
        for (e1, e2) in [(e1, e2), (e2, e1)] {
            if hit_this_frame {
                continue;
            }
            //if the player is colliding with an entity...
            if e1 != player_e {
                continue;
            };

            if !dmg_source.contains(e2) {
                continue;
            }
            let (mob_txfm, attack, is_attacking) = dmg_source.get(e2).unwrap();

            // mobs can only hit player during their attack animations
            if is_attacking.is_none() {
                continue;
            }

            let delta = player_txfm.translation - mob_txfm.translation;
            hit_this_frame = true;

            // Ignore hits when stealthed
            if stealth_opt.is_some() {
                continue;
            }
            // Ignore hits when lunging (lunge duration is active)
            // Check if lunge has started (percent > 0) but not finished (percent < 1.0)
            if let Some(lunge_state) = lunge_opt {
                let lunge_percent = lunge_state.lunge_duration.percent();
                if lunge_percent > 0.0 && lunge_percent < 1.0 {
                    continue;
                }
            }
            let mut rng = rand::thread_rng();
            if rng.gen_ratio(dodge.0.try_into().unwrap_or(0), 100) && !in_i_frame.contains(e1) {
                dodge_event.send(DodgeEvent { entity: e1 });
                commands
                    .entity(e1)
                    .insert(InvincibilityTimer(Timer::from_seconds(
                        i_frames.0,
                        TimerMode::Once,
                    )));
                continue;
            }
            let mut hit_successful = true;
            if let Some(ref mut parry) = parry_option {
                if parry.active && !parry.success {
                    parry_events.send(ParrySuccessEvent(e2));
                    hit_successful = false;
                    parry.success = true;

                    commands
                        .entity(e1)
                        .insert(InvincibilityTimer(Timer::from_seconds(
                            i_frames.0,
                            TimerMode::Once,
                        )))
                        .insert(PlayerAnimation::ParryHit);

                    commands.entity(e2).insert(Parried {
                        timer: Timer::from_seconds(0.75, TimerMode::Once),
                        kb_applied: false,
                    });
                }
            }
            if hit_successful {
                hit_event.send(HitEvent {
                    hit_by_pet: None,
                    hit_entity: e1,
                    damage: f32::round(attack.0 as f32 * (0.997_f32.powi(defence.0))) as i32,
                    dir: delta.normalize_or_zero().truncate(),
                    hit_with_melee: None,
                    hit_with_projectile: None,
                    ignore_tool: false,
                    hit_by_mob: Some(is_attacking.unwrap().0.clone()),
                    was_crit: false,
                    was_overcrit: false,
                    from_heirloom_effect: None,
                });
            }
            // hit back to attacker if we have Thorns
            // Thorns deals a percentage of PLAYER's damage back to the attacker
            // e.g., 100 thorns = 100% of player damage reflected
            if thorns.0 > 0 && in_i_frame.get(e1).is_err() {
                let thorns_damage =
                    f32::ceil(player_attack.0 as f32 * thorns.0 as f32 / 100.) as i32;

                // ThornsLifesteal: thorns damage has +25% chance to lifesteal per stack
                let thorns_lifesteal_stacks = player_skills.get_count(Heirloom::ThornsLifesteal);
                if thorns_lifesteal_stacks > 0 {
                    trigger_counts.increment(Heirloom::ThornsLifesteal);
                    lifesteal_events.send(LifestealEvent {
                        thorns_lifesteal_stacks,
                        is_direct_player_damage: false,
                    });
                }

                hit_event.send(HitEvent {
                    hit_by_pet: None,
                    hit_entity: e2,
                    damage: thorns_damage,
                    dir: delta.normalize_or_zero().truncate(),
                    hit_with_melee: None,
                    hit_with_projectile: None,
                    ignore_tool: false,
                    hit_by_mob: None,
                    was_crit: false,
                    was_overcrit: false,
                    from_heirloom_effect: None,
                });
            }

            // ThornsSpikes heirloom: spawn 2 spikes per stack in a circle around the player
            let thorns_spikes_stacks = player_skills.get_count(Heirloom::ThornsSpikes);

            if thorns_spikes_stacks > 0 && in_i_frame.get(e1).is_err() {
                trigger_counts.increment(Heirloom::ThornsSpikes);
                let spike_damage =
                    f32::ceil(player_attack.0 as f32 * thorns.0 as f32 / 100.) as i32;
                let num_spikes = thorns_spikes_stacks * 2;

                let mut rng = rand::thread_rng();
                for i in 0..num_spikes {
                    let base_angle = (i as f32 / num_spikes as f32) * std::f32::consts::TAU;
                    let angle_offset =
                        rng.gen_range(-std::f32::consts::PI / 6.0..std::f32::consts::PI / 6.0);
                    let angle = base_angle + angle_offset;
                    let direction = Vec2::new(angle.cos(), angle.sin());

                    ranged_attack_event.send(RangedAttackEvent {
                        projectile: Projectile::ThornsProjectile,
                        direction,
                        from_enemy: false,
                        is_followup_proj: false,
                        mana_cost: None,
                        from_entity: Some(player_e),
                        dmg_override: Some(spike_damage),
                        pos_override: Some(direction * 10.0),
                        spawn_delay: 0.0,
                    });
                }
            }
        }
    }
}

fn check_boss_to_objects_collisions(
    objs: Query<(Entity, &Transform, &WorldObject), (With<WorldObject>, Without<ItemStack>)>,
    dmg_source: Query<
        (Entity, &Transform, &Attack, Option<&MobIsAttacking>),
        With<DamagesWorldObjects>,
    >,
    rapier_context: Res<RapierContext>,
    mut hit_event: EventWriter<HitEvent>,
) {
    for (world_destroyer, world_destroyer_txfm, attack, is_attacking) in dmg_source.iter() {
        let mut hit_this_frame = vec![];
        'inner: for (e1, e2, _) in rapier_context.intersections_with(world_destroyer) {
            for (e1, e2) in [(e1, e2), (e2, e1)] {
                let target = if e1 == world_destroyer { e2 } else { e1 };
                if hit_this_frame.contains(&target) {
                    continue 'inner;
                }

                //if the enemy is colliding with an obj...
                let Ok((obj_e, obj_txfm, _obj)) = objs.get(target) else {
                    continue 'inner;
                };

                // mobs can only hit objs during their attack animations
                if is_attacking.is_none() {
                    continue 'inner;
                }
                hit_this_frame.push(target);

                let delta = obj_txfm.translation - world_destroyer_txfm.translation;

                hit_event.send(HitEvent {
                    hit_by_pet: None,
                    hit_entity: obj_e,
                    damage: f32::round(attack.0 as f32) as i32,
                    dir: delta.normalize_or_zero().truncate(),
                    hit_with_melee: Some(WorldObject::WoodAxe),
                    hit_with_projectile: None,
                    ignore_tool: true,
                    hit_by_mob: Some(is_attacking.unwrap().0.clone()),
                    was_crit: false,
                    was_overcrit: false,
                    from_heirloom_effect: None,
                });
            }
        }
    }
}
