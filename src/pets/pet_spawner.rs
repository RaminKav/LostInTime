use crate::item::item_actions::ItemActionParam;
use crate::pets::state::{Pet, PetSpawner, PetState};
use crate::player::achievements::{Achievement, AchievementUnlockedEvent};
use crate::player::Player;
use crate::ui::damage_numbers::spawn_floating_text_with_shadow;
use crate::ui::game_fonts::FLOATING_TEXT;
use crate::ui::key_input_guide::InteractionGuideTrigger;
use crate::ui::tips::{SeenTips, Tip, TipEvent};
use crate::world::y_sort::YSort;
use crate::{GameParam, InputMappings};
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
    pets: Query<(), With<Pet>>,
    game: GameParam,
    mut item_action_param: ItemActionParam,
    key_input: Res<ButtonInput<KeyCode>>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    keybinds: Res<InputMappings>,
    asset_server: Res<AssetServer>,
    seen_tips: Res<SeenTips>,
) {
    if !keybinds.check_interact_input(&key_input, &mouse_input) {
        return;
    }

    let player_t = match player_query.single() {
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
            let player_has_pet = pets.iter().next().is_some();

            let Some(achievement) = (match pet_type {
                Pet::Slime => Some(Achievement::SlimePet),
                Pet::Fairy => Some(Achievement::FairyPet),
                Pet::Porkipine => Some(Achievement::PorkipinePet),
                Pet::GoldenPig => Some(Achievement::GoldenPigPet),
                Pet::Goliath => None,
            }) else {
                // Goliath is unlocked by completing Act III, not via world spawner.
                continue;
            };

            // Check if achievement is already unlocked
            if let Some(achievements) = item_action_param.achievements.as_ref() {
                if achievements.has(achievement) {
                    continue; // Already unlocked, skip
                }
            }

            if let Some(achievements) = item_action_param.achievements.as_mut() {
                if achievements.unlock(achievement.clone()) {
                    // Achievement was newly unlocked, send event
                    item_action_param
                        .achievement_events
                        .write(AchievementUnlockedEvent {
                            achievement: achievement.clone(),
                            reward_currency: achievement.reward_currency(),
                        });
                }
            }

            // Add pet to PlayerClass for current run
            if let Some(player_class) = item_action_param.player_class.as_mut() {
                if !player_class.pets.contains(pet_type) {
                    player_class.pets.push(pet_type.clone());

                    if !player_has_pet {
                        commands.spawn((
                            pet_type.clone(),
                            PetState::default(),
                            YSort(0.001),
                            Collider::capsule(Vec2::new(0., -6.), Vec2::new(0., -6.), 5.0),
                            Transform::from_translation(spawner_t.translation()),
                            Name::new(format!("{:?} Pet", pet_type)),
                        ));
                    }

                    let player_pos = game.player().position;
                    let spawner_pos = spawner_t.translation().truncate();
                    spawn_floating_text_with_shadow(
                        &mut commands,
                        &asset_server,
                        spawner_pos.extend(player_pos.z) + Vec3::new(0., 20., 0.),
                        crate::colors::DMG_NUM_GREEN,
                        format!("{:?} Pet Found!", pet_type),
                        FLOATING_TEXT,
                    );

                    if !seen_tips.has_seen(&Tip::Pets) {
                        item_action_param.tip_event.write(TipEvent {
                            tip: Tip::Pets,
                            pos: Vec3::new(-184., -116., 50.),
                        });
                    }
                }
            }

            // Despawn the spawner object after interaction
            commands.entity(spawner_e).despawn();
        }
    }
}
