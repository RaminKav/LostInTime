use crate::{
    attributes::{Attack, AttributeModifier},
    enemy::Mob,
    inventory::{Inventory, ItemStack},
    item::{
        projectile::{Projectile, ProjectileState},
        Equipment, MainHand, WorldObject,
    },
    ui::InventoryState,
    Game, GameParam, GameState, Player,
};
use bevy::prelude::*;

use bevy_rapier2d::prelude::{CollisionEvent, RapierContext};

use super::{HitEvent, HitMarker};
pub struct CollisionPlugion;

impl Plugin for CollisionPlugion {
    fn build(&self, app: &mut App) {
        app.add_systems(
            (
                Self::check_melee_hit_collisions,
                Self::check_mob_to_player_collisions,
                Self::check_projectile_hit_collisions,
                Self::check_item_drop_collisions,
            )
                .in_set(OnUpdate(GameState::Main)),
        );
    }
}

impl CollisionPlugion {
    fn check_melee_hit_collisions(
        mut commands: Commands,
        context: ResMut<RapierContext>,
        weapons: Query<(Entity, &Parent, &WorldObject), (Without<HitMarker>, With<MainHand>)>,
        parent_attack: Query<&Attack>,
        mut hit_event: EventWriter<HitEvent>,
        game: Res<Game>,
        inv_state: Query<&InventoryState>,
        mut inv: Query<&mut Inventory>,
        world_obj: Query<Entity, (With<WorldObject>, Without<MainHand>)>,
    ) {
        if let Ok(weapon) = weapons.get_single() {
            let weapon_parent = weapon.1;
            if let Some(hit) = context.intersection_pairs().find(|c| {
                (c.0 == weapon.0 && c.1 != weapon_parent.get())
                    || (c.1 == weapon.0 && c.0 != weapon_parent.get())
            }) {
                let hit_entity = if hit.0 == weapon.0 { hit.1 } else { hit.0 };
                if !game.player_state.is_attacking || world_obj.get(hit_entity).is_ok() {
                    return;
                }
                if let Some(Some(wep)) = inv
                    .single()
                    .clone()
                    .items
                    .get(inv_state.single().active_hotbar_slot)
                {
                    wep.modify_attributes(
                        AttributeModifier {
                            modifier: "durability".to_owned(),
                            delta: -1,
                        },
                        &mut inv,
                    );
                }
                commands.entity(weapon.0).insert(HitMarker);
                hit_event.send(HitEvent {
                    hit_entity,
                    damage: parent_attack.get(**weapon_parent).unwrap().0,
                    dir: Vec2::new(0., 0.),
                    hit_with: Some(*weapon.2),
                });
            }
        }
    }
    fn check_projectile_hit_collisions(
        mut commands: Commands,
        player_attack: Query<(Entity, &Attack, &Children), With<Player>>,
        allowed_targets: Query<
            Entity,
            (Without<ItemStack>, Without<MainHand>, Without<Projectile>),
        >,
        mut hit_event: EventWriter<HitEvent>,
        mut collisions: EventReader<CollisionEvent>,
        projectiles: Query<(Entity, &ProjectileState), With<Projectile>>,
    ) {
        for evt in collisions.iter() {
            let CollisionEvent::Started(e1, e2, _) = evt else { continue };
            for (e1, e2) in [(e1, e2), (e2, e1)] {
                let Ok((e, state)) = projectiles.get(*e1) else {continue};
                let Ok((player_e, Attack(damage), children)) = player_attack.get_single() else {continue};
                if player_e == *e2 || children.contains(e2) || !allowed_targets.contains(*e2) {
                    continue;
                }
                hit_event.send(HitEvent {
                    hit_entity: *e2,
                    damage: *damage,
                    dir: state.direction,
                    hit_with: None,
                });
                commands.entity(e).despawn()
            }
        }
    }
    fn check_item_drop_collisions(
        mut commands: Commands,
        player: Query<Entity, With<Player>>,
        allowed_targets: Query<Entity, (With<ItemStack>, Without<MainHand>, Without<Equipment>)>,
        rapier_context: Res<RapierContext>,

        mut game: GameParam,
        mut inv: Query<&mut Inventory>,
    ) {
        let player_e = player.single();
        for (e1, e2, _) in rapier_context.intersections_with(player_e) {
            for (e1, e2) in [(e1, e2), (e2, e1)] {
                //if the player is colliding with an entity...
                let Ok(_) = player.get(e1) else {continue};
                if !allowed_targets.contains(e2) {
                    continue;
                }
                // ...and the entity is an item stack, add it to the player's inventory

                let item_stack = game.items_query.get(e2).unwrap().2.clone();
                item_stack.add_to_inventory(&mut inv, &mut game.inv_slot_query);

                game.world_obj_data.drop_entities.remove(&e2);
                commands.entity(e2).despawn();
            }
        }
    }
    fn check_mob_to_player_collisions(
        player: Query<(Entity, &Transform), With<Player>>,
        mobs: Query<(&Transform, &Attack), (With<Mob>, Without<Player>)>,
        rapier_context: Res<RapierContext>,
        mut hit_event: EventWriter<HitEvent>,
    ) {
        let (player_e, player_txfm) = player.single();
        for (e1, e2, _) in rapier_context.intersections_with(player_e) {
            for (e1, e2) in [(e1, e2), (e2, e1)] {
                //if the player is colliding with an entity...
                let Ok(_) = player.get(e1) else {continue};
                if !mobs.contains(e2) {
                    continue;
                }
                let (mob_txfm, attack) = mobs.get(e2).unwrap();
                let delta = player_txfm.translation - mob_txfm.translation;
                hit_event.send(HitEvent {
                    hit_entity: e1,
                    damage: attack.0,
                    dir: delta.normalize_or_zero().truncate(),
                    hit_with: None,
                });
            }
        }
    }
}
