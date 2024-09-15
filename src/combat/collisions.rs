use crate::{
    animations::player_sprite::PlayerAnimation,
    attributes::{
        modifiers::ModifyHealthEvent, Attack, Defence, Dodge, InvincibilityCooldown, Lifesteal,
        Thorns,
    },
    client::analytics::{AnalyticsTrigger, AnalyticsUpdateEvent},
    enemy::{Mob, MobIsAttacking},
    inventory::{Inventory, ItemStack},
    item::{
        projectile::{EnemyProjectile, Projectile, ProjectileState, RangedAttackEvent},
        Equipment, MainHand, WorldObject,
    },
    player::{
        melee_skills::{
            Parried, ParryState, ParrySuccessEvent, SecondHitDelay, SpearAttack, SpearGravity,
        },
        skills::{PlayerSkills, Skill},
        sprint::SprintState,
        teleport::TeleportShockDmg,
        ModifyTimeFragmentsEvent,
    },
    ui::damage_numbers::DodgeEvent,
    CustomFlush, GameParam, GameState, Player,
};
use bevy::prelude::*;
use bevy_rapier2d::prelude::{CollisionEvent, RapierContext};
use rand::Rng;

use super::{Burning, Frail, HitEvent, HitMarker, InvincibilityTimer, Slow};
pub struct CollisionPlugion;

impl Plugin for CollisionPlugion {
    fn build(&self, app: &mut App) {
        app.add_systems(
            (
                check_melee_hit_collisions,
                check_boss_to_objects_collisions,
                check_mob_to_player_collisions,
                check_projectile_hit_mob_collisions,
                check_projectile_hit_player_collisions,
                check_item_drop_collisions.after(CustomFlush),
            )
                .in_set(OnUpdate(GameState::Main)),
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
    lifesteal: Query<&Lifesteal>,
    skills: Query<(&PlayerSkills, Option<&SprintState>)>,
    mut modify_health_events: EventWriter<ModifyHealthEvent>,
    mobs: Query<(&GlobalTransform, Option<&Frail>), With<Mob>>,
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
            let Ok((mob_txfm, frail_option)) = mobs.get(hit_entity) else {
                continue;
            };
            let (skills, maybe_sprint) = skills.single();
            let sword_skill_bonus = if skills.has(Skill::SwordDMG) && weapon_obj.is_sword() {
                3
            } else {
                0
            };
            let (damage, was_crit) = game.calculate_player_damage(
                &mut commands,
                hit_entity,
                (frail_option.map(|f| f.num_stacks).unwrap_or(0) * 5) as u32,
                if skills.has(Skill::SprintLungeDamage)
                    && maybe_sprint.unwrap().lunge_duration.percent() != 0.
                {
                    Some(1.25)
                } else {
                    None
                },
                sword_skill_bonus,
                None,
            );
            let delta = weapon_t.translation() - mob_txfm.translation();
            if let Ok(lifesteal) = lifesteal.get(game.game.player) {
                modify_health_events.send(ModifyHealthEvent(f32::floor(
                    damage as f32 * lifesteal.0 as f32 / 100.,
                ) as i32));
            }
            if skills.has(Skill::SplitDamage) {
                let split_damage = f32::floor(damage as f32 / 2.) as i32;

                if let Ok(lifesteal) = lifesteal.get(game.game.player) {
                    modify_health_events.send(ModifyHealthEvent(f32::floor(
                        split_damage as f32 * lifesteal.0 as f32 / 100.,
                    ) as i32));
                }
                hit_event.send(HitEvent {
                    hit_entity,
                    damage: split_damage,
                    dir: delta.normalize_or_zero().truncate() * -1.,
                    hit_with_melee: Some(*weapon_obj),
                    hit_with_projectile: None,
                    was_crit,
                    hit_by_mob: None,
                    ignore_tool: false,
                });
                commands.entity(hit_entity).insert(SecondHitDelay {
                    delay: Timer::from_seconds(0.15, TimerMode::Once),
                    dir: delta.normalize_or_zero().truncate() * -1.,
                    weapon_obj: *weapon_obj,
                });
            } else {
                hit_event.send(HitEvent {
                    hit_entity,
                    damage: damage as i32,
                    dir: delta.normalize_or_zero().truncate() * -1.,
                    hit_with_melee: Some(*weapon_obj),
                    hit_with_projectile: None,
                    was_crit,
                    hit_by_mob: None,
                    ignore_tool: false,
                });
            }
        }
    }
}
fn check_projectile_hit_mob_collisions(
    mut commands: Commands,
    player_attack: Query<(Entity, &Children, Option<&Lifesteal>), With<Player>>,
    allowed_targets: Query<
        (Entity, &GlobalTransform),
        (Without<ItemStack>, Without<MainHand>, Without<Projectile>),
    >,
    mut hit_event: EventWriter<HitEvent>,
    mut collisions: EventReader<CollisionEvent>,
    mut projectiles: Query<
        (
            Entity,
            &mut ProjectileState,
            &Projectile,
            &Attack,
            Option<&TeleportShockDmg>,
            Option<&SpearAttack>,
        ),
        Without<EnemyProjectile>,
    >,
    is_world_obj: Query<&WorldObject>,
    mut children: Query<&Parent>,
    mut modify_health_events: EventWriter<ModifyHealthEvent>,
    status_check: Query<(Option<&Burning>, Option<&Slow>, Option<&Frail>)>,
    nearby_mobs: Query<(Entity, &GlobalTransform), With<Mob>>,
    game: GameParam,
) {
    for evt in collisions.iter() {
        let CollisionEvent::Started(e1, e2, _) = evt else {
            continue;
        };
        for (e1, e2) in [(e1, e2), (e2, e1)] {
            //TODO: fr gotta refasctor this...
            let (proj_entity, mut state, proj, att, tp_shock, spear_att) =
                if let Ok(parent_e) = children.get_mut(*e1) {
                    if let Ok((proj_entity, state, proj, att, tp_shock, spear_att)) =
                        projectiles.get_mut(parent_e.get())
                    {
                        //collider is on the child, proj data on the parent
                        (proj_entity, state, proj, att, tp_shock, spear_att)
                    } else if let Ok((proj_entity, state, proj, att, tp_shock, spear_att)) =
                        projectiles.get_mut(*e1)
                    {
                        //collider and proj data are on the same entity
                        (proj_entity, state, proj, att, tp_shock, spear_att)
                    } else {
                        continue;
                    }
                } else if let Ok((proj_entity, state, proj, att, tp_shock, spear_att)) =
                    projectiles.get_mut(*e1)
                {
                    //collider and proj data are on the same entity
                    (proj_entity, state, proj, att, tp_shock, spear_att)
                } else {
                    continue;
                };
            let Ok((player_e, children, lifesteal)) = player_attack.get_single() else {
                continue;
            };
            if player_e == *e2 || children.contains(e2) || !allowed_targets.contains(*e2) {
                continue;
            }
            if state.hit_entities.contains(e2) {
                continue;
            }
            state.hit_entities.push(*e2);
            let (burning, slow, frail) = status_check.get(*e2).unwrap();
            let is_slowed = slow.is_some();
            let is_status_effected = burning.is_some() || is_slowed || frail.is_some();
            let staff_skill_bonus = if game.has_skill(Skill::StaffDMG) && proj.is_staff_proj() {
                3
            } else {
                0
            };
            let crit_bonus = if is_slowed && game.has_skill(Skill::FrozenCrit) {
                10
            } else {
                0
            } + if state.mana_bar_full && game.has_skill(Skill::MPBarCrit) {
                10
            } else {
                0
            };
            let (mut damage, was_crit) = game.calculate_player_damage(
                &mut commands,
                *e2,
                crit_bonus,
                None,
                staff_skill_bonus,
                Some(att.0),
            );
            if let Some(lifesteal) = lifesteal {
                if !is_world_obj.contains(*e2) && tp_shock.is_none() {
                    modify_health_events.send(ModifyHealthEvent(f32::floor(
                        damage as f32 * lifesteal.0 as f32 / 100.,
                    ) as i32));
                }
            }

            if is_status_effected && tp_shock.is_some() && game.has_skill(Skill::TeleportStatusDMG)
            {
                damage = f32::ceil(damage as f32 * 2.) as u32;
            }
            let (_e, hit_txfm) = allowed_targets.get(*e2).unwrap();
            if let Some(_) = spear_att {
                for (mob_e, mob_txfm) in nearby_mobs.iter() {
                    let delta =
                        mob_txfm.translation().truncate() - hit_txfm.translation().truncate();
                    if delta.length() <= 70. {
                        commands.entity(mob_e).insert(SpearGravity {
                            target: hit_txfm.translation().truncate(),
                            timer: Timer::from_seconds(0.5, TimerMode::Once),
                        });
                    }
                }
            }
            hit_event.send(HitEvent {
                hit_entity: *e2,
                damage: damage as i32,
                dir: state.direction,
                hit_with_melee: None,
                hit_with_projectile: Some(proj.clone()),
                ignore_tool: false,
                hit_by_mob: None,
                was_crit,
            });
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
            Option<&PlayerSkills>,
        ),
        (
            Or<(With<Player>, With<WorldObject>)>,
            (Without<Projectile>, Without<MainHand>),
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
            let (_, mut parry_option, i_frames, p_attack, skills) =
                allowed_targets.get_mut(*e2).unwrap();
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
                    if skills.unwrap().has(Skill::ParryDeflectProj) {
                        //deflected proj
                        ranged_attack_event.send(RangedAttackEvent {
                            projectile: proj.clone(),
                            direction: -state.direction,
                            from_enemy: None,
                            is_followup_proj: false,
                            mana_cost: None,
                            dmg_override: Some(p_attack.unwrap().0),
                            pos_override: None,
                            spawn_delay: 0.,
                        })
                    }
                }
            }
            if hit_successful {
                hit_event.send(HitEvent {
                    hit_entity: *e2,
                    damage: att.0,
                    dir: state.direction,
                    hit_with_melee: None,
                    hit_with_projectile: Some(proj.clone()),
                    ignore_tool: false,
                    hit_by_mob: Some(enemy_proj.mob.clone()),
                    was_crit: false,
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
    allowed_targets: Query<Entity, (With<ItemStack>, Without<MainHand>, Without<Equipment>)>,
    rapier_context: Res<RapierContext>,
    items_query: Query<&ItemStack>,
    mut game: GameParam,
    mut inv: Query<&mut Inventory>,
    mut analytics: EventWriter<AnalyticsUpdateEvent>,
    mut currency_event: EventWriter<ModifyTimeFragmentsEvent>,
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
            let item_stack = items_query.get(e2).unwrap().clone();
            let obj = item_stack.obj_type;
            if obj == WorldObject::TimeFragment {
                currency_event.send(ModifyTimeFragmentsEvent {
                    delta: item_stack.count as i32,
                });
                commands.entity(e2).despawn_recursive();
                analytics.send(AnalyticsUpdateEvent {
                    update_type: AnalyticsTrigger::ItemCollected(obj),
                });
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

            commands.entity(e2).despawn_recursive();
            analytics.send(AnalyticsUpdateEvent {
                update_type: AnalyticsTrigger::ItemCollected(obj),
            });
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
    let (player_e, player_txfm, thorns, defence, dodge, i_frames, mut parry_option) =
        player.single_mut();
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
                    hit_entity: e1,
                    damage: f32::round(attack.0 as f32 * (0.99_f32.powi(defence.0))) as i32,
                    dir: delta.normalize_or_zero().truncate(),
                    hit_with_melee: None,
                    hit_with_projectile: None,
                    ignore_tool: false,
                    hit_by_mob: Some(is_attacking.unwrap().0.clone()),
                    was_crit: false,
                });
            }
            // hit back to attacker if we have Thorns
            if thorns.0 > 0 && in_i_frame.get(e1).is_err() {
                hit_event.send(HitEvent {
                    hit_entity: e2,
                    damage: f32::ceil(attack.0 as f32 * thorns.0 as f32 / 100.) as i32,
                    dir: delta.normalize_or_zero().truncate(),
                    hit_with_melee: None,
                    hit_with_projectile: None,
                    ignore_tool: false,
                    hit_by_mob: None,
                    was_crit: false,
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
                    hit_entity: obj_e,
                    damage: f32::round(attack.0 as f32) as i32,
                    dir: delta.normalize_or_zero().truncate(),
                    hit_with_melee: Some(WorldObject::WoodAxe),
                    hit_with_projectile: None,
                    ignore_tool: true,
                    hit_by_mob: Some(is_attacking.unwrap().0.clone()),
                    was_crit: false,
                });
            }
        }
    }
}
