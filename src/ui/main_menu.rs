use std::process::exit;

use bevy::{
    diagnostic::{Diagnostics, FrameTimeDiagnosticsPlugin},
    prelude::*,
    render::view::RenderLayers,
};

use crate::{
    assets::Graphics, colors::YELLOW_2, player::Player, GameState, GAME_HEIGHT, GAME_WIDTH,
};

use super::{Interactable, UIElement};

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

pub fn display_main_menu(mut commands: Commands, graphics: Res<Graphics>) {
    let mut menu = commands.spawn(SpriteBundle {
        texture: graphics
            .ui_image_handles
            .as_ref()
            .unwrap()
            .get(&UIElement::MainMenu)
            .unwrap()
            .clone(),
        transform: Transform {
            translation: Vec3::new(0., 0., 0.),
            scale: Vec3::new(1., 1., 1.),
            ..Default::default()
        },
        sprite: Sprite {
            custom_size: Some(Vec2::new(GAME_WIDTH, GAME_HEIGHT)),
            ..Default::default()
        },
        ..Default::default()
    });
    menu
        // .insert(RenderLayers::from_layers(&[1]))
        .insert(UIElement::MainMenu)
        .insert(MainMenu)
        .insert(Name::new("Main Menu"));
}

pub fn remove_main_menu(
    mut commands: Commands,
    query: Query<Entity, With<MainMenu>>,
    player: Query<Entity, With<Player>>,
    menu_buttons: Query<Entity, With<MenuButton>>,
) {
    for entity in query.iter() {
        println!("REMOVING MAIN MENU");
        commands.entity(entity).despawn_recursive();

        for button in menu_buttons.iter() {
            commands.entity(button).despawn_recursive();
        }

        commands.entity(player.single()).insert(Visibility::Visible);
    }
}

pub fn handle_menu_button_click_events(
    mut event_reader: EventReader<MenuButtonClickEvent>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    for event in event_reader.iter() {
        match event.button {
            MenuButton::Start => {
                println!("START GAME");
                next_state.0 = Some(GameState::Main);
            }
            MenuButton::Options => {
                println!("OPTIONS");
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
                translation: Vec3::new(6., -36., 1.),
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
                translation: Vec3::new(-16., -53.5, 1.),
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
                translation: Vec3::new(-47., -75., 1.),
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
