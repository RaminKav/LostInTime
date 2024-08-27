use std::{
    fs::{self, create_dir_all, File},
    process::exit,
};

use bevy::{prelude::*, render::view::RenderLayers, sprite::Anchor};

use crate::{
    ai::pathfinding::PathfindingCache,
    assets::{asset_helpers::spawn_sprite, Graphics},
    audio::UpdateBGMTrackEvent,
    client::analytics::{connect_server, AnalyticsData},
    colors::{overwrite_alpha, BLACK, WHITE, YELLOW_2},
    container::ContainerRegistry,
    datafiles,
    item::CraftingTracker,
    night::NightTracker,
    player::skills::{PlayerSkills, SkillChoiceQueue},
    ui::{ChestContainer, FurnaceContainer},
    world::{
        dimension::{ActiveDimension, EraManager, GenerationSeed},
        generation::WorldObjectCache,
    },
    DoNotDespawnOnGameOver, Game, GameState, ScreenResolution, DEBUG, GAME_HEIGHT, ZOOM_SCALE,
};

use super::{scrapper_ui::ScrapperEvent, Interactable, UIElement, UIState, OPTIONS_UI_SIZE};

#[derive(Component, Clone, Eq, PartialEq)]
pub enum MenuButton {
    Start,
    Options,
    Quit,
    InfoOK,
    GameOverOK,
    Scrapper,
}
#[derive(Component)]
pub struct InfoModal;

pub struct MenuButtonClickEvent {
    pub button: MenuButton,
}

#[derive(Component)]
pub struct MainMenu;

#[derive(Component)]
pub struct GameStartFadein(Timer);

pub fn display_main_menu(
    mut commands: Commands,
    graphics: Res<Graphics>,
    mut bgm_track_event: EventWriter<UpdateBGMTrackEvent>,
    res: Res<ScreenResolution>,
) {
    let mut menu = commands.spawn(SpriteBundle {
        texture: graphics.get_ui_element_texture(UIElement::MainMenu),

        transform: Transform {
            translation: Vec3::new(0., 0., 0.),
            scale: Vec3::new(1., 1., 1.),
            ..Default::default()
        },
        sprite: Sprite {
            custom_size: Some(Vec2::new(
                res.game_width / ZOOM_SCALE,
                GAME_HEIGHT / ZOOM_SCALE,
            )),
            ..Default::default()
        },
        ..Default::default()
    });
    menu.insert(UIElement::MainMenu)
        .insert(MainMenu)
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("Main Menu"));

    //start music
    bgm_track_event.send(UpdateBGMTrackEvent {
        asset_path: "sounds/bgm_day.ogg".to_owned(),
    });
}

pub fn remove_main_menu(
    mut commands: Commands,
    query: Query<Entity, With<MainMenu>>,
    menu_buttons: Query<Entity, With<MenuButton>>,
) {
    for entity in query.iter() {
        commands.entity(entity).despawn_recursive();

        for button in menu_buttons.iter() {
            commands.entity(button).despawn_recursive();
        }
    }
}

pub fn handle_menu_button_click_events(
    mut event_reader: EventReader<MenuButtonClickEvent>,
    mut next_state: ResMut<NextState<GameState>>,
    mut commands: Commands,
    info_modal: Query<Entity, With<InfoModal>>,
    everything: Query<
        Entity,
        (
            Or<(With<Visibility>, With<ActiveDimension>)>,
            Without<DoNotDespawnOnGameOver>,
        ),
    >,
    res: Res<ScreenResolution>,
    mut analytics_data: Option<ResMut<AnalyticsData>>,
    skills: Query<&PlayerSkills>,
    night_tracker: Option<Res<NightTracker>>,
    seed: Option<Res<GenerationSeed>>,
    mut scrapper_event: EventWriter<ScrapperEvent>,
) {
    for event in event_reader.iter() {
        match event.button {
            MenuButton::Start => {
                if info_modal.iter().count() != 0 {
                    continue;
                }
                info!("START GAME");
                commands
                    .spawn(SpriteBundle {
                        sprite: Sprite {
                            color: Color::rgba(0., 0., 0., 0.),
                            custom_size: Some(Vec2::new(res.game_width + 10., GAME_HEIGHT + 20.)),
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
                    .insert(GameStartFadein(Timer::from_seconds(3.0, TimerMode::Once)));

                next_state.0 = Some(GameState::Main);
                commands.init_resource::<Game>();
                commands.init_resource::<NightTracker>();
                commands.init_resource::<SkillChoiceQueue>();
                commands.init_resource::<ContainerRegistry>();
                commands.init_resource::<PathfindingCache>();
                commands.init_resource::<CraftingTracker>();
                commands.init_resource::<EraManager>();
            }
            MenuButton::Options => {
                if info_modal.iter().count() != 0 {
                    continue;
                }
                if webbrowser::open("https://discord.gg/c4Aqd6RXGm").is_ok() {
                    // ...
                }
            }
            MenuButton::Quit => {
                if info_modal.iter().count() != 0 {
                    continue;
                }
                info!("Quit button pressed, quitting!");
                exit(0);
            }
            MenuButton::InfoOK => {
                for e in info_modal.iter() {
                    commands.entity(e).despawn_recursive();
                }
            }
            MenuButton::Scrapper => {
                scrapper_event.send_default();
            }
            MenuButton::GameOverOK => {
                let analytics_data = analytics_data.as_mut().unwrap();

                //set end of game analytics data
                let night_tracker = night_tracker.as_ref().unwrap();
                analytics_data.skills = skills.iter().next().unwrap().skills.clone();
                analytics_data.timestamp = chrono::offset::Local::now().to_string();
                analytics_data.nights_survived = night_tracker.days as u32;
                let analytics_dir = datafiles::analytics_dir();

                // save analytics to file
                if let Ok(()) = create_dir_all(analytics_dir) {
                    let analytics_file = {
                        let mut file = datafiles::analytics_dir();
                        file.push(format!("analytics_{}.json", seed.as_ref().unwrap().seed));
                        file
                    };
                    let file = File::create(analytics_file)
                        .expect("Could not open file for serialization");

                    if let Err(result) = serde_json::to_writer(file, &analytics_data.clone()) {
                        error!("Failed to save game state: {result:?}");
                    } else {
                        info!("SAVED ANALYTICS!");
                    }
                }

                // send server analytics
                info!("Sending analytics data to server...");
                if !*DEBUG {
                    connect_server(analytics_data.clone());
                }
                info!("Despawning everything, Sending to main menu");
                for e in everything.iter() {
                    commands.entity(e).despawn();
                }
                let _ = fs::remove_file(datafiles::save_file());
                next_state.0 = Some(GameState::MainMenu);

                //cleanup resources with Entity refs
                commands.remove_resource::<ChestContainer>();
                commands.remove_resource::<FurnaceContainer>();
                commands.remove_resource::<AnalyticsData>();
                commands.remove_resource::<SkillChoiceQueue>();
                commands.remove_resource::<Game>();
                commands.remove_resource::<NightTracker>();
                commands.remove_resource::<ContainerRegistry>();
                commands.remove_resource::<CraftingTracker>();
                commands.remove_resource::<EraManager>();
                commands.remove_resource::<WorldObjectCache>();
            }
        }
    }
}
pub fn spawn_menu_text_buttons(mut commands: Commands, asset_server: Res<AssetServer>) {
    // MENU TEXT BUTTONS
    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                "Start",
                TextStyle {
                    font: asset_server.load("fonts/alagard.ttf"),
                    font_size: 15.0,
                    color: YELLOW_2,
                },
            ),
            // .with_alignment(TextAlignment::Right),
            transform: Transform {
                translation: Vec3::new(14., -28.5, 1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        },
        Name::new("MENU TEXT"),
        RenderLayers::from_layers(&[3]),
        Interactable::default(),
        UIElement::MenuButton,
        MenuButton::Start,
        Sprite {
            custom_size: Some(Vec2::new(38., 11.)),
            ..default()
        },
    ));
    let discord_icon = spawn_sprite(
        &mut commands,
        Vec3::new(-40., 0.5, 1.),
        asset_server.load("ui/icons/DiscordIcon.png"),
        3,
    );

    commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    "Discord",
                    TextStyle {
                        font: asset_server.load("fonts/alagard.ttf"),
                        font_size: 15.0,
                        color: YELLOW_2,
                    },
                ),
                // .with_alignment(TextAlignment::Right),
                transform: Transform {
                    translation: Vec3::new(-8., -49.5, 1.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..default()
            },
            Name::new("MENU TEXT"),
            RenderLayers::from_layers(&[3]),
            Interactable::default(),
            UIElement::MenuButton,
            MenuButton::Options,
            Sprite {
                custom_size: Some(Vec2::new(58., 11.)),
                ..default()
            },
        ))
        .add_child(discord_icon);

    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                "Quit",
                TextStyle {
                    font: asset_server.load("fonts/alagard.ttf"),
                    font_size: 15.0,
                    color: YELLOW_2,
                },
            ),
            // .with_alignment(TextAlignment::Right),
            transform: Transform {
                translation: Vec3::new(-39.5, -74., 1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        },
        Name::new("MENU TEXT"),
        RenderLayers::from_layers(&[3]),
        Interactable::default(),
        UIElement::MenuButton,
        MenuButton::Quit,
        Sprite {
            custom_size: Some(Vec2::new(30., 11.)),
            ..default()
        },
    ));
}

pub fn spawn_info_modal(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    graphics: Res<Graphics>,
) {
    // OK BUTTON
    let ok_text = commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    "OK!",
                    TextStyle {
                        font: asset_server.load("fonts/alagard.ttf"),
                        font_size: 15.0,
                        color: BLACK,
                    },
                ),
                transform: Transform {
                    translation: Vec3::new(0., -64.5, 1.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..default()
            },
            Name::new("INFO OK TEXT"),
            RenderLayers::from_layers(&[3]),
            Interactable::default(),
            UIElement::MenuButton,
            MenuButton::InfoOK,
            Sprite {
                custom_size: Some(Vec2::new(20., 11.)),
                ..default()
            },
        ))
        .id();
    // INFO TEXT
    let info_texts = commands
        .spawn((
            Text2dBundle {
                text: Text::from_sections([TextSection::new(
                    "                Note \n\n",
                    TextStyle {
                        font: asset_server.load("fonts/4x5.ttf"),
                        font_size: 10.0,
                        color: WHITE,
                    },
                ),
                TextSection::new(
                    "This is an alpha release. It is also the first time\n\nanyone's playtested this game! As such, many features\n\nmay be incomplete, unbalanced, or potentially buggy.\n\nAdditionally, some artwork and UI are placeholders.\n\n\nJoin the discord channel to follow along with the\n\nproject. If you encounter any issues or bugs, or have\n\nany feedback, please let me know.\n\n\nThank you for play testing my game.\n\nI hope you enjoy it!\n\n\n                                                                 - Fleam",
                    TextStyle {
                        font: asset_server.load("fonts/4x5.ttf"),
                        font_size: 5.0,
                        color: WHITE,
                    },
                )]),
                transform: Transform {
                    translation: Vec3::new(0., 7.5, 1.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..default()
            },
            RenderLayers::from_layers(&[3]),
        ))
        .id();

    commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::InfoModal),
            transform: Transform {
                translation: Vec3::new(0.5, 0.5, 10.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            sprite: Sprite {
                custom_size: Some(Vec2::new(251., 161.)),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(UIElement::InfoModal)
        .insert(InfoModal)
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("Info Modal"))
        .add_child(ok_text)
        .add_child(info_texts);
}

#[derive(Component)]
pub struct OptionsUI;

pub fn handle_enter_options_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    res: Res<ScreenResolution>,
) {
    let (size, texture, t_offset) = (
        OPTIONS_UI_SIZE,
        graphics.get_ui_element_texture(UIElement::Options),
        Vec2::new(0., 0.),
    );

    let overlay = commands
        .spawn(SpriteBundle {
            sprite: Sprite {
                color: Color::rgba(146. / 255., 116. / 255., 65. / 255., 0.3),
                custom_size: Some(Vec2::new(res.game_width + 10., GAME_HEIGHT + 10.)),
                ..default()
            },
            transform: Transform {
                translation: Vec3::new(-t_offset.x, -t_offset.y, -1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("overlay"))
        .id();
    let stats_e = commands
        .spawn(SpriteBundle {
            texture,
            sprite: Sprite {
                custom_size: Some(size),
                ..Default::default()
            },
            transform: Transform {
                translation: Vec3::new(t_offset.x, t_offset.y, 10.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(OptionsUI)
        .insert(Name::new("STATS UI"))
        .insert(UIState::Options)
        .insert(RenderLayers::from_layers(&[3]))
        .id();

    for i in 0..4 {
        let translation = Vec3::new(6., (-i as f32 * 21.) + 10., 1.);
        let mut slot_entity = commands.spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::StatsButton),
            transform: Transform {
                translation,
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            sprite: Sprite {
                custom_size: Some(Vec2::new(16., 16.)),
                ..Default::default()
            },
            ..Default::default()
        });
        slot_entity
            .set_parent(stats_e)
            .insert(Interactable::default())
            .insert(UIElement::StatsButton)
            .insert(RenderLayers::from_layers(&[3]))
            .insert(Name::new("STATS BUTTON"));

        let mut text = commands.spawn((
            Text2dBundle {
                text: Text::from_section(
                    "Update",
                    TextStyle {
                        font: asset_server.load("fonts/Kitchen Sink.ttf"),
                        font_size: 8.0,
                        color: BLACK,
                    },
                ),
                text_anchor: Anchor::CenterLeft,
                transform: Transform {
                    translation: Vec3::new(23., (-i as f32 * 21.) + 10., 1.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..default()
            },
            Name::new("STATS TEXT"),
            RenderLayers::from_layers(&[3]),
        ));
        text.set_parent(stats_e)
            .insert(RenderLayers::from_layers(&[3]));
    }

    // sp remaining text
    let mut sp_text = commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                "Update",
                TextStyle {
                    font: asset_server.load("fonts/Kitchen Sink.ttf"),
                    font_size: 8.0,
                    color: BLACK,
                },
            ),
            text_anchor: Anchor::CenterLeft,
            transform: Transform {
                translation: Vec3::new(29., 43., 1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        },
        Name::new("SP TEXT"),
        RenderLayers::from_layers(&[3]),
    ));
    sp_text
        .set_parent(stats_e)
        .insert(RenderLayers::from_layers(&[3]));

    commands.entity(stats_e).push_children(&[overlay]);
}

pub fn tick_game_start_overlay(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut GameStartFadein, &mut Sprite)>,
) {
    for (e, mut timer, mut sprite) in query.iter_mut() {
        timer.0.tick(time.delta());
        if timer.0.finished() {
            commands.entity(e).despawn();
        } else {
            let alpha = f32::max(0., 1. - timer.0.percent());
            sprite.color = overwrite_alpha(sprite.color, alpha);
        }
    }
}
