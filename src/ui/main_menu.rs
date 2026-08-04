use bevy::text::Justify;
use std::{
    fs::{self, create_dir_all, File, OpenOptions},
    io::{BufReader, BufWriter},
    process::exit,
};

use bevy::ecs::system::SystemParam;
use bevy::{camera::visibility::RenderLayers, prelude::*, sprite::Anchor};
use bevy_rapier2d::prelude::{
    Collider, RapierContextColliders, RapierContextJoints, RapierContextSimulation,
    RapierRigidBodySet,
};
use strum::IntoEnumIterator;
use strum_macros::Display;

use crate::{
    assets::Graphics,
    audio::UpdateBGMTrackEvent,
    client::analytics::{connect_server, AnalyticsData},
    client::GameData,
    colors::{overwrite_alpha, WHITE},
    combat::damage_tracker::{DamageTracker, MobStatTracker, PetAbilityStats},
    container::ContainerRegistry,
    datafiles,
    item::CraftingTracker,
    night::NightTracker,
    player::{
        achievements::{Achievement, Achievements},
        currency::TimeFragmentCurrency,
        skills::{HeirloomChoiceQueue, PlayerClass, PlayerSkills},
        time_crystals::TimeCrystals,
        unlocks::{
            persist_unlock_data, RunUnlockState, UnlockUpgrades, UnlockedClasses, UnlockedSkills,
        },
    },
    ui::{
        achievements_ui::{AchievementsPagination, ACHIEVEMENTS_PER_PAGE},
        class_selection::{
            persist_class_unlock_state, ClassSelectionState, ClassUnlockConfirmState,
            ClassUnlockHoverState, PendingGameStart, PlayerSelectSlot, SkillUnlockConfirmState,
        },
        options_ui::CheatSettings,
        ChestContainer, Focusable, FurnaceContainer, UIState,
    },
    world::{
        dimension::{ActiveDimension, EraManager, GenerationSeed},
        generation::WorldObjectCache,
        portal::BossKillTracker,
    },
    DoNotDespawnOnGameOver, Game, GameState, ScreenResolution, DEBUG,
};

use super::{
    essence_ui::EssenceShopCache,
    game_fonts as gf,
    minimap::{FogOfWarData, MinimapTileCache},
    options_ui::{spawn_wipe_data_popup, OptionsUI, WipeDataPopup},
    player_hud::XpBarFadeIn,
    scrapper_ui::ScrapperEvent,
    spawn_loading_overlay, ui_helpers, Interactable, UIElement, KEYBIND_BADGE_COLOR,
};

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
    scrapper_event: MessageWriter<'w, ScrapperEvent>,
    selection_state: ResMut<'w, ClassSelectionState>,
    confirm_state: ResMut<'w, ClassUnlockConfirmState>,
    skill_confirm_state: ResMut<'w, SkillUnlockConfirmState>,
    time_fragment_currency: Option<ResMut<'w, TimeFragmentCurrency>>,
    unlocked_classes: Option<ResMut<'w, UnlockedClasses>>,
    unlocked_skills: ResMut<'w, UnlockedSkills>,
    unlock_upgrades: Res<'w, UnlockUpgrades>,
    hover_state: ResMut<'w, ClassUnlockHoverState>,
    achievements: Option<Res<'w, Achievements>>,
    class_slots: Query<'w, 's, &'static mut PlayerSelectSlot>,
    pagination_state: ResMut<'w, AchievementsPagination>,
    run_unlock_state: ResMut<'w, RunUnlockState>,
    time_crystals: Res<'w, TimeCrystals>,
    screen_res: Res<'w, ScreenResolution>,
    player_class: Option<Res<'w, PlayerClass>>,
    game_state: Res<'w, State<GameState>>,
    wipe_popup: Query<'w, 's, Entity, With<WipeDataPopup>>,
    options_ui: Query<'w, 's, Entity, With<OptionsUI>>,
    graphics: Res<'w, Graphics>,
    asset_server: Res<'w, AssetServer>,
    leaderboard_visible: Option<ResMut<'w, MainMenuLeaderboardVisible>>,
    game_data: Option<ResMut<'w, GameData>>,
}

#[derive(Component, Clone, Eq, Display, Debug, PartialEq)]
pub enum MenuButton {
    Start,
    Unlocks,
    Options,
    Achievements,
    TimeCrystals,
    Beastiary,
    Quit,
    InfoOK,
    GameOverOK,
    Scrapper,
    Back,
    Begin,
    ClassUnlockYes,
    ClassUnlockNo,
    SkillUnlockYes,
    SkillUnlockNo,
    AchievementsPrev,
    AchievementsNext,
    OptionsRestart,
    OptionsExit,
    ShowTutorial,
    WipeGameData,
    WipeDataConfirm,
    WipeDataCancel,
    Archive,
    LeaderboardToggle,
}
#[derive(Component)]
pub struct InfoModal;

#[derive(Message)]
pub struct MenuButtonClickEvent {
    pub button: MenuButton,
}

#[derive(Component)]
pub struct MainMenu;

#[derive(Component)]
pub struct AchievementsButton;

#[derive(Component)]
pub struct AchievementsNotificationIcon;

#[derive(Component)]
pub struct GameStartFadein(pub Timer);

/// Short hover label for 28×28 main-menu icon buttons (Archives, Achievements, etc.).
#[derive(Component, Clone, Copy)]
pub struct MainMenuIconTooltipText(pub &'static str);

#[derive(Component)]
pub struct MainMenuIconTooltip;

#[derive(Component)]
pub struct ArchivesUI;

pub const MAIN_MENU_BG_SIZE: Vec2 = Vec2::new(714., 400.);
pub const MAIN_MENU_ICON_BUTTON_SIZE: Vec2 = Vec2::new(28., 28.);
pub const MAIN_MENU_WIDE_BUTTON_SIZE: Vec2 = Vec2::new(126., 22.);
/// Distance from the bottom screen edge to the bottom of the icon button row.
const MAIN_MENU_BOTTOM_INSET: f32 = 12.;
const MAIN_MENU_EDGE_PADDING: f32 = 22.;
const MAIN_MENU_ICON_GAP: f32 = 6.;

/// Background container art size (`assets/ui/BackgroundContainer.png`).
pub const ARCHIVES_CONTAINER_UI_SIZE: Vec2 = Vec2::new(162., 164.);

const ARCHIVES_BUTTON_SPACING_Y: f32 = 30.;
const ARCHIVES_EXIT_GAP_Y: f32 = 14.;

/// Whether the compact leaderboard panel is visible on the main menu.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct MainMenuLeaderboardVisible(pub bool);

fn main_menu_button_row_y(game_height: f32) -> f32 {
    -game_height * 0.5 + MAIN_MENU_BOTTOM_INSET + MAIN_MENU_ICON_BUTTON_SIZE.y * 0.5
}

fn main_menu_left_icon_x(game_width: f32, index: u32) -> f32 {
    let first_center =
        -game_width * 0.5 + MAIN_MENU_EDGE_PADDING + MAIN_MENU_ICON_BUTTON_SIZE.x * 0.5;
    first_center + index as f32 * (MAIN_MENU_ICON_BUTTON_SIZE.x + MAIN_MENU_ICON_GAP)
}

fn main_menu_right_icon_x(game_width: f32, index_from_right: u32) -> f32 {
    let quit_center =
        game_width * 0.5 - MAIN_MENU_EDGE_PADDING - MAIN_MENU_ICON_BUTTON_SIZE.x * 0.5;
    quit_center - index_from_right as f32 * (MAIN_MENU_ICON_BUTTON_SIZE.x + MAIN_MENU_ICON_GAP)
}

pub fn display_main_menu(
    mut commands: Commands,
    graphics: Res<Graphics>,
    mut bgm_track_event: MessageWriter<UpdateBGMTrackEvent>,
) {
    let mut menu = commands.spawn((
        Sprite {
            image: graphics.get_ui_element_texture(UIElement::MainMenuNew),
            custom_size: Some(MAIN_MENU_BG_SIZE),
            ..default()
        },
        Transform {
            translation: Vec3::new(16., 0., 0.),
            scale: Vec3::new(1., 1., 1.),
            ..Default::default()
        },
    ));
    menu.insert(UIElement::MainMenuNew)
        .insert(MainMenu)
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("Main Menu"));
    //start music
    bgm_track_event.write(UpdateBGMTrackEvent {
        asset_path: "sounds/bgm_day.ogg".to_owned(),
    });
}

pub fn remove_main_menu(
    mut commands: Commands,
    query: Query<Entity, With<MainMenu>>,
    menu_buttons: Query<Entity, With<MenuButton>>,
) {
    for entity in query.iter() {
        commands.entity(entity).despawn();

        for button in menu_buttons.iter() {
            commands.entity(button).despawn();
        }
    }
}

pub fn handle_menu_button_click_events(
    mut event_reader: MessageReader<MenuButtonClickEvent>,
    mut next_state: ResMut<NextState<GameState>>,
    mut next_ui_state: ResMut<NextState<UIState>>,
    mut commands: Commands,
    mut extras: MenuButtonExtras,
    current_ui_state: Res<State<UIState>>,
    mut cleanup_event: MessageWriter<CleanUpRunStateEvent>,
) {
    for event in event_reader.read() {
        let info_modal_open = extras.info_modal.iter().next().is_some();
        let wipe_popup_open = extras.wipe_popup.iter().next().is_some();
        if wipe_popup_open
            && !matches!(
                event.button,
                MenuButton::WipeDataConfirm | MenuButton::WipeDataCancel
            )
        {
            continue;
        }

        // Block all menu interactions when name entry popup is open
        if *current_ui_state.get() == UIState::EnterName {
            continue;
        }

        match event.button {
            MenuButton::Start => {
                if info_modal_open {
                    continue;
                }
                // Show class selection UI instead of starting game immediately
                next_ui_state.set(UIState::ClassSelection);
            }
            MenuButton::Unlocks => {
                if info_modal_open {
                    continue;
                }
                next_ui_state.set(UIState::Unlocks);
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
            MenuButton::Archive => {
                if info_modal_open {
                    continue;
                }
                next_ui_state.set(UIState::Archives);
            }
            MenuButton::LeaderboardToggle => {
                if info_modal_open {
                    continue;
                }
                if let Some(mut visible) = extras.leaderboard_visible.as_mut() {
                    visible.0 = !visible.0;
                    persist_main_menu_leaderboard_visible(visible.0);
                    if let Some(mut game_data) = extras.game_data.as_mut() {
                        game_data.show_main_menu_leaderboard = Some(visible.0);
                    }
                }
            }
            MenuButton::TimeCrystals => {
                if info_modal_open {
                    continue;
                }
                next_ui_state.set(UIState::TimeCrystalsBrowser);
            }
            MenuButton::Beastiary => {
                if info_modal_open {
                    continue;
                }
                next_ui_state.set(UIState::BeastiaryBrowser);
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
                    commands.entity(e).despawn();
                }
            }
            MenuButton::Scrapper => {
                extras.scrapper_event.write_default();
            }
            MenuButton::Back => {
                next_ui_state.set(crate::ui::UIState::Closed);
            }
            MenuButton::Begin => {
                // Confirm button clicked
                // Check if class is selected (pet is optional)
                if let Some(class) = &extras.selection_state.selected_class {
                    // Store the game start data - will be picked up by handle_portal_animation
                    // We'll use a resource to pass this data to the portal animation system
                    let pets = extras
                        .selection_state
                        .selected_pet
                        .iter()
                        .cloned()
                        .collect();

                    commands.insert_resource(PendingGameStart {
                        class: class.clone(),
                        pets,
                    });
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
                            extras.time_fragment_currency.as_mut(),
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
                                        currency.time_fragments
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
                                extras.time_fragment_currency.as_ref(),
                                extras.unlocked_classes.as_ref(),
                            ) {
                                let currency_ref = currency_res.as_ref();
                                let unlocked_ref = unlocked_res.as_ref();
                                persist_class_unlock_state(
                                    Some(currency_ref),
                                    unlocked_ref,
                                    extras.achievements.as_ref().map(|a| a.as_ref()),
                                    Some(&*extras.unlock_upgrades),
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
            MenuButton::SkillUnlockNo => {
                if extras.skill_confirm_state.active {
                    extras.skill_confirm_state.active = false;
                    extras.skill_confirm_state.class = None;
                    extras.skill_confirm_state.slot_index = 0;
                    extras.skill_confirm_state.cost = 0;
                    extras.skill_confirm_state.anchor_position = Vec3::ZERO;
                }
            }
            MenuButton::SkillUnlockYes => {
                if extras.skill_confirm_state.active {
                    if let Some(class) = extras.skill_confirm_state.class.clone() {
                        let cost = extras.skill_confirm_state.cost;
                        let slot = extras.skill_confirm_state.slot_index;
                        let mut unlocked_now = false;
                        if let Some(currency_res) = extras.time_fragment_currency.as_mut() {
                            let currency = currency_res.as_mut();
                            if currency.spend(cost) {
                                if extras.unlocked_skills.insert(class.clone(), slot) {
                                    unlocked_now = true;
                                }
                            } else {
                                warn!(
                                    "Attempted to unlock skill slot {} for {:?} without enough currency (cost: {}, owned: {})",
                                    slot, class, cost, currency.time_fragments
                                );
                            }
                        }
                        if unlocked_now {
                            persist_unlock_data(
                                extras.time_fragment_currency.as_ref().map(|c| c.as_ref()),
                                extras.unlocked_classes.as_ref().map(|u| u.as_ref()),
                                extras.achievements.as_ref().map(|a| a.as_ref()),
                                Some(&*extras.unlock_upgrades),
                                Some(&*extras.unlocked_skills),
                            );
                        }
                    }
                    extras.skill_confirm_state.active = false;
                    extras.skill_confirm_state.class = None;
                    extras.skill_confirm_state.slot_index = 0;
                    extras.skill_confirm_state.cost = 0;
                    extras.skill_confirm_state.anchor_position = Vec3::ZERO;
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
                spawn_loading_overlay(
                    &mut commands,
                    &extras.asset_server,
                    &extras.screen_res,
                    "Loading...",
                );
                for e in extras.world_entities.iter() {
                    if let Ok(mut entity_commands) = commands.get_entity(e) {
                        entity_commands.despawn();
                    }
                }
                let _ = fs::remove_file(datafiles::save_file());
                next_state.set(GameState::MainMenu);

                cleanup_event.write_default();
            }
            MenuButton::OptionsRestart => {
                info!("Options menu: Restarting with same class/pet");

                if let Some(player_class) = extras.player_class.as_ref() {
                    extras.selection_state.selected_class = Some(player_class.class.clone());
                    extras.selection_state.selected_pet = player_class.pets.first().cloned();
                    commands.insert_resource(PlayerClass {
                        class: player_class.class.clone(),
                        pets: player_class.pets.clone(),
                    });

                    extras.run_unlock_state.reset_for_run(
                        &*extras.unlock_upgrades,
                        extras.time_crystals.completed_count() as u32,
                    );
                }

                next_ui_state.set(UIState::Closed);
                next_state.set(GameState::Initializing);
                cleanup_event.write_default();
            }
            MenuButton::OptionsExit => {
                info!("Options menu: Exiting to main menu");
                spawn_loading_overlay(
                    &mut commands,
                    &extras.asset_server,
                    &extras.screen_res,
                    "Loading...",
                );
                for entity in extras.options_ui.iter() {
                    commands.entity(entity).despawn();
                }
                for entity in extras.wipe_popup.iter() {
                    commands.entity(entity).despawn();
                }
                next_ui_state.set(UIState::Closed);
                next_state.set(GameState::MainMenu);
                cleanup_event.write_default();
            }
            MenuButton::ShowTutorial => {
                if *extras.game_state.get() != GameState::Main
                    || *current_ui_state.get() != UIState::Options
                {
                    continue;
                }
                commands.insert_resource(crate::ui::tutorial_ui::TutorialReplayRequested);
                next_ui_state.set(UIState::Closed);
            }
            MenuButton::WipeGameData => {
                if *current_ui_state.get() != UIState::Options {
                    continue;
                }
                if extras.wipe_popup.iter().next().is_some() {
                    continue;
                }
                spawn_wipe_data_popup(&mut commands, &extras.graphics, &extras.asset_server);
            }
            MenuButton::WipeDataCancel => {
                for e in extras.wipe_popup.iter() {
                    commands.entity(e).despawn();
                }
            }
            MenuButton::WipeDataConfirm => {
                info!("Wiping game data: deleting game_data.json and save state");
                let _ = fs::remove_file(datafiles::game_data());
                let _ = fs::remove_file(datafiles::save_file());
                exit(0);
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
    ui_element: UIElement,
) -> Entity {
    let button_e = commands
        .spawn((
            (
                Sprite {
                    image: graphics.get_ui_element_texture(ui_element.clone()),
                    custom_size: Some(size),
                    ..default()
                },
                Transform::from_translation(button_pos),
            ),
            Interactable::default(),
            ui_element,
            button_type,
            RenderLayers::from_layers(&[3]),
            Name::new(format!("Menu Button: {}", text)),
        ))
        .id();

    // Button text
    commands
        .spawn(
            gf::MENU_TITLE
                .text(&asset_server, text, WHITE)
                .anchor(Anchor::CENTER_LEFT)
                .with_transform(Transform {
                    translation: text_offset,
                    scale: gf::MENU_TITLE.transform_scale(),
                    ..Default::default()
                }),
        )
        .insert(RenderLayers::from_layers(&[3]))
        .insert(ChildOf(button_e));

    button_e
}

pub fn spawn_main_menu_icon_button(
    button_pos: Vec3,
    button_type: MenuButton,
    ui_element: UIElement,
    tooltip: Option<&'static str>,
    commands: &mut Commands,
    graphics: &Graphics,
) -> Entity {
    let button_name = format!("Main Menu Icon Button: {:?}", button_type);
    let mut button = commands.spawn((
        (
            Sprite {
                image: graphics.get_ui_element_texture(ui_element.clone()),
                custom_size: Some(MAIN_MENU_ICON_BUTTON_SIZE),
                ..default()
            },
            Transform::from_translation(button_pos),
        ),
        Interactable::default(),
        ui_element,
        button_type,
        RenderLayers::from_layers(&[3]),
        Name::new(button_name),
    ));
    if let Some(label) = tooltip {
        button.insert(MainMenuIconTooltipText(label));
    }
    button.id()
}

/// Shared 28×28 exit icon used on main menu sub-screens (achievements, unlocks, bestiary, archives).
pub fn spawn_exit_icon_button(
    button_pos: Vec3,
    commands: &mut Commands,
    graphics: &Graphics,
) -> Entity {
    spawn_main_menu_icon_button(
        button_pos,
        MenuButton::Back,
        UIElement::ExitButton,
        None,
        commands,
        graphics,
    )
}

pub fn init_main_menu_leaderboard_visibility(
    mut visible: ResMut<MainMenuLeaderboardVisible>,
    game_data: Option<Res<GameData>>,
) {
    visible.0 = game_data
        .as_ref()
        .and_then(|data| data.show_main_menu_leaderboard)
        .unwrap_or(false);
}

pub fn persist_main_menu_leaderboard_visible(show: bool) {
    let path = datafiles::game_data();
    let mut game_data = if let Ok(file) = File::open(&path) {
        let reader = BufReader::new(file);
        GameData::try_from_json_reader(reader).unwrap_or_default()
    } else {
        GameData::default()
    };
    game_data.show_main_menu_leaderboard = Some(show);
    if let Ok(file) = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&path)
    {
        let writer = BufWriter::new(file);
        if let Err(err) = serde_json::to_writer_pretty(writer, &game_data) {
            error!("Failed to persist main-menu leaderboard visibility: {err:?}");
        }
    }
}

pub fn spawn_main_menu_wide_button(
    button_pos: Vec3,
    text: &str,
    button_type: MenuButton,
    ui_element: UIElement,
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
) -> Entity {
    let button_e = commands
        .spawn((
            (
                Sprite {
                    image: graphics.get_ui_element_texture(ui_element.clone()),
                    custom_size: Some(MAIN_MENU_WIDE_BUTTON_SIZE),
                    ..default()
                },
                Transform::from_translation(button_pos),
            ),
            Interactable::default(),
            ui_element,
            button_type,
            RenderLayers::from_layers(&[3]),
            Name::new(format!("Main Menu Button: {}", text)),
        ))
        .id();

    commands
        .spawn(
            gf::MENU_TITLE
                .text(&asset_server, text, WHITE)
                .justify(Justify::Center)
                .anchor(Anchor::CENTER)
                .with_transform(Transform {
                    translation: Vec3::new(0., -1., 1.),
                    scale: gf::MENU_TITLE.transform_scale(),
                    ..Default::default()
                }),
        )
        .insert(RenderLayers::from_layers(&[3]))
        .insert(ChildOf(button_e));

    button_e
}

fn main_menu_tooltip_backdrop_size(label: &str) -> Vec2 {
    const TOOLTIP_PAD_X: f32 = 10.;
    const TOOLTIP_PAD_Y: f32 = 5.;
    const TOOLTIP_LINE_HEIGHT: f32 = 10.0;
    const TOOLTIP_CHAR_WIDTH: f32 = 4.6;
    let text_w = label.chars().count() as f32 * TOOLTIP_CHAR_WIDTH;
    Vec2::new(
        (text_w + TOOLTIP_PAD_X * 2.).max(28.),
        TOOLTIP_LINE_HEIGHT + TOOLTIP_PAD_Y * 2.,
    )
}

fn main_menu_tooltip_world_pos(icon_center: Vec3, size: Vec2) -> Vec3 {
    Vec3::new(
        icon_center.x,
        icon_center.y + MAIN_MENU_ICON_BUTTON_SIZE.y * 0.5 + 4. + size.y * 0.5,
        30.,
    )
}

pub fn spawn_main_menu_icon_tooltip(
    commands: &mut Commands,
    asset_server: &AssetServer,
    label: &str,
    icon_center: Vec3,
) -> Entity {
    let size = main_menu_tooltip_backdrop_size(label);
    let pos = main_menu_tooltip_world_pos(icon_center, size);

    let root = commands
        .spawn((
            (Transform::from_translation(pos), Visibility::default()),
            RenderLayers::from_layers(&[3]),
            MainMenuIconTooltip,
            Name::new("Main Menu Icon Tooltip"),
        ))
        .id();

    commands
        .spawn((
            Sprite {
                color: KEYBIND_BADGE_COLOR,
                custom_size: Some(size),
                ..default()
            },
            Transform::default(),
        ))
        .insert(RenderLayers::from_layers(&[3]))
        .insert(ChildOf(root));

    commands
        .spawn(
            gf::ICON_HOVER_TOOLTIP
                .text(&asset_server, label, WHITE)
                .justify(Justify::Center)
                .anchor(Anchor::CENTER)
                .with_transform(Transform {
                    translation: Vec3::new(0., 0., 1.),
                    scale: gf::ICON_HOVER_TOOLTIP.transform_scale(),
                    ..default()
                }),
        )
        .insert(RenderLayers::from_layers(&[3]))
        .insert(ChildOf(root));

    root
}

pub fn handle_main_menu_icon_tooltips(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    cursor_pos: Res<crate::cursor::CursorPos>,
    game_state: Res<State<GameState>>,
    ui_state: Res<State<UIState>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    tooltip_targets: Query<(&GlobalTransform, &MainMenuIconTooltipText)>,
    existing: Query<Entity, With<MainMenuIconTooltip>>,
    mut last_hovered: Local<Option<Entity>>,
) {
    if *game_state != GameState::MainMenu || *ui_state != UIState::Closed {
        for tooltip_e in existing.iter() {
            commands.entity(tooltip_e).despawn();
        }
        *last_hovered = None;
        return;
    }

    let hovered = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None, None).and_then(
        |(entity, _, _)| {
            tooltip_targets
                .get(entity)
                .ok()
                .map(|(transform, text)| (entity, transform.translation(), text.0))
        },
    );

    let Some((entity, icon_center, label)) = hovered else {
        for tooltip_e in existing.iter() {
            commands.entity(tooltip_e).despawn();
        }
        *last_hovered = None;
        return;
    };

    if *last_hovered == Some(entity) {
        return;
    }

    for tooltip_e in existing.iter() {
        commands.entity(tooltip_e).despawn();
    }

    spawn_main_menu_icon_tooltip(&mut commands, &asset_server, label, icon_center);

    *last_hovered = Some(entity);
}

pub fn setup_archives_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    resolution: Res<ScreenResolution>,
) {
    let overlay = ui_helpers::spawn_full_screen_ui_overlay(
        &mut commands,
        &resolution,
        0.95,
        ui_helpers::Z_DEPTH_MAIN_MENU_MODAL_OVERLAY,
    );
    commands.entity(overlay).insert(ArchivesUI);

    let archives_root = commands
        .spawn((
            (
                Transform::from_translation(Vec3::new(
                    0.,
                    0.,
                    ui_helpers::Z_DEPTH_MAIN_MENU_MODAL_CONTENT,
                )),
                Visibility::default(),
            ),
            ArchivesUI,
            UIState::Archives,
            RenderLayers::from_layers(&[3]),
            Name::new("Archives UI"),
        ))
        .id();

    commands
        .spawn((
            Sprite {
                image: graphics.get_ui_element_texture(UIElement::BackgroundContainer),
                custom_size: Some(ARCHIVES_CONTAINER_UI_SIZE),
                ..default()
            },
            Transform::from_translation(Vec3::new(0., 0., 1.)),
        ))
        .insert(ArchivesUI)
        .insert(UIState::Archives)
        .insert(RenderLayers::from_layers(&[3]))
        .insert(crate::item::item_drop_outline::UiShadow::container())
        .insert(ChildOf(archives_root));

    let button_entries: [(&str, MenuButton); 2] = [
        ("Bestiary", MenuButton::Beastiary),
        ("Time Crystals", MenuButton::TimeCrystals),
    ];
    let top_y = ARCHIVES_BUTTON_SPACING_Y;
    for (i, (label, button_type)) in button_entries.iter().enumerate() {
        let y = top_y - i as f32 * ARCHIVES_BUTTON_SPACING_Y;
        let button = spawn_main_menu_wide_button(
            Vec3::new(0., y, 2.),
            label,
            button_type.clone(),
            UIElement::MainMenuStartButton,
            &mut commands,
            &graphics,
            &asset_server,
        );
        commands
            .entity(button)
            .insert(ArchivesUI)
            .insert(UIState::Archives)
            .insert(Focusable {
                group: UIState::Archives,
                index: i as u32,
            })
            .insert(ChildOf(archives_root));
    }

    let exit_y = top_y
        - ARCHIVES_BUTTON_SPACING_Y
        - ARCHIVES_EXIT_GAP_Y
        - MAIN_MENU_ICON_BUTTON_SIZE.y * 0.5
        - MAIN_MENU_WIDE_BUTTON_SIZE.y * 0.5;
    let exit_button = spawn_main_menu_icon_button(
        Vec3::new(0., exit_y, 2.),
        MenuButton::Back,
        UIElement::ExitButton,
        None,
        &mut commands,
        &graphics,
    );
    commands
        .entity(exit_button)
        .insert(ArchivesUI)
        .insert(UIState::Archives)
        .insert(Focusable {
            group: UIState::Archives,
            index: 100,
        })
        .insert(ChildOf(archives_root));
}

pub fn cleanup_archives_ui(mut commands: Commands, archives_ui: Query<Entity, With<ArchivesUI>>) {
    for entity in archives_ui.iter() {
        commands.entity(entity).despawn();
    }
}

pub fn spawn_menu_text_buttons(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    graphics: Res<Graphics>,
    resolution: Res<ScreenResolution>,
) {
    let row_y = main_menu_button_row_y(resolution.game_height);
    let button_z = ui_helpers::Z_DEPTH_MAIN_MENU_BUTTONS;

    let archive_button = spawn_main_menu_icon_button(
        Vec3::new(
            main_menu_left_icon_x(resolution.game_width, 0),
            row_y,
            button_z,
        ),
        MenuButton::Archive,
        UIElement::MainMenuArchiveButton,
        Some("Archives"),
        &mut commands,
        &graphics,
    );
    commands.entity(archive_button).insert(Focusable {
        group: UIState::Closed,
        index: 1,
    });

    let achievements_button = spawn_main_menu_icon_button(
        Vec3::new(
            main_menu_left_icon_x(resolution.game_width, 1),
            row_y,
            button_z,
        ),
        MenuButton::Achievements,
        UIElement::MainMenuAchievementsButton,
        Some("Achievements"),
        &mut commands,
        &graphics,
    );
    commands
        .entity(achievements_button)
        .insert(AchievementsButton)
        .insert(Focusable {
            group: UIState::Closed,
            index: 2,
        });

    let unlocks_button = spawn_main_menu_icon_button(
        Vec3::new(
            main_menu_left_icon_x(resolution.game_width, 2),
            row_y,
            button_z,
        ),
        MenuButton::Unlocks,
        UIElement::MainMenuUnlocksButton,
        Some("Unlocks"),
        &mut commands,
        &graphics,
    );
    commands.entity(unlocks_button).insert(Focusable {
        group: UIState::Closed,
        index: 3,
    });

    let start_button = spawn_main_menu_wide_button(
        Vec3::new(0., row_y, button_z),
        "Enter",
        MenuButton::Start,
        UIElement::MainMenuStartButton,
        &mut commands,
        &graphics,
        &asset_server,
    );
    // Index 0: the natural first focus when the title screen appears (front and center).
    commands.entity(start_button).insert(Focusable {
        group: UIState::Closed,
        index: 0,
    });

    let quit_button = spawn_main_menu_icon_button(
        Vec3::new(
            main_menu_right_icon_x(resolution.game_width, 0),
            row_y,
            button_z,
        ),
        MenuButton::Quit,
        UIElement::ExitButton,
        Some("Quit"),
        &mut commands,
        &graphics,
    );
    commands.entity(quit_button).insert(Focusable {
        group: UIState::Closed,
        index: 4,
    });

    let options_button = spawn_main_menu_icon_button(
        Vec3::new(
            main_menu_right_icon_x(resolution.game_width, 1),
            row_y,
            button_z,
        ),
        MenuButton::Options,
        UIElement::MainMenuOptionsButton,
        Some("Options"),
        &mut commands,
        &graphics,
    );
    commands.entity(options_button).insert(Focusable {
        group: UIState::Closed,
        index: 5,
    });

    let leaderboard_button = spawn_main_menu_icon_button(
        Vec3::new(
            main_menu_right_icon_x(resolution.game_width, 2),
            row_y,
            button_z,
        ),
        MenuButton::LeaderboardToggle,
        UIElement::LeaderboardButton,
        Some("Leaderboard"),
        &mut commands,
        &graphics,
    );
    commands.entity(leaderboard_button).insert(Focusable {
        group: UIState::Closed,
        index: 6,
    });
}

pub fn tick_game_start_overlay(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut GameStartFadein, &mut Sprite)>,
) {
    for (e, mut timer, mut sprite) in query.iter_mut() {
        timer.0.tick(time.delta());
        if timer.0.is_finished() {
            commands.insert_resource(
                crate::ui::tutorial_ui::PendingFindBossShrineHint::WaitingForTrigger,
            );
            commands.insert_resource(XpBarFadeIn(Timer::from_seconds(2.0, TimerMode::Once)));
            commands.insert_resource(crate::ui::tutorial_ui::PendingTutorialReady(
                Timer::from_seconds(
                    crate::ui::tutorial_ui::START_TUTORIAL_DELAY_SECS,
                    TimerMode::Once,
                ),
            ));
            commands.entity(e).despawn();
        } else {
            let alpha = f32::max(0., 1. - timer.0.fraction());
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
            (
                Sprite {
                    image: graphics.get_ui_element_texture(UIElement::BackButton),
                    custom_size: Some(Vec2::new(53., 18.)),
                    ..default()
                },
                Transform::from_translation(pos),
            ),
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
        .spawn(
            gf::MENU_TITLE
                .text(&asset_server, "BACK", WHITE)
                .anchor(Anchor::CENTER)
                .with_transform(Transform {
                    translation: Vec3::new(0., -1., 1.),
                    scale: gf::MENU_TITLE.transform_scale(),
                    ..Default::default()
                }),
        )
        .insert(RenderLayers::from_layers(&[3]))
        .insert(ChildOf(back_button_e));
    back_button_e
}

pub fn update_achievements_notification_icon(
    achievements: Res<Achievements>,
    achievements_button: Query<Entity, With<AchievementsButton>>,
    notification_icon: Query<Entity, With<AchievementsNotificationIcon>>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    let Ok(button_entity) = achievements_button.single() else {
        return;
    };

    let has_unclaimed = achievements.has_unclaimed_completed();

    // Check if notification icon exists
    if let Ok(icon_entity) = notification_icon.single() {
        // Update visibility
        commands.entity(icon_entity).insert(if has_unclaimed {
            Visibility::Visible
        } else {
            Visibility::Hidden
        });
    } else if has_unclaimed {
        // Spawn notification icon if it doesn't exist and we need it
        use crate::animations::enemy_sprites::spawn_attack_warning_aseprite;
        let icon = spawn_attack_warning_aseprite(
            &mut commands,
            &asset_server,
            Vec3::new(10., 8., 1.), // Position relative to button
            button_entity,
            999999.0, // Very long duration
        );
        commands
            .entity(icon)
            .insert(AchievementsNotificationIcon)
            .insert(RenderLayers::from_layers(&[3]));
    }
}

#[derive(Default, Message)]
pub struct CleanUpRunStateEvent;

pub fn cleanup_run_state(
    mut event: MessageReader<CleanUpRunStateEvent>,
    mut commands: Commands,
    world_entities: Query<
        Entity,
        (
            Or<(With<Visibility>, With<ActiveDimension>, With<Collider>)>,
            Without<DoNotDespawnOnGameOver>,
        ),
    >,
    players: Query<Entity, With<crate::player::Player>>,
    boss_health_bars: Query<
        Entity,
        Or<(
            With<crate::ui::boss_health_bar::BossHealthBar>,
            With<crate::ui::boss_health_bar::BossHealthBarFrame>,
            With<crate::ui::boss_health_bar::BossNameText>,
        )>,
    >,
    guide_hud: Query<Entity, With<crate::ui::key_input_guide::InteractGuide>>,
    time_crystals: Res<TimeCrystals>,
    cheat_settings: Res<CheatSettings>,
) {
    if event.is_empty() {
        return;
    }
    // Must consume — `is_empty()` alone does not advance the reader; unread messages
    // re-trigger cleanup on later non-MainMenu frames (e.g. Initializing) and can
    // despawn a freshly spawned player for the next run.
    event.clear();
    info!("Cleaning up ALL run data on GameState::Main exit");

    for entity in boss_health_bars.iter() {
        commands.entity(entity).despawn();
    }
    for entity in guide_hud.iter() {
        commands.entity(entity).despawn();
    }
    // Explicit: don't rely only on Visibility/Collider matching for the player.
    for entity in players.iter() {
        if let Ok(mut entity_commands) = commands.get_entity(entity) {
            entity_commands.despawn();
        }
    }

    for e in world_entities.iter() {
        if let Ok(mut entity_commands) = commands.get_entity(e) {
            entity_commands.despawn();
        }
    }

    let _ = fs::remove_file(datafiles::save_file());

    commands.remove_resource::<ChestContainer>();
    commands.remove_resource::<FurnaceContainer>();
    commands.insert_resource(AnalyticsData::default());
    commands.insert_resource(HeirloomChoiceQueue::new_for_run(
        &time_crystals,
        cheat_settings.bypass_time_crystal_pool,
    ));
    commands.insert_resource(Game::default());
    commands.insert_resource(NightTracker::default());
    commands.insert_resource(ContainerRegistry::default());
    commands.insert_resource(CraftingTracker::default());
    commands.insert_resource(EraManager::default());
    commands.remove_resource::<WorldObjectCache>();
    commands.insert_resource(DamageTracker::default());
    commands.insert_resource(MobStatTracker::default());
    commands.insert_resource(PetAbilityStats::default());
    commands.remove_resource::<XpBarFadeIn>();
    commands.insert_resource(crate::player::skills::HeirloomTriggerCounts::default());
    commands.insert_resource(crate::player::skills::ManaTrackerResetTimer::default());
    if !cheat_settings.persist_item_filters {
        commands.insert_resource(crate::inventory::BreakDropFilter::default());
    }
    commands.insert_resource(EssenceShopCache::default());
    commands.insert_resource(FogOfWarData::default());
    commands.insert_resource(MinimapTileCache::default());
    commands.insert_resource(crate::item::boss_shrine::BossSummonTracker::default());
    commands.insert_resource(BossKillTracker::default());

    commands.queue(|world: &mut World| {
        for (mut sim, mut colliders, mut joints, mut bodies) in world
            .query::<(
                &mut RapierContextSimulation,
                &mut RapierContextColliders,
                &mut RapierContextJoints,
                &mut RapierRigidBodySet,
            )>()
            .iter_mut(world)
        {
            *sim = RapierContextSimulation::default();
            *colliders = RapierContextColliders::default();
            *joints = RapierContextJoints::default();
            *bodies = RapierRigidBodySet::default();
        }
    });
}
