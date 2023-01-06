use bevy::sprite::collide_aabb::Collision;
use bevy::{prelude::*, sprite::collide_aabb::collide};
use bevy_rapier2d::prelude::Collider;

use crate::{item::Size, GameState, Player};

#[derive(Debug, PartialEq)]
pub struct CollisionEvent(pub Collision);

pub struct CollisionPlugin;

impl Plugin for CollisionPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<CollisionEvent>().add_system_set(
            SystemSet::on_update(GameState::Main)
                // .with_run_criteria(FixedTimestep::step(TIME_STEP as f64))
                .with_system(Self::check_for_collisions),
        );
    }
}

impl CollisionPlugin {
    pub fn check_for_collisions(
        mut player_query: Query<&Transform, With<Player>>,
        collider_query: Query<(&Transform, &Size), With<Collider>>,
        mut collision_events: EventWriter<CollisionEvent>,
    ) {
        // let player_transform = player_query.single_mut();
        // let player_size = player_transform.scale.truncate() + Vec2::new(16., 8.);
        // let player_translation = player_transform.translation + Vec3::new(16., 16., 0.);
        // // wall = 32 x 48
        // // size = + (x/2, (y-32)/2)

        // // check collision with walls
        // for (transform, s) in &collider_query {
        //     let collision = collide(
        //         player_translation,
        //         player_size,
        //         transform.translation + Vec3::new(s.0.x / 2., s.0.y / 2., 0.),
        //         transform.scale.truncate() + s.0 - Vec2::new(1., 1.),
        //     );
        //     println!(
        //         "Collision Details: {:?} {:?} {:?} {:?}",
        //         player_translation,
        //         player_size,
        //         transform.translation + Vec3::new(s.0.x / 2., s.0.y / 2., 0.),
        //         transform.scale.truncate() + s.0 - Vec2::new(1., 1.)
        //     );
        //     if let Some(collision) = collision {
        //         // Sends a collision event so that other systems can react to the collision
        //         // println!("COLLISION!! Sending Event");
        //         println!("TYPE: {:?}", collision);
        //         collision_events.send(CollisionEvent(collision));
        //     }
        // }
    }
}
