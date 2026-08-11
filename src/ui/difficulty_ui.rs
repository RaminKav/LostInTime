//! Difficulty picker modal — same sprite / UI-camera sizing path as Archives.
//!
//! Overlay + `BackgroundContainer` use world units on the UI camera (`custom_size`), so the
//! panel matches Archives pixel-for-pixel. Interactive controls use `Interactable` pointcasting
//! (not Bevy `Node` UI), which keeps the dim above the class-select / main-menu sprite stack.

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::text::Justify;

use crate::client::GameData;
use crate::cursor::CursorPos;
use crate::pets::Pet;
use crate::player::skills::SkillClass;
use crate::{
    assets::Graphics,
    audio::{AudioSoundEffect, SoundSpawner},
    colors::WHITE,
    difficulty::{
        difficulty_options_unlocked, DifficultySelectState, RunDifficulty, MAX_DIFFICULTY_TIER,
    },
    ui::{
        class_selection::PendingGameStart,
        game_fonts as gf,
        main_menu::{
            MainMenuIconTooltipText, ARCHIVES_CONTAINER_UI_SIZE, MAIN_MENU_ICON_BUTTON_SIZE,
        },
        options_ui::CheatSettings,
        set_sprite_image, ui_helpers, Interactable, Interaction, UIElement, UIState,
        KEYBIND_BADGE_COLOR,
    },
    ScreenResolution,
};

const DIFFICULTY_ICON_SIZE: f32 = 28.;
const ARROW_SIZE: Vec2 = Vec2::new(14., 19.);
const ACTION_BUTTON_GAP: f32 = 36.;
const SELECTOR_Y: f32 = 52.;
const SELECTOR_GAP: f32 = 10.;
const MODIFIER_START_Y: f32 = 28.;
const MODIFIER_LINE_SPACING: f32 = 12.;
const ACTION_Y: f32 = -66.;
/// Above class-select (panel ~10–12, unlock confirms ~30) so nothing peeks through the dim.
const DIFFICULTY_OVERLAY_Z: f32 = 40.;
const DIFFICULTY_CONTENT_Z: f32 = 42.;

#[derive(Component)]
pub struct DifficultyUI;

#[derive(Component)]
pub(crate) struct DifficultyIcon;

#[derive(Component)]
pub(crate) struct DifficultyModifiersRoot;

#[derive(Component)]
pub(crate) struct DifficultyModifierLine;

#[derive(Component)]
pub(crate) struct DifficultyLeftArrow;

#[derive(Component)]
pub(crate) struct DifficultyRightArrow;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DifficultyAction {
    Back,
    Start,
    Left,
    Right,
}

#[derive(Component)]
pub(crate) struct DifficultyHoverTooltip;

/// Stashed when Begin is pressed and difficulty options are unlocked.
#[derive(Resource, Debug, Clone)]
pub struct PendingDifficultySelection {
    pub class: SkillClass,
    pub pets: Vec<Pet>,
}

fn difficulty_icon_element(diff: RunDifficulty) -> UIElement {
    match diff {
        RunDifficulty::One => UIElement::Difficulty1,
        RunDifficulty::Two => UIElement::Difficulty2,
        RunDifficulty::Three => UIElement::Difficulty3,
        RunDifficulty::Four => UIElement::Difficulty4,
        RunDifficulty::Five => UIElement::Difficulty5,
        RunDifficulty::Max => UIElement::DifficultyMax,
    }
}

pub fn setup_difficulty_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    mut select: ResMut<DifficultySelectState>,
    game_data: Option<Res<GameData>>,
    cheat_settings: Res<CheatSettings>,
    resolution: Res<ScreenResolution>,
) {
    let max_unlocked = if *crate::DEBUG || cheat_settings.dev_mode {
        MAX_DIFFICULTY_TIER
    } else {
        game_data
            .as_ref()
            .map(|d| d.max_unlocked_difficulty)
            .unwrap_or(0)
            .clamp(0, MAX_DIFFICULTY_TIER)
    };
    select.max_unlocked = max_unlocked;

    let preferred = game_data
        .as_ref()
        .map(|d| d.last_selected_difficulty)
        .unwrap_or(1)
        .clamp(1, max_unlocked.max(1));
    select.selected = RunDifficulty::from_u8(preferred).unwrap_or(RunDifficulty::One);

    // Same full-screen radial dim path as Archives (UI-camera world units).
    let overlay = ui_helpers::spawn_full_screen_ui_overlay(
        &mut commands,
        &resolution,
        0.95,
        DIFFICULTY_OVERLAY_Z,
    );
    commands
        .entity(overlay)
        .insert(DifficultyUI)
        .insert(UIState::DifficultySelection);

    let root = commands
        .spawn((
            (
                Transform::from_translation(Vec3::new(0., 0., DIFFICULTY_CONTENT_Z)),
                Visibility::default(),
            ),
            DifficultyUI,
            UIState::DifficultySelection,
            RenderLayers::from_layers(&[3]),
            Name::new("Difficulty UI"),
        ))
        .id();

    // Identical container sizing to Archives: `custom_size` in UI world units.
    commands
        .spawn((
            Sprite {
                image: graphics.get_ui_element_texture(UIElement::BackgroundContainer),
                custom_size: Some(ARCHIVES_CONTAINER_UI_SIZE),
                ..default()
            },
            Transform::from_translation(Vec3::new(0., 0., 1.)),
        ))
        .insert(DifficultyUI)
        .insert(UIState::DifficultySelection)
        .insert(RenderLayers::from_layers(&[3]))
        .insert(crate::item::item_drop_outline::UiShadow::container())
        .insert(ChildOf(root));

    let arrow_x = DIFFICULTY_ICON_SIZE * 0.5 + SELECTOR_GAP + ARROW_SIZE.x * 0.5;

    spawn_arrow_button(
        &mut commands,
        root,
        &graphics,
        Vec3::new(-arrow_x, SELECTOR_Y, 2.),
        false,
        select.can_go_left(),
        DifficultyAction::Left,
        DifficultyLeftArrow,
    );
    commands.spawn((
        Sprite {
            image: graphics.get_ui_element_texture(difficulty_icon_element(select.selected)),
            custom_size: Some(Vec2::splat(DIFFICULTY_ICON_SIZE)),
            ..default()
        },
        Transform::from_translation(Vec3::new(0., SELECTOR_Y, 2.)),
        DifficultyIcon,
        DifficultyUI,
        UIState::DifficultySelection,
        RenderLayers::from_layers(&[3]),
        ChildOf(root),
    ));
    spawn_arrow_button(
        &mut commands,
        root,
        &graphics,
        Vec3::new(arrow_x, SELECTOR_Y, 2.),
        true,
        select.can_go_right(),
        DifficultyAction::Right,
        DifficultyRightArrow,
    );

    let mods_root = commands
        .spawn((
            (
                Transform::from_translation(Vec3::new(0., MODIFIER_START_Y, 2.)),
                Visibility::default(),
            ),
            DifficultyModifiersRoot,
            DifficultyUI,
            UIState::DifficultySelection,
            RenderLayers::from_layers(&[3]),
            ChildOf(root),
        ))
        .id();
    spawn_modifier_lines(&mut commands, &asset_server, mods_root, select.selected);

    let half_gap = ACTION_BUTTON_GAP * 0.5 + MAIN_MENU_ICON_BUTTON_SIZE.x * 0.5;
    spawn_action_button(
        &mut commands,
        root,
        &graphics,
        Vec3::new(-half_gap, ACTION_Y, 2.),
        DifficultyAction::Back,
        UIElement::ExitButton,
        "back",
    );
    spawn_action_button(
        &mut commands,
        root,
        &graphics,
        Vec3::new(half_gap, ACTION_Y, 2.),
        DifficultyAction::Start,
        UIElement::YesButton,
        "start",
    );
}

fn spawn_arrow_button<M: Component>(
    commands: &mut Commands,
    parent: Entity,
    graphics: &Graphics,
    pos: Vec3,
    flip_x: bool,
    visible: bool,
    action: DifficultyAction,
    marker: M,
) {
    commands.spawn((
        Sprite {
            image: graphics.get_ui_element_texture(UIElement::LeftArrow),
            custom_size: Some(ARROW_SIZE),
            flip_x,
            ..default()
        },
        Transform::from_translation(pos),
        Interactable::default(),
        UIElement::LeftArrow,
        action,
        marker,
        if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        },
        DifficultyUI,
        UIState::DifficultySelection,
        RenderLayers::from_layers(&[3]),
        ChildOf(parent),
    ));
}

fn spawn_action_button(
    commands: &mut Commands,
    parent: Entity,
    graphics: &Graphics,
    pos: Vec3,
    action: DifficultyAction,
    element: UIElement,
    tooltip: &'static str,
) {
    commands.spawn((
        Sprite {
            image: graphics.get_ui_element_texture(element.clone()),
            custom_size: Some(MAIN_MENU_ICON_BUTTON_SIZE),
            ..default()
        },
        Transform::from_translation(pos),
        Interactable::default(),
        element,
        action,
        MainMenuIconTooltipText(tooltip),
        DifficultyUI,
        UIState::DifficultySelection,
        RenderLayers::from_layers(&[3]),
        ChildOf(parent),
        Name::new(format!("Difficulty Action: {tooltip}")),
    ));
}

fn spawn_modifier_lines(
    commands: &mut Commands,
    asset_server: &AssetServer,
    parent: Entity,
    difficulty: RunDifficulty,
) {
    for (i, line) in difficulty.modifier_lines().iter().enumerate() {
        commands.spawn((
            gf::BODY
                .text(asset_server, *line, WHITE)
                .justify(Justify::Center)
                .anchor(Anchor::CENTER)
                .with_transform(Transform {
                    translation: Vec3::new(0., -(i as f32) * MODIFIER_LINE_SPACING, 0.),
                    scale: gf::BODY.transform_scale(),
                    ..default()
                }),
            DifficultyModifierLine,
            DifficultyUI,
            UIState::DifficultySelection,
            RenderLayers::from_layers(&[3]),
            ChildOf(parent),
        ));
    }
}

pub fn cleanup_difficulty_ui(
    mut commands: Commands,
    ui: Query<Entity, With<DifficultyUI>>,
    tooltips: Query<Entity, With<DifficultyHoverTooltip>>,
) {
    for entity in ui.iter() {
        commands.entity(entity).despawn();
    }
    for entity in tooltips.iter() {
        commands.entity(entity).despawn();
    }
}

pub fn handle_difficulty_ui_interactions(
    mut commands: Commands,
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut buttons: Query<
        (
            Entity,
            &mut Interactable,
            &DifficultyAction,
            &UIElement,
            &Visibility,
            Option<&MainMenuIconTooltipText>,
        ),
        With<DifficultyUI>,
    >,
    mut select: ResMut<DifficultySelectState>,
    mut next_ui_state: ResMut<NextState<UIState>>,
    pending_selection: Option<Res<PendingDifficultySelection>>,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    mut game_data: Option<ResMut<GameData>>,
    existing_tooltips: Query<Entity, With<DifficultyHoverTooltip>>,
    transforms: Query<&GlobalTransform>,
) {
    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None, None);
    let left_mouse_released = mouse_input.just_released(MouseButton::Left);

    for (entity, mut interactable, action, ui_element, visibility, tooltip) in buttons.iter_mut() {
        if matches!(*visibility, Visibility::Hidden) {
            if matches!(interactable.current(), Interaction::Hovering) {
                interactable.change(Interaction::None);
            }
            continue;
        }
        let is_hit = matches!(hit_test, Some(hit) if hit.0 == entity);
        if is_hit {
            match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonHover, 0.05));
                    if let Some(hover) = ui_element.get_hover_state() {
                        commands.entity(entity).insert(hover.clone());
                        set_sprite_image(
                            &mut commands,
                            entity,
                            graphics.get_ui_element_texture(hover),
                        );
                    }
                    if let Some(label) = tooltip {
                        for t in existing_tooltips.iter() {
                            commands.entity(t).despawn();
                        }
                        if let Ok(gt) = transforms.get(entity) {
                            spawn_difficulty_tooltip(
                                &mut commands,
                                &asset_server,
                                label.0,
                                gt.translation(),
                            );
                        }
                    }
                }
                Interaction::Hovering => {
                    if left_mouse_released {
                        commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonClick, 0.2));
                        match action {
                            DifficultyAction::Left => select.step_left(),
                            DifficultyAction::Right => select.step_right(),
                            DifficultyAction::Back => {
                                next_ui_state.set(UIState::ClassSelection);
                                commands.remove_resource::<PendingDifficultySelection>();
                            }
                            DifficultyAction::Start => {
                                let Some(pending) = pending_selection.as_deref() else {
                                    warn!(
                                        "Difficulty Start pressed with no PendingDifficultySelection"
                                    );
                                    continue;
                                };
                                if let Some(ref mut data) = game_data {
                                    data.last_selected_difficulty = select.selected.as_u8();
                                    crate::difficulty::persist_difficulty_progress(data);
                                }
                                commands.insert_resource(PendingGameStart {
                                    class: pending.class.clone(),
                                    pets: pending.pets.clone(),
                                    difficulty: Some(select.selected),
                                });
                                commands.remove_resource::<PendingDifficultySelection>();
                                next_ui_state.set(UIState::ClassSelection);
                            }
                        }
                    }
                }
                _ => {}
            }
        } else if matches!(interactable.current(), Interaction::Hovering) {
            interactable.change(Interaction::None);
            if let Some(normal) = ui_element.get_normal_state() {
                commands.entity(entity).insert(normal.clone());
                set_sprite_image(
                    &mut commands,
                    entity,
                    graphics.get_ui_element_texture(normal),
                );
            }
            if tooltip.is_some() {
                for t in existing_tooltips.iter() {
                    commands.entity(t).despawn();
                }
            }
        }
    }
}

fn spawn_difficulty_tooltip(
    commands: &mut Commands,
    asset_server: &AssetServer,
    label: &'static str,
    icon_center: Vec3,
) {
    let char_w = 4.6;
    let pad_x = 10.;
    let pad_y = 5.;
    let line_h = 10.;
    let text_w = label.chars().count() as f32 * char_w;
    let width = (text_w + pad_x * 2.).max(28.);
    let height = line_h + pad_y * 2.;
    let tooltip_pos = icon_center + Vec3::new(0., MAIN_MENU_ICON_BUTTON_SIZE.y * 0.5 + 10., 2.);

    let bg = commands
        .spawn((
            Sprite {
                color: KEYBIND_BADGE_COLOR,
                custom_size: Some(Vec2::new(width, height)),
                ..default()
            },
            Transform::from_translation(tooltip_pos),
            DifficultyHoverTooltip,
            DifficultyUI,
            RenderLayers::from_layers(&[3]),
            Name::new("Difficulty Hover Tooltip"),
        ))
        .id();
    commands.spawn((
        gf::ICON_HOVER_TOOLTIP
            .text(asset_server, label, WHITE)
            .justify(Justify::Center)
            .anchor(Anchor::CENTER)
            .with_transform(Transform {
                translation: Vec3::new(0., 0., 1.),
                scale: gf::ICON_HOVER_TOOLTIP.transform_scale(),
                ..default()
            }),
        DifficultyHoverTooltip,
        DifficultyUI,
        RenderLayers::from_layers(&[3]),
        ChildOf(bg),
    ));
}

pub fn sync_difficulty_selection_visuals(
    select: Res<DifficultySelectState>,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    mut icon_q: Query<&mut Sprite, (With<DifficultyIcon>, Without<Interactable>)>,
    mut left_q: Query<&mut Visibility, (With<DifficultyLeftArrow>, Without<DifficultyRightArrow>)>,
    mut right_q: Query<&mut Visibility, (With<DifficultyRightArrow>, Without<DifficultyLeftArrow>)>,
    mods_root: Query<Entity, With<DifficultyModifiersRoot>>,
    mod_lines: Query<Entity, With<DifficultyModifierLine>>,
) {
    if let Ok(mut sprite) = icon_q.single_mut() {
        sprite.image = graphics.get_ui_element_texture(difficulty_icon_element(select.selected));
    }
    if let Ok(mut vis) = left_q.single_mut() {
        *vis = if select.can_go_left() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    if let Ok(mut vis) = right_q.single_mut() {
        *vis = if select.can_go_right() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }

    for line_e in mod_lines.iter() {
        commands.entity(line_e).despawn();
    }
    let Ok(mods_e) = mods_root.single() else {
        return;
    };
    spawn_modifier_lines(&mut commands, &asset_server, mods_e, select.selected);
}

pub fn difficulty_modal_available(game_data: Option<Res<GameData>>) -> bool {
    game_data
        .map(|d| difficulty_options_unlocked(d.max_unlocked_difficulty))
        .unwrap_or(false)
}
