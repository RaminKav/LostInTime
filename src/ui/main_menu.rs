use std::process::exit;

use bevy::{prelude::*, render::view::RenderLayers, sprite::Anchor};

use crate::{
    ai::pathfinding::PathfindingCache,
    assets::Graphics,
    audio::UpdateBGMTrackEvent,
    colors::{BLACK, YELLOW_2},
    container::ContainerRegistry,
    item::CraftingTracker,
    night::NightTracker,
    world::dimension::EraManager,
    Game, GameState, GAME_HEIGHT, GAME_WIDTH, ZOOM_SCALE,
};

use super::{Interactable, UIElement, UIState, OPTIONS_UI_SIZE};

#[derive(Component, Clone)]
pub enum MenuButton {
    Start,
    Options,
    Quit,
}

pub struct MenuButtonClickEvent {
    pub button: MenuButton,
}

#[derive(Component)]
pub struct MainMenu;

pub fn display_main_menu(
    mut commands: Commands,
    graphics: Res<Graphics>,
    mut bgm_track_event: EventWriter<UpdateBGMTrackEvent>,
) {
    let mut menu = commands.spawn(SpriteBundle {
        texture: graphics.get_ui_element_texture(UIElement::MainMenu),

        transform: Transform {
            translation: Vec3::new(0., 0., 0.),
            scale: Vec3::new(1., 1., 1.),
            ..Default::default()
        },
        sprite: Sprite {
            custom_size: Some(Vec2::new(GAME_WIDTH / ZOOM_SCALE, GAME_HEIGHT / ZOOM_SCALE)),
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
    mut next_ui_state: ResMut<NextState<UIState>>,
    mut commands: Commands,
) {
    for event in event_reader.iter() {
        match event.button {
            MenuButton::Start => {
                println!("START GAME");
                next_state.0 = Some(GameState::Main);
                commands.init_resource::<Game>();
                commands.init_resource::<NightTracker>();
                commands.init_resource::<ContainerRegistry>();
                commands.init_resource::<PathfindingCache>();
                commands.init_resource::<CraftingTracker>();
                commands.init_resource::<EraManager>();
            }
            MenuButton::Options => {
                // next_ui_state.0 = Some(UIState::Options);
            }
            MenuButton::Quit => {
                exit(0);
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

    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                "Options",
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
    ));

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

#[derive(Component)]
pub struct OptionsUI;

pub fn handle_enter_options_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
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
                custom_size: Some(Vec2::new(GAME_WIDTH + 10., GAME_HEIGHT + 10.)),
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
