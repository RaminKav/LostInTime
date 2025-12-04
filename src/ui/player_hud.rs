use bevy::{prelude::*, render::view::RenderLayers, sprite::Anchor};
use bevy_aseprite::{anim::AsepriteAnimation, aseprite, Aseprite, AsepriteBundle};
use std::collections::HashMap;

use super::{
    damage_numbers::spawn_text, interactions::Interaction, spawn_inv_slot, spawn_item_stack_icon,
    InventorySlotType, InventoryState, InventoryUI, UIElement, UIState,
};
use crate::{
    assets::Graphics,
    attributes::{
        hunger::Hunger, CurrentHealth, CurrentMana, CurrentShield, MaxHealth, MaxMana, MaxShield,
    },
    audio::{AudioSoundEffect, SoundSpawner},
    client::GameOverEvent,
    colors::{BLACK, BLUE, RED, SHIELD_BLUE, WHITE, YELLOW},
    inventory::{Inventory, ItemStack},
    item::WorldObject,
    juice::bounce::BounceOnHit,
    night::NightTracker,
    player::{
        levels::PlayerLevel,
        skills::{ActiveSkillUsedEvent, Heirloom, HeirloomRarity, PlayerSkills},
        CoinCurrency, Player, RunScore, TimeFragmentCurrency,
    },
    GameState, ScreenResolution, GAME_HEIGHT,
};
use bevy::utils::Duration;
aseprite!(pub Clock, "ui/Clock.aseprite");

#[derive(Component)]
pub struct HealthBar;
#[derive(Component)]
pub struct ShieldBar;
#[derive(Component)]
pub struct FoodBar;
#[derive(Component)]
pub struct ManaBar;
#[derive(Component)]
pub struct XPBar;
#[derive(Component)]
pub struct XPBarText;
#[derive(Component)]
pub struct CurrencyText;
#[derive(Component)]
pub struct TimeFragmentText;
#[derive(Component)]
pub struct TimeFragmentIcon;
#[derive(Component)]
pub struct CoinIcon;
#[derive(Component)]
pub struct CoinText;
#[derive(Component)]
pub struct ScoreText;

#[derive(Component)]
pub struct ClockHUD;
#[derive(Component)]
pub struct ClockText;

#[derive(Component)]
pub struct ActiveSkillIcon;

#[derive(Component)]
pub struct ActiveSkillKeybindText {
    pub slot: usize,
}

#[derive(Component)]
pub struct ActiveSkillKeyBackground {
    pub slot: usize,
}

/// Helper function to determine key size and UI element based on KeyCode
fn get_key_size_and_element(key: KeyCode) -> (UIElement, f32) {
    match key {
        // Large keys (Space, Enter, etc.)
        KeyCode::Space => (UIElement::LargeKey, 30.0),
        KeyCode::Return => (UIElement::LargeKey, 30.0),
        KeyCode::Escape => (UIElement::LargeKey, 30.0),

        // Medium keys (Shift, Ctrl, Alt, Tab, Caps, etc.)
        KeyCode::LShift | KeyCode::RShift => (UIElement::MediumKey, 26.0),
        KeyCode::LControl | KeyCode::RControl => (UIElement::MediumKey, 26.0),
        KeyCode::LAlt | KeyCode::RAlt => (UIElement::MediumKey, 26.0),
        KeyCode::Tab => (UIElement::MediumKey, 26.0),
        KeyCode::Capital => (UIElement::MediumKey, 26.0),
        KeyCode::Back => (UIElement::MediumKey, 26.0),

        // Small keys (all single character keys, numbers, etc.)
        _ => (UIElement::SmallKey, 10.0),
    }
}

#[derive(Component)]
pub struct SkillChargeText {
    pub slot: usize, // Which skill slot this text is for (1 or 2)
}

const INNER_HUD_BAR_SIZE: Vec2 = Vec2::new(65.0, 3.0);

#[derive(Component)]
pub struct BarFlashTimer {
    pub timer: Timer,
    pub flash_color: Color,
    pub color: Color,
}
#[derive(Default)]
pub struct FlashExpBarEvent {
    pub amount: u32,
    pub did_level: bool,
}

pub fn setup_bars_ui(mut commands: Commands, graphics: Res<Graphics>, res: Res<ScreenResolution>) {
    let hud_bar_frame = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::PlayerHUDBars),

            sprite: Sprite {
                custom_size: Some(Vec2::new(84.5, 48.)),
                ..Default::default()
            },
            transform: Transform {
                translation: Vec3::new(
                    (-res.game_width + 91.) / 2.,
                    (GAME_HEIGHT - 15.) / 2. - 19.5,
                    5.,
                ),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(Name::new("HUD FRAME"))
        .insert(RenderLayers::from_layers(&[3]))
        .id();
    let inner_health = commands
        .spawn(SpriteBundle {
            sprite: Sprite {
                color: RED,
                custom_size: Some(INNER_HUD_BAR_SIZE),
                anchor: Anchor::CenterLeft,
                ..default()
            },
            transform: Transform {
                translation: Vec3::new(-25., 17., -2.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        })
        .insert(BarFlashTimer {
            timer: Timer::from_seconds(0.1, TimerMode::Once),
            flash_color: WHITE,
            color: RED,
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(HealthBar)
        .insert(Name::new("inner health bar"))
        .id();
    let inner_shield = commands
        .spawn(SpriteBundle {
            sprite: Sprite {
                color: SHIELD_BLUE,
                custom_size: Some(INNER_HUD_BAR_SIZE),
                anchor: Anchor::CenterLeft,
                ..default()
            },
            transform: Transform {
                translation: Vec3::new(-25., 17., -1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        })
        .insert(BarFlashTimer {
            timer: Timer::from_seconds(0.1, TimerMode::Once),
            flash_color: WHITE,
            color: SHIELD_BLUE,
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(ShieldBar)
        .insert(Name::new("inner shield bar"))
        .id();
    let inner_mana = commands
        .spawn(SpriteBundle {
            sprite: Sprite {
                color: BLUE,
                custom_size: Some(INNER_HUD_BAR_SIZE),
                anchor: Anchor::CenterLeft,
                ..default()
            },
            transform: Transform {
                translation: Vec3::new(-25., 9., -1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        })
        .insert(BarFlashTimer {
            timer: Timer::from_seconds(0.2, TimerMode::Once),
            flash_color: WHITE,
            color: BLUE,
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(ManaBar)
        .insert(Name::new("inner mana bar"))
        .id();
    let inner_food = commands
        .spawn(SpriteBundle {
            sprite: Sprite {
                color: YELLOW,
                custom_size: Some(INNER_HUD_BAR_SIZE),
                anchor: Anchor::CenterLeft,
                ..default()
            },
            transform: Transform {
                translation: Vec3::new(-25., 1., -1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        })
        .insert(BarFlashTimer {
            timer: Timer::from_seconds(0.2, TimerMode::Once),
            flash_color: WHITE,
            color: YELLOW,
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(FoodBar)
        .insert(Name::new("inner food bar"))
        .id();

    commands.entity(hud_bar_frame).push_children(&[
        inner_health,
        inner_food,
        inner_mana,
        inner_shield,
    ]);
}

pub fn setup_xp_bar_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
) {
    let inner_xp_prog = commands
        .spawn(SpriteBundle {
            sprite: Sprite {
                color: YELLOW,
                custom_size: Some(Vec2::new(0., 1.)), // Initialize to 0 width (0 XP at start)
                anchor: Anchor::CenterLeft,
                ..default()
            },
            transform: Transform {
                translation: Vec3::new(-111. / 2., -6., -1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        })
        .insert(BarFlashTimer {
            timer: Timer::from_seconds(0.2, TimerMode::Once),
            flash_color: WHITE,
            color: YELLOW,
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(XPBar)
        .insert(Name::new("inner xp bar"))
        .id();
    let xp_bar_frame = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::XPBarFrame),

            sprite: Sprite {
                custom_size: Some(Vec2::new(119.5, 24.)),
                ..Default::default()
            },
            transform: Transform {
                translation: Vec3::new(10., -GAME_HEIGHT / 2. + 34., 5.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(Name::new("XP BAR"))
        .insert(RenderLayers::from_layers(&[3]))
        .id();
    let text = commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    "1",
                    TextStyle {
                        font: asset_server.load("fonts/4x5.ttf"),
                        font_size: 5.0,
                        color: BLACK,
                    },
                ),
                text_anchor: Anchor::CenterLeft,
                transform: Transform {
                    translation: Vec3::new(-3., 3.5, 1.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..default()
            },
            Name::new("XP TEXT"),
            XPBarText,
            RenderLayers::from_layers(&[3]),
        ))
        .id();
    commands
        .entity(xp_bar_frame)
        .push_children(&[inner_xp_prog, text]);
}
pub fn setup_currency_ui(
    mut commands: Commands,
    currency: Res<TimeFragmentCurrency>,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    res: Res<ScreenResolution>,
    coins: Res<CoinCurrency>,
) {
    let time_fragments = currency.as_ref();
    let text = commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    format!("{:}", time_fragments.time_fragments.max(0)),
                    TextStyle {
                        font: asset_server.load("fonts/4x5.ttf"),
                        font_size: 5.0,
                        color: BLACK,
                    },
                ),
                text_anchor: Anchor::CenterLeft,
                transform: Transform {
                    translation: Vec3::new(
                        -res.game_width / 2. + 16.5,
                        GAME_HEIGHT / 2. - 43.5,
                        6.,
                    ),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..default()
            },
            Name::new("TIME FRAGMENTS TEXT"),
            CurrencyText,
            TimeFragmentText,
            RenderLayers::from_layers(&[3]),
        ))
        .id();
    let stack = spawn_item_stack_icon(
        &mut commands,
        &graphics,
        &ItemStack::crate_icon_stack(WorldObject::TimeFragment),
        &asset_server,
        Vec2::new(-6.5, 1.),
        Vec2::new(0., 0.),
        3,
    );
    commands
        .entity(stack)
        .insert(TimeFragmentIcon)
        .set_parent(text);

    let coin_text = commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    format!("{:}", coins.coins),
                    TextStyle {
                        font: asset_server.load("fonts/4x5.ttf"),
                        font_size: 5.0,
                        color: BLACK,
                    },
                ),
                text_anchor: Anchor::CenterLeft,
                transform: Transform {
                    translation: Vec3::new(-res.game_width / 2. + 46., GAME_HEIGHT / 2. - 43.5, 6.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..default()
            },
            Name::new("COIN TEXT"),
            CurrencyText,
            CoinText,
            RenderLayers::from_layers(&[3]),
        ))
        .id();
    let coin_stack = spawn_item_stack_icon(
        &mut commands,
        &graphics,
        &ItemStack::crate_icon_stack(WorldObject::Coin),
        &asset_server,
        Vec2::new(-6., 0.5),
        Vec2::new(0., 0.),
        3,
    );
    commands
        .entity(coin_stack)
        .insert(CoinIcon)
        .set_parent(coin_text);

    // SCORE TEXT
    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                format!("Score: {:}", 0),
                TextStyle {
                    font: asset_server.load("fonts/4x5.ttf"),
                    font_size: 5.0,
                    color: BLACK,
                },
            ),
            text_anchor: Anchor::CenterLeft,
            transform: Transform {
                translation: Vec3::new(-res.game_width / 2. + 64., GAME_HEIGHT / 2. - 43.5, 6.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        },
        Name::new("SCORE TEXT"),
        ScoreText,
        RenderLayers::from_layers(&[3]),
    ));

    // INVENTORY ICON
    let bag_icon = spawn_item_stack_icon(
        &mut commands,
        &graphics,
        &ItemStack::crate_icon_stack(WorldObject::InventoryBag),
        &asset_server,
        Vec2::new(86.5, -GAME_HEIGHT / 2. + 10.),
        Vec2::new(0., 0.),
        3,
    );
    commands
        .spawn(SpriteBundle {
            texture: asset_server.load("textures/EKey.png"),
            transform: Transform::from_translation(Vec3::new(-0.5, 13., 1.)),
            sprite: Sprite {
                custom_size: Some(Vec2::new(10., 10.)),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .set_parent(bag_icon);
}

pub fn update_currency_text(
    time_fragments: Res<TimeFragmentCurrency>,
    coins: Res<CoinCurrency>,
    mut time_fragment_text_query: Query<&mut Text, (With<TimeFragmentText>, Without<CoinText>)>,
    mut coin_text_query: Query<&mut Text, (With<CoinText>, Without<TimeFragmentText>)>,
    time_fragment_icon: Query<Entity, (With<TimeFragmentIcon>, Without<CoinIcon>)>,
    coin_icon: Query<Entity, (With<CoinIcon>, Without<TimeFragmentIcon>)>,
    mut commands: Commands,
    game_state: Res<State<GameState>>,
) {
    if time_fragments.is_changed() {
        if game_state.0 != GameState::GameOver {
            if let Ok(icon_e) = time_fragment_icon.get_single() {
                // Check if entity still exists before inserting components
                if let Some(mut entity_commands) = commands.get_entity(icon_e) {
                    entity_commands.insert(BounceOnHit::new());
                }
            }
        }

        for mut text in time_fragment_text_query.iter_mut() {
            text.sections[0].value = format!("{}", time_fragments.time_fragments.max(0));
        }
    }

    if coins.is_changed() {
        if game_state.0 != GameState::GameOver {
            if let Ok(icon_e) = coin_icon.get_single() {
                // Check if entity still exists before inserting components
                if let Some(mut entity_commands) = commands.get_entity(icon_e) {
                    entity_commands.insert(BounceOnHit::new());
                }
            }
        }

        for mut text in coin_text_query.iter_mut() {
            text.sections[0].value = format!("{}", coins.coins);
        }
    }
}
pub fn update_score_text(score: Res<RunScore>, mut text_query: Query<&mut Text, With<ScoreText>>) {
    // handles different text for two different UI elements, game end count and normal in-game
    for mut text in text_query.iter_mut() {
        text.sections[0].value = format!("Score: {:}", score.score);
    }
}
pub fn update_healthbar(
    player_health_query: Query<
        (&CurrentHealth, &MaxHealth),
        (
            Or<(Changed<CurrentHealth>, Changed<MaxHealth>)>,
            With<Player>,
        ),
    >,
    mut health_bar_query: Query<(&mut Sprite, &mut BarFlashTimer), With<HealthBar>>,
) {
    let Ok((player_health, player_max_health)) = player_health_query.get_single() else {
        return;
    };
    let (mut sprite, mut flash) = health_bar_query.single_mut();
    sprite.custom_size = Some(Vec2 {
        x: 65. * player_health.0 as f32 / player_max_health.0 as f32,
        y: INNER_HUD_BAR_SIZE.y,
    });
    flash.timer.tick(Duration::from_nanos(1));
}
pub fn update_shieldbar(
    player_health_query: Query<
        (&CurrentShield, &MaxShield),
        (
            Or<(Changed<CurrentShield>, Changed<MaxShield>)>,
            With<Player>,
        ),
    >,
    mut health_bar_query: Query<(&mut Sprite, &mut BarFlashTimer), With<ShieldBar>>,
) {
    let Ok((curr_shield, max_shield)) = player_health_query.get_single() else {
        return;
    };
    let (mut sprite, mut flash) = health_bar_query.single_mut();
    sprite.custom_size = Some(Vec2 {
        x: 65. * curr_shield.0 as f32 / max_shield.0 as f32,
        y: INNER_HUD_BAR_SIZE.y,
    });

    flash.timer.tick(Duration::from_nanos(1));
}
pub fn update_xp_bar(
    player_xp_query: Query<&PlayerLevel, With<Player>>,
    mut xp_bar_query: Query<(&mut Sprite, &mut BarFlashTimer), With<XPBar>>,
    mut xp_bar_text_query: Query<(&mut Text, &mut Transform), With<XPBarText>>,
    mut flash_event: EventReader<FlashExpBarEvent>,
    mut commands: Commands,
) {
    for event in flash_event.iter() {
        let level = player_xp_query.single();

        let (mut sprite, mut flash) = xp_bar_query.single_mut();
        sprite.custom_size = Some(Vec2 {
            x: 111. * level.xp as f32 / level.next_level_xp as f32,
            y: 1.,
        });
        let (mut text, mut txfm) = xp_bar_text_query.single_mut();
        text.sections[0].value = format!("{:}", level.level);
        if level.level >= 10 {
            txfm.translation.x = -5.5;
        }
        flash.timer.tick(Duration::from_nanos(1));
        if event.did_level {
            commands.spawn(SoundSpawner::new(AudioSoundEffect::LevelUp, 0.35));
        }
        if event.amount >= 50 {
            commands.spawn(SoundSpawner::new(AudioSoundEffect::GainExp, 0.08));
            commands.spawn(SoundSpawner::new(AudioSoundEffect::GainExp, 0.08).with_delay(0.15));
            commands.spawn(SoundSpawner::new(AudioSoundEffect::GainExp, 0.08).with_delay(0.22));
        } else if event.amount >= 10 {
            commands.spawn(SoundSpawner::new(AudioSoundEffect::GainExp, 0.08));
            commands.spawn(SoundSpawner::new(AudioSoundEffect::GainExp, 0.08).with_delay(0.15));
        } else {
            commands.spawn(SoundSpawner::new(AudioSoundEffect::GainExp, 0.08));
        }
    }
}

pub fn handle_flash_bars(mut query: Query<(&mut Sprite, &mut BarFlashTimer)>, time: Res<Time>) {
    for (mut sprite, mut flash) in query.iter_mut() {
        if flash.timer.finished() {
            sprite.color = flash.color;
            flash.timer.reset();
        } else if flash.timer.percent() != 0. {
            sprite.color = WHITE;
            flash.timer.tick(time.delta());
        }
    }
}
pub fn update_foodbar(
    player_hunger_query: Query<&Hunger, (With<Player>, Changed<Hunger>)>,
    mut food_bar_query: Query<(&mut Sprite, &mut BarFlashTimer), With<FoodBar>>,
) {
    let Ok(hunger) = player_hunger_query.get_single() else {
        return;
    };
    let (mut sprite, mut flash) = food_bar_query.single_mut();
    sprite.custom_size = Some(Vec2 {
        x: 53. * hunger.current as f32 / hunger.max as f32,
        y: INNER_HUD_BAR_SIZE.y,
    });
    flash.timer.tick(Duration::from_nanos(1));
}

#[derive(Component, Eq, PartialEq)]
pub struct SkillHudIcon(pub Heirloom);

#[derive(Component)]
pub struct HeirloomCounterText;

pub fn handle_update_player_skills(
    player_skills: Query<&PlayerSkills, Changed<PlayerSkills>>,
    mut commands: Commands,
    graphics: Res<Graphics>,
    mut prev_icons_tracker: Local<Vec<(Heirloom, i32)>>, // Track (heirloom, count) pairs
    res: Res<ScreenResolution>,
    // mut skill_class_text: Query<&mut Text, With<SkillClassText>>,
    game_over: EventReader<GameOverEvent>,
    asset_server: Res<AssetServer>,
    prev_active_skill_icons: Query<Entity, With<ActiveSkillIcon>>,
    existing_heirloom_icons: Query<(Entity, &SkillHudIcon)>, // Query existing heirloom icons
    _counter_texts: Query<&mut Text, With<HeirloomCounterText>>, // Query counter texts to update
    existing_cooldown_overlays: Query<(Entity, &SkillCooldownOverlay)>, // Query existing cooldown overlays to preserve state
    unlock_upgrades: Option<Res<crate::player::unlocks::UnlockUpgrades>>,
    keybinds: Res<crate::keybinds::KeyBindings>,
) {
    if !game_over.is_empty() {
        prev_icons_tracker.clear();
    }

    if let Ok(new_skills) = player_skills.get_single() {
        // Group heirlooms by type and count them
        let mut heirloom_counts: HashMap<Heirloom, (i32, HeirloomRarity)> = HashMap::new();

        for heirloom_with_rarity in &new_skills.heirlooms {
            let entry = heirloom_counts
                .entry(heirloom_with_rarity.heirloom.clone())
                .or_insert((0, heirloom_with_rarity.rarity.clone()));
            entry.0 += 1;
        }

        // Check if we need to update icons (compare with previous state)
        let current_state: Vec<(Heirloom, i32)> = heirloom_counts
            .iter()
            .map(|(heirloom, (count, _))| (heirloom.clone(), *count))
            .collect();

        let needs_update = prev_icons_tracker.len() != current_state.len()
            || prev_icons_tracker
                .iter()
                .zip(current_state.iter())
                .any(|(prev, curr)| prev != curr);

        if needs_update {
            // Despawn existing icons only when we need to update
            existing_heirloom_icons.for_each(|(e, _)| {
                commands.entity(e).despawn_recursive();
            });

            // Create a consolidated list of heirlooms in the correct order
            let mut ordered_heirlooms: Vec<(Heirloom, i32)> = Vec::new();

            // First, add existing heirlooms in their original order
            for (heirloom, _prev_count) in &prev_icons_tracker {
                if let Some((count, _)) = heirloom_counts.get(heirloom) {
                    ordered_heirlooms.push((heirloom.clone(), *count));
                }
            }

            // Then, add new heirlooms in sorted order
            let mut new_heirlooms: Vec<_> = heirloom_counts
                .iter()
                .filter(|(heirloom, _)| {
                    !prev_icons_tracker
                        .iter()
                        .any(|(prev_heirloom, _)| prev_heirloom == *heirloom)
                })
                .collect();

            // Sort by heirloom enum order for consistent positioning
            new_heirlooms.sort_by(|a, b| {
                // Convert heirlooms to strings and compare for consistent ordering
                format!("{:?}", a.0).cmp(&format!("{:?}", b.0))
            });

            for (heirloom, (count, _)) in new_heirlooms {
                ordered_heirlooms.push((heirloom.clone(), *count));
            }

            // Spawn all icons in one consolidated loop
            for (i, (heirloom, count)) in ordered_heirlooms.iter().enumerate() {
                const MAX_ICONS_PER_ROW: usize = 18;
                const ICON_SPACING: f32 = 16.;
                const ROW_SPACING: f32 = 16.;

                let row = i / MAX_ICONS_PER_ROW;
                let col = i % MAX_ICONS_PER_ROW;

                let offset = Vec2::new(
                    col as f32 * ICON_SPACING + (-res.game_width) / 2. + 98.,
                    (GAME_HEIGHT - 15.) / 2. - 2.5 - (row as f32 * ROW_SPACING),
                );

                // Create the main icon
                let icon = commands
                    .spawn(SpriteSheetBundle {
                        sprite: graphics.get_heirloom_icon(heirloom.clone()),
                        texture_atlas: graphics.texture_atlas.as_ref().unwrap().clone(),
                        transform: Transform {
                            translation: offset.extend(1.),
                            scale: Vec3::new(1., 1., 1.),
                            ..Default::default()
                        },
                        ..Default::default()
                    })
                    .insert(RenderLayers::from_layers(&[3]))
                    .insert(SkillHudIcon(heirloom.clone()))
                    .insert(Name::new("HUD ICON!!"))
                    .id();

                // Add counter text if count > 1
                if *count > 1 {
                    let _counter_text = commands
                        .spawn(Text2dBundle {
                            text: Text::from_section(
                                count.to_string(),
                                TextStyle {
                                    font: asset_server.load("fonts/4x5.ttf"),
                                    font_size: 5.0,
                                    color: BLACK,
                                },
                            ),
                            text_anchor: Anchor::BottomRight,
                            transform: Transform {
                                translation: Vec3::new(8., -8., 2.), // Bottom right of icon
                                scale: Vec3::new(1., 1., 1.),
                                ..Default::default()
                            },
                            ..default()
                        })
                        .insert(RenderLayers::from_layers(&[3]))
                        .insert(HeirloomCounterText)
                        .insert(Name::new("HEIRLOOM COUNTER"))
                        .set_parent(icon)
                        .id();
                }
            }

            // Update the tracker with the new stable order
            prev_icons_tracker.clear();
            prev_icons_tracker.extend(ordered_heirlooms);
        }

        // let mut text = skill_class_text.single_mut();
        // text.sections[0].value = format!(
        //     "  {:}     {:?}     {:?}",
        //     0, // melee_skill_count - removed
        //     0, // rogue_skill_count - removed
        //     0  // magic_skill_count - removed
        // );

        // Active Skill Icons
        // Preserve cooldown overlay state before despawning
        let mut preserved_cooldowns: Vec<(usize, f32, f32)> = Vec::new(); // (index, elapsed, duration)
        for (_, overlay) in existing_cooldown_overlays.iter() {
            let elapsed = overlay.timer.elapsed().as_secs_f32();
            let duration = overlay.timer.duration().as_secs_f32();
            if duration > 0.0 && elapsed < duration {
                // Only preserve if there's an active cooldown
                preserved_cooldowns.push((overlay.index, elapsed, duration));
            }
        }

        prev_active_skill_icons.for_each(|e| {
            commands.entity(e).despawn_recursive();
        });

        // Build list of active skill slots to display
        let mut active_skill_slots = vec![
            new_skills.active_skill_slot_1.clone(),
            new_skills.active_skill_slot_2.clone(),
        ];

        // Add third slot if unlocked
        if let Some(upgrades) = unlock_upgrades.as_ref() {
            if upgrades.third_active_skill_slot_unlocked {
                active_skill_slots.push(new_skills.active_skill_slot_3.clone());
            }
        }

        for (i, active_skill_option) in active_skill_slots.iter().enumerate() {
            let icon_bg = commands
                .spawn(SpriteBundle {
                    texture: graphics.get_ui_element_texture(UIElement::ScreenIconSlotLarge),
                    sprite: Sprite {
                        custom_size: Some(Vec2::new(20., 20.)),
                        ..default()
                    },
                    transform: Transform {
                        translation: Vec3::new(
                            -res.game_width / 2. + 18. + i as f32 * 31.,
                            -GAME_HEIGHT / 2. + 14.,
                            1.,
                        ),
                        scale: Vec3::new(1., 1., 1.),
                        ..Default::default()
                    },
                    ..default()
                })
                .insert(RenderLayers::from_layers(&[3]))
                .insert(ActiveSkillIcon)
                .id();
            // Get the actual keybind for this slot
            let keybind = keybinds.get_active_skill_key(i);
            let (key_element, key_width) = get_key_size_and_element(keybind);

            // Spawn generic key background
            let key_bg = commands
                .spawn(SpriteBundle {
                    texture: graphics.get_ui_element_texture(key_element),
                    transform: Transform::from_translation(Vec3::new(0., 13., 2. + i as f32)),
                    sprite: Sprite {
                        custom_size: Some(Vec2::new(key_width, 10.)),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .insert(RenderLayers::from_layers(&[3]))
                .insert(ActiveSkillKeyBackground { slot: i })
                .set_parent(icon_bg)
                .id();

            // Spawn keybind text as child of key background
            commands
                .spawn(Text2dBundle {
                    text: Text::from_section(
                        crate::keybinds::get_key_display_name(keybind),
                        TextStyle {
                            font: asset_server.load("fonts/4x5.ttf"),
                            font_size: 5.0,
                            color: crate::colors::DARK_WOOD_BROWN,
                        },
                    )
                    .with_alignment(TextAlignment::Center),
                    text_anchor: bevy::sprite::Anchor::Center,
                    transform: Transform::from_translation(Vec3::new(1., 0., 1.)),
                    ..Default::default()
                })
                .insert(RenderLayers::from_layers(&[3]))
                .insert(ActiveSkillKeybindText { slot: i })
                .set_parent(key_bg);
            if let Some(active_skill) = active_skill_option.clone() {
                commands
                    .spawn(SpriteBundle {
                        texture: graphics.get_active_skill_icon(active_skill.active_skill.clone()),
                        sprite: Sprite {
                            custom_size: Some(Vec2::new(16., 16.)),
                            ..Default::default()
                        },
                        transform: Transform {
                            translation: Vec3::new(0., 0., 1.),
                            scale: Vec3::new(1., 1., 1.),
                            ..Default::default()
                        },
                        ..Default::default()
                    })
                    .insert(RenderLayers::from_layers(&[3]))
                    .insert(ActiveSkillIcon)
                    .insert(Name::new("HUD ICON!!"))
                    .set_parent(icon_bg);
            }

            // Preserve cooldown state if it exists for this slot
            if let Some((_, elapsed, original_duration)) =
                preserved_cooldowns.iter().find(|(idx, _, _)| *idx == i)
            {
                // Apply cooldown multiplier to get the new remaining time
                let multiplier = new_skills.skill_cooldown_multiplier();
                // Calculate what percentage of the original cooldown was elapsed
                let progress_percent = if *original_duration > 0.0 {
                    *elapsed / *original_duration
                } else {
                    0.0
                };
                // Calculate the new duration and elapsed time based on the multiplier
                // The multiplier reduces the total cooldown, so we scale both duration and elapsed proportionally
                let new_duration = *original_duration * multiplier;
                let new_elapsed = new_duration * progress_percent;

                spawn_skill_cooldown_overlay_with_elapsed(
                    icon_bg,
                    &mut commands,
                    new_duration,
                    new_elapsed,
                    i,
                );
            } else {
                spawn_skill_cooldown_overlay(icon_bg, &mut commands, 0.0, i);
            }

            // For slots 1 and 2 (class skills), add charge count text (both share the charge system)
            if i == 1 || i == 2 {
                // Query for charge tracker to get current charges
                // We'll update this in a separate system that runs after this
                let _charge_text = commands
                    .spawn(Text2dBundle {
                        text: Text::from_section(
                            "",
                            TextStyle {
                                font: asset_server.load("fonts/4x5.ttf"),
                                font_size: 10.0,
                                color: WHITE,
                            },
                        ),
                        text_anchor: Anchor::Center,
                        transform: Transform {
                            translation: Vec3::new(1., -6., 4.), // Center bottom of icon
                            scale: Vec3::new(1., 1., 1.),
                            ..Default::default()
                        },
                        ..default()
                    })
                    .insert(RenderLayers::from_layers(&[3]))
                    .insert(SkillChargeText { slot: i })
                    .insert(Name::new("SKILL CHARGE TEXT"))
                    .set_parent(icon_bg)
                    .id();
            }
        }
    }
}

/// Updates skill charge text display for slot 1
pub fn update_skill_charge_text(
    slot1_trackers: Query<&crate::player::skills::Slot1ChargeTracker, With<Player>>,
    slot2_trackers: Query<&crate::player::skills::Slot2ChargeTracker, With<Player>>,
    mut charge_texts: Query<(&SkillChargeText, &mut Text)>,
) {
    for (charge_text, mut text) in charge_texts.iter_mut() {
        // Get the tracker for this text's slot
        let tracker_opt = match charge_text.slot {
            1 => slot1_trackers.get_single().ok().map(|t| &t.0),
            2 => slot2_trackers.get_single().ok().map(|t| &t.0),
            _ => None,
        };

        if let Some(tracker) = tracker_opt {
            // Only show text if max charges > 1
            if tracker.max_charges > 1 {
                text.sections[0].value = format!("{}", tracker.current_charges);
            } else {
                text.sections[0].value = String::new();
            }
        } else {
            // No tracker for this slot, hide text
            text.sections[0].value = String::new();
        }
    }
}

pub fn setup_hotbar_hud(
    mut commands: Commands,
    graphics: Res<Graphics>,
    inv_query: Query<Entity, With<InventoryUI>>,
    inv_state: Res<InventoryState>,
    asset_server: Res<AssetServer>,
    mut inv: Query<&mut Inventory>,
    inv_ui_state: Res<State<UIState>>,
) {
    for (slot_index, item) in inv.single_mut().items.items.iter().enumerate() {
        // hotbar slots
        if slot_index <= 5 {
            spawn_inv_slot(
                &mut commands,
                &inv_ui_state,
                &graphics,
                slot_index,
                Interaction::None,
                &inv_state,
                &inv_query,
                &asset_server,
                InventorySlotType::Hotbar,
                item.clone(),
            );
        }
    }
}

pub fn update_mana_bar(
    player_mana: Query<(&CurrentMana, &MaxMana), (With<Player>, Changed<CurrentMana>)>,
    mut mana_bar_query: Query<(&mut Sprite, &mut BarFlashTimer), With<ManaBar>>,
) {
    let Ok((current_mana, max_mana)) = player_mana.get_single() else {
        return;
    };
    let (mut sprite, mut flash) = mana_bar_query.single_mut();
    sprite.custom_size = Some(Vec2 {
        x: 60. * current_mana.0 as f32 / max_mana.0 as f32,
        y: INNER_HUD_BAR_SIZE.y,
    });
    flash.timer.tick(Duration::from_nanos(1));
}

pub fn setup_clock_hud(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    night_tracker: Res<NightTracker>,
    res: Res<ScreenResolution>,
) {
    let clock_hud_frame = commands
        .spawn(AsepriteBundle {
            animation: AsepriteAnimation::from(Clock::tags::ONE),
            aseprite: asset_server.load::<Aseprite, _>(Clock::PATH),
            transform: Transform {
                translation: Vec3::new(
                    -res.game_width / 2. + 17.5,
                    (GAME_HEIGHT - 15.) / 2. - 68.5,
                    6.,
                ),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(Name::new("CLOCK HUD"))
        .insert(RenderLayers::from_layers(&[3]))
        .insert(ClockHUD)
        .id();
    let text = spawn_text(
        &mut commands,
        &asset_server,
        Vec3::new(10.5, -8., 1.),
        BLACK,
        format!("{}:00", night_tracker.get_hour()),
        Anchor::CenterRight,
        1.,
        3,
    );
    commands
        .entity(text)
        .insert(ClockText)
        .set_parent(clock_hud_frame);
}

pub fn handle_update_clock_hud(
    night_tracker: Res<NightTracker>,
    mut clock_text: Query<&mut Text, With<ClockText>>,
    mut clock_anim: Query<Entity, With<ClockHUD>>,
    mut commands: Commands,
) {
    let hour = night_tracker.get_hour();
    let anim = match hour {
        0 | 1 => Clock::tags::ONE,
        2 | 3 => Clock::tags::TWO,
        4 | 5 => Clock::tags::THREE,
        6 | 7 => Clock::tags::FOUR,
        8 | 9 => Clock::tags::FIVE,
        10 | 11 => Clock::tags::SIX,
        12 | 13 => Clock::tags::SEVEN,
        14 | 15 => Clock::tags::EIGHT,
        16 | 17 => Clock::tags::NINE,
        18 | 19 => Clock::tags::TEN,
        20 | 21 => Clock::tags::ELEVEN,
        22 | 23 => Clock::tags::TWELVE,
        i => unreachable!("Invalid hour: {}", i),
    };
    for e in clock_anim.iter_mut() {
        commands.entity(e).insert(AsepriteAnimation::from(anim));
    }
    let mut text = clock_text.single_mut();
    text.sections[0].value = format!("{}:00", if hour > 12 { hour - 12 } else { hour });
}

#[derive(Component)]
pub struct SkillCooldownOverlay {
    pub timer: Timer,
    pub index: usize,
}

pub fn spawn_skill_cooldown_overlay(
    parent: Entity,
    commands: &mut Commands,
    duration: f32,
    index: usize,
) -> Entity {
    spawn_skill_cooldown_overlay_with_elapsed(parent, commands, duration, 0.0, index)
}

pub fn spawn_skill_cooldown_overlay_with_elapsed(
    parent: Entity,
    commands: &mut Commands,
    duration: f32,
    elapsed: f32,
    index: usize,
) -> Entity {
    use bevy::utils::Duration;
    let mut timer = Timer::from_seconds(duration, TimerMode::Once);
    // Tick the timer to the preserved elapsed time to maintain visual state
    if elapsed > 0.0 && duration > 0.0 {
        timer.tick(Duration::from_secs_f32(elapsed));
    }

    // Calculate initial overlay size based on timer progress
    let initial_size = if duration > 0.0 {
        16.0 * (1.0 - timer.percent())
    } else {
        0.0
    };

    commands
        .spawn(SpriteBundle {
            sprite: Sprite {
                color: Color::rgba(1., 1., 1., 0.45),
                custom_size: Some(Vec2::new(16., initial_size)),
                anchor: Anchor::BottomCenter,
                ..default()
            },
            transform: Transform {
                translation: Vec3::new(0., -8., 3.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        })
        .insert(SkillCooldownOverlay { timer, index })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("overlay"))
        .set_parent(parent)
        .id()
}

pub fn tick_skill_cooldown_overlays(
    mut overlays: Query<(&mut Sprite, &mut SkillCooldownOverlay), With<SkillCooldownOverlay>>,
    slot1_trackers: Query<&crate::player::skills::Slot1ChargeTracker, With<Player>>,
    slot2_trackers: Query<&crate::player::skills::Slot2ChargeTracker, With<Player>>,
    time: Res<Time>,
) {
    for (mut sprite, mut timer) in overlays.iter_mut() {
        // Each slot uses its own independent charge tracker
        let tracker_opt = match timer.index {
            1 => slot1_trackers.get_single().ok().map(|t| &t.0),
            2 => slot2_trackers.get_single().ok().map(|t| &t.0),
            _ => None,
        };

        if let Some(tracker) = tracker_opt {
            // This slot uses charge-based cooldown
            if tracker.current_charges < tracker.max_charges {
                // Show charge regeneration timer
                let elapsed = tracker.cooldown_timer.elapsed().as_secs_f32();
                let duration = tracker.cooldown_timer.duration().as_secs_f32();
                let percent = if duration > 0.0 {
                    elapsed / duration
                } else {
                    1.0
                };
                sprite.custom_size = Some(Vec2::new(16., 16. * (1.0 - percent)));
            } else {
                // Have max charges, hide overlay
                sprite.custom_size = Some(Vec2::new(16., 0.));
            }
        } else {
            // Use overlay's own independent timer (no charge system for this slot)
            timer.timer.tick(time.delta());
            sprite.custom_size = Some(Vec2::new(16., 16. * (1. - timer.timer.percent())));
        }
    }
}

pub fn handle_active_skill_event(
    mut active_skill_used: EventReader<ActiveSkillUsedEvent>,
    mut overlays: Query<&mut SkillCooldownOverlay>,
    slot1_trackers: Query<&crate::player::skills::Slot1ChargeTracker, With<Player>>,
    slot2_trackers: Query<&crate::player::skills::Slot2ChargeTracker, With<Player>>,
) {
    for e in active_skill_used.iter() {
        // Check if this slot has a charge tracker
        let has_tracker = match e.slot {
            1 => slot1_trackers.get_single().is_ok(),
            2 => slot2_trackers.get_single().is_ok(),
            _ => false,
        };

        for mut overlay in overlays.iter_mut() {
            if overlay.index == e.slot {
                if has_tracker {
                    // Charge tracker handles the cooldown, don't update overlay timer
                    // The charge regeneration timer is shown by tick_skill_cooldown_overlays
                    continue;
                }

                // Standard cooldown behavior for slots without charge trackers
                overlay.timer.reset();
                overlay
                    .timer
                    .set_duration(Duration::from_secs_f32(e.cooldown));
            }
        }
    }
}

pub fn update_active_skill_keybind_text(
    keybinds: Res<crate::keybinds::KeyBindings>,
    mut texts: Query<(&ActiveSkillKeybindText, &mut Text)>,
    mut key_backgrounds: Query<(&ActiveSkillKeyBackground, &mut Handle<Image>, &mut Sprite)>,
    graphics: Res<Graphics>,
) {
    if !keybinds.is_changed() {
        return;
    }

    // Update text
    for (keybind_text, mut text) in texts.iter_mut() {
        let key = keybinds.get_active_skill_key(keybind_text.slot);
        text.sections[0].value = crate::keybinds::get_key_display_name(key);
    }

    // Update key background size and texture
    for (key_bg, mut texture, mut sprite) in key_backgrounds.iter_mut() {
        let key = keybinds.get_active_skill_key(key_bg.slot);
        let (key_element, key_width) = get_key_size_and_element(key);
        *texture = graphics.get_ui_element_texture(key_element);
        sprite.custom_size = Some(Vec2::new(key_width, 10.));
    }
}
