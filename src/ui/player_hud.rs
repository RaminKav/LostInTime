use bevy::{ecs::system::SystemParam, prelude::*, render::view::RenderLayers, sprite::Anchor};
use rand::Rng;
use std::collections::HashMap;

use super::{
    heirloom_tooltip::{
        heirloom_hud_hover_tooltip_position, HeirloomTooltipRequest, HeirloomTooltipShow,
    },
    hud_currency_first_center_x, hud_currency_second_center_x, hud_era_timer_center_x,
    hud_heirloom_first_icon_x, hud_heirloom_row_y, hud_hotbar_slot_center_x,
    hud_keybind_badge_center_y, hud_progress_bar_center_x, hud_row_below_xp_y,
    hud_timeline_arrow_local_x, hud_timeline_center_x,
    icon_hover_tooltips::ICON_HOVER_TOOLTIP_BG_COLOR,
    interactions::{DraggedItem, Interaction},
    spawn_inv_slot, spawn_item_stack_icon,
    tooltips::spawn_world_item_tooltip_for_stack,
    tooltips::ConsumableBuffHudTooltip,
    ui_helpers::{
        spawn_hud_label_badge, spawn_keybind_badge, Z_DEPTH_HUD_ACTIVE_SKILLS,
        Z_DEPTH_HUD_HEIRLOOM_ICONS, Z_DEPTH_HUD_HEIRLOOM_ICONS_FOREGROUND,
        Z_DEPTH_HUD_ORB_TRACKERS_FOREGROUND,
    },
    InventorySlotState, InventorySlotType, InventoryState, InventoryUI, UIElement, UIState,
    CURRENCY_BACKGROUND_SIZE, HUD_ACTION_ROW_Y_FROM_BOTTOM, HUD_ERA_TIMER_ENDLESS_WIDTH,
    HUD_FRAME_Y_FROM_BOTTOM, HUD_HEIRLOOM_ICON_SPACING,
    HUD_HOTBAR_SLOTS, HUD_SKILLS_CENTER_X,
    HUD_SKILL_SLOT_HIT_SIZE, HUD_SKILL_SPACING_X, HUD_TIMELINE_ARROWS_SIZE, HUD_TIMELINE_SIZE,
    KEYBIND_BADGE_BOTTOM_INSET, KEYBIND_BADGE_SIZE, PROGRESS_BACKGROUND_SIZE,
};
use crate::{
    assets::Graphics,
    attributes::{
        attribute_helpers::skill_power_multiplier, ActiveConsumableBuffs, AttackSpeed,
        BonusAttackSpeed, CritChance, CurrentHealth, CurrentMana, MaxHealth, MaxMana,
        ProjectileSize, SkillPower, Speed,
    },
    audio::{AudioSoundEffect, SoundSpawner},
    blessings::OwnedBlessings,
    chaos::ChaosTracker,
    client::GameOverEvent,
    colors::{
        overwrite_alpha, LEVEL_BLUE, LEVEL_DARK_BLUE, LIGHT_BLUE, LIGHT_GREY, LIGHT_RED, RED,
        WHITE, YELLOW, YELLOW_2,
    },
    cursor::CursorPos,
    inventory::{Inventory, ItemStack},
    item::WorldObject,
    item::item_drop_outline::{HeirloomIconOutline, HeirloomIconOutlineStyle, UiShadow},
    juice::bounce::BounceOnHit,
    keybinds::InputBinding,
    night::{EraTimer, InfiniteMode, ERA_TIMER_SECONDS},
    player::{
        combat_heirlooms::{
            CrateBreakDamageTracker, EnergyBallBarrageTracker, MaxHPHuntTracker,
            SkillPowerHuntTracker,
        },
        levels::PlayerLevel,
        skills::{
            active_skill_scaling::METEOR_SHOWER_BASE_COUNT,
            effective_player_attack_speed_multiplier, ActiveSkill, ActiveSkillChoiceState,
            ActiveSkillUsedEvent, ClassSkillSlots, Heirloom, HeirloomRarity, HeirloomTriggerCounts,
            ManaGainSource, ManaTrackerResetTimer, PlayerClass, PlayerSkills, SkillClass,
            VISIBLE_CLASS_SKILL_COUNT,
        },
        unlocks::{UnlockUpgrades, UnlockedSkills},
        CoinCurrency, Player, RunScore, TimeFragmentCurrency,
    },
    proto::proto_param::ProtoParam,
    ui::{game_fonts as gf, CheatSettings, Interactable, SKILL_TOOLTIP_SIZE},
    GameState, InputMappings, Pet, ScreenResolution,
};
use bevy::utils::Duration;
#[derive(Component)]
pub struct HealthBar;
#[derive(Component)]
pub struct HealthBarText;
#[derive(Component)]
pub struct ManaBar;
#[derive(Component)]
pub struct ManaBarText;

/// Invisible hit target on the mana orb for the mana consumption tracker tooltip.
#[derive(Component)]
pub struct ManaOrbHudHover;

/// Invisible hit target on the health orb for the health gain tracker tooltip.
#[derive(Component)]
pub struct HealthOrbHudHover;

/// Floating tooltip shown while hovering the mana orb.
#[derive(Component)]
pub struct ManaTrackerHudTooltip;

/// Floating tooltip shown while hovering the health orb.
#[derive(Component)]
pub struct HealthTrackerHudTooltip;

/// Marker for the new bottom HUD frame sprite (replaces the old top "bars" frame).
#[derive(Component)]
pub struct HudFrame;
#[derive(Component)]
pub struct XPBar;
#[derive(Component)]
pub struct XPBarText;
/// Marker for the XP bar background (hidden with the rest of the bar in game over).
#[derive(Component)]
pub struct XPBarBg;

/// Resource: when present, the XP bar is fading in over 2s (inserted when game-start overlay ends).
#[derive(Resource)]
pub struct XpBarFadeIn(pub Timer);

/// Marker for the level frame sprite (child of XP bar text) so we can fade it with the bar.
#[derive(Component)]
pub struct XPBarLevelFrame;

/// Component to store pending XP that will be drained over time
#[derive(Component, Default)]
pub struct PendingXP {
    pub stored: f32,
    pub displayed: f32,
}
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

/// Center-screen compact progress bar (`ProgressBackground.png`, score + chaos).
#[derive(Component)]
pub struct ProgressHudBar;

/// One of the two currency count backgrounds below the XP bar.
#[derive(Component)]
pub struct CurrencyHudBackground;

/// `0` = time fragments, `1` = coins (left-to-right HUD row).
#[derive(Component)]
pub struct CurrencyHudSlotIndex(pub u8);

#[derive(Component)]
pub struct TimelineHUD;
#[derive(Component)]
pub struct TimelineProgressArrows;

#[derive(Component)]
pub struct EraTimerHUD;
#[derive(Component)]
pub struct EraTimerText;
#[derive(Component)]
pub struct EndlessElapsedText;

#[derive(Component)]
pub struct ActiveSkillIcon {
    pub skill: crate::player::skills::ActiveSkill,
    pub slot_index: usize,
}

/// Marker on the invisible HUD skill-slot anchor. Used as the drop target during
/// drag-and-drop reordering — the icon child (`UIElement::HeirloomHudIcon` +
/// `Interactable`) is the drag *source*, while this anchor is what the cursor must
/// overlap to land a drop ([`HUD_SKILL_SLOT_HIT_SIZE`]).
#[derive(Component)]
pub struct ActiveSkillSlotBg {
    pub slot_index: usize,
}

pub const ACTIVE_SKILL_LOCK_ICON_PATH: &str = "ui/Icons/lock.png";
pub const ACTIVE_SKILL_LOCK_ICON_SIZE: Vec2 = Vec2::new(16., 16.);

/// Lock overlay on HUD skill slots that have not been unlocked via Time Fragments.
#[derive(Component)]
pub struct ActiveSkillLockIcon {
    pub slot_index: usize,
}

#[derive(SystemParam)]
pub struct SkillSlotUnlockState<'w> {
    unlocked_skills: Res<'w, UnlockedSkills>,
    unlock_upgrades: Res<'w, UnlockUpgrades>,
    player_class: Option<Res<'w, PlayerClass>>,
    cheat_settings: Option<Res<'w, CheatSettings>>,
}

impl SkillSlotUnlockState<'_> {
    pub fn is_slot_locked(&self, slot_index: usize) -> bool {
        if self
            .cheat_settings
            .as_ref()
            .map(|settings| settings.bypass_class_unlocks)
            .unwrap_or(false)
        {
            return false;
        }
        let class = self
            .player_class
            .as_ref()
            .map(|player_class| player_class.class.clone())
            .unwrap_or(SkillClass::None);
        !self
            .unlocked_skills
            .is_unlocked(&class, slot_index, &self.unlock_upgrades)
    }
}

/// Marker for the floating drag preview sprite that follows the cursor while the
/// player is reordering skill slots. Despawned on drop / cancel.
#[derive(Component)]
pub struct ActiveSkillDragIcon;

/// Tracks the in-progress drag of an active skill HUD icon (if any). Drag starts on
/// left-click of a populated slot icon and ends on left-release; releasing onto a
/// different slot bg swaps the two slots, releasing anywhere else cancels.
#[derive(Resource, Default)]
pub struct ActiveSkillDragState {
    pub origin_slot: Option<usize>,
    pub drag_icon_entity: Option<Entity>,
}

#[derive(Component)]
pub struct ActiveSkillKeybindText {
    pub slot: usize,
}

#[derive(Component)]
pub struct ActiveSkillKeyBackground {
    pub slot: usize,
}

#[derive(Component)]
pub struct InventoryKeybindText;

#[derive(Component)]
pub struct InventoryKeyBackground;

#[derive(Component)]
pub struct MinimapKeybindText;

#[derive(Component)]
pub struct MinimapKeyBackground;

#[derive(Component)]
pub struct OptionsKeybindText;

#[derive(Component)]
pub struct OptionsKeyBackground;

/// Bottom-left HUD corner icons (minimap / inventory / settings).
#[derive(Component, Clone, Copy)]
pub enum HudBottomCornerIcon {
    Minimap,
    Inventory,
    Settings,
}

/// Center-to-center spacing between the minimap and inventory HUD corner icons.
const HUD_CORNER_ICON_SPACING: f32 = 30.0;

/// Inset from the left screen edge to the first corner icon center.
const HUD_CORNER_LEFT_PADDING: f32 = 6.0;

/// `assets/ui/InventoryIcon.png` / `MapIcon.png` / `SettingsIcon.png` draw size.
const HUD_CORNER_ICON_SIZE: Vec2 = Vec2::new(27., 27.);

#[derive(Component)]
pub struct HotbarKeybindText {
    pub slot: usize,
}

#[derive(Component)]
pub struct HotbarKeyBackground {
    pub slot: usize,
}

#[derive(Component)]
pub struct ChaosText;

#[derive(Component)]
pub struct ChaosBar;

/// Helper function to blend two colors
fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color::rgba(
        a.r() + (b.r() - a.r()) * t,
        a.g() + (b.g() - a.g()) * t,
        a.b() + (b.b() - a.b()) * t,
        a.a() + (b.a() - a.a()) * t,
    )
}

#[derive(Component)]
pub struct SkillChargeText {
    pub slot: usize, // Which skill slot this text is for (1 or 2)
}

/// Size of the new HUD frame sprite (`assets/ui/HudBar.png`).
pub const HUD_FRAME_SIZE: Vec2 = Vec2::new(394.0, 62.0);

/// Pixel size of a single HP/mana fill texture (`HpBarFill.png` / `ManaBarFill.png`).
pub const HUD_FILL_PIXEL_SIZE: Vec2 = Vec2::new(50.0, 50.0);

/// Horizontal offset (from the frame's center) of each semicircular fill region's center.
/// The 28×34 fill texture sits inside the cap of the frame, nudged ~8px inward from the
/// outer edge so the liquid is centered on the visible cap interior rather than the
/// outermost pixel column.
pub const HUD_FILL_X_OFFSET: f32 = HUD_FRAME_SIZE.x * 0.5 - HUD_FILL_PIXEL_SIZE.x;

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

/// Spawns the new bottom HUD frame plus shader-driven HP/mana fills that sit inside the
/// two semicircular caps of the frame. The fills use [`HudBarFillMaterial`] so the
/// non-rectangular silhouette is respected automatically and the liquid surface line is
/// highlighted in-shader.
///
/// All four legacy inner sprite bars (HP / shield / mana / food) and their old frame
/// asset have been removed. Shield + food visualizations are gone entirely; their
/// underlying stats are unaffected.
pub fn setup_bars_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<crate::ui::hud_bar_fill::HudBarFillMaterial>>,
    res: Res<ScreenResolution>,
    player_stats: Query<(&CurrentHealth, &CurrentMana), With<Player>>,
) {
    use crate::ui::hud_bar_fill::HudBarFillMaterial;
    use bevy::sprite::{MaterialMesh2dBundle, Mesh2dHandle};

    let row_y = -res.game_height * 0.5 + HUD_FRAME_Y_FROM_BOTTOM;
    let bar_label_style = gf::HUD_CURRENCY_COUNT.text_style(&asset_server, WHITE);
    let (hp_amount, mana_amount) = player_stats
        .get_single()
        .map(|(hp, mana)| (hp.0, mana.0))
        .unwrap_or((0, 0));

    let hud_bar_frame = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::HudBar),
            sprite: Sprite {
                custom_size: Some(HUD_FRAME_SIZE),
                ..Default::default()
            },
            // Z below `Z_DEPTH_HUD_ACTIVE_SKILLS` (4.0) so the hotbar / skill slot
            // backgrounds and their icons render on top of the frame.
            transform: Transform::from_translation(Vec3::new(0., row_y, 1.)),
            ..Default::default()
        })
        .insert(Name::new("HUD FRAME"))
        .insert(HudFrame)
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UiShadow::hud())
        .id();

    // Shader-driven HP/mana fills sitting inside the two semicircular caps.
    // Z is slightly above the frame so the liquid renders on top of the dark cap interior.
    let hp_mesh: Mesh2dHandle = meshes
        .add(Mesh::from(shape::Quad::new(HUD_FILL_PIXEL_SIZE)))
        .into();
    let hp_material = materials.add(HudBarFillMaterial::new(
        asset_server.load("ui/HpBarFill.png"),
        1.0,
        HUD_FILL_PIXEL_SIZE.y as u32,
    ));
    let hp_fill = commands
        .spawn(MaterialMesh2dBundle {
            mesh: hp_mesh,
            material: hp_material,
            transform: Transform::from_translation(Vec3::new(-HUD_FILL_X_OFFSET + 1., -3.0, 1.0)),
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(HealthBar)
        .insert(Name::new("HUD HP FILL"))
        .id();
    commands
        .spawn(Text2dBundle {
            text: Text::from_section(format!("{hp_amount}"), bar_label_style.clone()),
            text_anchor: Anchor::Center,
            transform: Transform::from_translation(Vec3::new(0., 0., 2.)),
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(HealthBarText)
        .insert(Name::new("HUD HP TEXT"))
        .set_parent(hp_fill);

    commands
        .spawn(SpriteBundle {
            sprite: Sprite {
                color: Color::rgba(0., 0., 0., 0.),
                custom_size: Some(HUD_FILL_PIXEL_SIZE),
                ..default()
            },
            transform: Transform::from_translation(Vec3::new(0., 0., 3.)),
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(HealthOrbHudHover)
        .insert(Interactable::default())
        .insert(Name::new("HUD HP HOVER"))
        .set_parent(hp_fill);

    let mana_mesh: Mesh2dHandle = meshes
        .add(Mesh::from(shape::Quad::new(HUD_FILL_PIXEL_SIZE)))
        .into();
    let mana_material = materials.add(HudBarFillMaterial::new(
        asset_server.load("ui/ManaBarFill.png"),
        1.0,
        HUD_FILL_PIXEL_SIZE.y as u32,
    ));
    let mana_fill = commands
        .spawn(MaterialMesh2dBundle {
            mesh: mana_mesh,
            material: mana_material,
            transform: Transform::from_translation(Vec3::new(HUD_FILL_X_OFFSET + 1., -5.0, 1.0)),
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(ManaBar)
        .insert(Name::new("HUD MANA FILL"))
        .id();
    commands
        .spawn(Text2dBundle {
            text: Text::from_section(format!("{mana_amount}"), bar_label_style),
            text_anchor: Anchor::Center,
            transform: Transform::from_translation(Vec3::new(1., 0., 2.)),
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(ManaBarText)
        .insert(Name::new("HUD MANA TEXT"))
        .set_parent(mana_fill);

    commands
        .spawn(SpriteBundle {
            sprite: Sprite {
                color: Color::rgba(0., 0., 0., 0.),
                custom_size: Some(HUD_FILL_PIXEL_SIZE),
                ..default()
            },
            transform: Transform::from_translation(Vec3::new(0., 0., 3.)),
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(ManaOrbHudHover)
        .insert(Interactable::default())
        .insert(Name::new("HUD MANA HOVER"))
        .set_parent(mana_fill);

    commands
        .entity(hud_bar_frame)
        .push_children(&[hp_fill, mana_fill]);
}

pub fn setup_xp_bar_ui(
    mut commands: Commands,
    _graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    res: Res<ScreenResolution>,
) {
    let _inner_xp_prog = commands
        .spawn(SpriteBundle {
            sprite: Sprite {
                color: overwrite_alpha(LEVEL_BLUE, 0.),
                custom_size: Some(Vec2::new(0., 6.)), // Initialize to 0 width (0 XP at start)
                anchor: Anchor::CenterLeft,
                ..default()
            },
            transform: Transform {
                translation: Vec3::new(-res.game_width / 2., res.game_height / 2. - 3., 11.),
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
        .insert(PendingXP::default())
        .insert(Name::new("inner xp bar"))
        .id();
    let _inner_xp_bg = commands
        .spawn(SpriteBundle {
            sprite: Sprite {
                color: overwrite_alpha(*LEVEL_DARK_BLUE.clone().set_a(0.85), 0.),
                custom_size: Some(Vec2::new(res.game_width, 6.)), // Initialize to 0 width (0 XP at start)
                anchor: Anchor::CenterLeft,
                ..default()
            },
            transform: Transform {
                translation: Vec3::new(-res.game_width / 2., res.game_height / 2. - 3., 10.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(XPBarBg)
        .insert(Name::new("inner xp bar"))
        .id();
    let level_frame = commands
        .spawn(SpriteBundle {
            sprite: Sprite {
                color: overwrite_alpha(Color::rgba(0.1, 0.1, 0.1, 0.0), 0.),
                custom_size: Some(Vec2::new(46., 11.)),
                ..default()
            },
            transform: Transform {
                translation: Vec3::new(0., 0., -1.),
                ..Default::default()
            },
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(XPBarLevelFrame)
        .id();
    let _text = commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    "Level 1",
                    TextStyle {
                        font: asset_server.load("fonts/slkscr.ttf"),
                        font_size: 8.4,
                        color: overwrite_alpha(WHITE, 0.),
                    },
                ),
                text_anchor: Anchor::Center,
                transform: Transform {
                    translation: Vec3::new(0., res.game_height / 2. - 3., 12.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..default()
            },
            Name::new("XP TEXT"),
            XPBarText,
            RenderLayers::from_layers(&[3]),
        ))
        .add_child(level_frame)
        .id();
    // commands
    //     .entity(xp_bar_frame)
    //     .push_children(&[inner_xp_prog, text]);
}
/// Spawns the HUD row just below the XP bar: two currency backgrounds (time fragments +
/// coins) on the left, and the compact centered progress background (score + chaos only).
pub fn setup_currency_ui(
    mut commands: Commands,
    currency: Res<TimeFragmentCurrency>,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    res: Res<ScreenResolution>,
    coins: Res<CoinCurrency>,
    chaos_tracker: Res<ChaosTracker>,
    infinite_mode: Res<InfiniteMode>,
    keybinds: Res<InputMappings>,
) {
    let row_y = hud_row_below_xp_y(res.game_height);
    let currency_style = gf::HUD_CURRENCY_COUNT.text_style(&asset_server, WHITE);
    let progress_stat_style = gf::HUD_PROGRESS_STAT.text_style(&asset_server, WHITE);

    let first_center_x = hud_currency_first_center_x(res.game_width);
    let second_center_x = hud_currency_second_center_x(res.game_width);
    let progress_center_x = hud_progress_bar_center_x(&res);

    // Time fragments slot
    {
        let bg = commands
            .spawn(SpriteBundle {
                texture: graphics.get_ui_element_texture(UIElement::CurrencyBackground),
                sprite: Sprite {
                    custom_size: Some(CURRENCY_BACKGROUND_SIZE),
                    ..default()
                },
                transform: Transform::from_translation(Vec3::new(first_center_x, row_y, 3.)),
                ..default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .insert(CurrencyHudBackground)
            .insert(CurrencyHudSlotIndex(0))
            .insert(UiShadow::hud())
            .insert(Name::new("TIME FRAGMENT CURRENCY BG"))
            .id();

        let text = commands
            .spawn((
                Text2dBundle {
                    text: Text::from_section(
                        format!("{}", currency.time_fragments.max(0)),
                        currency_style.clone(),
                    )
                    .with_alignment(TextAlignment::Center),
                    text_anchor: Anchor::CenterLeft,
                    transform: Transform::from_translation(Vec3::new(-12., 0., 2.)),
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
            Vec2::new(-8., 1.),
            Vec2::ZERO,
            3,
        );
        commands
            .entity(stack)
            .insert(TimeFragmentIcon)
            .set_parent(text);
        commands.entity(text).set_parent(bg);
    }

    // Coins slot
    {
        let bg = commands
            .spawn(SpriteBundle {
                texture: graphics.get_ui_element_texture(UIElement::CurrencyBackground),
                sprite: Sprite {
                    custom_size: Some(CURRENCY_BACKGROUND_SIZE),
                    ..default()
                },
                transform: Transform::from_translation(Vec3::new(second_center_x, row_y, 3.)),
                ..default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .insert(CurrencyHudBackground)
            .insert(CurrencyHudSlotIndex(1))
            .insert(UiShadow::hud())
            .insert(Name::new("COIN CURRENCY BG"))
            .id();

        let coin_text = commands
            .spawn((
                Text2dBundle {
                    text: Text::from_section(format!("{}", coins.coins), currency_style)
                        .with_alignment(TextAlignment::Center),
                    text_anchor: Anchor::CenterLeft,
                    transform: Transform::from_translation(Vec3::new(-12., 0., 2.)),
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
            Vec2::new(-8., 1.),
            Vec2::ZERO,
            3,
        );
        commands
            .entity(coin_stack)
            .insert(CoinIcon)
            .set_parent(coin_text);
        commands.entity(coin_text).set_parent(bg);
    }

    // Compact center progress bar (102×24): score + chaos only.
    let progress_bar = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::ProgressBackground),
            sprite: Sprite {
                custom_size: Some(PROGRESS_BACKGROUND_SIZE),
                ..default()
            },
            transform: Transform::from_translation(Vec3::new(progress_center_x, row_y + 1., 7.)),
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(ProgressHudBar)
        .insert(UiShadow::hud())
        .insert(Name::new("PROGRESS HUD BAR"))
        .id();

    let chaos_value = chaos_tracker.get_chaos() + infinite_mode.get_chaos_bonus();

    commands
        .spawn((
            Text2dBundle {
                text: Text::from_section("Score: 0", progress_stat_style.clone())
                    .with_alignment(TextAlignment::Center),
                text_anchor: Anchor::Center,
                transform: Transform::from_translation(Vec3::new(0., 4., 2.)),
                ..default()
            },
            Name::new("SCORE TEXT"),
            ScoreText,
            RenderLayers::from_layers(&[3]),
        ))
        .set_parent(progress_bar);

    commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(format!("Chaos: {:.1}", chaos_value), progress_stat_style)
                    .with_alignment(TextAlignment::Center),
                text_anchor: Anchor::Center,
                transform: Transform::from_translation(Vec3::new(0., -5., 2.)),
                ..default()
            },
            ChaosText,
            RenderLayers::from_layers(&[3]),
            Name::new("CHAOS TEXT"),
        ))
        .set_parent(progress_bar);

    // Minimap + inventory + options icons (bottom-left HUD corner).
    let corner_y = -res.game_height / 2. + 18.;
    let map_x = -res.game_width * 0.5 + HUD_CORNER_ICON_SIZE.x * 0.5 + HUD_CORNER_LEFT_PADDING;
    let bag_x = map_x + HUD_CORNER_ICON_SPACING;
    let settings_x = bag_x + HUD_CORNER_ICON_SPACING;

    let map_icon = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::MapIcon),
            sprite: Sprite {
                custom_size: Some(HUD_CORNER_ICON_SIZE),
                ..default()
            },
            transform: Transform::from_translation(Vec3::new(map_x, corner_y, 6.)),
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(HudBottomCornerIcon::Minimap)
        .insert(Name::new("MINIMAP HUD ICON"))
        .id();

    let minimap_key = keybinds.get_minimap_key();
    let (map_key_bg, map_key_text) = spawn_keybind_badge(
        &mut commands,
        &asset_server,
        minimap_key,
        Transform::from_translation(Vec3::new(-0.5, 13., 1.)),
        Some(map_icon),
        3,
    );
    commands.entity(map_key_bg).insert(MinimapKeyBackground);
    commands.entity(map_key_text).insert(MinimapKeybindText);

    let bag_icon = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::InventoryIcon),
            sprite: Sprite {
                custom_size: Some(HUD_CORNER_ICON_SIZE),
                ..default()
            },
            transform: Transform::from_translation(Vec3::new(bag_x, corner_y, 6.)),
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(HudBottomCornerIcon::Inventory)
        .insert(Name::new("INVENTORY HUD ICON"))
        .id();

    let inventory_key = keybinds.get_inventory_key();
    let (key_bg, key_text) = spawn_keybind_badge(
        &mut commands,
        &asset_server,
        inventory_key,
        Transform::from_translation(Vec3::new(-0.5, 13., 1.)),
        Some(bag_icon),
        3,
    );
    commands.entity(key_bg).insert(InventoryKeyBackground);
    commands.entity(key_text).insert(InventoryKeybindText);

    let settings_icon = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::SettingsIcon),
            sprite: Sprite {
                custom_size: Some(Vec2::new(26., 27.)),
                ..default()
            },
            transform: Transform::from_translation(Vec3::new(settings_x, corner_y, 6.)),
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(HudBottomCornerIcon::Settings)
        .insert(Name::new("OPTIONS HUD ICON"))
        .id();

    let options_key = InputBinding::KeyBinding(KeyCode::Escape);
    let (settings_key_bg, settings_key_text) = spawn_keybind_badge(
        &mut commands,
        &asset_server,
        options_key,
        Transform::from_translation(Vec3::new(0., 13., 1.)),
        Some(settings_icon),
        3,
    );
    commands
        .entity(settings_key_bg)
        .insert(OptionsKeyBackground);
    commands
        .entity(settings_key_text)
        .insert(OptionsKeybindText);
}

/// Chaos label is spawned inside [`setup_currency_ui`] on the progress bar; this stub
/// remains so existing plugin registration does not need reshuffling.
pub fn setup_chaos_ui() {}

pub fn update_chaos_ui(
    chaos_tracker: Res<ChaosTracker>,
    infinite_chaos: Res<InfiniteMode>,
    mut chaos_text_query: Query<&mut Text, With<ChaosText>>,
) {
    if !chaos_tracker.is_changed() && !infinite_chaos.is_changed() {
        return;
    }

    let chaos_value = chaos_tracker.get_chaos() + infinite_chaos.get_chaos_bonus();
    for mut text in chaos_text_query.iter_mut() {
        text.sections[0].value = format!("Chaos: {:.1}", chaos_value);
    }
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
    health_bar_query: Query<&Handle<crate::ui::hud_bar_fill::HudBarFillMaterial>, With<HealthBar>>,
    mut health_text: Query<&mut Text, With<HealthBarText>>,
    mut materials: ResMut<Assets<crate::ui::hud_bar_fill::HudBarFillMaterial>>,
) {
    let Ok((player_health, player_max_health)) = player_health_query.get_single() else {
        return;
    };
    let Ok(handle) = health_bar_query.get_single() else {
        return;
    };
    if let Some(material) = materials.get_mut(handle) {
        material.fill =
            (player_health.0 as f32 / player_max_health.0.max(1) as f32).clamp(0.0, 1.0);
    }
    if let Ok(mut text) = health_text.get_single_mut() {
        text.sections[0].value = format!("{}", player_health.0);
    }
}
/// Hide the XP bar (progress, background, level text) when in GameOver; show it again in Main.
pub fn hide_xp_bar_in_game_over(
    game_state: Res<State<GameState>>,
    mut commands: Commands,
    xp_bar_parts: Query<Entity, Or<(With<XPBar>, With<XPBarText>, With<XPBarBg>)>>,
) {
    let visibility = if game_state.0 == GameState::GameOver {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    for entity in xp_bar_parts.iter() {
        commands.entity(entity).insert(visibility);
    }
}

/// Fade in the XP bar over 2s; runs when XpBarFadeIn resource is present (inserted when game-start overlay ends).
pub fn tick_xp_bar_fade_in(
    time: Res<Time>,
    mut commands: Commands,
    fade: Option<ResMut<XpBarFadeIn>>,
    mut xp_bar: Query<&mut Sprite, (With<XPBar>, Without<XPBarBg>, Without<XPBarLevelFrame>)>,
    mut xp_bar_bg: Query<&mut Sprite, (With<XPBarBg>, Without<XPBar>, Without<XPBarLevelFrame>)>,
    mut xp_bar_text: Query<&mut Text, With<XPBarText>>,
    mut xp_bar_frame: Query<&mut Sprite, (With<XPBarLevelFrame>, Without<XPBar>, Without<XPBarBg>)>,
) {
    let Some(mut fade) = fade else {
        return;
    };
    fade.0.tick(time.delta());
    let t = fade.0.percent();
    if fade.0.finished() {
        for mut sprite in xp_bar.iter_mut() {
            sprite.color = overwrite_alpha(sprite.color, 1.);
        }
        for mut sprite in xp_bar_bg.iter_mut() {
            sprite.color = overwrite_alpha(sprite.color, 0.85);
        }
        for mut text in xp_bar_text.iter_mut() {
            for section in text.sections.iter_mut() {
                section.style.color = overwrite_alpha(section.style.color, 1.);
            }
        }
        for mut sprite in xp_bar_frame.iter_mut() {
            // sprite.color = overwrite_alpha(sprite.color, 0.85);
        }
        commands.remove_resource::<XpBarFadeIn>();
        return;
    }
    for mut sprite in xp_bar.iter_mut() {
        sprite.color = overwrite_alpha(sprite.color, t);
    }
    for mut sprite in xp_bar_bg.iter_mut() {
        sprite.color = overwrite_alpha(sprite.color, 0.85 * t);
    }
    for mut text in xp_bar_text.iter_mut() {
        for section in text.sections.iter_mut() {
            section.style.color = overwrite_alpha(section.style.color, t);
        }
    }
    for mut sprite in xp_bar_frame.iter_mut() {
        // sprite.color = overwrite_alpha(sprite.color, 0.7 * t);
    }
}

pub fn update_xp_bar(
    player_xp_query: Query<&PlayerLevel, With<Player>>,
    mut xp_bar_query: Query<(&mut PendingXP, &mut BarFlashTimer), With<XPBar>>,
    mut xp_bar_text_query: Query<&mut Text, With<XPBarText>>,
    mut flash_event: EventReader<FlashExpBarEvent>,
    mut commands: Commands,
    _res: Res<ScreenResolution>,
    ui_state: Res<State<UIState>>,
) {
    // If we're in the skill choice UI, don't update the bar (keep it full)
    if ui_state.0 == UIState::Skills {
        return;
    }

    for event in flash_event.iter() {
        let level = player_xp_query.single();

        let (mut pending_xp, _flash) = xp_bar_query.single_mut();

        pending_xp.stored += event.amount as f32;

        let mut text = xp_bar_text_query.single_mut();
        text.sections[0].value = format!("Level {:}", level.level);
        if event.did_level {
            commands.spawn(SoundSpawner::new(AudioSoundEffect::LevelUp, 0.35));
            pending_xp.displayed = level.xp as f32;
            pending_xp.stored = 0.0;
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

/// System to drain pending XP and smoothly update the XP bar
pub fn drain_pending_xp(
    mut xp_bar_query: Query<(&mut PendingXP, &mut Sprite), With<XPBar>>,
    player_xp_query: Query<&PlayerLevel, With<Player>>,
    res: Res<ScreenResolution>,
    time: Res<Time>,
    ui_state: Res<State<UIState>>,
) {
    if ui_state.0 == UIState::Skills {
        return;
    }

    let Ok(level) = player_xp_query.get_single() else {
        return;
    };

    let Ok((mut pending_xp, mut sprite)) = xp_bar_query.get_single_mut() else {
        return;
    };

    let drain_rate = if pending_xp.stored <= 0.0 {
        0.0
    } else {
        let normalized = (pending_xp.stored / 500.0).min(1.0);
        20.0 + (normalized * 500.0)
    };

    if pending_xp.stored > 0.0 {
        let drain_amount = drain_rate * time.delta().as_secs_f32();
        let actual_drain = drain_amount.min(pending_xp.stored);

        pending_xp.stored -= actual_drain;
        pending_xp.displayed += actual_drain;

        pending_xp.displayed = pending_xp.displayed.min(level.next_level_xp as f32);
    }

    let bar_width = if level.next_level_xp > 0 {
        res.game_width * pending_xp.displayed / level.next_level_xp as f32
    } else {
        0.0
    };

    sprite.custom_size = Some(Vec2 {
        x: bar_width,
        y: 6.,
    });
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

/// Updates XP bar to rainbow color when in skill choice UI and spawns decorative shards
pub fn update_xp_bar_rainbow(
    mut xp_bar_query: Query<&mut Sprite, With<XPBar>>,
    ui_state: Res<State<UIState>>,
    time: Res<Time>,
    res: Res<ScreenResolution>,
    graphics: Res<Graphics>,
    mut commands: Commands,
    mut spawn_timer: Local<Timer>,
    existing_shards: Query<Entity, With<DecorativeXPShard>>,
) {
    if ui_state.0 != UIState::Skills {
        // Clean up any remaining decorative shards when not in Skills UI
        for shard_e in existing_shards.iter() {
            commands.entity(shard_e).despawn_recursive();
        }
        return;
    }

    // Initialize spawn timer if needed
    if spawn_timer.duration().as_secs_f32() == 0.0 {
        *spawn_timer = Timer::from_seconds(0.13, TimerMode::Repeating); // Spawn every 0.15 seconds
    }

    spawn_timer.tick(time.delta());

    // Keep bar full when in skill choice UI and apply rainbow color
    for mut sprite in xp_bar_query.iter_mut() {
        sprite.custom_size = Some(Vec2 {
            x: res.game_width,
            y: 6.,
        });

        // Rainbow color effect - smooth back-and-forth through blue hue spectrum
        // Use sine wave to smoothly oscillate between blue tones (200-280 degrees)
        let elapsed = time.elapsed().as_secs_f32();
        let sine_wave = (elapsed * 3.).sin(); // Oscillates between -1 and 1
        let hue_progress = (sine_wave + 1.0) / 2.0; // Normalize to 0-1 range
        let hue = 200.0 + (hue_progress * 80.0); // Range from 200 (cyan-blue) to 280 (blue-purple)
        let color = Color::hsl(hue, 0.8, 0.6); // Slightly reduced saturation and higher lightness for softer blue tones
        sprite.color = color;
    }

    // Spawn decorative shards periodically
    if spawn_timer.just_finished() {
        let Some(spritesheet_map) = graphics.spritesheet_map.as_ref() else {
            return;
        };
        let Some(texture_atlas) = graphics.texture_atlas.as_ref() else {
            return;
        };

        let mut rng = rand::thread_rng();

        // Spawn 2-4 shards per spawn cycle
        let num_shards = rng.gen_range(3..=4);

        for _ in 0..num_shards {
            // Choose shard type: 70% small, 25% medium, 5% large
            let shard_type = match rng.gen_range(0..100) {
                0..=69 => WorldObject::XPShard,
                70..=93 => WorldObject::XPShardMedium,
                _ => WorldObject::XPShardLarge,
            };

            let Some(sprite) = spritesheet_map.get(&shard_type).cloned() else {
                continue;
            };

            // Random X position across screen width
            let x_pos = rng.gen_range(-res.game_width / 2.0..res.game_width / 2.0);
            let over_overlay = rng.gen_bool(0.5);
            let z_pos = if over_overlay { 10.0 } else { 5.0 };
            let start_y = res.game_height / 2.0;

            // Random fall speed
            let fall_speed = rng.gen_range(50.0..150.0);

            commands
                .spawn(SpriteSheetBundle {
                    sprite,
                    texture_atlas: texture_atlas.clone(),
                    transform: Transform::from_translation(Vec3::new(x_pos, start_y, z_pos)),
                    ..Default::default()
                })
                .insert(DecorativeXPShard {
                    fall_speed,
                    start_y,
                })
                .insert(RenderLayers::from_layers(&[3]))
                .insert(Name::new("DecorativeXPShard"));
        }
    }
}

/// Updates decorative XP shards to fall down and fade out
pub fn update_decorative_xp_shards(
    mut shards: Query<
        (
            Entity,
            &mut Transform,
            &mut TextureAtlasSprite,
            &DecorativeXPShard,
        ),
        With<DecorativeXPShard>,
    >,
    time: Res<Time>,
    res: Res<ScreenResolution>,
    mut commands: Commands,
) {
    let screen_bottom = -res.game_height / 2.0;

    for (entity, mut transform, mut sprite, shard) in shards.iter_mut() {
        // Move shard down
        transform.translation.y -= shard.fall_speed * time.delta().as_secs_f32();

        // Calculate alpha based on distance fallen
        // Start at full opacity, fade to 0 as it approaches bottom of screen
        let distance_fallen = shard.start_y - transform.translation.y;
        let total_distance = shard.start_y - screen_bottom;
        let alpha = (1.0 - (distance_fallen / total_distance).min(1.0)).max(0.0);

        // Update sprite color with fading alpha
        let current_color = sprite.color;
        sprite.color = Color::rgba(
            current_color.r(),
            current_color.g(),
            current_color.b(),
            alpha,
        );

        // Despawn when off screen or fully transparent
        if transform.translation.y < screen_bottom - 20.0 || alpha <= 0.0 {
            commands.entity(entity).despawn_recursive();
        }
    }
}

/// Detects when skill choice UI closes and sends FlashExpBarEvent to update the bar
pub fn handle_skill_choice_ui_close(
    mut flash_event: EventWriter<FlashExpBarEvent>,
    ui_state: Res<State<UIState>>,
    mut prev_state: Local<UIState>,
    player_xp_query: Query<&PlayerLevel, With<Player>>,
    mut xp_bar_query: Query<(&mut Sprite, &mut PendingXP), With<XPBar>>,
    decorative_shards: Query<Entity, With<DecorativeXPShard>>,
    mut commands: Commands,
) {
    let current_state = ui_state.0.clone();

    // If we just transitioned from Skills to something else, send update event and reset color
    if *prev_state == UIState::Skills && current_state != UIState::Skills {
        let level = player_xp_query.single();

        // Reset bar color to default and sync pending XP
        for (mut sprite, mut pending_xp) in xp_bar_query.iter_mut() {
            sprite.color = LEVEL_BLUE;
            // Sync displayed XP with actual level XP when exiting skill choice
            pending_xp.displayed = level.xp as f32;
        }

        // Clean up all decorative shards
        for shard_e in decorative_shards.iter() {
            commands.entity(shard_e).despawn_recursive();
        }

        flash_event.send(FlashExpBarEvent {
            amount: 0, // No new XP, just syncing
            did_level: false,
        });
    }

    *prev_state = current_state;
}

#[derive(Component, Eq, PartialEq)]
pub struct SkillHudIcon(pub Heirloom);

/// Marker for the pet's 4th-slot HUD icon (background quad). Sits at the rightmost
/// position of the skills group on the action row. Uses a static "PET" label badge
/// instead of a keybind — the pet auto-casts its ability on an internal timer.
#[derive(Component)]
pub struct PetSkillSlotBg;

/// Static "PET" label badge under the pet skill slot (same style as skill keybind badges).
#[derive(Component)]
pub struct PetSkillLabelBackground;

/// Marker for the pet's skill icon sprite (child of the pet skill slot anchor).
#[derive(Component)]
pub struct PetSkillIcon;

/// Records which pet the current HUD pet slot was built for so we can rebuild on swap.
#[derive(Component)]
pub struct PetSkillSlotFor(pub Pet);

/// Marker for the cooldown overlay sprite on the pet skill slot.
#[derive(Component)]
pub struct PetSkillCooldownOverlay;

#[derive(Component)]
pub struct HeirloomCounterText;

#[derive(Component)]
pub struct ActiveSkillHudTooltip;

/// Stores which skill and slot the HUD tooltip is for (used to show remaining cooldown).
#[derive(Component)]
pub struct ActiveSkillHudTooltipSkill(pub usize);

#[derive(Component)]
pub struct SkillTooltipCooldownText;

/// Tags tooltip cooldown text so the correct HUD updater keeps it in sync.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum SkillTooltipCooldownMarker {
    ActiveSkill,
    Pet,
}

/// Icon, title, body lines, and optional cooldown corner for a skill tooltip panel.
pub struct SkillTooltipContent {
    pub icon: Handle<Image>,
    pub title: String,
    pub description_lines: Vec<String>,
    pub icon_size: Vec2,
    pub title_color: Color,
    pub body_color: Color,
    pub cooldown_marker: Option<SkillTooltipCooldownMarker>,
}

/// System to handle tooltips for heirloom icons in the HUD
pub fn handle_heirloom_hud_tooltip(
    mut tooltip_requests: EventWriter<HeirloomTooltipRequest>,
    cursor_pos: Res<crate::cursor::CursorPos>,
    hit_detection_sprites: Query<
        (Entity, &Sprite, &GlobalTransform),
        With<super::interactions::Interactable>,
    >,
    mut hud_icons: Query<(
        Entity,
        &GlobalTransform,
        &UIElement,
        &mut super::interactions::Interactable,
        &SkillHudIcon,
    )>,
    mut last_hovered: Local<Option<Heirloom>>,
    player_query: Query<
        (
            &PlayerSkills,
            &crate::attributes::MaxHealth,
            Option<&crate::player::combat_heirlooms::MaxHPHuntTracker>,
            Option<&crate::player::combat_heirlooms::CrateBreakDamageTracker>,
            Option<&crate::player::combat_heirlooms::ThornsOnDamageTracker>,
            Option<&crate::player::combat_heirlooms::SkillPowerHuntTracker>,
            Option<&crate::player::combat_heirlooms::EnergyBallBarrageTracker>,
            &crate::attributes::PickupRange,
        ),
        With<Player>,
    >,
    coins: Res<CoinCurrency>,
    trigger_counts: Res<crate::player::skills::HeirloomTriggerCounts>,
    res: Res<ScreenResolution>,
) {
    use super::interactions::Interaction;

    // First, do hit detection and update interactable states
    let hit_entity = super::ui_helpers::pointcast_2d(&cursor_pos, &hit_detection_sprites, None);

    // Update all heirloom hud icons' interactable state based on cursor position
    for (entity, _, ui_elem, mut interactable, _) in hud_icons.iter_mut() {
        if ui_elem == &UIElement::HeirloomHudIcon {
            let is_hit = hit_entity
                .as_ref()
                .map(|(e, _sprite, _transform)| *e == entity)
                .unwrap_or(false);

            if is_hit && !matches!(interactable.current(), Interaction::Hovering) {
                interactable.change(Interaction::Hovering);
            } else if !is_hit && matches!(interactable.current(), Interaction::Hovering) {
                interactable.change(Interaction::None);
            }
        }
    }

    // Now find the currently hovered heirloom directly from the icon
    let currently_hovered = hud_icons
        .iter()
        .filter(|(_, _, ui_elem, _, _)| ui_elem == &&UIElement::HeirloomHudIcon)
        .find(|(_, _, _, interactable, _)| matches!(interactable.current(), Interaction::Hovering))
        .map(|(_, transform, _, _, skill_icon)| (skill_icon.0.clone(), transform.translation()));

    let hovered_heirloom = currently_hovered.as_ref().map(|(h, _)| h.clone());

    // Only update if the hover state changed
    if *last_hovered == hovered_heirloom {
        return;
    }

    match &currently_hovered {
        None => tooltip_requests.send(HeirloomTooltipRequest::Clear),
        Some((heirloom, icon_pos)) => {
            let Ok((
                skills,
                max_health,
                hunt_tracker,
                crate_tracker,
                thorns_tracker,
                skill_power_hunt_tracker,
                energy_ball_tracker,
                pickup_range,
            )) = player_query.get_single()
            else {
                *last_hovered = hovered_heirloom;
                return;
            };

            let rarity = skills
                .heirlooms
                .iter()
                .find(|h| h.heirloom == *heirloom)
                .map(|h| h.rarity)
                .unwrap_or(HeirloomRarity::Common);

            let scaling_text = get_heirloom_scaling_text(
                heirloom.clone(),
                skills,
                coins.coins,
                max_health.0,
                hunt_tracker,
                crate_tracker,
                thorns_tracker,
                skill_power_hunt_tracker,
                energy_ball_tracker,
                pickup_range.0,
            );

            let trigger_count = trigger_counts.get(heirloom);

            let (_, tooltip_size) = heirloom.get_ui_element(rarity);
            let tooltip_pos = heirloom_hud_hover_tooltip_position(
                *icon_pos,
                tooltip_size.x * 0.5,
                res.game_width,
            );
            tooltip_requests.send(HeirloomTooltipRequest::Show(HeirloomTooltipShow {
                heirloom: heirloom.clone(),
                rarity,
                position: tooltip_pos,
                scaling_text,
                trigger_count,
                ui_state: None,
            }));
        }
    }

    *last_hovered = hovered_heirloom;
}

/// Default skill tooltip icon size (HUD, shrines, blessing choice).
pub const SKILL_TOOLTIP_ICON_SIZE: Vec2 = Vec2::new(22., 22.);

const HUD_SKILL_TOOLTIP_OFFSET_X: f32 = -20.;
const HUD_SKILL_TOOLTIP_OFFSET_Y: f32 = 46.;
/// Local offset of the [`UIElement::SkillTooltip`] background inside a tooltip container.
pub const SKILL_TOOLTIP_BG_LOCAL: Vec3 = Vec3::new(61., 1., 1.);
const HUD_SKILL_TOOLTIP_Z_BUMP: f32 = 10.;

fn set_interactable_hover(is_hit: bool, interactable: &mut Interactable) {
    use crate::ui::interactions::Interaction;

    if is_hit && !matches!(interactable.current(), Interaction::Hovering) {
        interactable.change(Interaction::Hovering);
    } else if !is_hit && matches!(interactable.current(), Interaction::Hovering) {
        interactable.change(Interaction::None);
    }
}

/// Pixel-snapped world position for a HUD skill/pet tooltip anchored to a hotbar icon.
fn hud_skill_tooltip_world_position(icon_pos: Vec3, ui_scale: u32) -> Vec3 {
    Vec3::new(
        super::snap_world_to_pixel_grid(icon_pos.x + HUD_SKILL_TOOLTIP_OFFSET_X, ui_scale),
        super::snap_world_to_pixel_grid(icon_pos.y + HUD_SKILL_TOOLTIP_OFFSET_Y, ui_scale),
        icon_pos.z + HUD_SKILL_TOOLTIP_Z_BUMP,
    )
}

/// Rootless tooltip container + shared [`UIElement::SkillTooltip`] background.
pub fn spawn_skill_tooltip_shell(
    commands: &mut Commands,
    graphics: &Graphics,
    tooltip_pos: Vec3,
    bg_name: &'static str,
) -> (Entity, Entity) {
    let container = commands
        .spawn((
            RenderLayers::from_layers(&[3]),
            SpatialBundle::from_transform(Transform::from_translation(tooltip_pos)),
        ))
        .id();

    let bg = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::SkillTooltip),
            sprite: Sprite {
                custom_size: Some(SKILL_TOOLTIP_SIZE),
                ..Default::default()
            },
            transform: Transform::from_translation(SKILL_TOOLTIP_BG_LOCAL),
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new(bg_name))
        .insert(UiShadow::hud())
        .set_parent(container)
        .id();

    (container, bg)
}

/// Shared layout for skill / pet tooltips. Coordinates match [`UIElement::SkillTooltip`] banners.
pub fn spawn_skill_tooltip_layout(
    commands: &mut Commands,
    asset_server: &AssetServer,
    parent_entity: Entity,
    content: &SkillTooltipContent,
) {
    const ICONS_X_OFFSET: f32 = -24.;
    const TEXT_Y_OFFSET: f32 = 14.;
    const DESC_TEXT_X: f32 = ICONS_X_OFFSET + 32.;
    const COOLDOWN_TEXT_X: f32 = 158.;
    const TITLE_Y: f32 = TEXT_Y_OFFSET + 10.;

    let desc_body_style = gf::SKILL_PANEL_BODY.text_style(asset_server, content.body_color);

    commands
        .spawn(SpriteBundle {
            texture: content.icon.clone(),
            sprite: Sprite {
                custom_size: Some(content.icon_size),
                ..Default::default()
            },
            transform: Transform {
                translation: Vec3::new(ICONS_X_OFFSET, 0., 2.),
                scale: Vec3::ONE,
                ..Default::default()
            },
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("SKILL TOOLTIP ICON"))
        .set_parent(parent_entity);

    commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                content.title.as_str(),
                TextStyle {
                    font: gf::SKILL_PANEL_TITLE_BOLD.load_font(asset_server),
                    font_size: gf::SKILL_PANEL_TITLE_BOLD.size,
                    color: content.title_color,
                },
            )
            .with_alignment(TextAlignment::Left),
            text_anchor: Anchor::TopLeft,
            transform: Transform {
                translation: Vec3::new(DESC_TEXT_X, TITLE_Y, 2.),
                ..Default::default()
            },
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("SKILL TOOLTIP NAME"))
        .set_parent(parent_entity);

    if let Some(marker) = content.cooldown_marker {
        let mut cooldown = commands.spawn((
            Text2dBundle {
                text: Text::from_section(
                    "",
                    TextStyle {
                        font: gf::SKILL_PANEL_BODY.load_font(asset_server),
                        font_size: gf::SKILL_PANEL_BODY.size,
                        color: LIGHT_GREY,
                    },
                )
                .with_alignment(TextAlignment::Right),
                text_anchor: Anchor::TopRight,
                transform: Transform {
                    translation: Vec3::new(COOLDOWN_TEXT_X, TITLE_Y, 2.),
                    ..Default::default()
                },
                ..default()
            },
            RenderLayers::from_layers(&[3]),
            Name::new("SKILL TOOLTIP COOLDOWN"),
        ));
        match marker {
            SkillTooltipCooldownMarker::ActiveSkill => {
                cooldown.insert(SkillTooltipCooldownText);
            }
            SkillTooltipCooldownMarker::Pet => {
                cooldown.insert(PetSkillTooltipCooldownText);
            }
        }
        cooldown.set_parent(parent_entity);
    }

    for (j, line) in content.description_lines.iter().enumerate() {
        commands
            .spawn(Text2dBundle {
                text: Text::from_section(line.as_str(), desc_body_style.clone())
                    .with_alignment(TextAlignment::Left),
                text_anchor: Anchor::TopLeft,
                transform: Transform {
                    translation: Vec3::new(
                        DESC_TEXT_X,
                        (TEXT_Y_OFFSET - 2.) - j as f32 * gf::SKILL_TOOLTIP_DESC_LINE_STEP,
                        2.,
                    ),
                    ..Default::default()
                },
                ..default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .insert(Name::new("SKILL TOOLTIP DESCRIPTION LINE"))
            .set_parent(parent_entity);
    }
}

fn pet_skill_description_lines(pet_data: &crate::assets::PetData) -> Vec<String> {
    pet_data
        .skill_description
        .iter()
        .flat_map(|entry| entry.split('\n').map(str::to_string))
        .collect()
}

/// Spawns tooltip content for an [`ActiveSkill`]. Used by HUD hovers, class selection, shrines, etc.
pub fn spawn_skill_tooltip_content(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    active_skill: ActiveSkill,
    slot_index: Option<usize>,
    parent_entity: Entity,
    skill_power: f32,
    max_mana: i32,
    max_health: i32,
    bonus_attack_speed_mult: f32,
    crit_chance: i32,
    speed: i32,
    size: i32,
    meteor_count: u32,
    icon_size: Vec2,
) {
    let content = SkillTooltipContent {
        icon: graphics.get_active_skill_icon(active_skill.clone()),
        title: active_skill.get_title(),
        description_lines: active_skill.get_desc(
            skill_power,
            max_mana,
            max_health,
            bonus_attack_speed_mult,
            crit_chance,
            speed,
            size,
            meteor_count,
        ),
        icon_size,
        title_color: YELLOW_2,
        body_color: WHITE,
        cooldown_marker: slot_index
            .is_some()
            .then_some(SkillTooltipCooldownMarker::ActiveSkill),
    };
    spawn_skill_tooltip_layout(commands, asset_server, parent_entity, &content);
}

/// Live player stats passed into [`spawn_skill_tooltip_content`].
pub struct ActiveSkillTooltipParams {
    pub skill_power: f32,
    pub max_mana: i32,
    pub max_health: i32,
    pub bonus_attack_speed_mult: f32,
    pub crit_chance: i32,
    pub speed: i32,
    pub size: i32,
    pub meteor_count: u32,
}

impl ActiveSkillTooltipParams {
    pub fn preview() -> Self {
        Self {
            skill_power: 1.,
            max_mana: 100,
            max_health: 100,
            bonus_attack_speed_mult: 1.0,
            crit_chance: 10,
            speed: 0,
            size: 0,
            meteor_count: METEOR_SHOWER_BASE_COUNT,
        }
    }
}

pub fn active_skill_tooltip_params_from_player(
    skill_power: &Query<
        (
            &SkillPower,
            &OwnedBlessings,
            &MaxMana,
            &MaxHealth,
            Option<&BonusAttackSpeed>,
            Option<&AttackSpeed>,
            &CritChance,
            &Speed,
            &ProjectileSize,
        ),
        With<Player>,
    >,
    meteor_shower_state: &Query<
        &crate::player::skills::MeteorShowerSkillState,
        With<Player>,
    >,
) -> ActiveSkillTooltipParams {
    let Ok((
        skill_power,
        blessings,
        max_mana,
        max_health,
        bonus_as,
        attack_speed,
        crit,
        spd,
        size,
    )) = skill_power.get_single()
    else {
        return ActiveSkillTooltipParams::preview();
    };

    ActiveSkillTooltipParams {
        skill_power: skill_power_multiplier(skill_power, blessings.get_skill_power_bonus()),
        max_mana: max_mana.0,
        max_health: max_health.0,
        bonus_attack_speed_mult: effective_player_attack_speed_multiplier(
            attack_speed.map(|a| a.0).unwrap_or(0),
            bonus_as.map(|b| b.get_multiplier()).unwrap_or(1.0),
        ),
        crit_chance: crit.0,
        speed: spd.0,
        size: size.0,
        meteor_count: meteor_shower_state
            .get_single()
            .map(|s| s.meteor_count)
            .unwrap_or(METEOR_SHOWER_BASE_COUNT),
    }
}

/// System to handle tooltips for active skill icons in the HUD
pub fn handle_active_skill_hud_tooltip(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    cursor_pos: Res<CursorPos>,
    hit_detection_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut skill_icons: Query<(
        Entity,
        &GlobalTransform,
        &UIElement,
        &mut Interactable,
        &ActiveSkillIcon,
    )>,
    existing_tooltips: Query<Entity, With<ActiveSkillHudTooltip>>,
    mut last_hovered: Local<Option<ActiveSkill>>,
    res: Res<ScreenResolution>,
    skill_power: Query<
        (
            &SkillPower,
            &OwnedBlessings,
            &MaxMana,
            &MaxHealth,
            Option<&BonusAttackSpeed>,
            Option<&AttackSpeed>,
            &CritChance,
            &Speed,
            &ProjectileSize,
        ),
        With<Player>,
    >,
    meteor_shower_state: Query<&crate::player::skills::MeteorShowerSkillState, With<Player>>,
) {
    // First, do hit detection and update interactable states
    let hit_entity = super::ui_helpers::pointcast_2d(&cursor_pos, &hit_detection_sprites, None);

    // Update all skill icons' interactable state based on cursor position
    for (entity, _, ui_elem, mut interactable, _) in skill_icons.iter_mut() {
        if *ui_elem != UIElement::HeirloomHudIcon {
            continue;
        }
        let is_hit = hit_entity
            .as_ref()
            .map(|(e, _, _)| *e == entity)
            .unwrap_or(false);
        set_interactable_hover(is_hit, &mut interactable);
    }

    // Now find the currently hovered skill directly from the icon
    let currently_hovered = skill_icons
        .iter()
        .filter(|(_, _, ui_elem, _, _)| **ui_elem == UIElement::HeirloomHudIcon)
        .find(|(_, _, _, interactable, _)| matches!(interactable.current(), Interaction::Hovering))
        .map(|(_, transform, _, _, skill_slot)| {
            (
                skill_slot.skill.clone(),
                skill_slot.slot_index,
                transform.translation(),
            )
        });

    let hovered_skill = currently_hovered.as_ref().map(|(s, _, _)| s.clone());

    // Only update if the hover state changed
    if *last_hovered == hovered_skill {
        return;
    }

    // Despawn all existing tooltips
    for tooltip_e in existing_tooltips.iter() {
        commands.entity(tooltip_e).despawn_recursive();
    }

    // Spawn new tooltip if hovering
    if let Some((skill, slot_index, icon_pos)) = currently_hovered {
        // The container is a rootless `SpatialBundle` (no `Sprite`/`Text`), so it is skipped by
        // `snap_layer3_visuals_to_pixel_grid`. Snap onto the physical pixel grid so anchored
        // tooltip text lands on-grid (see `hud_skill_tooltip_world_position`).
        let (container, _) = spawn_skill_tooltip_shell(
            &mut commands,
            &graphics,
            hud_skill_tooltip_world_position(icon_pos, res.scale),
            "ACTIVE SKILL TOOLTIP",
        );
        commands
            .entity(container)
            .insert(ActiveSkillHudTooltip)
            .insert(ActiveSkillHudTooltipSkill(slot_index));

        let params =
            active_skill_tooltip_params_from_player(&skill_power, &meteor_shower_state);
        spawn_skill_tooltip_content(
            &mut commands,
            &graphics,
            &asset_server,
            skill,
            Some(slot_index),
            container,
            params.skill_power,
            params.max_mana,
            params.max_health,
            params.bonus_attack_speed_mult,
            params.crit_chance,
            params.speed,
            params.size,
            params.meteor_count,
            SKILL_TOOLTIP_ICON_SIZE,
        );
    }

    *last_hovered = hovered_skill;
}

const ORB_TRACKER_COLUMNS: usize = 2;
const ORB_TRACKER_ICON_SIZE: f32 = 12.0;
const ORB_TRACKER_COL_WIDTH: f32 = 54.0;
const ORB_TRACKER_ROW_HEIGHT: f32 = 14.0;
const ORB_TRACKER_TITLE_HEIGHT: f32 = 12.0;
const ORB_TRACKER_RATE_HEIGHT: f32 = 10.0;
const ORB_TRACKER_PAD: f32 = 6.0;

fn orb_tracker_tooltip_size(entry_count: usize) -> Vec2 {
    let rows = entry_count.div_ceil(ORB_TRACKER_COLUMNS).max(1) as f32;
    Vec2::new(
        ORB_TRACKER_COL_WIDTH * ORB_TRACKER_COLUMNS as f32 + ORB_TRACKER_PAD * 2.,
        ORB_TRACKER_TITLE_HEIGHT
            + rows * ORB_TRACKER_ROW_HEIGHT
            + ORB_TRACKER_RATE_HEIGHT
            + ORB_TRACKER_PAD * 2.,
    )
}

/// Mana tooltip has two stacked sections (consume + gain), each followed by a rate line.
fn mana_tracker_tooltip_size(consume_count: usize, gain_count: usize) -> Vec2 {
    let consume_rows = consume_count.div_ceil(ORB_TRACKER_COLUMNS).max(1) as f32;
    let gain_rows = gain_count.div_ceil(ORB_TRACKER_COLUMNS).max(1) as f32;
    Vec2::new(
        ORB_TRACKER_COL_WIDTH * ORB_TRACKER_COLUMNS as f32 + ORB_TRACKER_PAD * 2.,
        ORB_TRACKER_TITLE_HEIGHT
            + consume_rows * ORB_TRACKER_ROW_HEIGHT
            + ORB_TRACKER_RATE_HEIGHT
            + gain_rows * ORB_TRACKER_ROW_HEIGHT
            + ORB_TRACKER_RATE_HEIGHT
            + ORB_TRACKER_PAD * 2.,
    )
}

fn spawn_orb_tracker_rate_line(
    commands: &mut Commands,
    asset_server: &AssetServer,
    parent: Entity,
    label: &str,
    rate: f32,
    panel_height: f32,
) {
    let y = -panel_height * 0.5 + ORB_TRACKER_PAD + ORB_TRACKER_RATE_HEIGHT * 0.5;
    spawn_orb_tracker_rate_line_at(commands, asset_server, parent, label, rate, y);
}

/// Spawns a grey `"Label: x.x/s"` rate line centered at the given local `y`.
fn spawn_orb_tracker_rate_line_at(
    commands: &mut Commands,
    asset_server: &AssetServer,
    parent: Entity,
    label: &str,
    rate: f32,
    y: f32,
) {
    commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                format!("{label}: {rate:.1}/s"),
                gf::HUD_MICRO.text_style(asset_server, LIGHT_GREY),
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: Anchor::Center,
            transform: Transform::from_translation(Vec3::new(0., y, 1.)),
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .set_parent(parent);
}

/// Spawns a single mana-gain entry (icon or text label + percentage) at the given local pos.
fn spawn_mana_gain_entry(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    texture_atlas: &Handle<TextureAtlas>,
    parent: Entity,
    x: f32,
    y: f32,
    source: &ManaGainSource,
    pct: u32,
) {
    let row_root = commands
        .spawn(SpatialBundle::from_transform(Transform::from_translation(
            Vec3::new(x, y, 1.),
        )))
        .insert(RenderLayers::from_layers(&[3]))
        .set_parent(parent)
        .id();

    // Pick an icon: heirloom sprite, the mana orb sprite, or fall back to a text label.
    let icon_sprite = source
        .heirloom_icon()
        .map(|h| graphics.get_heirloom_icon(h))
        .or_else(|| {
            if matches!(source, ManaGainSource::ManaOrbs) {
                graphics
                    .spritesheet_map
                    .as_ref()
                    .and_then(|m| m.get(&WorldObject::ManaOrb).cloned())
            } else {
                None
            }
        });

    let is_icon = icon_sprite.is_some();
    if let Some(sprite) = icon_sprite {
        commands
            .spawn(SpriteSheetBundle {
                texture_atlas: texture_atlas.clone(),
                sprite,
                transform: Transform::from_translation(Vec3::new(
                    -ORB_TRACKER_COL_WIDTH * 0.5 + ORB_TRACKER_ICON_SIZE * 0.5 + 2.,
                    0.,
                    1.,
                )),
                ..default()
            })
            .insert(Sprite {
                custom_size: Some(Vec2::splat(ORB_TRACKER_ICON_SIZE)),
                ..default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .set_parent(row_root);
    } else {
        commands
            .spawn(Text2dBundle {
                text: Text::from_section(
                    source.label(),
                    gf::HUD_MICRO.text_style(asset_server, WHITE),
                )
                .with_alignment(TextAlignment::Center),
                text_anchor: Anchor::CenterLeft,
                transform: Transform::from_translation(Vec3::new(
                    -ORB_TRACKER_COL_WIDTH * 0.5 + 2.,
                    0.,
                    1.,
                )),
                ..default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .set_parent(row_root);
    }

    let x_offset = if is_icon {
        0.
    } else {
        (source.label().len().saturating_sub(5)) as f32 * 4. + 12.
    };
    commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                format!("{pct}%"),
                gf::HUD_MICRO.text_style(asset_server, WHITE),
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: Anchor::CenterLeft,
            transform: Transform::from_translation(Vec3::new(
                -ORB_TRACKER_COL_WIDTH * 0.5 + ORB_TRACKER_ICON_SIZE + 6. + x_offset,
                0.,
                2.,
            )),
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .set_parent(row_root);
}

fn spawn_mana_tracker_tooltip(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    anchor_pos: Vec3,
    tracker: &HeirloomTriggerCounts,
    window_elapsed_secs: f32,
) -> Entity {
    let consume_heirlooms = tracker.sorted_mana_entries();
    let consume_weapons = tracker.sorted_weapon_mana_entries();
    let consume_count = consume_heirlooms.len() + consume_weapons.len();
    let gain_entries = tracker.sorted_mana_gain_entries();
    let size = mana_tracker_tooltip_size(consume_count, gain_entries.len());
    let tooltip_pos = Vec3::new(
        anchor_pos.x,
        anchor_pos.y + HUD_FILL_PIXEL_SIZE.y * 0.5 + size.y * 0.5 + 6.,
        Z_DEPTH_HUD_ORB_TRACKERS_FOREGROUND,
    );

    let root = commands
        .spawn((
            SpatialBundle::from_transform(Transform::from_translation(tooltip_pos)),
            RenderLayers::from_layers(&[3]),
            ManaTrackerHudTooltip,
            Name::new("MANA TRACKER TOOLTIP"),
        ))
        .id();

    commands
        .spawn(SpriteBundle {
            sprite: Sprite {
                color: ICON_HOVER_TOOLTIP_BG_COLOR,
                custom_size: Some(size),
                ..default()
            },
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .set_parent(root);

    let title_y = size.y * 0.5 - ORB_TRACKER_PAD - ORB_TRACKER_TITLE_HEIGHT * 0.5;
    commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                "Mana Tracker",
                gf::ICON_HOVER_TOOLTIP.text_style(asset_server, LIGHT_BLUE),
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: Anchor::Center,
            transform: Transform::from_translation(Vec3::new(0., title_y, 1.)),
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .set_parent(root);

    let texture_atlas = graphics.texture_atlas.as_ref().unwrap().clone();
    let left_x = -size.x * 0.5 + ORB_TRACKER_PAD + ORB_TRACKER_COL_WIDTH * 0.5;

    // Running vertical cursor that walks down from just below the title.
    let mut section_top = title_y - ORB_TRACKER_TITLE_HEIGHT * 0.5;

    // --- Consume section: heirlooms and weapons that spent mana ---
    let consume_grid_top = section_top - ORB_TRACKER_ROW_HEIGHT * 0.5;
    if consume_count == 0 {
        commands
            .spawn(Text2dBundle {
                text: Text::from_section(
                    "No mana spent yet",
                    gf::HUD_MICRO.text_style(asset_server, LIGHT_GREY),
                )
                .with_alignment(TextAlignment::Center),
                text_anchor: Anchor::Center,
                transform: Transform::from_translation(Vec3::new(0., consume_grid_top, 1.)),
                ..default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .set_parent(root);
    } else {
        let mut consume_index = 0usize;
        for (heirloom, _amount) in &consume_heirlooms {
            let col = consume_index % ORB_TRACKER_COLUMNS;
            let row = consume_index / ORB_TRACKER_COLUMNS;
            let x = left_x + col as f32 * ORB_TRACKER_COL_WIDTH;
            let y = consume_grid_top - row as f32 * ORB_TRACKER_ROW_HEIGHT;
            let pct = tracker.mana_consumed_percentage(heirloom);

            let row_root = commands
                .spawn(SpatialBundle::from_transform(Transform::from_translation(
                    Vec3::new(x, y, 1.),
                )))
                .insert(RenderLayers::from_layers(&[3]))
                .set_parent(root)
                .id();

            commands
                .spawn(SpriteSheetBundle {
                    texture_atlas: texture_atlas.clone(),
                    sprite: graphics.get_heirloom_icon(heirloom.clone()),
                    transform: Transform::from_translation(Vec3::new(
                        -ORB_TRACKER_COL_WIDTH * 0.5 + ORB_TRACKER_ICON_SIZE * 0.5 + 2.,
                        0.,
                        1.,
                    )),
                    ..default()
                })
                .insert(Sprite {
                    custom_size: Some(Vec2::splat(ORB_TRACKER_ICON_SIZE)),
                    ..default()
                })
                .insert(RenderLayers::from_layers(&[3]))
                .set_parent(row_root);

            commands
                .spawn(Text2dBundle {
                    text: Text::from_section(
                        format!("{pct}%"),
                        gf::HUD_MICRO.text_style(asset_server, WHITE),
                    )
                    .with_alignment(TextAlignment::Center),
                    text_anchor: Anchor::CenterLeft,
                    transform: Transform::from_translation(Vec3::new(
                        -ORB_TRACKER_COL_WIDTH * 0.5 + ORB_TRACKER_ICON_SIZE + 6.,
                        0.,
                        2.,
                    )),
                    ..default()
                })
                .insert(RenderLayers::from_layers(&[3]))
                .set_parent(row_root);
            consume_index += 1;
        }
        for (weapon, _amount) in &consume_weapons {
            let col = consume_index % ORB_TRACKER_COLUMNS;
            let row = consume_index / ORB_TRACKER_COLUMNS;
            let x = left_x + col as f32 * ORB_TRACKER_COL_WIDTH;
            let y = consume_grid_top - row as f32 * ORB_TRACKER_ROW_HEIGHT;
            let pct = tracker.weapon_mana_consumed_percentage(weapon);

            let row_root = commands
                .spawn(SpatialBundle::from_transform(Transform::from_translation(
                    Vec3::new(x, y, 1.),
                )))
                .insert(RenderLayers::from_layers(&[3]))
                .set_parent(root)
                .id();

            if let Some(sprite) = graphics
                .spritesheet_map
                .as_ref()
                .and_then(|m| m.get(weapon).cloned())
            {
                commands
                    .spawn(SpriteSheetBundle {
                        texture_atlas: texture_atlas.clone(),
                        sprite,
                        transform: Transform::from_translation(Vec3::new(
                            -ORB_TRACKER_COL_WIDTH * 0.5 + ORB_TRACKER_ICON_SIZE * 0.5 + 2.,
                            0.,
                            1.,
                        )),
                        ..default()
                    })
                    .insert(Sprite {
                        custom_size: Some(Vec2::splat(ORB_TRACKER_ICON_SIZE)),
                        ..default()
                    })
                    .insert(RenderLayers::from_layers(&[3]))
                    .set_parent(row_root);
            }

            commands
                .spawn(Text2dBundle {
                    text: Text::from_section(
                        format!("{pct}%"),
                        gf::HUD_MICRO.text_style(asset_server, WHITE),
                    )
                    .with_alignment(TextAlignment::Center),
                    text_anchor: Anchor::CenterLeft,
                    transform: Transform::from_translation(Vec3::new(
                        -ORB_TRACKER_COL_WIDTH * 0.5 + ORB_TRACKER_ICON_SIZE + 6.,
                        0.,
                        2.,
                    )),
                    ..default()
                })
                .insert(RenderLayers::from_layers(&[3]))
                .set_parent(row_root);
            consume_index += 1;
        }
    }
    let consume_rows = consume_count.div_ceil(ORB_TRACKER_COLUMNS).max(1) as f32;
    section_top -= consume_rows * ORB_TRACKER_ROW_HEIGHT;

    // --- Consume rate line ---
    spawn_orb_tracker_rate_line_at(
        commands,
        asset_server,
        root,
        "Consume",
        tracker.mana_consumed_per_second(window_elapsed_secs),
        section_top - ORB_TRACKER_RATE_HEIGHT * 0.5,
    );
    section_top -= ORB_TRACKER_RATE_HEIGHT;

    // --- Gain section: where mana came from ---
    let gain_grid_top = section_top - ORB_TRACKER_ROW_HEIGHT * 0.5;
    if gain_entries.is_empty() {
        commands
            .spawn(Text2dBundle {
                text: Text::from_section(
                    "No mana gained yet",
                    gf::HUD_MICRO.text_style(asset_server, LIGHT_GREY),
                )
                .with_alignment(TextAlignment::Center),
                text_anchor: Anchor::Center,
                transform: Transform::from_translation(Vec3::new(0., gain_grid_top, 1.)),
                ..default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .set_parent(root);
    } else {
        for (index, (source, _amount)) in gain_entries.iter().enumerate() {
            let col = index % ORB_TRACKER_COLUMNS;
            let row = index / ORB_TRACKER_COLUMNS;
            let x = left_x + col as f32 * ORB_TRACKER_COL_WIDTH;
            let y = gain_grid_top - row as f32 * ORB_TRACKER_ROW_HEIGHT;
            let pct = tracker.mana_gained_percentage(source);
            spawn_mana_gain_entry(
                commands,
                graphics,
                asset_server,
                &texture_atlas,
                root,
                x,
                y,
                source,
                pct,
            );
        }
    }
    let gain_rows = gain_entries.len().div_ceil(ORB_TRACKER_COLUMNS).max(1) as f32;
    section_top -= gain_rows * ORB_TRACKER_ROW_HEIGHT;

    // --- Gain rate line (bottom) ---
    spawn_orb_tracker_rate_line_at(
        commands,
        asset_server,
        root,
        "Gain",
        tracker.mana_gained_per_second(window_elapsed_secs),
        section_top - ORB_TRACKER_RATE_HEIGHT * 0.5,
    );

    root
}

fn spawn_health_tracker_tooltip(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    anchor_pos: Vec3,
    tracker: &HeirloomTriggerCounts,
    window_elapsed_secs: f32,
) -> Entity {
    let entries = tracker.sorted_health_gain_entries();
    let size = orb_tracker_tooltip_size(entries.len());
    let tooltip_pos = Vec3::new(
        anchor_pos.x,
        anchor_pos.y + HUD_FILL_PIXEL_SIZE.y * 0.5 + size.y * 0.5 + 6.,
        Z_DEPTH_HUD_ORB_TRACKERS_FOREGROUND,
    );

    let root = commands
        .spawn((
            SpatialBundle::from_transform(Transform::from_translation(tooltip_pos)),
            RenderLayers::from_layers(&[3]),
            HealthTrackerHudTooltip,
            Name::new("HEALTH TRACKER TOOLTIP"),
        ))
        .id();

    commands
        .spawn(SpriteBundle {
            sprite: Sprite {
                color: ICON_HOVER_TOOLTIP_BG_COLOR,
                custom_size: Some(size),
                ..default()
            },
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .set_parent(root);

    let title_y = size.y * 0.5 - ORB_TRACKER_PAD - ORB_TRACKER_TITLE_HEIGHT * 0.5;
    commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                "Health Tracker",
                gf::ICON_HOVER_TOOLTIP.text_style(asset_server, LIGHT_RED),
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: Anchor::Center,
            transform: Transform::from_translation(Vec3::new(0., title_y, 1.)),
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .set_parent(root);

    let grid_top = title_y - ORB_TRACKER_TITLE_HEIGHT * 0.5 - ORB_TRACKER_ROW_HEIGHT * 0.5;
    let left_x = -size.x * 0.5 + ORB_TRACKER_PAD + ORB_TRACKER_COL_WIDTH * 0.5;

    if entries.is_empty() {
        commands
            .spawn(Text2dBundle {
                text: Text::from_section(
                    "No health gained yet",
                    gf::HUD_MICRO.text_style(asset_server, LIGHT_GREY),
                )
                .with_alignment(TextAlignment::Center),
                text_anchor: Anchor::Center,
                transform: Transform::from_translation(Vec3::new(0., grid_top, 1.)),
                ..default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .set_parent(root);
    } else {
        let texture_atlas = graphics.texture_atlas.as_ref().unwrap().clone();
        for (index, (source, _amount)) in entries.iter().enumerate() {
            let col = index % ORB_TRACKER_COLUMNS;
            let row = index / ORB_TRACKER_COLUMNS;
            let x = left_x + col as f32 * ORB_TRACKER_COL_WIDTH;
            let y = grid_top - row as f32 * ORB_TRACKER_ROW_HEIGHT;
            let pct = tracker.health_gained_percentage(source);

            let row_root = commands
                .spawn(SpatialBundle::from_transform(Transform::from_translation(
                    Vec3::new(x, y, 1.),
                )))
                .insert(RenderLayers::from_layers(&[3]))
                .set_parent(root)
                .id();
            let mut is_icon = false;
            if let Some(heirloom) = source.heirloom_icon() {
                is_icon = true;
                commands
                    .spawn(SpriteSheetBundle {
                        texture_atlas: texture_atlas.clone(),
                        sprite: graphics.get_heirloom_icon(heirloom),
                        transform: Transform::from_translation(Vec3::new(
                            -ORB_TRACKER_COL_WIDTH * 0.5 + ORB_TRACKER_ICON_SIZE * 0.5 + 2.,
                            0.,
                            1.,
                        )),
                        ..default()
                    })
                    .insert(Sprite {
                        custom_size: Some(Vec2::splat(ORB_TRACKER_ICON_SIZE)),
                        ..default()
                    })
                    .insert(RenderLayers::from_layers(&[3]))
                    .set_parent(row_root);
            } else {
                commands
                    .spawn(Text2dBundle {
                        text: Text::from_section(
                            source.label(),
                            gf::HUD_MICRO.text_style(asset_server, WHITE),
                        )
                        .with_alignment(TextAlignment::Center),
                        text_anchor: Anchor::CenterLeft,
                        transform: Transform::from_translation(Vec3::new(
                            -ORB_TRACKER_COL_WIDTH * 0.5 + 2.,
                            0.,
                            1.,
                        )),
                        ..default()
                    })
                    .insert(RenderLayers::from_layers(&[3]))
                    .set_parent(row_root);
            }
            let x_offset = if is_icon {
                0.
            } else {
                (source.label().len() - 5) as f32 * 4. + 12.
            };
            commands
                .spawn(Text2dBundle {
                    text: Text::from_section(
                        format!("{pct}%"),
                        gf::HUD_MICRO.text_style(asset_server, WHITE),
                    )
                    .with_alignment(TextAlignment::Center),
                    text_anchor: Anchor::CenterLeft,
                    transform: Transform::from_translation(Vec3::new(
                        -ORB_TRACKER_COL_WIDTH * 0.5 + ORB_TRACKER_ICON_SIZE + 6. + x_offset,
                        0.,
                        2.,
                    )),
                    ..default()
                })
                .insert(RenderLayers::from_layers(&[3]))
                .set_parent(row_root);
        }
    }

    spawn_orb_tracker_rate_line(
        commands,
        asset_server,
        root,
        "Gain",
        tracker.health_gained_per_second(window_elapsed_secs),
        size.y,
    );

    root
}

/// Shows a breakdown of mana spent per heirloom while hovering the mana orb.
pub fn handle_mana_tracker_hud_tooltip(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    cursor_pos: Res<CursorPos>,
    trigger_counts: Res<HeirloomTriggerCounts>,
    tracker_timer: Res<ManaTrackerResetTimer>,
    hit_detection_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut hover_targets: Query<(Entity, &GlobalTransform, &mut Interactable), With<ManaOrbHudHover>>,
    existing_tooltips: Query<Entity, With<ManaTrackerHudTooltip>>,
    mut last_hovered: Local<bool>,
    mut last_snapshot: Local<(u64, u64)>,
) {
    use Interaction;

    let hit_entity = super::ui_helpers::pointcast_2d(&cursor_pos, &hit_detection_sprites, None);

    for (entity, _, mut interactable) in hover_targets.iter_mut() {
        let is_hit = hit_entity
            .as_ref()
            .map(|(e, _s, _t)| *e == entity)
            .unwrap_or(false);
        if is_hit && !matches!(interactable.current(), Interaction::Hovering) {
            interactable.change(Interaction::Hovering);
        } else if !is_hit && matches!(interactable.current(), Interaction::Hovering) {
            interactable.change(Interaction::None);
        }
    }

    let hovering = hover_targets
        .iter()
        .any(|(_, _, interactable)| matches!(interactable.current(), Interaction::Hovering));

    let snapshot = (
        trigger_counts.total_mana_consumed(),
        trigger_counts.total_mana_gained(),
    );
    if hovering == *last_hovered && (!hovering || snapshot == *last_snapshot) {
        return;
    }
    *last_hovered = hovering;
    *last_snapshot = snapshot;

    for tooltip_e in existing_tooltips.iter() {
        commands.entity(tooltip_e).despawn_recursive();
    }

    if hovering {
        let anchor_pos = hover_targets
            .iter()
            .find(|(_, _, interactable)| matches!(interactable.current(), Interaction::Hovering))
            .map(|(_, transform, _)| transform.translation())
            .unwrap_or(Vec3::ZERO);
        let window_elapsed_secs = tracker_timer.0.elapsed().as_secs_f32();
        spawn_mana_tracker_tooltip(
            &mut commands,
            &graphics,
            &asset_server,
            anchor_pos,
            &trigger_counts,
            window_elapsed_secs,
        );
    }
}

/// Shows a breakdown of health gained by source while hovering the health orb.
pub fn handle_health_tracker_hud_tooltip(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    cursor_pos: Res<CursorPos>,
    trigger_counts: Res<HeirloomTriggerCounts>,
    tracker_timer: Res<ManaTrackerResetTimer>,
    hit_detection_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut hover_targets: Query<
        (Entity, &GlobalTransform, &mut Interactable),
        With<HealthOrbHudHover>,
    >,
    existing_tooltips: Query<Entity, With<HealthTrackerHudTooltip>>,
    mut last_hovered: Local<bool>,
    mut last_snapshot: Local<u64>,
) {
    use Interaction;

    let hit_entity = super::ui_helpers::pointcast_2d(&cursor_pos, &hit_detection_sprites, None);

    for (entity, _, mut interactable) in hover_targets.iter_mut() {
        let is_hit = hit_entity
            .as_ref()
            .map(|(e, _s, _t)| *e == entity)
            .unwrap_or(false);
        if is_hit && !matches!(interactable.current(), Interaction::Hovering) {
            interactable.change(Interaction::Hovering);
        } else if !is_hit && matches!(interactable.current(), Interaction::Hovering) {
            interactable.change(Interaction::None);
        }
    }

    let hovering = hover_targets
        .iter()
        .any(|(_, _, interactable)| matches!(interactable.current(), Interaction::Hovering));

    let snapshot = trigger_counts.total_health_gained();
    if hovering == *last_hovered && (!hovering || snapshot == *last_snapshot) {
        return;
    }
    *last_hovered = hovering;
    *last_snapshot = snapshot;

    for tooltip_e in existing_tooltips.iter() {
        commands.entity(tooltip_e).despawn_recursive();
    }

    if hovering {
        let anchor_pos = hover_targets
            .iter()
            .find(|(_, _, interactable)| matches!(interactable.current(), Interaction::Hovering))
            .map(|(_, transform, _)| transform.translation())
            .unwrap_or(Vec3::ZERO);
        let window_elapsed_secs = tracker_timer.0.elapsed().as_secs_f32();
        spawn_health_tracker_tooltip(
            &mut commands,
            &graphics,
            &asset_server,
            anchor_pos,
            &trigger_counts,
            window_elapsed_secs,
        );
    }
}

/// Helper function to get current scaling value for heirlooms that scale
fn get_heirloom_scaling_text(
    heirloom: Heirloom,
    skills: &PlayerSkills,
    coins: u32,
    max_health: i32,
    hunt_tracker: Option<&MaxHPHuntTracker>,
    crate_tracker: Option<&CrateBreakDamageTracker>,
    thorns_tracker: Option<&crate::player::combat_heirlooms::ThornsOnDamageTracker>,
    skill_power_hunt_tracker: Option<&SkillPowerHuntTracker>,
    energy_ball_tracker: Option<&EnergyBallBarrageTracker>,
    pickup_range: i32,
) -> Option<String> {
    match heirloom {
        Heirloom::GoldIntoDamage => {
            let stacks = skills.get_count(Heirloom::GoldIntoDamage);
            if stacks > 0 {
                let gold_bonus_percent = (coins as f32 / 10.0) * 1.0 * stacks as f32;
                Some(format!("(+{}% damage)", gold_bonus_percent as i32))
            } else {
                None
            }
        }
        Heirloom::MaxHPDamage => {
            let stacks = skills.get_count(Heirloom::MaxHPDamage);
            if stacks > 0 {
                let hp_bonus_percent = (max_health as f32 / 100.0) * 10.0 * stacks as f32;
                Some(format!("(+{}% damage)", hp_bonus_percent as i32))
            } else {
                None
            }
        }
        Heirloom::MaxHPHunt => {
            // Show total max HP gained from this heirloom
            if let Some(tracker) = hunt_tracker {
                if tracker.total_hp_gained > 0 {
                    Some(format!("(+{} Max HP)", tracker.total_hp_gained))
                } else {
                    None
                }
            } else {
                None
            }
        }
        Heirloom::CrateBreakDamage => {
            // Show current damage bonus from crate breaks
            if let Some(tracker) = crate_tracker {
                Some(format!("(+{:.1}% damage)", tracker.bonus_damage_percent))
            } else {
                None
            }
        }
        Heirloom::ThornsOnDamage => {
            // Show total thorns gained from taking damage
            if let Some(tracker) = thorns_tracker {
                if tracker.thorns_gained > 0 {
                    Some(format!("(+{}% Thorns)", tracker.thorns_gained))
                } else {
                    None
                }
            } else {
                None
            }
        }
        Heirloom::SkillPowerHunt => {
            if let Some(tracker) = skill_power_hunt_tracker {
                if tracker.bonus_skill_power > 0 {
                    Some(format!("(+{} Skill Power)", tracker.bonus_skill_power))
                } else {
                    None
                }
            } else {
                None
            }
        }
        Heirloom::EnergyBallBarrage => {
            let tracker = energy_ball_tracker?;
            Some(format!(
                "(Next cast: {}/{})",
                tracker.accumulated,
                crate::player::combat_heirlooms::ENERGY_BALL_DAMAGE_THRESHOLD
            ))
        }
        Heirloom::SkillCDReduction => {
            let stacks = skills.get_count(Heirloom::SkillCDReduction);
            if stacks > 0 {
                // Mirror `PlayerSkills::skill_cooldown_multiplier` (0.92 per stack).
                let reduction_percent = (1.0 - skills.skill_cooldown_multiplier()) * 100.0;
                Some(format!("(-{:.1}% cooldown)", reduction_percent))
            } else {
                None
            }
        }
        Heirloom::GravityScales => {
            let stacks = skills.get_count(Heirloom::GravityScales);
            if stacks > 0 {
                let size_bonus = pickup_range * 25 * stacks / 100;
                Some(format!("(+{} Size)", size_bonus))
            } else {
                None
            }
        }
        _ => None,
    }
}

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
    existing_skill_keybinds: Query<Entity, With<ActiveSkillKeyBackground>>,
    keybinds: Res<crate::keybinds::InputMappings>,
    mut prev_active_skills: Local<Vec<Option<ActiveSkill>>>, // Track previous active skills per slot to detect swaps
    slot_unlock_state: SkillSlotUnlockState,
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
            let heirloom_row_y = hud_heirloom_row_y(res.game_height);
            let heirloom_start_x = hud_heirloom_first_icon_x(res.game_width);

            let max_icons_per_row = super::hud_heirloom_max_per_row(&res).max(1);
            for (i, (heirloom, count)) in ordered_heirlooms.iter().enumerate() {
                const ROW_SPACING: f32 = 16.;

                let row = i / max_icons_per_row;
                let col = i % max_icons_per_row;

                let offset = Vec2::new(
                    heirloom_start_x + col as f32 * HUD_HEIRLOOM_ICON_SPACING,
                    heirloom_row_y - row as f32 * ROW_SPACING,
                );

                let rarity = heirloom_counts
                    .get(heirloom)
                    .map(|(_, rarity)| *rarity)
                    .unwrap_or(HeirloomRarity::Common);

                // Create the main icon with interactability directly attached
                let icon = commands
                    .spawn(SpriteSheetBundle {
                        sprite: graphics.get_heirloom_icon(heirloom.clone()),
                        texture_atlas: graphics.texture_atlas.as_ref().unwrap().clone(),
                        transform: Transform {
                            translation: offset.extend(Z_DEPTH_HUD_HEIRLOOM_ICONS),
                            scale: Vec3::new(1., 1., 1.),
                            ..Default::default()
                        },
                        ..Default::default()
                    })
                    .insert(Sprite {
                        custom_size: Some(Vec2::new(16., 16.)),
                        ..Default::default()
                    })
                    .insert(RenderLayers::from_layers(&[3]))
                    .insert(SkillHudIcon(heirloom.clone()))
                    .insert(HeirloomIconOutline::new(
                        rarity,
                        HeirloomIconOutlineStyle::Hud,
                    ))
                    .insert(super::interactions::Interactable::default())
                    .insert(UIElement::HeirloomHudIcon)
                    .insert(Name::new("HUD ICON!!"))
                    .id();

                // Add counter text if count > 1
                if *count > 1 {
                    let _counter_text = commands
                        .spawn(Text2dBundle {
                            text: Text::from_section(
                                count.to_string(),
                                TextStyle {
                                    font: asset_server.load("fonts/slkscr.ttf"),
                                    font_size: 8.4,
                                    color: WHITE,
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

        // Build list of active skill slots to display (first `VISIBLE_CLASS_SKILL_COUNT`
        // class skills; fourth class slot omitted while experimenting — see
        // `VISIBLE_CLASS_SKILL_COUNT` in `skills.rs`).
        let mut active_skill_slots = vec![
            (new_skills.active_skill_slot_0.clone(), 0),
            (new_skills.active_skill_slot_1.clone(), 1),
            (new_skills.active_skill_slot_2.clone(), 2),
        ];
        if VISIBLE_CLASS_SKILL_COUNT >= 4 {
            active_skill_slots.push((new_skills.active_skill_slot_3.clone(), 3));
        }
        if new_skills.active_skill_slot_4.is_some() {
            active_skill_slots.push((new_skills.active_skill_slot_4.clone(), 4));
        }

        // Initialize prev_active_skills if needed
        if prev_active_skills.len() < active_skill_slots.len() {
            prev_active_skills.resize(active_skill_slots.len(), None);
        }

        // Detect which slots had skill changes (swaps) BEFORE preserving cooldowns
        let mut skill_changed_slots = std::collections::HashSet::new();
        for (i, (active_skill_option, slot_index)) in active_skill_slots.iter().enumerate() {
            let current_skill = active_skill_option.as_ref().map(|s| s.active_skill.clone());
            let prev_skill = prev_active_skills.get(i).cloned().flatten();

            // If the skill changed (not just None -> Some or Some -> None, but actual different skill)
            if current_skill != prev_skill && (current_skill.is_some() || prev_skill.is_some()) {
                skill_changed_slots.insert(*slot_index);
            }
        }

        // Active Skill Icons
        // Preserve cooldown overlay state before despawning, but EXCLUDE slots where skill changed
        let mut preserved_cooldowns: Vec<(usize, f32, f32)> = Vec::new(); // (index, elapsed, duration)
        for (_, overlay) in existing_cooldown_overlays.iter() {
            // Don't preserve cooldown if the skill in this slot was swapped
            if skill_changed_slots.contains(&overlay.index) {
                continue;
            }
            let elapsed = overlay.timer.elapsed().as_secs_f32();
            let duration = overlay.timer.duration().as_secs_f32();
            if duration > 0.0 && elapsed < duration {
                // Only preserve if there's an active cooldown
                preserved_cooldowns.push((overlay.index, elapsed, duration));
            }
        }

        for e in existing_skill_keybinds.iter() {
            commands.entity(e).despawn_recursive();
        }

        prev_active_skill_icons.for_each(|e| {
            commands.entity(e).despawn_recursive();
        });

        // Update prev_active_skills for next time
        for (i, (active_skill_option, _)) in active_skill_slots.iter().enumerate() {
            if i < prev_active_skills.len() {
                prev_active_skills[i] =
                    active_skill_option.as_ref().map(|s| s.active_skill.clone());
            }
        }

        // Skills are centered around `HUD_SKILLS_CENTER_X` on the right side of the action
        // row. The group reserves an extra rightmost position for the pet skill slot
        // (spawned separately by `update_pet_skill_hud_slot`), so the 3 class skills stay
        // visually centered alongside the pet icon as a 4-wide group.
        let num_skills = (active_skill_slots.len() + 1) as f32;
        let skill_half_span = (num_skills - 1.0) * 0.5;
        for (i, (active_skill_option, slot_index)) in active_skill_slots.iter().enumerate() {
            let icon_bg = commands
                .spawn(SpatialBundle::from_transform(Transform::from_translation(
                    Vec3::new(
                        HUD_SKILLS_CENTER_X + (i as f32 - skill_half_span) * HUD_SKILL_SPACING_X,
                        -res.game_height / 2. + HUD_ACTION_ROW_Y_FROM_BOTTOM,
                        Z_DEPTH_HUD_ACTIVE_SKILLS,
                    ),
                )))
                .insert(RenderLayers::from_layers(&[3]))
                .insert(ActiveSkillIcon {
                    skill: active_skill_option
                        .clone()
                        .unwrap_or_default()
                        .active_skill
                        .clone(),
                    slot_index: *slot_index,
                })
                .insert(ActiveSkillSlotBg {
                    slot_index: *slot_index,
                })
                .id();
            let skill_x = HUD_SKILLS_CENTER_X + (i as f32 - skill_half_span) * HUD_SKILL_SPACING_X;
            let keybind = keybinds.get_active_skill_key(*slot_index);
            let (key_bg, key_text) = spawn_keybind_badge(
                &mut commands,
                &asset_server,
                keybind,
                Transform::from_translation(Vec3::new(
                    skill_x,
                    hud_keybind_badge_center_y(res.game_height),
                    2.,
                )),
                None,
                3,
            );
            commands
                .entity(key_bg)
                .insert(ActiveSkillKeyBackground { slot: *slot_index });
            commands
                .entity(key_text)
                .insert(ActiveSkillKeybindText { slot: *slot_index });
            let slot_locked = slot_unlock_state.is_slot_locked(*slot_index);
            if slot_locked {
                commands
                    .spawn(SpriteBundle {
                        texture: asset_server.load(ACTIVE_SKILL_LOCK_ICON_PATH),
                        sprite: Sprite {
                            custom_size: Some(ACTIVE_SKILL_LOCK_ICON_SIZE),
                            ..Default::default()
                        },
                        transform: Transform::from_translation(Vec3::new(0., 0., 1.)),
                        ..Default::default()
                    })
                    .insert(RenderLayers::from_layers(&[3]))
                    .insert(ActiveSkillLockIcon {
                        slot_index: *slot_index,
                    })
                    .insert(Name::new("HUD SKILL LOCK"))
                    .set_parent(icon_bg);
            } else if let Some(active_skill) = active_skill_option.clone() {
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
                    .insert(ActiveSkillIcon {
                        skill: active_skill.active_skill.clone(),
                        slot_index: *slot_index,
                    })
                    .insert(super::interactions::Interactable::default())
                    .insert(UIElement::HeirloomHudIcon) // Reuse this for hit detection
                    .insert(Name::new("HUD ICON!!"))
                    .set_parent(icon_bg);
            }

            // Preserve cooldown state if it exists for this slot (we already filtered out changed slots)
            // Use slot_index to match preserved cooldowns (they're stored by slot_index, not loop index)
            if let Some((_, elapsed, original_duration)) = preserved_cooldowns
                .iter()
                .find(|(idx, _, _)| *idx == *slot_index)
            {
                // Skill didn't change - preserve cooldown state.
                // Use the skill's base cooldown multiplied by the CURRENT total multiplier
                // to avoid compounding reductions when PlayerSkills changes multiple times.
                let multiplier = new_skills.skill_cooldown_multiplier();
                let base_cd = active_skill_option
                    .as_ref()
                    .map(|a| a.active_skill.get_base_cooldown())
                    .unwrap_or(*original_duration);
                let new_duration = base_cd * multiplier;

                // Preserve the same fractional progress through the cooldown
                let progress_percent = if *original_duration > 0.0 {
                    (*elapsed / *original_duration).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let new_elapsed = new_duration * progress_percent;

                spawn_skill_cooldown_overlay_with_elapsed(
                    icon_bg,
                    &mut commands,
                    new_duration,
                    new_elapsed,
                    *slot_index,
                );
            } else {
                // No preserved cooldown (either skill changed or no cooldown was active)
                spawn_skill_cooldown_overlay(icon_bg, &mut commands, 0.0, *slot_index);
            }

            // For slots 1-4 (class skills), add charge count text (all use the charge system)
            if *slot_index == 0 || *slot_index == 1 || *slot_index == 2 || *slot_index == 3 {
                // Query for charge tracker to get current charges
                // We'll update this in a separate system that runs after this
                let _charge_text = commands
                    .spawn(Text2dBundle {
                        text: Text::from_section(
                            "",
                            TextStyle {
                                font: asset_server.load("fonts/slkscr.ttf"),
                                font_size: 8.4,
                                color: WHITE,
                            },
                        ),
                        text_anchor: Anchor::Center,
                        transform: Transform {
                            translation: Vec3::new(1., 10., 4.), // Center bottom of icon
                            scale: Vec3::new(1., 1., 1.),
                            ..Default::default()
                        },
                        ..default()
                    })
                    .insert(RenderLayers::from_layers(&[3]))
                    .insert(SkillChargeText { slot: *slot_index })
                    .insert(Name::new("SKILL CHARGE TEXT"))
                    .set_parent(icon_bg)
                    .id();
            }
        }
    }
}

/// Updates skill charge text display for slots 1-4
pub fn update_skill_charge_text(
    class_slots: Query<&ClassSkillSlots, With<Player>>,
    mut charge_texts: Query<(&SkillChargeText, &mut Text)>,
) {
    let Ok(slots) = class_slots.get_single() else {
        return;
    };
    for (charge_text, mut text) in charge_texts.iter_mut() {
        if charge_text.slot < 4 {
            let tracker = &slots.0[charge_text.slot];
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

/// Z for the floating skill preview while dragging. Kept in sync with
/// `interactions::handle_dragging` (inventory drag icons): the UI orthographic camera
/// uses `far = 1000.0`, and values at/above the far plane can clip or depth-sort badly.
const ACTIVE_SKILL_DRAG_PREVIEW_Z: f32 = 998.;

/// Drag-and-drop reordering for the active skill HUD slots (three class skills
/// by default, plus the optional blessing bonus slot).
///
/// Behaviour:
/// - Left-press on a populated slot icon starts a drag — the original icon stays in
///   place but is faded to ~40% alpha, and a follow-the-cursor preview sprite is spawned.
/// - Releasing left-click on a *different* slot bg swaps the two slot contents in
///   `PlayerSkills` and (for slots 0–3) swaps the matching `ClassSkillSlots` runtime
///   so cooldowns and charges follow the skill rather than the slot index.
/// - Releasing on the same slot or on no slot cancels the drag — alpha is restored
///   and no swap is applied.
///
/// The drop hit-test uses a direct AABB check on `ActiveSkillSlotBg` anchors instead
/// of `pointcast_2d` so the slot is reliably picked even when the icon child is in the
/// same screen position (`pointcast_2d` is order-dependent and would otherwise return
/// the icon entity).
pub fn handle_active_skill_slot_drag_drop(
    mut commands: Commands,
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    mut drag_state: ResMut<ActiveSkillDragState>,
    mut skill_icons: Query<
        (
            Entity,
            &ActiveSkillIcon,
            &UIElement,
            &mut Sprite,
            &Interactable,
        ),
        Without<ActiveSkillSlotBg>,
    >,
    slot_bgs: Query<(&ActiveSkillSlotBg, &GlobalTransform), Without<UIElement>>,
    mut player_skills: Query<&mut PlayerSkills, With<Player>>,
    mut class_slots_q: Query<&mut ClassSkillSlots, With<Player>>,
    mut drag_preview_t: Query<&mut Transform, With<ActiveSkillDragIcon>>,
    graphics: Res<Graphics>,
    inv_dragging: Query<(), With<DraggedItem>>,
) {
    let cursor = cursor_pos.ui_coords.truncate();

    if drag_state.origin_slot.is_none() && !inv_dragging.is_empty() {
        return;
    }

    let hovered_slot = slot_bgs.iter().find_map(|(bg, xform)| {
        let pos = xform.translation();
        let half = HUD_SKILL_SLOT_HIT_SIZE * 0.5;
        if cursor.x >= pos.x - half.x
            && cursor.x <= pos.x + half.x
            && cursor.y >= pos.y - half.y
            && cursor.y <= pos.y + half.y
        {
            Some(bg.slot_index)
        } else {
            None
        }
    });

    if drag_state.origin_slot.is_none() && mouse_input.just_pressed(MouseButton::Left) {
        let mut start: Option<(usize, ActiveSkill)> = None;
        for (_, icon, ui_elem, _, interactable) in skill_icons.iter() {
            if *ui_elem != UIElement::HeirloomHudIcon {
                continue;
            }
            if !matches!(interactable.current(), Interaction::Hovering) {
                continue;
            }
            if let Ok(skills) = player_skills.get_single() {
                if let Some(skill) = skills.get_active_skill_in_slot(icon.slot_index) {
                    start = Some((icon.slot_index, skill));
                    break;
                }
            }
        }

        if let Some((slot_index, skill)) = start {
            let drag_e = commands
                .spawn(SpriteBundle {
                    texture: graphics.get_active_skill_icon(skill),
                    sprite: Sprite {
                        custom_size: Some(Vec2::new(16., 16.)),
                        color: Color::rgba(1., 1., 1., 0.7),
                        ..default()
                    },
                    transform: Transform::from_translation(Vec3::new(
                        cursor.x,
                        cursor.y,
                        ACTIVE_SKILL_DRAG_PREVIEW_Z,
                    )),
                    ..default()
                })
                .insert(RenderLayers::from_layers(&[3]))
                .insert(ActiveSkillDragIcon)
                .insert(Name::new("ACTIVE SKILL DRAG ICON"))
                .id();

            drag_state.origin_slot = Some(slot_index);
            drag_state.drag_icon_entity = Some(drag_e);
        }
    }

    let Some(origin) = drag_state.origin_slot else {
        return;
    };

    if let Some(drag_e) = drag_state.drag_icon_entity {
        if let Ok(mut t) = drag_preview_t.get_mut(drag_e) {
            t.translation = Vec3::new(cursor.x, cursor.y, ACTIVE_SKILL_DRAG_PREVIEW_Z);
        }
    }

    for (_, icon, ui_elem, mut sprite, _) in skill_icons.iter_mut() {
        if *ui_elem == UIElement::HeirloomHudIcon && icon.slot_index == origin {
            sprite.color = Color::rgba(1., 1., 1., 0.4);
        }
    }

    if mouse_input.just_released(MouseButton::Left) {
        if let Some(drag_e) = drag_state.drag_icon_entity {
            commands.entity(drag_e).despawn_recursive();
        }

        for (_, icon, ui_elem, mut sprite, _) in skill_icons.iter_mut() {
            if *ui_elem == UIElement::HeirloomHudIcon && icon.slot_index == origin {
                sprite.color = Color::WHITE;
            }
        }

        if let Some(target) = hovered_slot {
            if target != origin {
                if let Ok(mut skills) = player_skills.get_single_mut() {
                    let from = skills.get_active_skill_choice_in_slot(origin).cloned();
                    let to = skills.get_active_skill_choice_in_slot(target).cloned();
                    set_skill_slot(&mut skills, origin, to);
                    set_skill_slot(&mut skills, target, from);
                }
                if origin < 4 && target < 4 {
                    if let Ok(mut runtime) = class_slots_q.get_single_mut() {
                        runtime.0.swap(origin, target);
                    }
                }
            }
        }

        drag_state.origin_slot = None;
        drag_state.drag_icon_entity = None;
    }
}

/// Sets the chosen `ActiveSkillChoiceState` (or clears it) on the given slot index.
/// Mirrors `PlayerSkills::insert_active_skill` but works with `Option` to support
/// emptying slot 4 during a drag-swap.
fn set_skill_slot(skills: &mut PlayerSkills, slot: usize, choice: Option<ActiveSkillChoiceState>) {
    match slot {
        0 => skills.active_skill_slot_0 = choice,
        1 => skills.active_skill_slot_1 = choice,
        2 => skills.active_skill_slot_2 = choice,
        3 => skills.active_skill_slot_3 = choice,
        4 => skills.active_skill_slot_4 = choice,
        _ => {}
    }
}

/// Cancels any in-progress active skill drag and despawns the floating preview. Called
/// when the player exits `GameState::Main` (death, returning to menu) so the drag state
/// resource never references stale entities on the next run.
pub fn cancel_active_skill_drag_on_state_exit(
    mut commands: Commands,
    mut drag_state: ResMut<ActiveSkillDragState>,
    mut skill_icons: Query<(&ActiveSkillIcon, &UIElement, &mut Sprite)>,
) {
    if let Some(drag_e) = drag_state.drag_icon_entity.take() {
        if let Some(ec) = commands.get_entity(drag_e) {
            ec.despawn_recursive();
        }
    }
    if let Some(origin) = drag_state.origin_slot.take() {
        for (icon, ui_elem, mut sprite) in skill_icons.iter_mut() {
            if *ui_elem == UIElement::HeirloomHudIcon && icon.slot_index == origin {
                sprite.color = Color::WHITE;
            }
        }
    }
}

/// Spawns a hotbar keybind badge (key-cap + text) above the given hotbar slot entity,
/// tagged with `HotbarKeyBackground`/`HotbarKeybindText` so `update_hotbar_keybind_text`
/// can refresh it when the player rebinds the key.
///
/// Called both from initial HUD setup and from `update_inventory_ui` after a dirty
/// hotbar slot is despawned and respawned (which would otherwise nuke its badge).
pub fn spawn_hotbar_keybind_badge_for_slot(
    commands: &mut Commands,
    asset_server: &AssetServer,
    keybinds: &crate::keybinds::InputMappings,
    slot: usize,
    game_height: f32,
) {
    let key = keybinds.get_hotbar_key(slot);
    let (bg_entity, text_entity) = spawn_keybind_badge(
        commands,
        asset_server,
        key,
        Transform::from_translation(Vec3::new(
            hud_hotbar_slot_center_x(slot),
            hud_keybind_badge_center_y(game_height),
            2.,
        )),
        None,
        3,
    );
    commands
        .entity(bg_entity)
        .insert(HotbarKeyBackground { slot });
    commands
        .entity(text_entity)
        .insert(HotbarKeybindText { slot });
}

/// Spawns the on-screen hotbar (`HUD_HOTBAR_SLOTS` slots, each bound to a user-configurable
/// key that defaults to `1`-`N`) and a static keybind badge above each slot. The underlying
/// `Inventory::items` container still has more slots (for passive storage), but only the
/// first `HUD_HOTBAR_SLOTS` are rendered and wired to keys.
pub fn setup_hotbar_hud(
    mut commands: Commands,
    graphics: Res<Graphics>,
    inv_query: Query<Entity, With<InventoryUI>>,
    inv_state: Res<InventoryState>,
    asset_server: Res<AssetServer>,
    mut inv: Query<&mut Inventory>,
    inv_ui_state: Res<State<UIState>>,
    keybinds: Res<crate::keybinds::InputMappings>,
    resolution: Res<ScreenResolution>,
) {
    for (slot_index, item) in inv
        .single_mut()
        .items
        .items
        .iter()
        .take(HUD_HOTBAR_SLOTS)
        .enumerate()
    {
        let _slot_entity = spawn_inv_slot(
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
            &resolution,
        );

        spawn_hotbar_keybind_badge_for_slot(
            &mut commands,
            &asset_server,
            &keybinds,
            slot_index,
            resolution.game_height,
        );
    }
}

pub fn update_mana_bar(
    player_mana: Query<
        (&CurrentMana, &MaxMana),
        (Or<(Changed<CurrentMana>, Changed<MaxMana>)>, With<Player>),
    >,
    mana_bar_query: Query<&Handle<crate::ui::hud_bar_fill::HudBarFillMaterial>, With<ManaBar>>,
    mut mana_text: Query<&mut Text, With<ManaBarText>>,
    mut materials: ResMut<Assets<crate::ui::hud_bar_fill::HudBarFillMaterial>>,
) {
    let Ok((current_mana, max_mana)) = player_mana.get_single() else {
        return;
    };
    let Ok(handle) = mana_bar_query.get_single() else {
        return;
    };
    if let Some(material) = materials.get_mut(handle) {
        material.fill = (current_mana.0 as f32 / max_mana.0.max(1) as f32).clamp(0.0, 1.0);
    }
    if let Ok(mut text) = mana_text.get_single_mut() {
        text.sections[0].value = format!("{}", current_mana.0);
    }
}

/// Era progress timeline (`Timeline.png`) with draggable-style arrows (`TimelineArrows.png`).
pub fn setup_timeline_hud(
    mut commands: Commands,
    graphics: Res<Graphics>,
    res: Res<ScreenResolution>,
) {
    let row_y = hud_row_below_xp_y(res.game_height) + 2.;
    let timeline_x = hud_timeline_center_x(res.game_width);

    let timeline = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::Timeline),
            sprite: Sprite {
                custom_size: Some(HUD_TIMELINE_SIZE),
                ..default()
            },
            transform: Transform::from_translation(Vec3::new(timeline_x, row_y, 6.)),
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(TimelineHUD)
        .insert(UiShadow::hud())
        .insert(Name::new("TIMELINE HUD"))
        .id();

    commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::TimelineArrows),
            sprite: Sprite {
                custom_size: Some(HUD_TIMELINE_ARROWS_SIZE),
                ..default()
            },
            transform: Transform::from_translation(Vec3::new(
                hud_timeline_arrow_local_x(0.),
                0.,
                1.,
            )),
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(TimelineProgressArrows)
        .insert(Name::new("TIMELINE ARROWS"))
        .set_parent(timeline);
}

fn era_timeline_progress(era_timer: &EraTimer, infinite_mode: &InfiniteMode) -> f32 {
    if infinite_mode.active {
        1.
    } else {
        1. - (era_timer.remaining_seconds / ERA_TIMER_SECONDS).clamp(0., 1.)
    }
}

/// Endless-mode timer HUD (hidden until infinite mode is active).
pub fn setup_era_timer_hud(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    res: Res<ScreenResolution>,
    existing: Query<Entity, With<EraTimerHUD>>,
) {
    if !existing.is_empty() {
        return;
    }

    let row_y = hud_row_below_xp_y(res.game_height) + 2.;
    let timer_width = HUD_ERA_TIMER_ENDLESS_WIDTH;
    let timer_x = hud_era_timer_center_x(res.game_width, timer_width);

    let era_timer_frame = commands
        .spawn((
            SpriteBundle {
                sprite: Sprite {
                    color: Color::rgba(0.4, 0.1, 0.1, 0.8),
                    custom_size: Some(Vec2::new(timer_width, 24.)),
                    ..default()
                },
                transform: Transform {
                    translation: Vec3::new(timer_x, row_y, 5.),
                    ..Default::default()
                },
                visibility: Visibility::Hidden,
                ..default()
            },
            Name::new("ERA TIMER HUD"),
            RenderLayers::from_layers(&[3]),
            EraTimerHUD,
        ))
        .id();

    let _timer_text = commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    "ENDLESS",
                    TextStyle {
                        font: asset_server.load("fonts/alagard.ttf"),
                        font_size: 15.0,
                        color: RED,
                    },
                ),
                transform: Transform {
                    translation: Vec3::new(0., -2., 1.),
                    ..Default::default()
                },
                ..default()
            },
            EraTimerText,
            RenderLayers::from_layers(&[3]),
        ))
        .set_parent(era_timer_frame);

    // Spawn the endless elapsed timer (hidden initially, shown only during endless mode)
    let _endless_elapsed_text = commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    "00:00",
                    TextStyle {
                        font: asset_server.load("fonts/slkscr.ttf"),
                        font_size: 8.4,
                        color: WHITE.with_a(0.), // Hidden initially
                    },
                ),
                transform: Transform {
                    translation: Vec3::new(0., -10., 1.), // Below the ENDLESS text
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..default()
            },
            EndlessElapsedText,
            RenderLayers::from_layers(&[3]),
        ))
        .set_parent(era_timer_frame);
}

/// Update era timer HUD text
pub fn handle_update_era_timer_hud(
    era_timer: Res<crate::night::EraTimer>,
    infinite_mode: Res<crate::night::InfiniteMode>,
    mut timer_text: Query<&mut Text, (With<EraTimerText>, Without<EndlessElapsedText>)>,
    mut elapsed_text: Query<&mut Text, (With<EndlessElapsedText>, Without<EraTimerText>)>,
    mut hud_transforms: ParamSet<(
        Query<(&mut Sprite, &mut Transform, &mut Visibility), With<EraTimerHUD>>,
        Query<&mut Transform, With<TimelineHUD>>,
        Query<&mut Transform, With<TimelineProgressArrows>>,
        Query<(&CurrencyHudSlotIndex, &mut Transform), With<CurrencyHudBackground>>,
        Query<&mut Transform, With<ProgressHudBar>>,
    )>,
    res: Res<ScreenResolution>,
) {
    let endless = infinite_mode.active;

    for mut text in timer_text.iter_mut() {
        if endless {
            text.sections[0].value = "ENDLESS".to_string();
            text.sections[0].style.color = RED;
        } else {
            text.sections[0].style.color = WHITE.with_a(0.);
        }
    }

    for mut text in elapsed_text.iter_mut() {
        if endless {
            text.sections[0].value = infinite_mode.get_elapsed_display_string();
            text.sections[0].style.color = YELLOW;
        } else {
            text.sections[0].style.color = WHITE.with_a(0.);
        }
    }

    let row_y = hud_row_below_xp_y(res.game_height) + 2.;
    let progress_row_y = hud_row_below_xp_y(res.game_height);

    for (slot, mut transform) in hud_transforms.p3().iter_mut() {
        transform.translation.x = match slot.0 {
            0 => hud_currency_first_center_x(res.game_width),
            1 => hud_currency_second_center_x(res.game_width),
            _ => transform.translation.x,
        };
        transform.translation.y = progress_row_y;
    }
    for mut progress_txfm in hud_transforms.p4().iter_mut() {
        progress_txfm.translation.x = hud_progress_bar_center_x(&res);
        progress_txfm.translation.y = progress_row_y + 1.;
    }

    let timer_size = Vec2::new(HUD_ERA_TIMER_ENDLESS_WIDTH, 24.);
    for (mut sprite, mut transform, mut visibility) in hud_transforms.p0().iter_mut() {
        transform.translation.y = row_y;
        transform.translation.x = hud_era_timer_center_x(res.game_width, timer_size.x);
        sprite.custom_size = Some(timer_size);
        sprite.color = Color::rgba(0.4, 0.1, 0.1, 0.8);
        *visibility = if endless {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }

    let progress = era_timeline_progress(&era_timer, &infinite_mode);
    let arrow_x = hud_timeline_arrow_local_x(progress);

    for mut timeline_txfm in hud_transforms.p1().iter_mut() {
        timeline_txfm.translation.y = row_y;
        timeline_txfm.translation.x = hud_timeline_center_x(res.game_width);
    }

    for mut arrows_txfm in hud_transforms.p2().iter_mut() {
        arrows_txfm.translation.x = arrow_x;
    }
}

#[derive(Component)]
pub struct SkillCooldownOverlay {
    pub timer: Timer,
    pub index: usize,
}

/// Cooldown progress for a skill hotkey slot (same sources as [`SkillCooldownOverlay`]).
///
/// `Some(0.0)` = just used, `Some(1.0)` = almost ready, `None` = ready / not on cooldown.
pub fn skill_slot_cooldown_progress(
    slot_index: usize,
    roll_slot: Option<usize>,
    slots: &ClassSkillSlots,
    dash_cooldown: &Timer,
    overlay: Option<&SkillCooldownOverlay>,
) -> Option<f32> {
    // Roll now lives in the slot charge system (slot_index < 4). Only fall back to the
    // legacy dash cooldown timer if the slot hasn't been set up with charges yet.
    if Some(slot_index) == roll_slot && !(slot_index < 4 && slots.0[slot_index].max_charges > 0) {
        if dash_cooldown.finished() {
            return None;
        }
        let duration = dash_cooldown.duration().as_secs_f32();
        if duration <= 0.0 {
            return None;
        }
        let elapsed = dash_cooldown.elapsed().as_secs_f32();
        return Some((elapsed / duration).clamp(0.0, 1.0));
    }

    if slot_index < 4 {
        let tracker = &slots.0[slot_index];
        if tracker.max_charges > 0 && tracker.current_charges < tracker.max_charges {
            let duration = tracker.cooldown_timer.duration().as_secs_f32();
            if duration <= 0.0 {
                return None;
            }
            let elapsed = tracker.cooldown_timer.elapsed().as_secs_f32();
            return Some((elapsed / duration).clamp(0.0, 1.0));
        }
        return None;
    }

    let overlay = overlay?;
    if overlay.timer.finished() {
        return None;
    }
    Some(overlay.timer.percent())
}

/// Marker component for decorative XP shards that rain during skill choice UI
#[derive(Component)]
pub struct DecorativeXPShard {
    pub fall_speed: f32,
    pub start_y: f32,
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
    let duration = duration.max(0.001); // avoid negative/zero Duration panic
    let mut timer = Timer::from_seconds(duration, TimerMode::Once);
    // Tick the timer to the preserved elapsed time to maintain visual state
    let elapsed = elapsed.max(0.0);
    if elapsed > 0.0 && duration > 0.0 {
        timer.tick(Duration::from_secs_f32(elapsed.min(duration)));
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
    class_slots: Query<&ClassSkillSlots, With<Player>>,
    player_skills: Query<&PlayerSkills, With<Player>>,
    game: Res<crate::Game>,
    time: Res<Time>,
) {
    let roll_slot = player_skills
        .get_single()
        .ok()
        .and_then(|skills| skills.has_active_skill(ActiveSkill::Roll));

    let Ok(slots) = class_slots.get_single() else {
        return;
    };

    let dash_cooldown = &game.player_state.player_dash_cooldown;

    for (mut sprite, mut overlay) in overlays.iter_mut() {
        if overlay.index >= 4 {
            overlay.timer.tick(time.delta());
        }

        let height = skill_slot_cooldown_progress(
            overlay.index,
            roll_slot,
            slots,
            dash_cooldown,
            Some(&*overlay),
        )
        .map(|progress| 16.0 * (1.0 - progress))
        .unwrap_or(0.0);

        sprite.custom_size = Some(Vec2::new(16., height));
    }
}

pub fn handle_active_skill_event(
    mut active_skill_used: EventReader<ActiveSkillUsedEvent>,
    mut overlays: Query<&mut SkillCooldownOverlay>,
    class_slots: Query<&ClassSkillSlots, With<Player>>,
    player_skills: Query<&PlayerSkills, With<Player>>,
) {
    let roll_slot = player_skills
        .get_single()
        .ok()
        .and_then(|skills| skills.has_active_skill(ActiveSkill::Roll));

    let Ok(slots) = class_slots.get_single() else {
        return;
    };

    for e in active_skill_used.iter() {
        if Some(e.slot) == roll_slot {
            continue;
        }

        let has_class_slot = e.slot < 4 && slots.0[e.slot].max_charges > 0;

        for mut overlay in overlays.iter_mut() {
            if overlay.index == e.slot {
                if has_class_slot {
                    continue;
                }

                overlay.timer.reset();
                let cooldown_secs = e.cooldown.max(0.0);
                overlay
                    .timer
                    .set_duration(Duration::from_secs_f32(cooldown_secs));
            }
        }
    }
}

/// Updates the "Xs" cooldown text on active skill HUD tooltips.
pub fn update_skill_tooltip_cooldown(
    mut cooldown_texts: Query<(&Parent, &mut Text), With<SkillTooltipCooldownText>>,
    tooltip_containers: Query<&ActiveSkillHudTooltipSkill>,
    game: Res<crate::Game>,
    player_skills: Query<&PlayerSkills, With<Player>>,
    class_slots: Query<&ClassSkillSlots, With<Player>>,
    overlays: Query<&SkillCooldownOverlay>,
) {
    let roll_slot = player_skills
        .get_single()
        .ok()
        .and_then(|s| s.has_active_skill(ActiveSkill::Roll));

    let Ok(slots) = class_slots.get_single() else {
        return;
    };

    for (parent, mut text) in cooldown_texts.iter_mut() {
        let Ok(tooltip_skill) = tooltip_containers.get(parent.get()) else {
            continue;
        };
        let slot_index = tooltip_skill.0;

        let roll_uses_slot = slot_index < 4 && slots.0[slot_index].max_charges > 0;
        let (remaining, max_cooldown) = if Some(slot_index) == roll_slot && !roll_uses_slot {
            let dash = &game.player_state.player_dash_cooldown;
            let max = dash.duration().as_secs_f32();
            let remaining = (max - dash.elapsed().as_secs_f32()).max(0.0);
            (remaining, max)
        } else if slot_index < 4 {
            let tracker = &slots.0[slot_index];
            if tracker.max_charges > 0 {
                let max = tracker.cooldown_timer.duration().as_secs_f32();
                let remaining = if tracker.current_charges < tracker.max_charges {
                    (max - tracker.cooldown_timer.elapsed().as_secs_f32()).max(0.0)
                } else {
                    0.0
                };
                (remaining, max)
            } else {
                overlays
                    .iter()
                    .find(|o| o.index == slot_index)
                    .map(|o| {
                        let max = o.timer.duration().as_secs_f32();
                        let remaining = (max - o.timer.elapsed().as_secs_f32()).max(0.0);
                        (remaining, max)
                    })
                    .unwrap_or((0.0, 0.0))
            }
        } else {
            overlays
                .iter()
                .find(|o| o.index == slot_index)
                .map(|o| {
                    let max = o.timer.duration().as_secs_f32();
                    let remaining = (max - o.timer.elapsed().as_secs_f32()).max(0.0);
                    (remaining, max)
                })
                .unwrap_or((0.0, 0.0))
        };

        text.sections[0].value = if remaining > 0.05 {
            format!("{:.1}s", remaining)
        } else if max_cooldown > 0.0 {
            format!("{:.1}s", max_cooldown)
        } else {
            String::new()
        };
    }
}

/// Updates the cooldown corner text on pet skill HUD tooltips.
pub fn update_pet_skill_tooltip_cooldown(
    mut cooldown_texts: Query<&mut Text, With<PetSkillTooltipCooldownText>>,
    tooltips: Query<Entity, With<PetSkillHudTooltip>>,
    pet_q: Query<&Pet>,
    slime: Query<&crate::pets::pet_abilities::SlimeShieldTimer, With<Pet>>,
    fairy: Query<&crate::pets::pet_abilities::FairyHealTimer, With<Pet>>,
    porkipine: Query<&crate::pets::pet_abilities::PorkipineDamageTimer, With<Pet>>,
    coin: Query<&crate::pets::pet_abilities::GoldenPigCoinTimer, With<Pet>>,
) {
    if tooltips.is_empty() {
        return;
    }

    let Some(pet) = pet_q.iter().next() else {
        return;
    };

    let label = if let Some((elapsed, duration)) =
        pet_ability_cooldown(pet, &slime, &fairy, &porkipine, &coin)
    {
        let remaining = (duration - elapsed).max(0.0);
        if remaining > 0.05 {
            format!("{:.1}s", remaining)
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    for mut text in cooldown_texts.iter_mut() {
        text.sections[0].value = label.clone();
    }
}

pub fn update_active_skill_keybind_text(
    keybinds: Res<crate::keybinds::InputMappings>,
    mut texts: Query<(&ActiveSkillKeybindText, &mut Text)>,
) {
    if !keybinds.is_changed() {
        return;
    }

    for (keybind_text, mut text) in texts.iter_mut() {
        let key = keybinds.get_active_skill_key(keybind_text.slot);
        text.sections[0].value = crate::keybinds::get_key_display_name(key);
    }
}

pub fn update_hotbar_keybind_text(
    keybinds: Res<crate::keybinds::InputMappings>,
    mut texts: Query<(&HotbarKeybindText, &mut Text)>,
) {
    if !keybinds.is_changed() {
        return;
    }

    for (keybind_text, mut text) in texts.iter_mut() {
        let key = keybinds.get_hotbar_key(keybind_text.slot);
        text.sections[0].value = crate::keybinds::get_key_display_name(key);
    }
}

pub fn update_inventory_keybind_text(
    keybinds: Res<crate::keybinds::InputMappings>,
    mut texts: Query<&mut Text, With<InventoryKeybindText>>,
) {
    if !keybinds.is_changed() {
        return;
    }

    let inventory_key = keybinds.get_inventory_key();
    for mut text in texts.iter_mut() {
        text.sections[0].value = crate::keybinds::get_key_display_name(inventory_key);
    }
}

pub fn update_minimap_keybind_text(
    keybinds: Res<crate::keybinds::InputMappings>,
    mut texts: Query<&mut Text, With<MinimapKeybindText>>,
) {
    if !keybinds.is_changed() {
        return;
    }

    let minimap_key = keybinds.get_minimap_key();
    for mut text in texts.iter_mut() {
        text.sections[0].value = crate::keybinds::get_key_display_name(minimap_key);
    }
}

pub const CONSUMABLE_BUFF_HUD_ICON_PX: f32 = 14.;

#[derive(Component, Clone)]
pub struct ConsumableBuffHudMarker {
    pub item_stack: ItemStack,
}

#[derive(Component)]
pub struct ConsumableBuffHudDurationOverlay {
    pub hud_slot: usize,
}

fn consumable_buff_hud_layout_keys(buffs: &ActiveConsumableBuffs) -> Vec<(usize, WorldObject)> {
    buffs
        .entries
        .iter()
        .enumerate()
        .filter_map(|(i, e)| e.item_stack.as_ref().map(|s| (i, s.obj_type)))
        .collect()
}

/// Rebuilds consumable-buff HUD icons when the set of buffs (indices + item types) changes.
pub fn sync_consumable_buff_hud(
    mut commands: Commands,
    player: Query<&ActiveConsumableBuffs, With<Player>>,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    mut last_keys: Local<Option<Vec<(usize, WorldObject)>>>,
    existing: Query<Entity, With<ConsumableBuffHudMarker>>,
    res: Res<ScreenResolution>,
) {
    let Ok(buffs) = player.get_single() else {
        return;
    };
    let keys = consumable_buff_hud_layout_keys(buffs);
    if last_keys.as_ref() == Some(&keys) {
        return;
    }
    *last_keys = Some(keys);

    for e in existing.iter() {
        commands.entity(e).despawn_recursive();
    }

    let visible: Vec<(usize, &crate::attributes::ConsumableBuffEntry)> = buffs
        .entries
        .iter()
        .enumerate()
        .filter(|(_, e)| e.item_stack.is_some())
        .collect();

    for (hud_slot, (_entry_index, entry)) in visible.iter().enumerate() {
        let stack = entry.item_stack.as_ref().unwrap().clone();
        let i = hud_slot as f32;
        let x = -120.
            - 2.
            - (CONSUMABLE_BUFF_HUD_ICON_PX / 2.)
            - i * (CONSUMABLE_BUFF_HUD_ICON_PX + 2.);
        let y = -res.game_height / 2. + 14.;

        let icon_root = commands
            .spawn((
                SpriteBundle {
                    texture: graphics.get_ui_element_texture(UIElement::InventorySlotHotbar),
                    sprite: Sprite {
                        custom_size: Some(Vec2::splat(CONSUMABLE_BUFF_HUD_ICON_PX)),
                        ..default()
                    },
                    transform: Transform::from_translation(Vec3::new(x, y, 2.)),
                    ..default()
                },
                RenderLayers::from_layers(&[3]),
                Name::new("consumable_buff_hud"),
                Interactable::default(),
                ConsumableBuffHudMarker {
                    item_stack: stack.clone(),
                },
            ))
            .id();

        let icon_e = spawn_item_stack_icon(
            &mut commands,
            &graphics,
            &stack,
            &asset_server,
            Vec2::ZERO,
            Vec2::ZERO,
            3,
        );
        commands
            .entity(icon_e)
            .insert(Transform::from_translation(Vec3::new(0., 0., 1.)));
        commands.entity(icon_root).add_child(icon_e);

        let _ = spawn_consumable_buff_duration_overlay(icon_root, &mut commands, hud_slot);
    }
}

fn spawn_consumable_buff_duration_overlay(
    parent: Entity,
    commands: &mut Commands,
    hud_slot: usize,
) -> Entity {
    commands
        .spawn((
            SpriteBundle {
                sprite: Sprite {
                    color: Color::rgba(1., 1., 1., 0.45),
                    custom_size: Some(Vec2::new(
                        CONSUMABLE_BUFF_HUD_ICON_PX,
                        CONSUMABLE_BUFF_HUD_ICON_PX,
                    )),
                    anchor: Anchor::BottomCenter,
                    ..default()
                },
                transform: Transform::from_translation(Vec3::new(
                    0.,
                    -CONSUMABLE_BUFF_HUD_ICON_PX / 2.,
                    4.,
                )),
                ..default()
            },
            RenderLayers::from_layers(&[3]),
            ConsumableBuffHudDurationOverlay { hud_slot },
            Name::new("consumable_buff_duration"),
        ))
        .set_parent(parent)
        .id()
}

fn nth_visible_consumable_buff<'a>(
    buffs: &'a ActiveConsumableBuffs,
    hud_slot: usize,
) -> Option<&'a crate::attributes::ConsumableBuffEntry> {
    buffs
        .entries
        .iter()
        .filter(|e| e.item_stack.is_some())
        .nth(hud_slot)
}

pub fn tick_consumable_buff_hud_overlays(
    buffs: Query<&ActiveConsumableBuffs, With<Player>>,
    mut overlays: Query<(&ConsumableBuffHudDurationOverlay, &mut Sprite)>,
) {
    let Ok(b) = buffs.get_single() else {
        return;
    };
    for (ov, mut sprite) in overlays.iter_mut() {
        if let Some(entry) = nth_visible_consumable_buff(b, ov.hud_slot) {
            let p = entry.display_timer.percent();
            sprite.custom_size = Some(Vec2::new(
                CONSUMABLE_BUFF_HUD_ICON_PX,
                CONSUMABLE_BUFF_HUD_ICON_PX * (1.0 - p),
            ));
        }
    }
}

pub fn handle_consumable_buff_hud_tooltip(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    proto: ProtoParam,
    cursor_pos: Res<CursorPos>,
    hit_detection_sprites: Query<
        (Entity, &Sprite, &GlobalTransform),
        With<super::interactions::Interactable>,
    >,
    mut hud_icons: Query<(
        Entity,
        &GlobalTransform,
        &mut super::interactions::Interactable,
        &ConsumableBuffHudMarker,
    )>,
    existing_tooltips: Query<Entity, With<ConsumableBuffHudTooltip>>,
    mut last_hovered: Local<Option<ItemStack>>,
) {
    use super::interactions::Interaction;

    let hit_entity = super::ui_helpers::pointcast_2d(&cursor_pos, &hit_detection_sprites, None);

    for (entity, _, mut interactable, _) in hud_icons.iter_mut() {
        let is_hit = hit_entity
            .as_ref()
            .map(|(e, _s, _t)| *e == entity)
            .unwrap_or(false);
        if is_hit && !matches!(interactable.current(), Interaction::Hovering) {
            interactable.change(Interaction::Hovering);
        } else if !is_hit && matches!(interactable.current(), Interaction::Hovering) {
            interactable.change(Interaction::None);
        }
    }

    let currently_hovered = hud_icons
        .iter()
        .find(|(_, _, interactable, _)| matches!(interactable.current(), Interaction::Hovering))
        .map(|(_, transform, _, m)| (m.item_stack.clone(), transform.translation()));

    let hovered_stack = currently_hovered.as_ref().map(|(s, _)| s.clone());

    if *last_hovered == hovered_stack {
        return;
    }
    *last_hovered = hovered_stack.clone();

    for tooltip_e in existing_tooltips.iter() {
        commands.entity(tooltip_e).despawn_recursive();
    }

    if let Some((stack, icon_pos)) = currently_hovered {
        let _ = spawn_world_item_tooltip_for_stack(
            &mut commands,
            &graphics,
            &asset_server,
            &proto,
            &stack,
            Vec3::new(icon_pos.x - 4., icon_pos.y + 8., 15.),
        );
    }
}

// ---------------------------------------------------------------------------
// Pet skill HUD slot (4th icon in the skills group)
// ---------------------------------------------------------------------------

/// Marker on the floating tooltip container spawned for the pet skill slot.
#[derive(Component)]
pub struct PetSkillHudTooltip;

/// Marker on the cooldown text inside the pet tooltip (parallels [`SkillTooltipCooldownText`]).
#[derive(Component)]
pub struct PetSkillTooltipCooldownText;

/// Read `(elapsed_secs, duration_secs)` for the given pet's auto-cast timer. Returns
/// `None` for pets without a timer-based ability (e.g. Goliath).
fn pet_ability_cooldown(
    pet: &Pet,
    slime: &Query<&crate::pets::pet_abilities::SlimeShieldTimer, With<Pet>>,
    fairy: &Query<&crate::pets::pet_abilities::FairyHealTimer, With<Pet>>,
    porkipine: &Query<&crate::pets::pet_abilities::PorkipineDamageTimer, With<Pet>>,
    coin: &Query<&crate::pets::pet_abilities::GoldenPigCoinTimer, With<Pet>>,
) -> Option<(f32, f32)> {
    use Pet;
    match pet {
        Pet::Slime => slime
            .get_single()
            .ok()
            .map(|t| (t.0.elapsed().as_secs_f32(), t.0.duration().as_secs_f32())),
        Pet::Fairy => fairy
            .get_single()
            .ok()
            .map(|t| (t.0.elapsed().as_secs_f32(), t.0.duration().as_secs_f32())),
        Pet::Porkipine => porkipine
            .get_single()
            .ok()
            .map(|t| (t.0.elapsed().as_secs_f32(), t.0.duration().as_secs_f32())),
        Pet::GoldenPig => coin
            .get_single()
            .ok()
            .map(|t| (t.0.elapsed().as_secs_f32(), t.0.duration().as_secs_f32())),
        Pet::Goliath => None,
    }
}

/// Spawns / refreshes the pet skill HUD slot. The slot sits at the rightmost position
/// of the 4-wide skills group (reserved in `handle_update_player_skills` by adding `+1`
/// to the centering count). Renders the pet active skill icon from
/// `assets/ui/SkillIcons/` plus a static "PET" label badge (no keybind).
pub fn update_pet_skill_hud_slot(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    graphics: Res<Graphics>,
    pet_q: Query<&Pet>,
    existing: Query<(Entity, &PetSkillSlotFor), With<PetSkillSlotBg>>,
    res: Res<ScreenResolution>,
) {
    let Some(pet) = pet_q.iter().next() else {
        for (e, _) in existing.iter() {
            commands.entity(e).despawn_recursive();
        }
        return;
    };

    if existing.iter().any(|(_, m)| m.0 == *pet) {
        return;
    }

    for (e, _) in existing.iter() {
        commands.entity(e).despawn_recursive();
    }

    // Position matches the formula in `handle_update_player_skills`, with 4 reserved
    // positions and the pet at index 3.
    let num_skills = 4.0_f32;
    let skill_half_span = (num_skills - 1.0) * 0.5;
    let x = HUD_SKILLS_CENTER_X + (3.0 - skill_half_span) * HUD_SKILL_SPACING_X;
    let y = -res.game_height / 2. + HUD_ACTION_ROW_Y_FROM_BOTTOM;

    let slot_bg = commands
        .spawn(SpatialBundle::from_transform(Transform::from_translation(
            Vec3::new(x, y, Z_DEPTH_HUD_ACTIVE_SKILLS),
        )))
        .insert(RenderLayers::from_layers(&[3]))
        .insert(PetSkillSlotBg)
        .insert(PetSkillSlotFor(pet.clone()))
        .insert(Name::new("PET SKILL SLOT"))
        .id();

    // Parented to the slot so `sync_player_hud_slots_layout_to_resolution` keeps the label
    // aligned when UI scale / resolution changes (same pattern as corner icon keybinds).
    let label_local_y =
        KEYBIND_BADGE_BOTTOM_INSET + KEYBIND_BADGE_SIZE.y * 0.5 - HUD_ACTION_ROW_Y_FROM_BOTTOM;
    let (pet_label_bg, _) = spawn_hud_label_badge(
        &mut commands,
        &asset_server,
        "PET",
        Transform::from_translation(Vec3::new(0., label_local_y, 2.)),
        Some(slot_bg),
        3,
    );
    commands
        .entity(pet_label_bg)
        .insert(PetSkillLabelBackground);

    commands
        .spawn(SpriteBundle {
            texture: graphics.get_pet_active_skill_icon(pet.clone()),
            sprite: Sprite {
                custom_size: Some(Vec2::new(16., 16.)),
                ..default()
            },
            transform: Transform::from_translation(Vec3::new(0., 0., 1.)),
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(PetSkillIcon)
        .insert(super::interactions::Interactable::default())
        .insert(Name::new("PET SKILL ICON"))
        .set_parent(slot_bg);

    // Cooldown overlay (matches the class-skill overlay visual style).
    commands
        .spawn(SpriteBundle {
            sprite: Sprite {
                color: Color::rgba(1., 1., 1., 0.45),
                custom_size: Some(Vec2::new(16., 0.)),
                anchor: Anchor::BottomCenter,
                ..default()
            },
            transform: Transform::from_translation(Vec3::new(0., -8., 3.)),
            ..default()
        })
        .insert(PetSkillCooldownOverlay)
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("pet skill cooldown overlay"))
        .set_parent(slot_bg);
}

/// Drives the pet skill slot's cooldown overlay from the pet's auto-cast timer.
/// Shrinks the dark overlay from 16px tall down to 0 as the timer progresses, mirroring
/// `tick_skill_cooldown_overlays`. Pets without timers (Goliath) keep the overlay
/// invisible.
pub fn tick_pet_skill_cooldown_overlay(
    mut overlays: Query<&mut Sprite, With<PetSkillCooldownOverlay>>,
    pet_q: Query<&Pet>,
    slime: Query<&crate::pets::pet_abilities::SlimeShieldTimer, With<Pet>>,
    fairy: Query<&crate::pets::pet_abilities::FairyHealTimer, With<Pet>>,
    porkipine: Query<&crate::pets::pet_abilities::PorkipineDamageTimer, With<Pet>>,
    coin: Query<&crate::pets::pet_abilities::GoldenPigCoinTimer, With<Pet>>,
) {
    let progress = pet_q
        .iter()
        .next()
        .and_then(|pet| pet_ability_cooldown(pet, &slime, &fairy, &porkipine, &coin))
        .map(|(elapsed, duration)| {
            if duration <= 0.0 {
                0.0
            } else {
                (elapsed / duration).clamp(0.0, 1.0)
            }
        });

    let height = progress.map(|p| 16.0 * (1.0 - p)).unwrap_or(0.0);
    for mut sprite in overlays.iter_mut() {
        sprite.custom_size = Some(Vec2::new(16., height));
    }
}

/// Hover-tooltip handler for the pet skill slot. Reuses the skill tooltip background
/// + body styling so the pet ability reads as a 4th skill in the same visual language
/// as the class skills. The cooldown text is the seconds remaining on the pet's
/// auto-cast timer (or blank for timer-less pets).
pub fn handle_pet_skill_hud_tooltip(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    cursor_pos: Res<CursorPos>,
    hit_detection_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut pet_icons: Query<
        (Entity, &GlobalTransform, &mut Interactable, &Parent),
        With<PetSkillIcon>,
    >,
    slot_for: Query<&PetSkillSlotFor>,
    existing_tooltips: Query<Entity, With<PetSkillHudTooltip>>,
    mut last_hovered: Local<Option<Pet>>,
    res: Res<ScreenResolution>,
) {
    let hit_entity = super::ui_helpers::pointcast_2d(&cursor_pos, &hit_detection_sprites, None);
    for (entity, _, mut interactable, _) in pet_icons.iter_mut() {
        let is_hit = hit_entity
            .as_ref()
            .map(|(e, _, _)| *e == entity)
            .unwrap_or(false);
        set_interactable_hover(is_hit, &mut interactable);
    }

    let currently_hovered = pet_icons
        .iter()
        .find_map(|(_, xform, interactable, parent)| {
            if !matches!(
                interactable.current(),
                crate::ui::interactions::Interaction::Hovering
            ) {
                return None;
            }
            let pet = slot_for.get(parent.get()).ok()?;
            Some((pet.0.clone(), xform.translation()))
        });

    let hovered_pet = currently_hovered.as_ref().map(|(p, _)| p.clone());

    if *last_hovered == hovered_pet {
        return;
    }

    for tt in existing_tooltips.iter() {
        commands.entity(tt).despawn_recursive();
    }

    if let Some((pet, pos)) = currently_hovered {
        let pet_data = graphics.get_pet_data(pet.clone());
        let (container, _) = spawn_skill_tooltip_shell(
            &mut commands,
            &graphics,
            hud_skill_tooltip_world_position(pos, res.scale),
            "PET SKILL TOOLTIP",
        );
        commands.entity(container).insert(PetSkillHudTooltip);

        spawn_pet_skill_tooltip_content(
            &mut commands,
            &graphics,
            &asset_server,
            &pet,
            pet_data,
            container,
        );
    }

    *last_hovered = hovered_pet;
}

pub fn spawn_pet_skill_tooltip_content(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    pet: &Pet,
    pet_data: &crate::assets::PetData,
    parent_entity: Entity,
) {
    let content = SkillTooltipContent {
        icon: graphics.get_pet_active_skill_icon(pet.clone()),
        title: pet_data.skill_name.clone(),
        description_lines: pet_skill_description_lines(pet_data),
        icon_size: SKILL_TOOLTIP_ICON_SIZE,
        title_color: YELLOW_2,
        body_color: WHITE,
        cooldown_marker: Some(SkillTooltipCooldownMarker::Pet),
    };
    spawn_skill_tooltip_layout(commands, asset_server, parent_entity, &content);
}

/// Keep HUD heirloom icons below modal overlays during play, but above the game-over fade.
pub fn sync_heirloom_hud_depth(
    game_state: Res<State<GameState>>,
    mut icons: Query<&mut Transform, With<SkillHudIcon>>,
) {
    let target_z = if game_state.0 == GameState::GameOver {
        Z_DEPTH_HUD_HEIRLOOM_ICONS_FOREGROUND
    } else {
        Z_DEPTH_HUD_HEIRLOOM_ICONS
    };
    for mut transform in icons.iter_mut() {
        if transform.translation.z != target_z {
            transform.translation.z = target_z;
        }
    }
}

/// Captures the pre-slide Y position for a HUD heirloom icon on game over.
#[derive(Component)]
pub struct HudGameOverHeirloomSlide {
    pub start_y: f32,
    pub timer: Timer,
}

/// Re-anchor HUD elements when [`ScreenResolution::game_width`] / [`game_height`] change (UI
/// zoom or window resize). Spawn systems only run once at run start; without this, the UI
/// camera projection updates immediately but HUD transforms stay at the old coordinates.
pub fn sync_player_hud_layout_to_resolution(
    res: Res<ScreenResolution>,
    sync_state: Res<super::layout_sync::UiLayoutSyncState>,
    mut layout: ParamSet<(
        Query<&mut Transform, With<HudFrame>>,
        Query<(&mut Transform, &mut Sprite), (With<XPBar>, Without<XPBarBg>)>,
        Query<(&mut Transform, &mut Sprite), (With<XPBarBg>, Without<XPBar>, Without<XPBarText>)>,
        Query<&mut Transform, (With<XPBarText>, Without<XPBar>, Without<XPBarBg>)>,
        Query<(&HudBottomCornerIcon, &mut Transform)>,
    )>,
) {
    if !super::layout_sync::ui_layout_needs_sync(&res, &sync_state) {
        return;
    }

    let hud_row_y = -res.game_height * 0.5 + HUD_FRAME_Y_FROM_BOTTOM;
    for mut transform in layout.p0().iter_mut() {
        transform.translation.y = hud_row_y;
    }

    let xp_y = res.game_height / 2. - 3.;
    let xp_x = -res.game_width / 2.;
    for (mut transform, mut sprite) in layout.p1().iter_mut() {
        transform.translation.x = xp_x;
        transform.translation.y = xp_y;
        sprite.custom_size = Some(Vec2::new(sprite.custom_size.map(|s| s.x).unwrap_or(0.), 6.));
    }
    for (mut transform, mut sprite) in layout.p2().iter_mut() {
        transform.translation.x = xp_x;
        transform.translation.y = xp_y;
        sprite.custom_size = Some(Vec2::new(res.game_width, 6.));
    }
    for mut transform in layout.p3().iter_mut() {
        transform.translation.y = xp_y;
    }

    let corner_y = -res.game_height / 2. + 18.;
    let map_x = -res.game_width * 0.5 + HUD_CORNER_ICON_SIZE.x * 0.5 + HUD_CORNER_LEFT_PADDING;
    let bag_x = map_x + HUD_CORNER_ICON_SPACING;
    let settings_x = bag_x + HUD_CORNER_ICON_SPACING;
    for (icon, mut transform) in layout.p4().iter_mut() {
        transform.translation.y = corner_y;
        transform.translation.x = match icon {
            HudBottomCornerIcon::Minimap => map_x,
            HudBottomCornerIcon::Inventory => bag_x,
            HudBottomCornerIcon::Settings => settings_x,
        };
    }
}

pub fn sync_player_hud_progress_layout_to_resolution(
    res: Res<ScreenResolution>,
    sync_state: Res<super::layout_sync::UiLayoutSyncState>,
    mut progress: ParamSet<(
        Query<(&CurrencyHudSlotIndex, &mut Transform), With<CurrencyHudBackground>>,
        Query<&mut Transform, With<ProgressHudBar>>,
        Query<&mut Transform, With<TimelineHUD>>,
        Query<&mut Transform, With<EraTimerHUD>>,
    )>,
) {
    if !super::layout_sync::ui_layout_needs_sync(&res, &sync_state) {
        return;
    }

    let progress_row_y = hud_row_below_xp_y(res.game_height);
    let timeline_row_y = progress_row_y + 2.;
    for (slot, mut transform) in progress.p0().iter_mut() {
        transform.translation.x = match slot.0 {
            0 => hud_currency_first_center_x(res.game_width),
            1 => hud_currency_second_center_x(res.game_width),
            _ => transform.translation.x,
        };
        transform.translation.y = progress_row_y;
    }
    for mut transform in progress.p1().iter_mut() {
        transform.translation.x = hud_progress_bar_center_x(&res);
        transform.translation.y = progress_row_y + 1.;
    }
    for mut transform in progress.p2().iter_mut() {
        transform.translation.x = hud_timeline_center_x(res.game_width);
        transform.translation.y = timeline_row_y;
    }
    for mut transform in progress.p3().iter_mut() {
        let timer_width = HUD_ERA_TIMER_ENDLESS_WIDTH;
        transform.translation.x = hud_era_timer_center_x(res.game_width, timer_width);
        transform.translation.y = timeline_row_y;
    }
}

pub fn sync_player_hud_slots_layout_to_resolution(
    res: Res<ScreenResolution>,
    sync_state: Res<super::layout_sync::UiLayoutSyncState>,
    mut slots: ParamSet<(
        Query<(&InventorySlotState, &mut Transform)>,
        Query<(&HotbarKeyBackground, &mut Transform)>,
        Query<(&ActiveSkillSlotBg, &mut Transform)>,
        Query<(&ActiveSkillKeyBackground, &mut Transform)>,
        Query<&mut Transform, With<PetSkillSlotBg>>,
        Query<&mut Transform, With<SkillHudIcon>>,
        Query<&mut Transform, With<crate::ui::fps_text::FPSText>>,
        Query<&mut Transform, With<ConsumableBuffHudMarker>>,
    )>,
) {
    if !super::layout_sync::ui_layout_needs_sync(&res, &sync_state) {
        return;
    }

    let action_row_y = -res.game_height * 0.5 + HUD_ACTION_ROW_Y_FROM_BOTTOM;
    for (slot, mut transform) in slots.p0().iter_mut() {
        if slot.r#type != InventorySlotType::Hotbar {
            continue;
        }
        transform.translation.x = hud_hotbar_slot_center_x(slot.slot_index);
        transform.translation.y = action_row_y;
    }
    for (key_bg, mut transform) in slots.p1().iter_mut() {
        transform.translation.x = hud_hotbar_slot_center_x(key_bg.slot);
        transform.translation.y = hud_keybind_badge_center_y(res.game_height);
    }

    let active_skill_count = slots.p2().iter().count();
    if active_skill_count > 0 {
        let num_skills = active_skill_count as f32 + 1.0;
        let skill_half_span = (num_skills - 1.0) * 0.5;
        for (bg, mut transform) in slots.p2().iter_mut() {
            transform.translation.x = HUD_SKILLS_CENTER_X
                + (bg.slot_index as f32 - skill_half_span) * HUD_SKILL_SPACING_X;
            transform.translation.y = action_row_y;
        }
        for (key_bg, mut transform) in slots.p3().iter_mut() {
            let skill_x =
                HUD_SKILLS_CENTER_X + (key_bg.slot as f32 - skill_half_span) * HUD_SKILL_SPACING_X;
            transform.translation.x = skill_x;
            transform.translation.y = hud_keybind_badge_center_y(res.game_height);
        }
        let pet_x = HUD_SKILLS_CENTER_X + (3.0 - skill_half_span) * HUD_SKILL_SPACING_X;
        for mut transform in slots.p4().iter_mut() {
            transform.translation.x = pet_x;
            transform.translation.y = action_row_y;
        }
    }

    let heirloom_row_y = hud_heirloom_row_y(res.game_height);
    let heirloom_start_x = hud_heirloom_first_icon_x(res.game_width);
    let max_icons_per_row = super::hud_heirloom_max_per_row(&res).max(1);
    for (i, mut transform) in slots.p5().iter_mut().enumerate() {
        let row = i / max_icons_per_row;
        let col = i % max_icons_per_row;
        transform.translation.x = heirloom_start_x + col as f32 * HUD_HEIRLOOM_ICON_SPACING;
        transform.translation.y = heirloom_row_y - row as f32 * 16.;
    }

    let raw_fps = Vec2::new(res.game_width / 2. - 28.5, -res.game_height / 2. + 10.5);
    for mut transform in slots.p6().iter_mut() {
        transform.translation.x = super::snap_world_to_pixel_grid(raw_fps.x, res.scale);
        transform.translation.y = super::snap_world_to_pixel_grid(raw_fps.y, res.scale);
    }

    let consumable_buff_y = -res.game_height / 2. + 14.;
    for mut transform in slots.p7().iter_mut() {
        transform.translation.y = consumable_buff_y;
    }
}
