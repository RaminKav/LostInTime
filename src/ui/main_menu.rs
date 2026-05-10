use std::{
    fs::{self, create_dir_all, File},
    process::exit,
};

use bevy::ecs::system::SystemParam;
use bevy::{prelude::*, render::view::RenderLayers, sprite::Anchor};
use bevy_rapier2d::prelude::{Collider, RapierContext};
use strum::IntoEnumIterator;
use strum_macros::Display;

use crate::{
    assets::Graphics,
    audio::UpdateBGMTrackEvent,
    client::analytics::{connect_server, AnalyticsData},
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
        ChestContainer, FurnaceContainer, UIState,
    },
    world::{
        dimension::{ActiveDimension, EraManager, GenerationSeed},
        generation::WorldObjectCache,
        portal::BossKillTracker,
    },
    DoNotDespawnOnGameOver, Game, GameState, ScreenResolution, DEBUG, GAME_HEIGHT,
};

use super::{
    essence_ui::EssenceShopCache,
    minimap::{FogOfWarData, MinimapTileCache},
    player_hud::XpBarFadeIn,
    scrapper_ui::ScrapperEvent,
    Interactable, UIElement,
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
    scrapper_event: EventWriter<'w, ScrapperEvent>,
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
}

#[derive(Component, Clone, Eq, Display, Debug, PartialEq)]
pub enum MenuButton {
    Start,
    Unlocks,
    Options,
    Achievements,
    TimeCrystals,
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
}
#[derive(Component)]
pub struct InfoModal;

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
            custom_size: Some(Vec2::new(res.game_width, GAME_HEIGHT)),
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
    current_ui_state: Res<State<UIState>>,
    mut cleanup_event: EventWriter<CleanUpRunStateEvent>,
) {
    for event in event_reader.iter() {
        let info_modal_open = extras.info_modal.iter().next().is_some();

        // Block all menu interactions when name entry popup is open
        if current_ui_state.0 == UIState::EnterName {
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
            MenuButton::TimeCrystals => {
                if info_modal_open {
                    continue;
                }
                next_ui_state.set(UIState::TimeCrystalsBrowser);
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
                for e in extras.world_entities.iter() {
                    if let Some(entity_commands) = commands.get_entity(e) {
                        entity_commands.despawn_recursive();
                    }
                }
                let _ = fs::remove_file(datafiles::save_file());
                next_state.0 = Some(GameState::MainMenu);

                cleanup_event.send_default();
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
                cleanup_event.send_default();
            }
            MenuButton::OptionsExit => {
                info!("Options menu: Exiting to main menu");
                next_ui_state.set(UIState::Closed);
                next_state.set(GameState::MainMenu);
                cleanup_event.send_default();
            }
            MenuButton::ShowTutorial => {
                if extras.game_state.0 != GameState::Main || current_ui_state.0 != UIState::Options {
                    continue;
                }
                commands.insert_resource(crate::ui::tutorial_ui::TutorialReplayRequested);
                next_ui_state.set(UIState::Closed);
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
            SpriteBundle {
                texture: graphics.get_ui_element_texture(ui_element.clone()),
                sprite: Sprite {
                    custom_size: Some(size),
                    ..Default::default()
                },
                transform: Transform::from_translation(button_pos),
                ..Default::default()
            },
            Interactable::default(),
            ui_element,
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
        Vec3::new(22., -38., 1.),
        Vec3::new(-19., -1., 1.),
        "Start ",
        MenuButton::Start,
        Vec2::new(60., 18.),
        &mut commands,
        &graphics,
        &asset_server,
        UIElement::MenuButton,
    );

    // Unlocks Button
    spawn_menu_button(
        Vec3::new(-14., -62.5, 1.),
        Vec3::new(-29., -0.5, 1.),
        "Unlocks",
        MenuButton::Unlocks,
        Vec2::new(84., 18.),
        &mut commands,
        &graphics,
        &asset_server,
        UIElement::UnlocksButton,
    );

    // Achievements Button
    let achievements_button = spawn_menu_button(
        Vec3::new(-22., -86., 1.),
        Vec3::new(-50., -0., 1.),
        "Achievements",
        MenuButton::Achievements,
        Vec2::new(118., 18.),
        &mut commands,
        &graphics,
        &asset_server,
        UIElement::AchievementsButton,
    );
    commands
        .entity(achievements_button)
        .insert(AchievementsButton);

    spawn_menu_button(
        Vec3::new(-34., -112., 1.),
        Vec3::new(-50., -0., 1.),
        "Time Crystals",
        MenuButton::TimeCrystals,
        Vec2::new(118., 18.),
        &mut commands,
        &graphics,
        &asset_server,
        UIElement::AchievementsButton,
    );

    // Options Button
    spawn_menu_button(
        Vec3::new(140., -139., 1.),
        Vec3::new(-27., 0.0, 1.),
        "Options",
        MenuButton::Options,
        Vec2::new(60., 18.),
        &mut commands,
        &graphics,
        &asset_server,
        UIElement::MenuButton,
    );

    // Quit Button
    spawn_menu_button(
        Vec3::new(-82., -139., 1.),
        Vec3::new(-14., -0., 1.),
        "Quit",
        MenuButton::Quit,
        Vec2::new(60., 18.),
        &mut commands,
        &graphics,
        &asset_server,
        UIElement::MenuButton,
    );
}

pub fn tick_game_start_overlay(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut GameStartFadein, &mut Sprite)>,
) {
    for (e, mut timer, mut sprite) in query.iter_mut() {
        timer.0.tick(time.delta());
        if timer.0.finished() {
            commands.insert_resource(XpBarFadeIn(Timer::from_seconds(2.0, TimerMode::Once)));
            commands.insert_resource(crate::ui::tutorial_ui::TutorialReady);
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
                    custom_size: Some(Vec2::new(53., 18.)),
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

pub fn update_achievements_notification_icon(
    achievements: Res<Achievements>,
    achievements_button: Query<Entity, With<AchievementsButton>>,
    notification_icon: Query<Entity, With<AchievementsNotificationIcon>>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    let Ok(button_entity) = achievements_button.get_single() else {
        return;
    };

    let has_unclaimed = achievements.has_unclaimed_completed();

    // Check if notification icon exists
    if let Ok(icon_entity) = notification_icon.get_single() {
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
            Vec3::new(55., -1., 1.), // Position relative to button
            button_entity,
            999999.0, // Very long duration
        );
        commands
            .entity(icon)
            .insert(AchievementsNotificationIcon)
            .insert(RenderLayers::from_layers(&[3]));
    }
}

#[derive(Default)]
pub struct CleanUpRunStateEvent;

pub fn cleanup_run_state(
    event: EventReader<CleanUpRunStateEvent>,
    mut commands: Commands,
    world_entities: Query<
        Entity,
        (
            Or<(With<Visibility>, With<ActiveDimension>, With<Collider>)>,
            Without<DoNotDespawnOnGameOver>,
        ),
    >,
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
    info!("Cleaning up ALL run data on GameState::Main exit");

    for entity in boss_health_bars.iter() {
        commands.entity(entity).despawn_recursive();
    }
    for entity in guide_hud.iter() {
        commands.entity(entity).despawn_recursive();
    }

    for e in world_entities.iter() {
        if let Some(entity_commands) = commands.get_entity(e) {
            entity_commands.despawn_recursive();
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
    commands.insert_resource(EssenceShopCache::default());
    commands.insert_resource(FogOfWarData::default());
    commands.insert_resource(MinimapTileCache::default());
    commands.insert_resource(crate::item::boss_shrine::BossSummonTracker::default());
    commands.insert_resource(BossKillTracker::default());

    // Reset Rapier physics world to free accumulated internal arena allocations
    commands.insert_resource(RapierContext::default());
}
