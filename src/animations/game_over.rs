use std::{fs::File, io::BufReader};

use bevy::{prelude::*, render::view::RenderLayers, sprite::Anchor};
use bevy_aseprite::{anim::AsepriteAnimation, Aseprite};
use rand::seq::IteratorRandom;

use crate::{
    assets::Graphics,
    client::{GameData, GameOverEvent},
    colors::{overwrite_alpha, WHITE},
    datafiles,
    inputs::FacingDirection,
    inventory::ItemStack,
    item::WorldObject,
    player::{Player, TimeFragmentCurrency},
    proto::proto_param::ProtoParam,
    ui::{
        damage_numbers::spawn_text, spawn_item_stack_icon, CurrencyIcon, CurrencyText,
        Interactable, MenuButton, UIElement, UIState,
    },
    world::y_sort::YSort,
    GameState, RawPosition, ScreenResolution, GAME_HEIGHT,
};

use super::{
    player_sprite::PlayerDeadAseprite,
    ui_animaitons::{MoveUIAnimation, UIIconMover},
};

#[derive(Component)]
pub struct GameOverFadeout(Timer);

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
    mut next_ui_state: ResMut<NextState<UIState>>,
    resolution: Res<ScreenResolution>,
) {
    if !game_over_events.is_empty() {
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
                    translation: Vec3::new(0., 80., 21.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..default()
            },
            GameOverText,
            RenderLayers::from_layers(&[3]),
        ));
        // OK BUTTON
        commands.spawn((
            Text2dBundle {
                text: Text::from_section(
                    "Try Again",
                    TextStyle {
                        font: asset_server.load("fonts/alagard.ttf"),
                        font_size: 15.0,
                        color: WHITE.with_a(0.),
                    },
                ),
                transform: Transform {
                    translation: Vec3::new(0., -100.5, 23.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..default()
            },
            Name::new("INFO OK TEXT"),
            RenderLayers::from_layers(&[3]),
            Interactable::default(),
            UIElement::MenuButton,
            GameOverText,
            MenuButton::GameOverOK,
            Sprite {
                custom_size: Some(Vec2::new(70., 13.)),
                ..default()
            },
        ));
        next_state.0 = Some(GameState::GameOver);
        // move player to UI camera to be above the fade out overlay
        commands
            .entity(player_e)
            .remove::<YSort>()
            .remove::<RawPosition>()
            .insert(RenderLayers::from_layers(&[3]));
        player_t.translation = Vec3::new(0., 0., 100.);

        // set player to death sprite
        anim.pause();
        commands
            .entity(player_e)
            .remove::<TextureAtlasSprite>()
            .insert(asset_server.load::<Aseprite, _>(PlayerDeadAseprite::PATH));
        if dir == &FacingDirection::Left {
            sprite.flip_x = true;
        }
    }
}
#[derive(Component)]
pub struct GameOverText;

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
    mut tip_check: Local<bool>,
    graphics: Res<Graphics>,
    res: Res<ScreenResolution>,
    time_fragments: Query<&TimeFragmentCurrency>,
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
                "The forest is dense. Craft an Axe as soon as you can.",
                "Elite mobs are much tougher, but they give more exp and drop more loot.",
                "At night, enemies will hunt you down. Be prepared!",
                "Build a Crafting table as soon as possible to unlock important recipes.",
                "Item colors correspond to rarity:\n\n     Common (Grey), Uncommon (green), Rare (blue), Legendary (Red).",
                "Press Shift + Left Click to quickly move items\n\n     between your hotbar, inventory, and chests.",
                "Exploring the dense forest can be dangerous, especially at night!\n\n     You have more room to fight in clearings.",
                "Enemies drop higher level gear the higher level you are!\n\n      Higher level gear have better base stats.",
                "Press ESC to manually save your game! Otherwise it will save every 10.",
                "Inventory, Crafting, and Chest menus pause the game."
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
            let time_fragments = time_fragments.single();
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
            commands.entity(stack).insert(CurrencyIcon).set_parent(text);

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
            // update text alpha
            game_over_text.iter_mut().for_each(|mut s| {
                s.sections[0].style.color = overwrite_alpha(
                    s.sections[0].style.color,
                    f32::min(1., timer.0.percent() * 2.),
                );
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
