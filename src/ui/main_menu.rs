use std::{
    fs::{self, create_dir_all, File},
    process::exit,
};

use bevy::ecs::system::SystemParam;
use bevy::{prelude::*, render::view::RenderLayers, sprite::Anchor};
use bevy_rapier2d::prelude::Collider;
use strum::IntoEnumIterator;

use crate::{
    ai::pathfinding::PathfindingCache,
    assets::Graphics,
    audio::UpdateBGMTrackEvent,
    chaos::ChaosTracker,
    client::analytics::{connect_server, AnalyticsData},
    colors::{overwrite_alpha, BLACK, WHITE},
    container::ContainerRegistry,
    datafiles,
    item::CraftingTracker,
    night::NightTracker,
    player::{
        achievements::{Achievement, Achievements},
        skills::{HeirloomChoiceQueue, PlayerClass, PlayerSkills},
        unlocks::{UnlockCurrency, UnlockedClasses},
    },
    ui::{
        achievements_ui::{AchievementsPagination, ACHIEVEMENTS_PER_PAGE},
        class_selection::{
            persist_class_unlock_state, ClassSelectionState, ClassUnlockConfirmState,
            ClassUnlockHoverState, PlayerSelectSlot,
        },
        ChestContainer, FurnaceContainer, UIState,
    },
    world::{
        dimension::{ActiveDimension, EraManager, GenerationSeed},
        generation::WorldObjectCache,
    },
    DoNotDespawnOnGameOver, Game, GameState, ScreenResolution, DEBUG, GAME_HEIGHT, ZOOM_SCALE,
};

use super::{scrapper_ui::ScrapperEvent, ui_helpers::spawn_ui_overlay, Interactable, UIElement};

#[derive(SystemParam)]
pub struct MenuButtonExtras<'w, 's> {
    info_modal: Query<'w, 's, Entity, With<InfoModal>>,
    world_entities: Query<
        'w,
        's,
        Entity,
        (
            Or<(With<Visibility>, With<ActiveDimension>, With<Collider>)>,
            Without<DoNotDespawnOnGameOver>,
        ),
    >,
    analytics_data: Option<ResMut<'w, AnalyticsData>>,
    skills: Query<'w, 's, &'static PlayerSkills>,
    night_tracker: Option<Res<'w, NightTracker>>,
    seed: Option<Res<'w, GenerationSeed>>,
    scrapper_event: EventWriter<'w, ScrapperEvent>,
    selection_state: ResMut<'w, ClassSelectionState>,
    confirm_state: ResMut<'w, ClassUnlockConfirmState>,
    unlock_currency: Option<ResMut<'w, UnlockCurrency>>,
    unlocked_classes: Option<ResMut<'w, UnlockedClasses>>,
    hover_state: ResMut<'w, ClassUnlockHoverState>,
    achievements: Option<Res<'w, Achievements>>,
    class_slots: Query<'w, 's, &'static mut PlayerSelectSlot>,
    pagination_state: ResMut<'w, AchievementsPagination>,
    screen_res: Res<'w, ScreenResolution>,
}

#[derive(Component, Clone, Eq, PartialEq)]
pub enum MenuButton {
    Start,
    Options,
    Achievements,
    Quit,
    InfoOK,
    GameOverOK,
    Scrapper,
    Back,
    Begin,
    ClassUnlockYes,
    ClassUnlockNo,
    AchievementsPrev,
    AchievementsNext,
}
#[derive(Component)]
pub struct InfoModal;

pub struct MenuButtonClickEvent {
    pub button: MenuButton,
}

#[derive(Component)]
pub struct MainMenu;

#[derive(Component)]
pub struct GameStartFadein(pub Timer);

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
    mut next_ui_state: ResMut<NextState<UIState>>,
    mut commands: Commands,
    mut extras: MenuButtonExtras,
) {
    for event in event_reader.iter() {
        let info_modal_open = extras.info_modal.iter().next().is_some();
        match event.button {
            MenuButton::Start => {
                if info_modal_open {
                    continue;
                }
                // Show class selection UI instead of starting game immediately
                next_ui_state.set(UIState::ClassSelection);
            }
            MenuButton::Options => {
                if info_modal_open {
                    continue;
                }
                next_ui_state.set(UIState::Options);
            }
            MenuButton::Achievements => {
                if info_modal_open {
                    continue;
                }
                next_ui_state.set(UIState::Achievements);
                extras.pagination_state.page = 0;
            }
            MenuButton::AchievementsPrev => {
                let total_achievements = Achievement::iter().count();
                let total_pages =
                    (total_achievements + ACHIEVEMENTS_PER_PAGE - 1) / ACHIEVEMENTS_PER_PAGE;
                if total_pages == 0 {
                    continue;
                }
                if extras.pagination_state.page > 0 {
                    extras.pagination_state.page -= 1;
                }
            }
            MenuButton::AchievementsNext => {
                let total_achievements = Achievement::iter().count();
                let total_pages =
                    (total_achievements + ACHIEVEMENTS_PER_PAGE - 1) / ACHIEVEMENTS_PER_PAGE;
                if total_pages == 0 {
                    continue;
                }
                if extras.pagination_state.page + 1 < total_pages {
                    extras.pagination_state.page += 1;
                }
            }
            MenuButton::Quit => {
                if info_modal_open {
                    continue;
                }
                info!("Quit button pressed, quitting!");
                exit(0);
            }
            MenuButton::InfoOK => {
                for e in extras.info_modal.iter() {
                    commands.entity(e).despawn_recursive();
                }
            }
            MenuButton::Scrapper => {
                extras.scrapper_event.send_default();
            }
            MenuButton::Back => {
                next_ui_state.set(crate::ui::UIState::Closed);
            }
            MenuButton::Begin => {
                // Confirm button clicked
                // Check if class is selected (pet is optional)
                if let Some(class) = &extras.selection_state.selected_class {
                    // Close the class selection UI and transition to loading state
                    next_ui_state.set(UIState::Closed);
                    next_state.set(crate::GameState::Initializing);
                    // Add PlayerClass component to the game
                    commands.insert_resource(PlayerClass {
                        class: class.clone(),
                        pets: extras
                            .selection_state
                            .selected_pet
                            .iter()
                            .cloned()
                            .collect(),
                    });

                    // Initialize game resources
                    commands.init_resource::<crate::Game>();
                    commands.init_resource::<NightTracker>();
                    commands.init_resource::<ChaosTracker>();
                    commands.init_resource::<HeirloomChoiceQueue>();
                    commands.init_resource::<ContainerRegistry>();
                    commands.init_resource::<PathfindingCache>();
                    commands.init_resource::<CraftingTracker>();
                    commands.init_resource::<EraManager>();

                    // Start the game with fade-in overlay
                    commands
                        .spawn(SpriteBundle {
                            sprite: Sprite {
                                color: Color::rgba(0., 0., 0., 0.),
                                custom_size: Some(Vec2::new(
                                    extras.screen_res.game_width + 10.,
                                    crate::GAME_HEIGHT + 20.,
                                )),
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
                        .insert(crate::ui::main_menu::GameStartFadein(Timer::from_seconds(
                            3.0,
                            TimerMode::Once,
                        )));

                    // Despawn the class selection UI
                    // commands.entity(e).despawn_recursive();
                } else {
                    info!("Please select a class before confirming");
                }
            }
            MenuButton::ClassUnlockNo => {
                if extras.confirm_state.active {
                    extras.confirm_state.active = false;
                    extras.confirm_state.class = None;
                    extras.confirm_state.cost = 0;
                    extras.confirm_state.anchor_position = Vec3::ZERO;
                }
            }
            MenuButton::ClassUnlockYes => {
                if extras.confirm_state.active {
                    if let Some(class) = extras.confirm_state.class.clone() {
                        let cost = extras.confirm_state.cost;
                        let mut unlocked_class = false;

                        match (
                            extras.unlock_currency.as_mut(),
                            extras.unlocked_classes.as_mut(),
                        ) {
                            (Some(currency_res), Some(unlocked_res)) => {
                                let currency = currency_res.as_mut();
                                let unlocked = unlocked_res.as_mut();

                                if currency.spend(cost) {
                                    if unlocked.insert(class.clone()) {
                                        unlocked_class = true;
                                    }
                                } else {
                                    warn!(
                                        "Attempted to unlock {:?} without enough currency (cost: {}, owned: {})",
                                        class,
                                        cost,
                                        currency.amount
                                    );
                                }
                            }
                            _ => {
                                warn!(
                                    "Unlock resources unavailable when attempting to unlock {:?}",
                                    class
                                );
                            }
                        }

                        if unlocked_class {
                            for mut slot in extras.class_slots.iter_mut() {
                                if slot.class == class {
                                    slot.is_locked = false;
                                }
                            }

                            extras.selection_state.selected_class = Some(class.clone());
                            extras.hover_state.hovered_class = None;
                            extras.hover_state.slot_position = Vec3::ZERO;

                            if let (Some(currency_res), Some(unlocked_res)) = (
                                extras.unlock_currency.as_ref(),
                                extras.unlocked_classes.as_ref(),
                            ) {
                                let currency_ref = currency_res.as_ref();
                                let unlocked_ref = unlocked_res.as_ref();
                                persist_class_unlock_state(
                                    Some(currency_ref),
                                    unlocked_ref,
                                    extras.achievements.as_ref().map(|a| a.as_ref()),
                                );
                            }
                        }
                    }
                    extras.confirm_state.active = false;
                    extras.confirm_state.class = None;
                    extras.confirm_state.cost = 0;
                    extras.confirm_state.anchor_position = Vec3::ZERO;
                }
            }
            MenuButton::GameOverOK => {
                let Some(analytics_data_res) = extras.analytics_data.as_mut() else {
                    continue;
                };
                let analytics_data = analytics_data_res.as_mut();

                //set end of game analytics data
                let night_tracker = extras.night_tracker.as_ref().unwrap();
                analytics_data.skills = extras
                    .skills
                    .iter()
                    .next()
                    .unwrap()
                    .heirlooms
                    .iter()
                    .map(|h| h.heirloom.clone())
                    .collect();
                analytics_data.timestamp = chrono::offset::Local::now().to_string();
                analytics_data.nights_survived = night_tracker.days as u32;
                let analytics_dir = datafiles::analytics_dir();

                // save analytics to file
                if let Ok(()) = create_dir_all(analytics_dir) {
                    let analytics_file = {
                        let mut file = datafiles::analytics_dir();
                        file.push(format!(
                            "analytics_{}.json",
                            extras.seed.as_ref().unwrap().seed
                        ));
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
                for e in extras.world_entities.iter() {
                    commands.entity(e).despawn();
                }
                let _ = fs::remove_file(datafiles::save_file());
                next_state.0 = Some(GameState::MainMenu);

                //cleanup resources with Entity refs
                commands.remove_resource::<ChestContainer>();
                commands.remove_resource::<FurnaceContainer>();
                commands.remove_resource::<AnalyticsData>();
                commands.remove_resource::<HeirloomChoiceQueue>();
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
pub fn spawn_menu_button(
    button_pos: Vec3,
    text_offset: Vec3,
    text: &str,
    button_type: MenuButton,
    size: Vec2,
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
) -> Entity {
    let button_e = commands
        .spawn((
            SpriteBundle {
                texture: graphics.get_ui_element_texture(UIElement::BackButton),
                sprite: Sprite {
                    custom_size: Some(size),
                    ..Default::default()
                },
                transform: Transform::from_translation(button_pos),
                ..Default::default()
            },
            Interactable::default(),
            UIElement::BackButton,
            button_type,
            RenderLayers::from_layers(&[3]),
            Name::new(format!("Menu Button: {}", text)),
        ))
        .id();

    // Button text
    commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                text,
                TextStyle {
                    font: asset_server.load("fonts/alagard.ttf"),
                    font_size: 15.0,
                    color: WHITE,
                },
            ),
            text_anchor: Anchor::CenterLeft,
            transform: Transform {
                translation: text_offset,
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .set_parent(button_e);

    button_e
}

pub fn spawn_menu_text_buttons(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    graphics: Res<Graphics>,
) {
    // Start Button
    spawn_menu_button(
        Vec3::new(42., -10.5, 1.),
        Vec3::new(-19., -1., 1.),
        "Start ",
        MenuButton::Start,
        Vec2::new(48., 22.),
        &mut commands,
        &graphics,
        &asset_server,
    );

    // Achievements Button
    spawn_menu_button(
        Vec3::new(8., -34.5, 1.),
        Vec3::new(-50., -1., 1.),
        "Achievements",
        MenuButton::Achievements,
        Vec2::new(118., 22.),
        &mut commands,
        &graphics,
        &asset_server,
    );
    // Options Button
    spawn_menu_button(
        Vec3::new(-8., -58., 1.),
        Vec3::new(-26., -1., 1.),
        "Options",
        MenuButton::Options,
        Vec2::new(68., 22.),
        &mut commands,
        &graphics,
        &asset_server,
    );

    // Quit Button
    spawn_menu_button(
        Vec3::new(-39.5, -81., 1.),
        Vec3::new(-14., -1., 1.),
        "Quit",
        MenuButton::Quit,
        Vec2::new(40., 22.),
        &mut commands,
        &graphics,
        &asset_server,
    );
}

#[derive(Component)]
pub struct OptionsUI;

pub fn _handle_enter_options_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    res: Res<ScreenResolution>,
) {
    let (size, texture, t_offset) = (
        crate::ui::OPTIONS_UI_SIZE,
        graphics.get_ui_element_texture(UIElement::Options),
        Vec2::new(0., 0.),
    );

    let overlay = spawn_ui_overlay(
        &mut commands,
        Vec2::new(res.game_width + 10., GAME_HEIGHT + 20.),
        0.95,
        9.,
    );

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
pub fn spawn_back_button_texture_only(
    pos: Vec3,
    commands: &mut Commands,
    graphics: &Graphics,
) -> Entity {
    // Back Button (parent sprite + child text)
    let back_button_e = commands
        .spawn((
            SpriteBundle {
                texture: graphics.get_ui_element_texture(UIElement::BackButton),
                sprite: Sprite {
                    custom_size: Some(Vec2::new(53., 20.)),
                    ..Default::default()
                },
                transform: Transform::from_translation(pos),
                ..Default::default()
            },
            Interactable::default(),
            UIElement::BackButton,
            RenderLayers::from_layers(&[3]),
            Name::new("Back Button"),
        ))
        .id();
    back_button_e
}

pub fn spawn_back_button(
    pos: Vec3,
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
) -> Entity {
    // Back Button (parent sprite + child text)
    let back_button_e = spawn_back_button_texture_only(pos, commands, graphics);
    commands.entity(back_button_e).insert(MenuButton::Back);
    // Back button text
    commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                "BACK",
                TextStyle {
                    font: asset_server.load("fonts/alagard.ttf"),
                    font_size: 15.0,
                    color: WHITE,
                },
            ),
            text_anchor: Anchor::Center,
            transform: Transform {
                translation: Vec3::new(0., -1., 1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .set_parent(back_button_e);
    back_button_e
}
