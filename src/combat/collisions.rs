use super::{
    try_add_slow_stacks, Burning, Frail, HitEvent, HitMarker, InvincibilityTimer, Slow,
    StatusEffectEvent,
};
use crate::client::is_not_paused;
use crate::ui::damage_numbers::FloatingTextQueue;
use crate::{
    animations::{player_sprite::PlayerAnimation, ui_animaitons::UIIconMover},
    attributes::{
        modifiers::ModifyManaEvent, Attack, Defence, Dodge, InvincibilityCooldown, Thorns,
    },
    audio::{AudioSoundEffect, SoundSpawner},
    client::analytics::{AnalyticsTrigger, AnalyticsUpdateEvent},
    enemy::{Mob, MobIsAttacking},
    inventory::{Inventory, ItemStack},
    item::{
        item_actions::ItemActionParam,
        object_actions::TouchTriggerObjectAction,
        projectile::{
            EnemyProjectile, PetProjectileMarker, Projectile, ProjectileState, RangedAttackEvent,
        },
        Equipment, MainHand, WorldObject,
    },
    player::rogue_skills::LungeState,
    player::skill_heirlooms::Stealthed,
    player::{
        mage_skills::IceExplosionDmg,
        melee_skills::{Parried, ParryState, ParrySuccessEvent, SpearAttack, SpearGravity},
        skills::Heirloom,
    },
    ui::damage_numbers::DodgeEvent,
    CustomFlush, GameParam, GameState, Player, ScreenResolution,
};
use bevy::prelude::*;
use bevy_rapier2d::prelude::{CollisionEvent, RapierContext};
use rand::Rng;

use crate::pets::state::Pet;

const MANA_ORB_RESTORE: i32 = 10;

pub struct CollisionPlugion;

impl Plugin for CollisionPlugion {
    fn build(&self, app: &mut App) {
        app.add_systems(
            (
                check_melee_hit_collisions.run_if(is_not_paused),
                check_boss_to_objects_collisions.run_if(is_not_paused),
                check_mob_to_player_collisions.run_if(is_not_paused),
                check_projectile_hit_mob_collisions.run_if(is_not_paused),
                check_projectile_hit_player_collisions.run_if(is_not_paused),
                check_object_trigger_collisions
                    .run_if(is_not_paused)
                    .after(CustomFlush),
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
            Option<&Burning>,
            Option<&mut Slow>,
            Option<&Frail>,
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
            let Ok((mob_txfm, burning_option, slow_option, frail_option)) = mobs.get(hit_entity)
            else {
                continue;
            };

            let (mut damage, was_crit, was_overcrit) = game.calculate_player_damage(
                &mut commands,
                hit_entity,
                (frail_option.map(|f| f.num_stacks).unwrap_or(0) * 5) as u32,
                None,
                0,
                None,
            );

            let is_status_effected =
                burning_option.is_some() || slow_option.is_some() || frail_option.is_some();

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
                from_heirloom_effect: false,
            });

            commands.spawn(SoundSpawner::new(AudioSoundEffect::DefaultEnemyHit, 0.4));
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
    mut status_check: Query<(Option<&Burning>, Option<&mut Slow>, Option<&Frail>)>,
    nearby_mobs: Query<(Entity, &GlobalTransform), With<Mob>>,
    game: GameParam,
    mut status_event: EventWriter<StatusEffectEvent>,
    pet_check: Query<Entity, With<PetProjectileMarker>>,
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
            let (burning, mut slow, frail) = status_check.get_mut(*e2).unwrap();
            let is_slowed = slow.is_some();
            let is_status_effected = burning.is_some() || is_slowed || frail.is_some();

            let crit_bonus = if is_slowed && game.has_skill(Heirloom::FrozenCrit) {
                10
            } else {
                0
            } + if state.mana_bar_full && game.has_skill(Heirloom::MPBarCrit) {
                10
            } else {
                0
            };
            let (mut damage, was_crit, was_overcrit) =
                game.calculate_player_damage(&mut commands, *e2, crit_bonus, None, 0, Some(att.0));
            if is_status_effected && game.has_skill(Heirloom::TeleportStatusDMG) {
                damage = f32::ceil(damage as f32 * 1.2) as u32;
            }
            let (_e, hit_txfm) = allowed_targets.get(*e2).unwrap();
            let enemy_pos = hit_txfm.translation().truncate();
            if let Some(_) = spear_att {
                for (mob_e, mob_txfm) in nearby_mobs.iter() {
                    let delta = mob_txfm.translation().truncate() - enemy_pos;
                    if delta.length() <= 70. {
                        commands.entity(mob_e).insert(SpearGravity {
                            target: enemy_pos,
                            timer: Timer::from_seconds(0.5, TimerMode::Once),
                        });
                    }
                }
            }
            if ice_aoe.is_some() {
                try_add_slow_stacks(*e2, &mut commands, &mut status_event, slow.as_deref_mut());
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
            let is_from_heirloom = matches!(
                proj,
                Projectile::IceExplosionAOE // FrozenAoE heirloom
                                            // Add other heirloom effect projectiles here if needed
            );

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
                from_heirloom_effect: is_from_heirloom,
            });
            if nearby_mobs.get(*e2).is_ok() {
                if proj.clone() == Projectile::IceShard
                    || proj.clone() == Projectile::IceExplosionAOE
                    || proj.clone() == Projectile::FireRing
                {
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::IceStaffHit, 0.4));
                } else if proj.clone() == Projectile::Electricity {
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::LightningStaffHit, 0.4));
                } else {
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::DefaultEnemyHit, 0.4));
                }
            }

            //non-animating sprites are despawned immediately
            if state.despawn_on_hit {
                commands.entity(proj_entity).despawn_recursive();
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
                    f32::round(att.0 as f32 * (0.99_f32.powi(defence.0))) as i32
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
                    from_heirloom_effect: false,
                });
            }
            if state.despawn_on_hit {
                commands.entity(proj_entity).despawn_recursive();
            }
        }
    }
}
pub fn check_item_drop_collisions(
    mut commands: Commands,
    player: Query<Entity, With<Player>>,
    allowed_targets: Query<
        Entity,
        (
            With<ItemStack>,
            Without<MainHand>,
            Without<Equipment>,
            Without<TouchTriggerObjectAction>,
        ),
    >,
    rapier_context: Res<RapierContext>,
    items_query: Query<&ItemStack>,
    mut game: GameParam,
    mut inv: Query<&mut Inventory>,
    mut analytics: EventWriter<AnalyticsUpdateEvent>,
    mut modify_mana_event: EventWriter<ModifyManaEvent>,
    mut text_timer: ResMut<FloatingTextQueue>,
    resolution: Res<ScreenResolution>,
) {
    if !game.player().is_moving && !inv.single().is_empty() {
        return;
    }
    let player_e = player.single();
    for (e1, e2, _) in rapier_context.intersections_with(player_e) {
        for (e1, e2) in [(e1, e2), (e2, e1)] {
            //if the player is colliding with an entity...
            let Ok(_) = player.get(e1) else { continue };
            if !allowed_targets.contains(e2) {
                continue;
            }
            let item_stack = items_query.get(e2).unwrap().clone();
            let obj = item_stack.obj_type;
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
                modify_mana_event.send(ModifyManaEvent(MANA_ORB_RESTORE));
                analytics.send(AnalyticsUpdateEvent {
                    update_type: AnalyticsTrigger::ItemCollected(obj),
                });
                commands.entity(e2).despawn_recursive();
                commands.spawn(SoundSpawner::new(AudioSoundEffect::ItemPickup, 0.35));
                continue;
            }
            // ...and the entity is an item stack...
            let inv_container = inv.single().items.clone();
            if inv_container.get_first_empty_slot().is_none()
                && inv_container
                    .get_slot_for_item_in_container_with_space(&item_stack, None)
                    .is_none()
            {
                return;
            }
            // ...and inventory has room, add it to the player's inventory

            item_stack.add_to_inventory(&mut inv.single_mut().items, &mut game.inv_slot_query);

            // Add item to notification queue (exclude currency items that have special handling)
            if obj != WorldObject::TimeFragment
                && obj != WorldObject::Coin
                && obj != WorldObject::ManaOrb
            {
                text_timer.add_item(obj);
            }

            commands.entity(e2).despawn_recursive();
            analytics.send(AnalyticsUpdateEvent {
                update_type: AnalyticsTrigger::ItemCollected(obj),
            });
            commands.spawn(SoundSpawner::new(AudioSoundEffect::ItemPickup, 0.35));
        }
    }
}
pub fn check_object_trigger_collisions(
    mut commands: Commands,
    player: Query<Entity, With<Player>>,
    allowed_targets: Query<
        Entity,
        (
            Without<MainHand>,
            Without<Equipment>,
            With<TouchTriggerObjectAction>,
        ),
    >,
    rapier_context: Res<RapierContext>,
    items_query: Query<&TouchTriggerObjectAction>,
    game: GameParam,
    mut item_action_param: ItemActionParam,
) {
    if !game.player().is_moving {
        return;
    }
    let player_e = player.single();
    for (e1, e2, _) in rapier_context.intersections_with(player_e) {
        for (e1, e2) in [(e1, e2), (e2, e1)] {
            //if the player is colliding with an entity...
            let Ok(_) = player.get(e1) else { continue };
            if !allowed_targets.contains(e2) {
                continue;
            }
            let action = items_query.get(e2).unwrap();
            action.run_action(e2, &mut commands, &mut item_action_param);
        }
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
                    damage: f32::round(attack.0 as f32 * (0.99_f32.powi(defence.0))) as i32,
                    dir: delta.normalize_or_zero().truncate(),
                    hit_with_melee: None,
                    hit_with_projectile: None,
                    ignore_tool: false,
                    hit_by_mob: Some(is_attacking.unwrap().0.clone()),
                    was_crit: false,
                    was_overcrit: false,
                    from_heirloom_effect: false,
                });
            }
            // hit back to attacker if we have Thorns
            // Thorns deals a percentage of PLAYER's damage back to the attacker
            // e.g., 100 thorns = 100% of player damage reflected
            if thorns.0 > 0 && in_i_frame.get(e1).is_err() {
                hit_event.send(HitEvent {
                    hit_by_pet: None,
                    hit_entity: e2,
                    damage: f32::ceil(player_attack.0 as f32 * thorns.0 as f32 / 100.) as i32,
                    dir: delta.normalize_or_zero().truncate(),
                    hit_with_melee: None,
                    hit_with_projectile: None,
                    ignore_tool: false,
                    hit_by_mob: None,
                    was_crit: false,
                    was_overcrit: false,
                    from_heirloom_effect: false,
                });
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
                    from_heirloom_effect: false,
                });
            }
        }
    }
}
