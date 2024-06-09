use std::process::exit;

use bevy::{prelude::*, render::view::RenderLayers};

use crate::{
    assets::Graphics, audio::UpdateBGMTrackEvent, colors::YELLOW_2, container::ContainerRegistry,
    item::CraftingTracker, night::NightTracker, world::generation::WorldObjectCache, Game,
    GameState, GAME_HEIGHT, GAME_WIDTH,
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
            custom_size: Some(Vec2::new(GAME_WIDTH, GAME_HEIGHT)),
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
) {
    for event in event_reader.iter() {
        match event.button {
            MenuButton::Start => {
                println!("START GAME");
                next_state.0 = Some(GameState::Main);
                commands.init_resource::<Game>();
                commands.init_resource::<NightTracker>();
                commands.init_resource::<ContainerRegistry>();
                commands.init_resource::<CraftingTracker>();
                commands.insert_resource(WorldObjectCache::default());
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
