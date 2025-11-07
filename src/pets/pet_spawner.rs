use crate::item::item_actions::ItemActionParam;
use crate::pets::state::{Pet, PetSpawner, PetState};
use crate::player::achievements::{Achievement, AchievementUnlockedEvent};
use crate::player::Player;
use crate::ui::damage_numbers::spawn_floating_text_with_shadow;
use crate::ui::key_input_guide::InteractionGuideTrigger;
use crate::world::y_sort::YSort;
use crate::GameParam;
use bevy::prelude::*;
use bevy_rapier2d::prelude::Collider;

pub fn handle_pet_spawner_interaction(
    mut commands: Commands,
    pet_spawners: Query<
        (
            Entity,
            &GlobalTransform,
            &PetSpawner,
            &InteractionGuideTrigger,
        ),
        (With<PetSpawner>, Without<Pet>),
    >,
    player_query: Query<&GlobalTransform, With<Player>>,
    game: GameParam,
    mut item_action_param: ItemActionParam,
    key_input: Res<Input<KeyCode>>,
    asset_server: Res<AssetServer>,
) {
    if !key_input.just_pressed(KeyCode::F) {
        return;
    }

    let player_t = match player_query.get_single() {
        Ok(t) => t,
        Err(_) => return,
    };

    for (spawner_e, spawner_t, spawner, trigger) in pet_spawners.iter() {
        let distance = spawner_t
            .translation()
            .truncate()
            .distance(player_t.translation().truncate());
        if distance <= trigger.activation_distance {
            let pet_type = &spawner.pet_type;

            // Check if achievement is already unlocked
            if let Some(achievements) = item_action_param.achievements.as_ref() {
                let achievement = match pet_type {
                    Pet::Slime => Achievement::SlimePet,
                    Pet::Fairy => Achievement::FairyPet,
                };
                if achievements.has(achievement) {
                    continue; // Already unlocked, skip
                }
            }

            // Unlock the achievement
            let achievement = match pet_type {
                Pet::Slime => Achievement::SlimePet,
                Pet::Fairy => Achievement::FairyPet,
            };

            if let Some(achievements) = item_action_param.achievements.as_mut() {
                if achievements.unlock(achievement.clone()) {
                    // Achievement was newly unlocked, send event
                    item_action_param
                        .achievement_events
                        .send(AchievementUnlockedEvent {
                            achievement: achievement.clone(),
                            reward_currency: achievement.reward_currency(),
                        });
                }
            }

            // Add pet to PlayerClass for current run
            if let Some(player_class) = item_action_param.player_class.as_mut() {
                if !player_class.pets.contains(pet_type) {
                    player_class.pets.push(pet_type.clone());

                    // Spawn the pet entity near player
                    let player_pos = game.player().position;
                    commands.spawn((
                        pet_type.clone(),
                        PetState::default(),
                        YSort(0.001),
                        Collider::capsule(Vec2::new(0., -6.), Vec2::new(0., -6.), 5.0),
                        Transform::from_xyz(player_pos.x + 40.0, player_pos.y - 40.0, 1.0),
                        Name::new(format!("{:?} Pet", pet_type)),
                    ));

                    // Show feedback text at spawner location
                    let spawner_pos = spawner_t.translation().truncate();
                    spawn_floating_text_with_shadow(
                        &mut commands,
                        &asset_server,
                        spawner_pos.extend(player_pos.z) + Vec3::new(0., 20., 0.),
                        crate::colors::DMG_NUM_GREEN,
                        format!("{:?} Pet Found!", pet_type),
                    );
                }
            }

            // Despawn the spawner object after interaction
            commands.entity(spawner_e).despawn_recursive();
        }
    }
}
