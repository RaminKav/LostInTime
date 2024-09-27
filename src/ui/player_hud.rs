use bevy::{prelude::*, render::view::RenderLayers, sprite::Anchor};
use bevy_aseprite::{anim::AsepriteAnimation, aseprite, Aseprite, AsepriteBundle};

use super::{
    damage_numbers::spawn_text, interactions::Interaction, spawn_inv_slot, spawn_item_stack_icon,
    InventorySlotType, InventoryState, InventoryUI, UIElement, UIState,
};
use crate::{
    assets::Graphics,
    attributes::{hunger::Hunger, CurrentHealth, CurrentMana, MaxHealth, MaxMana},
    client::GameOverEvent,
    colors::{BLACK, BLUE, RED, WHITE, YELLOW},
    inventory::{Inventory, ItemStack},
    item::WorldObject,
    juice::bounce::BounceOnHit,
    night::NightTracker,
    player::{
        levels::PlayerLevel,
        skills::{PlayerSkills, Skill},
        Player, TimeFragmentCurrency,
    },
    ScreenResolution, GAME_HEIGHT,
};
use bevy::utils::Duration;
aseprite!(pub Clock, "ui/Clock.aseprite");

#[derive(Component)]
pub struct HealthBar;
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
pub struct ClockHUD;
#[derive(Component)]
pub struct ClockText;

const INNER_HUD_BAR_SIZE: Vec2 = Vec2::new(65.0, 3.0);

#[derive(Component)]
pub struct BarFlashTimer {
    pub timer: Timer,
    pub flash_color: Color,
    pub color: Color,
}
#[derive(Component)]
pub struct CurrencyIcon;
#[derive(Default)]
pub struct FlashExpBarEvent;

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
                translation: Vec3::new(-25., 17., -1.),
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

    commands
        .entity(hud_bar_frame)
        .push_children(&[inner_health, inner_food, inner_mana]);
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
                custom_size: Some(Vec2::new(111., 1.)),
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
                    "",
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
    currency: Query<&TimeFragmentCurrency>,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    res: Res<ScreenResolution>,
) {
    let time_fragments = currency.single();
    let text = commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    format!("{:}", time_fragments.time_fragments),
                    TextStyle {
                        font: asset_server.load("fonts/4x5.ttf"),
                        font_size: 5.0,
                        color: BLACK,
                    },
                ),
                text_anchor: Anchor::Center,
                transform: Transform {
                    translation: Vec3::new(-res.game_width / 2. + 22., GAME_HEIGHT / 2. - 43.5, 6.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..default()
            },
            Name::new("TIME FRAGMENTS TEXT"),
            CurrencyText,
            RenderLayers::from_layers(&[3]),
        ))
        .id();

    let stack = spawn_item_stack_icon(
        &mut commands,
        &graphics,
        &ItemStack::crate_icon_stack(WorldObject::TimeFragment),
        &asset_server,
        Vec2::new(-10., 1.),
        Vec2::new(0., 0.),
        3,
    );
    commands.entity(stack).insert(CurrencyIcon).set_parent(text);

    // INVENTORY ICON
    let bag_icon = spawn_item_stack_icon(
        &mut commands,
        &graphics,
        &ItemStack::crate_icon_stack(WorldObject::InventoryBag),
        &asset_server,
        Vec2::new(-res.game_width / 2. + 18.5, -GAME_HEIGHT / 2. + 10.),
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

    // DODGE ICON
    let dodge_icon = spawn_item_stack_icon(
        &mut commands,
        &graphics,
        &ItemStack::crate_icon_stack(WorldObject::Dodge),
        &asset_server,
        Vec2::new(-res.game_width / 2. + 42.5, -GAME_HEIGHT / 2. + 10.),
        Vec2::new(0., 0.),
        3,
    );
    commands
        .spawn(SpriteBundle {
            texture: asset_server.load("textures/SpaceKey.png"),
            transform: Transform::from_translation(Vec3::new(-0.5, 13.0, 1.)),
            sprite: Sprite {
                custom_size: Some(Vec2::new(30., 10.)),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .set_parent(dodge_icon);
}
pub fn update_currency_text(
    mut currency: Query<&TimeFragmentCurrency, Changed<TimeFragmentCurrency>>,
    mut text_query: Query<&mut Text, With<CurrencyText>>,
    icon: Query<Entity, With<CurrencyIcon>>,
    mut commands: Commands,
) {
    let Ok(time_fragments) = currency.get_single_mut() else {
        return;
    };
    let icon_e = icon.single();
    commands.entity(icon_e).insert(BounceOnHit::new());

    let mut text = text_query.single_mut();
    text.sections[0].value = format!("{:}", time_fragments.time_fragments);
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
pub fn update_xp_bar(
    player_xp_query: Query<&PlayerLevel, With<Player>>,
    mut xp_bar_query: Query<(&mut Sprite, &mut BarFlashTimer), With<XPBar>>,
    mut xp_bar_text_query: Query<(&mut Text, &mut Transform), With<XPBarText>>,
    mut flash_event: EventReader<FlashExpBarEvent>,
) {
    for _e in flash_event.iter() {
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
pub struct SkillHudIcon(pub Skill);

#[derive(Component)]
pub struct SkillClassText;

pub fn setup_skills_class_text(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    res: Res<ScreenResolution>,
    graphics: Res<Graphics>,
) {
    let skill_class_hud_frame = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::SkillClassTracker),
            sprite: Sprite {
                custom_size: Some(Vec2::new(64., 16.)),
                ..Default::default()
            },
            transform: Transform {
                translation: Vec3::new(
                    -res.game_width / 2. + 68.5,
                    (GAME_HEIGHT - 15.) / 2. - 35.5,
                    6.,
                ),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(Name::new("SKILL CLASS HUD"))
        .insert(RenderLayers::from_layers(&[3]))
        .id();
    let text = spawn_text(
        &mut commands,
        &asset_server,
        Vec3::new(-23., -1., 1.),
        BLACK,
        "  0     0     0".to_string(),
        Anchor::CenterLeft,
        1.,
        3,
    );
    commands
        .entity(text)
        .set_parent(skill_class_hud_frame)
        .insert(Name::new("SKILLS CLASS TEXT"))
        .insert(SkillClassText);
}
pub fn handle_update_player_skills(
    player_skills: Query<&PlayerSkills, Changed<PlayerSkills>>,
    mut commands: Commands,
    graphics: Res<Graphics>,
    mut prev_icons_tracker: Local<PlayerSkills>,
    res: Res<ScreenResolution>,
    mut skill_class_text: Query<&mut Text, With<SkillClassText>>,
    game_over: EventReader<GameOverEvent>,
) {
    if !game_over.is_empty() {
        prev_icons_tracker.skills.clear();
    }
    if let Ok(new_skills) = player_skills.get_single() {
        for (i, skill) in new_skills.skills.clone().iter().enumerate() {
            if prev_icons_tracker.skills.get(i) == Some(skill) {
                continue;
            }
            prev_icons_tracker.skills.push(skill.clone());
            let offset = Vec2::new(
                i as f32 * 19. + (-res.game_width) / 2. + 98.,
                (GAME_HEIGHT - 15.) / 2. - 12.5,
            );
            let icon = commands
                .spawn(SpriteBundle {
                    texture: graphics.get_skill_icon(skill.clone()),
                    sprite: Sprite {
                        custom_size: Some(Vec2::new(16., 16.)),
                        ..Default::default()
                    },
                    transform: Transform {
                        translation: offset.extend(1.),
                        scale: Vec3::new(1., 1., 1.),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .insert(RenderLayers::from_layers(&[3]))
                .insert(SkillHudIcon(skill.clone()))
                .insert(Name::new("HUD ICON!!"))
                .id();
            commands
                .spawn(SpriteBundle {
                    sprite: Sprite {
                        color: BLACK,
                        custom_size: Some(Vec2::new(18., 18.)),
                        ..default()
                    },
                    transform: Transform {
                        translation: Vec3::new(0., 0., -1.),
                        scale: Vec3::new(1., 1., 1.),
                        ..Default::default()
                    },
                    ..default()
                })
                .insert(RenderLayers::from_layers(&[3]))
                .set_parent(icon);

            let mut text = skill_class_text.single_mut();
            text.sections[0].value = format!(
                "  {:}     {:?}     {:?}",
                new_skills.melee_skill_count,
                new_skills.rogue_skill_count,
                new_skills.magic_skill_count
            );
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
                    -res.game_width / 2. + 28.5,
                    (GAME_HEIGHT - 15.) / 2. - 65.5,
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
