use std::{fs::File, io::BufReader};

use bevy::{prelude::*, render::view::RenderLayers, sprite::Anchor};
use bevy_aseprite::{anim::AsepriteAnimation, Aseprite};
use rand::seq::IteratorRandom;

use crate::{
    assets::Graphics,
    client::{leaderboard::LastSubmittedScore, GameData, GameOverEvent},
    colors::{overwrite_alpha, GREY, WHITE, YELLOW_2},
    combat::damage_tracker::{format_damage, DamageTracker, spawn_damage_tracker_ui},
    datafiles,
    inputs::FacingDirection,
    inventory::ItemStack,
    item::WorldObject,
    player::{score::RunScore, Player, TimeFragmentCurrency},
    proto::proto_param::ProtoParam,
    ui::{
        boss_health_bar::{BossHealthBar, BossHealthBarFrame, BossNameText},
        damage_numbers::spawn_text,
        key_input_guide::InteractGuide,
        spawn_item_stack_icon, CurrencyText, Interactable, MenuButton, TimeFragmentIcon, UIElement,
        UIState,
    },
    world::y_sort::YSort,
    GameState, RawPosition, ScreenResolution, GAME_HEIGHT,
};

use super::ui_animaitons::{MoveUIAnimation, UIIconMover};

#[derive(Component)]
pub struct GameOverFadeout(Timer);

/// Helper function to format rank with ordinal suffix (1st, 2nd, 3rd, 4th, etc.)
fn format_rank(rank: i64) -> String {
    let suffix = match rank % 10 {
        1 if rank % 100 != 11 => "st",
        2 if rank % 100 != 12 => "nd",
        3 if rank % 100 != 13 => "rd",
        _ => "th",
    };
    format!("#{}{} Worldwide", rank, suffix)
}

pub fn handle_game_over_fadeout(
    mut commands: Commands,
    game_over_events: EventReader<GameOverEvent>,
    mut next_state: ResMut<NextState<GameState>>,
    mut player: Query<
        (
            Entity,
            &FacingDirection,
            &mut Transform,
            &mut TextureAtlasSprite,
            &mut AsepriteAnimation,
        ),
        With<Player>,
    >,
    asset_server: Res<AssetServer>,
    graphics: Res<Graphics>,
    mut next_ui_state: ResMut<NextState<UIState>>,
    resolution: Res<ScreenResolution>,
    run_score: Res<RunScore>,
    last_submitted: Res<LastSubmittedScore>,
    damage_tracker: Res<DamageTracker>,
    // Cleanup queries
    boss_health_bars: Query<
        Entity,
        Or<(
            With<BossHealthBar>,
            With<BossHealthBarFrame>,
            With<BossNameText>,
        )>,
    >,
    guide_hud: Query<Entity, With<InteractGuide>>,
) {
    if !game_over_events.is_empty() {
        // Clean up boss health bars and guide HUD
        for entity in boss_health_bars.iter() {
            commands.entity(entity).despawn_recursive();
        }
        for entity in guide_hud.iter() {
            commands.entity(entity).despawn_recursive();
        }
        let (player_e, dir, mut player_t, mut sprite, mut anim) = player.single_mut();
        next_ui_state.set(UIState::Closed);
        // BLACK OVERLAY
        commands
            .spawn(SpriteBundle {
                sprite: Sprite {
                    color: Color::rgba(0., 0., 0., 0.),
                    custom_size: Some(Vec2::new(resolution.game_width + 10., GAME_HEIGHT + 20.)),
                    ..default()
                },
                transform: Transform {
                    translation: Vec3::new(0., 0., 10.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .insert(Name::new("overlay"))
            .insert(GameOverFadeout(Timer::from_seconds(6.5, TimerMode::Once)));
        // GAME OVER TEXT
        commands.spawn((
            Text2dBundle {
                text: Text::from_section(
                    "Game Over",
                    TextStyle {
                        font: asset_server.load("fonts/alagard.ttf"),
                        font_size: 30.0,
                        color: WHITE.with_a(0.),
                    },
                ),
                transform: Transform {
                    translation: Vec3::new(0., 100., 21.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..default()
            },
            GameOverText,
            RenderLayers::from_layers(&[3]),
        ));

        // RANK TEXT - spawn as "Submitting..." initially, will be updated by a system
        let rank_text = if let Some(rank) = last_submitted.rank {
            format_rank(rank)
        } else {
            "Submitting to leaderboard...".to_string()
        };

        commands.spawn((
            Text2dBundle {
                text: Text::from_section(
                    rank_text,
                    TextStyle {
                        font: asset_server.load("fonts/alagard.ttf"),
                        font_size: 15.0,
                        color: WHITE.with_a(0.),
                    },
                ),
                transform: Transform {
                    translation: Vec3::new(0., 64., 21.), // Moved 30px higher
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..default()
            },
            GameOverText,
            GameOverRankText, // Special marker for updating
            RenderLayers::from_layers(&[3]),
            Name::new("Rank Text"),
        ));

        // SCORE TEXT - always show current run's score
        commands.spawn((
            Text2dBundle {
                text: Text::from_section(
                    format!("Score: {}", run_score.score),
                    TextStyle {
                        font: asset_server.load("fonts/alagard.ttf"),
                        font_size: 15.0,
                        color: WHITE.with_a(0.),
                    },
                ),
                transform: Transform {
                    translation: Vec3::new(0., 48., 21.), // Moved 30px higher
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..default()
            },
            GameOverText,
            RenderLayers::from_layers(&[3]),
            Name::new("Score Text"),
        ));

        // DAMAGE BREAKDOWN - left side list with category headers
        let panel_x = -resolution.game_width / 2. + 10.;
        let start_y = resolution.game_height / 2. - 60.;
        
        if let Some(entities) = spawn_damage_tracker_ui(
            &mut commands,
            &asset_server,
            &damage_tracker,
            Transform::from_translation(Vec3::new(panel_x + 42.0, start_y, 21.0)),
            0.0,
            84.0,
        ) {
            for e in entities {
                commands.entity(e).insert(GameOverText);
            }
        }

        // OK BUTTON - spawn like main menu buttons
        let button_entity = commands
            .spawn((
                SpriteBundle {
                    texture: graphics.get_ui_element_texture(UIElement::UnlocksButton),
                    sprite: Sprite {
                        color: Color::rgba(1.0, 1.0, 1.0, 0.0), // Start transparent
                        custom_size: Some(Vec2::new(84., 18.)),
                        ..Default::default()
                    },
                    transform: Transform::from_translation(Vec3::new(0., -99., 23.)),
                    ..Default::default()
                },
                Interactable::default(),
                UIElement::UnlocksButton,
                MenuButton::GameOverOK,
                GameOverText,
                RenderLayers::from_layers(&[3]),
                Name::new("Game Over OK Button"),
            ))
            .id();

        // Button text as child
        commands
            .spawn((
                Text2dBundle {
                    text: Text::from_section(
                        "Try Again",
                        TextStyle {
                            font: asset_server.load("fonts/alagard.ttf"),
                            font_size: 15.0,
                            color: WHITE.with_a(0.),
                        },
                    ),
                    transform: Transform::from_translation(Vec3::new(0., 0., 1.)),
                    ..default()
                },
                GameOverText,
                RenderLayers::from_layers(&[3]),
                Name::new("Game Over OK Text"),
            ))
            .set_parent(button_entity);
        next_state.0 = Some(GameState::GameOver);
        // move player to UI camera to be above the fade out overlay
        commands
            .entity(player_e)
            .remove::<YSort>()
            .remove::<RawPosition>()
            .insert(RenderLayers::from_layers(&[3]));
        player_t.translation = Vec3::new(0., 0., 100.);

        // Tint player sprite red instead of swapping to death sprite
        anim.pause();
        sprite.color = Color::rgba(1.0, 0.2, 0.2, 1.0); // Brighter red tint, fully opaque
        if dir == &FacingDirection::Left {
            sprite.flip_x = true;
        }

        // Add a marker component to ensure color persists
        commands.entity(player_e).insert(GameOverPlayerTint);
    }
}

/// Marker component for the game over player tint
#[derive(Component)]
pub struct GameOverPlayerTint;

/// System to ensure player stays red during game over
pub fn maintain_player_red_tint(
    mut player: Query<&mut TextureAtlasSprite, (With<Player>, With<GameOverPlayerTint>)>,
) {
    if let Ok(mut sprite) = player.get_single_mut() {
        // Keep the sprite red
        if sprite.color != Color::rgba(1.0, 0.2, 0.2, 1.0) {
            sprite.color = Color::rgba(1.0, 0.2, 0.2, 1.0);
        }
    }
}
#[derive(Component)]
pub struct GameOverText;

#[derive(Component)]
pub struct GameOverRankText;

#[derive(Resource)]
pub struct GameOverUITracker {
    pub game_over_text_check: bool,
    pub tip_check: bool,
    pub analytics_check: bool,
}
pub fn tick_game_over_overlay(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut GameOverFadeout, &mut Sprite), Without<GameOverText>>,
    asset_server: Res<AssetServer>,
    mut game_over_text: Query<&mut Text, With<GameOverText>>,
    mut game_over_sprites: Query<&mut Sprite, (With<GameOverText>, Without<GameOverFadeout>)>,
    mut tip_check: Local<bool>,
    graphics: Res<Graphics>,
    res: Res<ScreenResolution>,
    time_fragments: Res<TimeFragmentCurrency>,
) {
    if query.iter().count() == 0 {
        *tip_check = false;
    }
    for (_e, mut timer, mut sprite) in query.iter_mut() {
        if timer.0.percent() >= 0.25 && !*tip_check {
            *tip_check = true;
            // Try to load tips from save
            let tips = vec![
                "Enemies get tougher every night. If you take too long, they will overpower you!",
                "Stars represent the overall quality of the stat lines on equipment.",
                "If your hunger bar is empty, you will move slower and lose health over time.",
                "Press Shift while inspecting an item to view the range\n\n     of possible values for each stat line.",
                "You can drop an item by dragging it out of your inventory.",
                "Elite mobs are much tougher, but they give more exp and drop more loot.",
                "At night, enemies will spawn much faster. Be prepared!",
                "Item colors correspond to rarity:\n\n     Common (Grey), Uncommon (green), Rare (blue), Legendary (Red).",
                "Press Shift + Left Click to quickly move items\n\n     between your hotbar and inventory.",
                "Enemies drop higher level gear the higher level you are!\n\n      Higher level gear have better base stats.",
                "Rolling through Crates instantly breaks them!",
                "Increasing Chaos is dangerous, but grants more score!",
                "Enemies drop mana orbs if you have a magic item!"
              ];

            let picked_tip = tips.iter().choose(&mut rand::thread_rng()).unwrap();
            spawn_text(
                &mut commands,
                &asset_server,
                Vec3::new(0., -GAME_HEIGHT / 2. + 56.5, 21.),
                WHITE,
                format!("Tip: {}", picked_tip),
                Anchor::Center,
                1.,
                3,
            );

            // total currency counter
            let time_fragments = time_fragments.as_ref();
            let currency_this_run = time_fragments.total_collected_time_fragments_this_run as u32;
            let game_data_file_path = datafiles::game_data();
            let mut total_currency = 0;
            if let Ok(file_file) = File::open(game_data_file_path) {
                let reader = BufReader::new(file_file);

                // Read the JSON contents of the file as an instance of `GameData`.
                match serde_json::from_reader::<_, GameData>(reader) {
                    Ok(data) => total_currency = data.time_fragments,
                    Err(err) => error!(
                        "Failed to load data from game_data.json file to get currency {err:?}"
                    ),
                }
            };

            let text = spawn_text(
                &mut commands,
                &asset_server,
                Vec3::new(res.game_width / 2. - 35., GAME_HEIGHT / 2. - 43.5, 21.),
                WHITE,
                format!("{:}", total_currency - currency_this_run as u128),
                Anchor::CenterLeft,
                1.,
                3,
            );
            commands.entity(text).insert(CurrencyText);

            let stack = spawn_item_stack_icon(
                &mut commands,
                &graphics,
                &ItemStack::crate_icon_stack(WorldObject::TimeFragment),
                &asset_server,
                Vec2::new(-8., 1.),
                Vec2::new(0., 0.),
                3,
            );
            commands
                .entity(stack)
                .insert(TimeFragmentIcon)
                .set_parent(text);

            commands.spawn(GameEndTimeFragmentSpawner {
                timer: Timer::from_seconds(0.05, TimerMode::Once),
                total_spawns: currency_this_run,
                remaining_spawns: currency_this_run,
            });
        }
        timer.0.tick(time.delta());

        let alpha = f32::min(1., timer.0.percent() * 5.);
        sprite.color = overwrite_alpha(sprite.color, alpha);
        if alpha >= 0.45 {
            let text_alpha = f32::min(1., timer.0.percent() * 2.);

            // Update text alpha
            game_over_text.iter_mut().for_each(|mut s| {
                s.sections[0].style.color = overwrite_alpha(s.sections[0].style.color, text_alpha);
            });

            // Update button sprite alpha
            game_over_sprites.iter_mut().for_each(|mut s| {
                s.color = overwrite_alpha(s.color, text_alpha);
            });
        }
    }
}

#[derive(Component)]
pub struct GameEndTimeFragmentSpawner {
    pub timer: Timer,
    pub total_spawns: u32,
    pub remaining_spawns: u32,
}
pub fn handle_spawn_collected_time_fragments(
    mut commands: Commands,
    mut spawner_query: Query<(Entity, &mut GameEndTimeFragmentSpawner)>,
    time: Res<Time>,
    proto: ProtoParam,
    res: Res<ScreenResolution>,
    mut all_time_fragments: Query<(&GlobalTransform, &mut MoveUIAnimation)>,
) {
    for (e, mut spawner) in spawner_query.iter_mut() {
        spawner.timer.tick(time.delta());
        if spawner.timer.just_finished() {
            spawner.timer.reset();
            if spawner.remaining_spawns > 0 {
                let spacing = 5.;
                let total_offset =
                    f32::min(spawner.total_spawns as f32 * spacing, res.game_width * 0.8);
                let max_per_row = res.game_width * 0.8 / spacing;
                let i = spawner.total_spawns - spawner.remaining_spawns;
                let row_i = i % max_per_row as u32;
                let col_i = i / max_per_row as u32;
                let stack = proto
                    .get_item_data(WorldObject::TimeFragment)
                    .unwrap()
                    .clone()
                    .copy_with_count(0);
                commands.spawn(UIIconMover::new(
                    Vec3::new(0., 10., 21.),
                    Vec3::new(
                        -total_offset / 2. + row_i as f32 * spacing,
                        -10. + (col_i as f32 + 1.) * -8.,
                        21.,
                    ),
                    WorldObject::TimeFragment,
                    0.,
                    800.,
                    None,
                    false,
                    stack,
                    false,
                ));
                spawner.remaining_spawns -= 1;
            } else {
                for (txfm, mut mover) in all_time_fragments.iter_mut() {
                    if mover.end == txfm.translation() {
                        mover.start = txfm.translation();
                        mover.end =
                            Vec3::new(res.game_width / 2. - 30., GAME_HEIGHT / 2. - 43.5, 21.);
                        mover.startup_delay = Timer::from_seconds(1.0, TimerMode::Once);
                        mover.despawn_when_done = true;
                        mover.item_stack.count = 1;
                    }
                }
                if all_time_fragments.iter().count() == 0 {
                    commands.entity(e).despawn_recursive();
                }
            }
        }
    }
}

/// System to update the rank text on game over screen when rank becomes available
pub fn update_game_over_rank_text(
    last_submitted: Res<LastSubmittedScore>,
    mut rank_text_query: Query<&mut Text, With<GameOverRankText>>,
) {
    // Only update if the rank changed
    if !last_submitted.is_changed() {
        return;
    }

    if let Ok(mut text) = rank_text_query.get_single_mut() {
        if let Some(rank) = last_submitted.rank {
            // Update the text to show the actual rank
            text.sections[0].value = format_rank(rank);
        }
    }
}
