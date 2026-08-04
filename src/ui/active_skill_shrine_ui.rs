use bevy::text::Justify;
use bevy::{camera::visibility::RenderLayers, prelude::*, sprite::Anchor};

use crate::{
    assets::Graphics,
    attributes::{
        attribute_helpers::skill_power_multiplier, AttackSpeed, BonusAttackSpeed, CritChance,
        MaxHealth, MaxMana, ProjectileSize, SkillPower, Speed,
    },
    audio::{AudioSoundEffect, SoundSpawner},
    blessings::OwnedBlessings,
    colors::{WHITE, YELLOW_2},
    item::active_skill_shrine::{
        assign_shrine_skill_to_slot, reroll_active_skill_shrine_offer_skills, shrine_assign_action,
        shrine_assignable_slots, skill_choices_from_offer_skills, ActiveSkillShrineOverwrite,
        ActiveSkillShrineSelection, ShrineAssignAction,
    },
    juice::bounce::BounceOnHit,
    player::{
        skills::{
            active_skill_scaling::METEOR_SHOWER_BASE_COUNT,
            effective_player_attack_speed_multiplier, ActiveSkillChoiceState, PlayerClass,
            PlayerSkills,
        },
        unlocks::{RunUnlockState, UnlockUpgrades, UnlockedSkills},
        Player,
    },
    ui::{
        essence_ui::{MERCHANT_REROLL_ICON_PATH, MERCHANT_REROLL_ICON_SIZE},
        game_fonts as gf,
        ui_helpers::spawn_full_screen_ui_overlay_tuned,
        SKILL_TOOLTIP_SIZE,
    },
    GameParam, ScreenResolution,
};

use super::{
    interactions::Interaction,
    main_menu::spawn_back_button,
    options_ui::CheatSettings,
    player_hud::{
        spawn_skill_tooltip_content, spawn_skill_tooltip_shell, SKILL_TOOLTIP_BG_LOCAL,
        SKILL_TOOLTIP_ICON_SIZE,
    },
    Focusable, Interactable, UIElement, UIState, KEYBIND_BADGE_COLOR, TOOLTIP_INFO_BOX_SIZE,
};

/// Bundles the params needed to fire the skills tutorial popup when the active skill shrine UI
/// closes, keeping the interaction-handler system param counts under Bevy's tuple limit.
#[derive(bevy::ecs::system::SystemParam)]
pub struct ActiveSkillShrineTutorialTrigger<'w, 's> {
    popup_events: MessageWriter<'w, crate::ui::tutorial_ui::TutorialPopupEvent>,
    seen_chunks: Option<Res<'w, crate::ui::tutorial_ui::SeenTutorialChunks>>,
    tutorial_ui: Query<'w, 's, (), With<crate::ui::tutorial_ui::TutorialUI>>,
}

impl<'w, 's> ActiveSkillShrineTutorialTrigger<'w, 's> {
    fn try_trigger(&mut self) {
        if let Some(seen_chunks) = self.seen_chunks.as_ref() {
            crate::ui::tutorial_ui::try_active_skill_shrine_tutorial(
                &mut self.popup_events,
                seen_chunks,
                &self.tutorial_ui,
            );
        }
    }
}

/// Bundles the read-only unlock/cheat resources needed to decide how a shrine skill pick gets
/// assigned, keeping the interaction-handler system param counts under Bevy's tuple limit.
#[derive(bevy::ecs::system::SystemParam)]
pub struct ShrineAssignUnlockParams<'w> {
    unlocked_skills: Res<'w, UnlockedSkills>,
    unlock_upgrades: Res<'w, UnlockUpgrades>,
    cheat_settings: Res<'w, CheatSettings>,
}

/// Focus + mouseless gate for shrine UI hover/confirm (avoids stuck bounce when mouse-only).
#[derive(bevy::ecs::system::SystemParam)]
pub struct ShrineUiFocus<'w> {
    focus: crate::ui::focus::FocusInput<'w>,
    mouseless: Res<'w, crate::inputs::MouselessModeState>,
}

impl ShrineUiFocus<'_> {
    fn is_focused(&self, entity: Entity, cursor_pos: &crate::cursor::CursorPos) -> bool {
        (self.mouseless.0 || cursor_pos.suppress_ui_hover) && self.focus.is_focused(entity)
    }

    fn confirm_just_pressed(&self) -> bool {
        self.focus.confirm_just_pressed()
    }
}

const SHRINE_TOOLTIP_GAP: f32 = 24.;
const SHRINE_TOOLTIP_SPACING: f32 = SKILL_TOOLTIP_SIZE.y + SHRINE_TOOLTIP_GAP;
/// Below the lower skill tooltip panel.
const SHRINE_REROLL_BUTTON_Y: f32 =
    -SHRINE_TOOLTIP_SPACING * 0.5 - SKILL_TOOLTIP_SIZE.y * 0.5 - 16.;
const SHRINE_REROLL_BADGE_SIZE: Vec2 = Vec2::new(14., 12.);
/// Above [`SHRINE_UI_OVERLAY_Z`] and shrine tooltip content.
const SHRINE_BACK_BUTTON_Z: f32 = 25.;
const SHRINE_UI_OVERLAY_Z: f32 = 20.;

struct ShrineSkillTooltipStats {
    skill_power_mult: f32,
    max_mana: i32,
    max_health: i32,
    bonus_as_mult: f32,
    crit: i32,
    spd: i32,
    size: i32,
}

#[derive(Component)]
pub struct ActiveSkillShrineRoot;

#[derive(Component)]
pub struct ActiveSkillShrineChoicesRoot;

#[derive(Component)]
pub struct ActiveSkillShrineRerollButton;

#[derive(Component)]
pub struct ActiveSkillShrineRerollIcon;

#[derive(Component)]
pub struct ActiveSkillShrineRerollsText;

#[derive(Component)]
pub struct ActiveSkillShrineUI {
    pub skill_choice: ActiveSkillChoiceState,
    pub interaction_lock_timer: Timer,
}

#[derive(Component)]
pub struct ActiveSkillSlotChoiceUI {
    pub index: usize,
    /// `None` marks an empty unlocked slot the player can fill.
    pub skill_choice: Option<ActiveSkillChoiceState>,
    pub interaction_lock_timer: Timer,
}

fn shrine_skill_tooltip_container_pos(visual_center: Vec3) -> Vec3 {
    visual_center - SKILL_TOOLTIP_BG_LOCAL
}

fn shrine_skill_tooltip_stats(
    skill_power: (
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
) -> ShrineSkillTooltipStats {
    let (skill_power, blessings, max_mana, max_health, bonus_as, attack_speed, crit, spd, size) =
        skill_power;
    let bonus_as_mult = effective_player_attack_speed_multiplier(
        attack_speed.map(|a| a.0).unwrap_or(0),
        bonus_as.map(|b| b.get_multiplier()).unwrap_or(1.0),
    );
    ShrineSkillTooltipStats {
        skill_power_mult: skill_power_multiplier(skill_power, blessings.get_skill_power_bonus()),
        max_mana: max_mana.0,
        max_health: max_health.0,
        bonus_as_mult,
        crit: crit.0,
        spd: spd.0,
        size: size.0,
    }
}

fn spawn_shrine_skill_tooltip_content(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    container: Entity,
    active_skill: crate::player::skills::ActiveSkill,
    stats: &ShrineSkillTooltipStats,
) {
    spawn_skill_tooltip_content(
        commands,
        graphics,
        asset_server,
        active_skill,
        None,
        container,
        stats.skill_power_mult,
        stats.max_mana,
        stats.max_health,
        stats.bonus_as_mult,
        stats.crit,
        stats.spd,
        stats.size,
        METEOR_SHOWER_BASE_COUNT,
        SKILL_TOOLTIP_ICON_SIZE,
    );
}

fn spawn_shrine_gain_skill_icon(
    commands: &mut Commands,
    graphics: &Graphics,
    parent: Entity,
    center_y: f32,
    active_skill: crate::player::skills::ActiveSkill,
) {
    const GAIN_SKILL_ICON_SIZE: Vec2 = Vec2::new(32., 32.);

    commands
        .spawn((
            Sprite {
                image: graphics.get_active_skill_icon(active_skill),
                custom_size: Some(GAIN_SKILL_ICON_SIZE),
                ..default()
            },
            Transform::from_translation(Vec3::new(0., center_y, 21.)),
        ))
        .insert(UIState::ActiveSkills)
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("NEW_ACTIVE_SKILL_ICON"))
        .insert(ChildOf(parent));
}

fn spawn_shrine_interactive_skill_tooltip(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    parent: Entity,
    visual_center_y: f32,
    shrine_ui: ActiveSkillShrineUI,
    stats: &ShrineSkillTooltipStats,
    container_name: &'static str,
    hit_name: &'static str,
    focus_index: u32,
) {
    let container_pos = shrine_skill_tooltip_container_pos(Vec3::new(0., visual_center_y, 21.));
    let (container, bg) =
        spawn_skill_tooltip_shell(commands, graphics, container_pos, "SHRINE SKILL TOOLTIP");
    let active_skill = shrine_ui.skill_choice.active_skill.clone();

    commands
        .entity(container)
        .insert(BounceOnHit::shrine_hover())
        .insert(UIState::ActiveSkillShrine)
        .insert(Name::new(container_name))
        .insert(ChildOf(parent));

    commands
        .entity(bg)
        .insert(shrine_ui)
        .insert(Interactable::default())
        .insert(Focusable {
            group: UIState::ActiveSkillShrine,
            index: focus_index,
        })
        .insert(UIState::ActiveSkillShrine)
        .insert(Name::new(hit_name));

    spawn_shrine_skill_tooltip_content(
        commands,
        graphics,
        asset_server,
        container,
        active_skill,
        stats,
    );
}

fn spawn_shrine_interactive_slot_tooltip(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    parent: Entity,
    visual_center_y: f32,
    slot_ui: ActiveSkillSlotChoiceUI,
    stats: &ShrineSkillTooltipStats,
    container_name: &'static str,
    hit_name: &'static str,
    focus_index: u32,
) {
    let container_pos = shrine_skill_tooltip_container_pos(Vec3::new(0., visual_center_y, 21.));
    let (container, bg) = spawn_skill_tooltip_shell(
        commands,
        graphics,
        container_pos,
        "SHRINE SLOT SKILL TOOLTIP",
    );

    let active_skill = slot_ui
        .skill_choice
        .as_ref()
        .map(|choice| choice.active_skill.clone());

    commands
        .entity(container)
        .insert(BounceOnHit::shrine_hover())
        .insert(UIState::ActiveSkills)
        .insert(Name::new(container_name))
        .insert(ChildOf(parent));

    commands
        .entity(bg)
        .insert(slot_ui)
        .insert(Interactable::default())
        .insert(Focusable {
            group: UIState::ActiveSkills,
            index: focus_index,
        })
        .insert(UIState::ActiveSkills)
        .insert(Name::new(hit_name));

    if let Some(active_skill) = active_skill {
        spawn_shrine_skill_tooltip_content(
            commands,
            graphics,
            asset_server,
            container,
            active_skill,
            stats,
        );
    }
}

fn spawn_empty_shrine_slot_tooltip(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    parent: Entity,
    visual_center_y: f32,
    slot_index: usize,
) {
    let container_pos = shrine_skill_tooltip_container_pos(Vec3::new(0., visual_center_y, 21.));
    let (container, bg) = spawn_skill_tooltip_shell(
        commands,
        graphics,
        container_pos,
        "SHRINE EMPTY SLOT TOOLTIP",
    );

    commands
        .entity(container)
        .insert(BounceOnHit::shrine_hover())
        .insert(UIState::ActiveSkills)
        .insert(Name::new("EMPTY_SKILL_SLOT_CONTAINER"))
        .insert(ChildOf(parent));

    commands
        .entity(bg)
        .insert(ActiveSkillSlotChoiceUI {
            index: slot_index,
            skill_choice: None,
            interaction_lock_timer: Timer::from_seconds(0.75, TimerMode::Once),
        })
        .insert(Interactable::default())
        .insert(Focusable {
            group: UIState::ActiveSkills,
            index: slot_index as u32,
        })
        .insert(UIState::ActiveSkills)
        .insert(Name::new("EMPTY_SKILL_SLOT_HIT"));

    commands
        .spawn(
            gf::MENU_TITLE
                .text(&asset_server, "Empty Slot", WHITE)
                .anchor(Anchor::CENTER_LEFT)
                .with_transform(Transform {
                    translation: Vec3::new(-24., 14., 2.),
                    scale: gf::MENU_TITLE.transform_scale(),
                    ..default()
                }),
        )
        .insert(RenderLayers::from_layers(&[3]))
        .insert(ChildOf(container));
}

fn spawn_active_skill_shrine_skill_choices(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    parent: Entity,
    skill_choices: &[ActiveSkillChoiceState],
    skill_power: (
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
) {
    let half_spacing = SHRINE_TOOLTIP_SPACING * 0.5;
    let stats = shrine_skill_tooltip_stats(skill_power);

    for (index, skill_choice) in skill_choices.iter().enumerate() {
        let tooltip_y = half_spacing - index as f32 * SHRINE_TOOLTIP_SPACING;
        spawn_shrine_interactive_skill_tooltip(
            commands,
            graphics,
            asset_server,
            parent,
            tooltip_y,
            ActiveSkillShrineUI {
                skill_choice: skill_choice.clone(),
                interaction_lock_timer: Timer::from_seconds(0.75, TimerMode::Once),
            },
            &stats,
            if index == 0 {
                "SHRINE_SKILL_TOOLTIP_0_CONTAINER"
            } else {
                "SHRINE_SKILL_TOOLTIP_1_CONTAINER"
            },
            if index == 0 {
                "SHRINE_SKILL_TOOLTIP_0_HIT"
            } else {
                "SHRINE_SKILL_TOOLTIP_1_HIT"
            },
            index as u32,
        );
    }
}

fn spawn_active_skill_shrine_reroll_button(
    commands: &mut Commands,
    asset_server: &AssetServer,
    parent: Entity,
    enabled: bool,
) {
    let color = if enabled {
        Color::WHITE
    } else {
        Color::srgb(0.45, 0.45, 0.45)
    };

    let mut btn = commands.spawn((
        Sprite {
            color: KEYBIND_BADGE_COLOR,
            custom_size: Some(SHRINE_REROLL_BADGE_SIZE),
            ..default()
        },
        Transform::from_translation(Vec3::new(0., SHRINE_REROLL_BUTTON_Y, 3.)),
    ));
    btn.insert(RenderLayers::from_layers(&[3]))
        .insert(UIState::ActiveSkillShrine)
        .insert(ActiveSkillShrineRerollButton)
        .insert(Name::new("Active Skill Shrine Reroll"));

    if enabled {
        btn.insert(Interactable::default()).insert(Focusable {
            group: UIState::ActiveSkillShrine,
            index: 10,
        });
    }

    let btn_e = btn.id();

    commands
        .spawn((
            Sprite {
                image: asset_server.load(MERCHANT_REROLL_ICON_PATH),
                custom_size: Some(MERCHANT_REROLL_ICON_SIZE),
                color,
                ..default()
            },
            Transform::from_translation(Vec3::new(0., 0., 1.)),
        ))
        .insert(RenderLayers::from_layers(&[3]))
        .insert(ActiveSkillShrineRerollIcon)
        .insert(ChildOf(btn_e));

    commands.entity(btn_e).insert(ChildOf(parent));
}

fn spawn_active_skill_shrine_reroll_info_box(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    parent: Entity,
    rerolls_remaining: u32,
) {
    let pos = Vec2::new(
        SKILL_TOOLTIP_SIZE.x / 2. + TOOLTIP_INFO_BOX_SIZE.x / 2. + 8.,
        SHRINE_REROLL_BUTTON_Y,
    );
    let box_e = commands
        .spawn((
            Sprite {
                image: graphics.get_ui_element_texture(UIElement::TooltipInfoBox),
                custom_size: Some(TOOLTIP_INFO_BOX_SIZE),
                ..default()
            },
            Transform::from_translation(Vec3::new(pos.x, pos.y, 2.)),
        ))
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIState::ActiveSkillShrine)
        .insert(Name::new("Active Skill Shrine Reroll Info Box"))
        .insert(ChildOf(parent))
        .id();

    commands
        .spawn((
            gf::BODY
                .text(&asset_server, "Rerolls left:".to_string(), WHITE)
                .justify(Justify::Center)
                .anchor(Anchor::CENTER)
                .with_transform(Transform {
                    translation: Vec3::new(0., 5., 2.),
                    scale: gf::BODY.transform_scale(),
                    ..default()
                }),
            RenderLayers::from_layers(&[3]),
        ))
        .insert(ChildOf(box_e));

    commands
        .spawn((
            gf::BODY
                .text(&asset_server, rerolls_remaining.to_string(), WHITE)
                .justify(Justify::Center)
                .anchor(Anchor::CENTER)
                .with_transform(Transform {
                    translation: Vec3::new(0., -6., 2.),
                    scale: gf::BODY.transform_scale(),
                    ..default()
                }),
            RenderLayers::from_layers(&[3]),
            ActiveSkillShrineRerollsText,
        ))
        .insert(ChildOf(box_e));
}

pub fn setup_active_skill_shrine_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    shrine_selection: Res<ActiveSkillShrineSelection>,
    run_unlocks: Res<RunUnlockState>,
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
) {
    let shrine_overlay =
        spawn_full_screen_ui_overlay_tuned(&mut commands, &res, 0.0, 0.95, SHRINE_UI_OVERLAY_Z);
    commands
        .entity(shrine_overlay)
        .insert(UIState::ActiveSkillShrine);
    let _title_text = commands
        .spawn((
            gf::MENU_TITLE_LARGE
                .text(&asset_server, "Choose a New Skill".to_string(), WHITE)
                .with_transform(Transform {
                    translation: Vec3::new(0., res.game_height / 2. - 40., 21.),
                    scale: gf::MENU_TITLE_LARGE.transform_scale(),
                    ..Default::default()
                }),
            RenderLayers::from_layers(&[3]),
            UIState::BlessingChoice,
        ))
        .id();
    let _title_text2 = commands
        .spawn((
            gf::MENU_TITLE
                .text(
                    &asset_server,
                    "(Assign to an active skill slot)".to_string(),
                    WHITE,
                )
                .with_transform(Transform {
                    translation: Vec3::new(0., res.game_height / 2. - 62., 21.),
                    scale: gf::MENU_TITLE.transform_scale(),
                    ..Default::default()
                }),
            RenderLayers::from_layers(&[3]),
            UIState::BlessingChoice,
        ))
        .id();

    let shrine_root = commands
        .spawn((
            (Transform::IDENTITY, Visibility::default()),
            RenderLayers::from_layers(&[3]),
            UIState::ActiveSkillShrine,
            ActiveSkillShrineRoot,
            Name::new("Active Skill Shrine UI"),
        ))
        .insert(ChildOf(shrine_overlay))
        .id();

    let choices_root = commands
        .spawn((
            (Transform::IDENTITY, Visibility::default()),
            RenderLayers::from_layers(&[3]),
            UIState::ActiveSkillShrine,
            ActiveSkillShrineChoicesRoot,
            Name::new("Active Skill Shrine Choices"),
        ))
        .insert(ChildOf(shrine_root))
        .id();

    let Ok(skill_power) = skill_power.single() else {
        return;
    };
    spawn_active_skill_shrine_skill_choices(
        &mut commands,
        &graphics,
        &asset_server,
        choices_root,
        &shrine_selection.skill_choices,
        skill_power,
    );

    let reroll_enabled = run_unlocks.rerolls_remaining > 0;
    spawn_active_skill_shrine_reroll_button(
        &mut commands,
        &asset_server,
        shrine_root,
        reroll_enabled,
    );
    spawn_active_skill_shrine_reroll_info_box(
        &mut commands,
        &graphics,
        &asset_server,
        shrine_root,
        run_unlocks.rerolls_remaining,
    );

    let back_button = spawn_back_button(
        Vec3::new(
            res.game_width / 2. - 55.,
            -res.game_height / 2. + 38.,
            SHRINE_BACK_BUTTON_Z,
        ),
        &mut commands,
        &graphics,
        &asset_server,
    );
    commands
        .entity(back_button)
        .insert(UIState::ActiveSkillShrine);
}

pub fn tick_active_skill_shrine_ui_interaction_lock_timers(
    time: Res<Time>,
    mut query: Query<&mut ActiveSkillShrineUI>,
) {
    for mut skill_ui in query.iter_mut() {
        if skill_ui.interaction_lock_timer.is_finished() {
            continue;
        }
        skill_ui.interaction_lock_timer.tick(time.delta());
    }
}

pub fn tick_active_skill_slot_choice_ui_interaction_lock_timers(
    time: Res<Time>,
    mut query: Query<&mut ActiveSkillSlotChoiceUI>,
) {
    for mut slot_ui in query.iter_mut() {
        if slot_ui.interaction_lock_timer.is_finished() {
            continue;
        }
        slot_ui.interaction_lock_timer.tick(time.delta());
    }
}

pub fn handle_active_skill_shrine_ui_interaction(
    cursor_pos: Res<crate::cursor::CursorPos>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut skill_choices: Query<(Entity, &mut Interactable, &ActiveSkillShrineUI)>,
    parents: Query<&ChildOf>,
    mut containers: Query<(&mut Transform, &mut BounceOnHit)>,
    shrine_selection: ResMut<ActiveSkillShrineSelection>,
    mut next_ui_state: ResMut<NextState<UIState>>,
    mut commands: Commands,
    mut player_skills: Query<(Entity, &mut PlayerSkills), With<Player>>,
    player_class: Res<PlayerClass>,
    unlock_params: ShrineAssignUnlockParams,
    mut att_event: MessageWriter<crate::attributes::AttributeChangeEvent>,
    mut shrine_query: Query<&mut crate::item::active_skill_shrine::ActiveSkillShrineState>,
    shrine_focus: ShrineUiFocus,
    mut tutorial_trigger: ActiveSkillShrineTutorialTrigger,
) {
    let hit_test = super::ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None, None);
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);

    for (e, mut interactable, skill_ui) in skill_choices.iter_mut() {
        let is_hit = matches!(hit_test, Some(hit_ent) if hit_ent.0 == e);
        let is_focused = shrine_focus.is_focused(e, &cursor_pos);
        let confirm_pressed =
            (is_hit && left_mouse_pressed) || (is_focused && shrine_focus.confirm_just_pressed());

        if is_hit || is_focused {
            match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                    commands.spawn(crate::audio::SoundSpawner::new(
                        crate::audio::AudioSoundEffect::UISkillHover,
                        0.2,
                    ));
                    if let Ok(parent) = parents.get(e) {
                        if let Ok((mut transform, mut bounce)) = containers.get_mut(parent.parent())
                        {
                            super::ui_helpers::apply_ui_hover_scale(
                                &mut transform,
                                Some(&mut bounce),
                                true,
                            );
                            bounce.activate();
                        }
                    }
                }
                Interaction::Hovering => {
                    if confirm_pressed && skill_ui.interaction_lock_timer.is_finished() {
                        let picked_skill = skill_ui.skill_choice.clone();
                        let Ok((player_e, mut skills)) = player_skills.single_mut() else {
                            return;
                        };

                        match shrine_assign_action(
                            &skills,
                            &player_class.class,
                            &unlock_params.unlocked_skills,
                            &unlock_params.unlock_upgrades,
                            unlock_params.cheat_settings.bypass_class_unlocks,
                        ) {
                            ShrineAssignAction::AutoFill(slot) => {
                                assign_shrine_skill_to_slot(
                                    &mut skills,
                                    slot,
                                    picked_skill.clone(),
                                );
                                picked_skill
                                    .active_skill
                                    .add_skill_components(player_e, &mut commands);
                                if let Ok(mut shrine_state) =
                                    shrine_query.get_mut(shrine_selection.shrine_entity)
                                {
                                    shrine_state.is_used = true;
                                }
                                commands.remove_resource::<ActiveSkillShrineSelection>();
                                next_ui_state.set(UIState::Closed);
                                att_event.write(crate::attributes::AttributeChangeEvent);
                                tutorial_trigger.try_trigger();
                            }
                            ShrineAssignAction::ShowSlotPicker => {
                                commands.insert_resource(ActiveSkillShrineOverwrite {
                                    skill_choice: picked_skill.clone(),
                                    shrine_entity: shrine_selection.shrine_entity,
                                });
                                commands.remove_resource::<ActiveSkillShrineSelection>();
                                next_ui_state.set(UIState::ActiveSkills);
                            }
                        }
                        return;
                    }
                }
                _ => (),
            }
        } else {
            // reset hovering states if we stop hovering
            let Interaction::Hovering = interactable.current() else {
                continue;
            };
            interactable.change(Interaction::None);
            if let Ok(parent) = parents.get(e) {
                if let Ok((mut transform, mut bounce)) = containers.get_mut(parent.parent()) {
                    super::ui_helpers::apply_ui_hover_scale(
                        &mut transform,
                        Some(&mut bounce),
                        false,
                    );
                }
            }
        }
    }
}

pub fn handle_active_skill_shrine_reroll_button(
    cursor_pos: Res<crate::cursor::CursorPos>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    mut sprites: ParamSet<(
        Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
        Query<&mut Sprite, With<ActiveSkillShrineRerollIcon>>,
    )>,
    mut reroll_buttons: Query<(
        Entity,
        &mut Interactable,
        &ActiveSkillShrineRerollButton,
        &Children,
    )>,
    mut reroll_text: Query<
        (&mut Text2d, &mut TextColor),
        With<ActiveSkillShrineRerollsText>,
    >,
    mut commands: Commands,
    mut run_unlocks: ResMut<RunUnlockState>,
    mut shrine_selection: ResMut<ActiveSkillShrineSelection>,
    mut game: GameParam,
    player_skills: Query<&PlayerSkills, With<Player>>,
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
    choices_root: Query<Entity, With<ActiveSkillShrineChoicesRoot>>,
    shrine_state: Query<&crate::item::active_skill_shrine::ActiveSkillShrineState>,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    shrine_focus: ShrineUiFocus,
) {
    let hit_entity = {
        let ui_sprites = sprites.p0();
        super::ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None, None).map(|(e, _, _)| e)
    };
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);
    let mut any_hovered = false;

    for (e, mut interactable, _btn, btn_children) in reroll_buttons.iter_mut() {
        let enabled = run_unlocks.rerolls_remaining > 0;
        let icon_color = if enabled {
            Color::WHITE
        } else {
            Color::srgb(0.45, 0.45, 0.45)
        };
        let is_hit = hit_entity == Some(e);
        let is_focused = shrine_focus.is_focused(e, &cursor_pos);
        let confirm_pressed =
            (is_hit && left_mouse_pressed) || (is_focused && shrine_focus.confirm_just_pressed());

        if is_hit || is_focused {
            match interactable.current() {
                Interaction::None if enabled => {
                    interactable.change(Interaction::Hovering);
                    for child in btn_children.iter() {
                        if let Ok(mut sprite) = sprites.p1().get_mut(child) {
                            sprite.color = YELLOW_2;
                        }
                    }
                }
                Interaction::Hovering => {
                    any_hovered = true;
                    if confirm_pressed && enabled {
                        run_unlocks.rerolls_remaining =
                            run_unlocks.rerolls_remaining.saturating_sub(1);

                        let previous_offer: Vec<_> = shrine_selection
                            .skill_choices
                            .iter()
                            .map(|c| c.active_skill)
                            .collect();
                        let offer_skills = reroll_active_skill_shrine_offer_skills(
                            &previous_offer,
                            player_skills.single().ok(),
                        );
                        if offer_skills.is_empty() {
                            return;
                        }

                        shrine_selection.skill_choices =
                            skill_choices_from_offer_skills(&offer_skills);

                        if let Ok(shrine) = shrine_state.get(shrine_selection.shrine_entity) {
                            game.world_obj_cache
                                .active_skill_shrine_offers
                                .insert(shrine.tile_pos, offer_skills);
                        }

                        if let Ok(choices_e) = choices_root.single() {
                            commands.entity(choices_e).despawn_related::<Children>();
                            if let Ok(stats) = skill_power.single() {
                                spawn_active_skill_shrine_skill_choices(
                                    &mut commands,
                                    &graphics,
                                    &asset_server,
                                    choices_e,
                                    &shrine_selection.skill_choices,
                                    stats,
                                );
                            }
                        }

                        interactable.change(Interaction::None);
                        commands.spawn(SoundSpawner::new(AudioSoundEffect::UISkillReRoll, 0.4));
                        for child in btn_children.iter() {
                            if let Ok(mut sprite) = sprites.p1().get_mut(child) {
                                sprite.color = icon_color;
                            }
                        }
                    }
                }
                _ => (),
            }
        } else if matches!(interactable.current(), Interaction::Hovering) {
            interactable.change(Interaction::None);
            for child in btn_children.iter() {
                if let Ok(mut sprite) = sprites.p1().get_mut(child) {
                    sprite.color = icon_color;
                }
            }
        }
    }

    for (mut text, mut text_color) in reroll_text.iter_mut() {
        text.0 = run_unlocks.rerolls_remaining.to_string();
        text_color.0 = if any_hovered { YELLOW_2 } else { WHITE };
    }
}

pub fn update_active_skill_shrine_reroll_button_state(
    run_unlocks: Res<RunUnlockState>,
    mut reroll_buttons: Query<
        (Entity, &mut Sprite, &Children),
        (With<ActiveSkillShrineRerollButton>, With<Interactable>),
    >,
    mut reroll_icons: Query<
        &mut Sprite,
        (
            With<ActiveSkillShrineRerollIcon>,
            Without<ActiveSkillShrineRerollButton>,
        ),
    >,
    mut commands: Commands,
) {
    if !run_unlocks.is_changed() {
        return;
    }

    let enabled = run_unlocks.rerolls_remaining > 0;
    let badge_color = if enabled {
        KEYBIND_BADGE_COLOR
    } else {
        Color::srgba(62. / 255., 58. / 255., 58. / 255., 0.45)
    };
    let icon_color = if enabled {
        Color::WHITE
    } else {
        Color::srgb(0.45, 0.45, 0.45)
    };

    for (e, mut sprite, children) in reroll_buttons.iter_mut() {
        sprite.color = badge_color;
        if !enabled {
            commands.entity(e).remove::<Interactable>();
        }
        for child in children.iter() {
            if let Ok(mut icon) = reroll_icons.get_mut(child) {
                icon.color = icon_color;
            }
        }
    }
}

pub fn update_active_skill_shrine_reroll_count_text(
    run_unlocks: Res<RunUnlockState>,
    mut reroll_text: Query<&mut Text2d, With<ActiveSkillShrineRerollsText>>,
) {
    if !run_unlocks.is_changed() {
        return;
    }

    for mut text in reroll_text.iter_mut() {
        text.0 = run_unlocks.rerolls_remaining.to_string();
    }
}

fn spawn_empty_shrine_slot_row(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    parent: Entity,
    visual_center_y: f32,
    slot_index: usize,
) {
    spawn_empty_shrine_slot_tooltip(
        commands,
        graphics,
        asset_server,
        parent,
        visual_center_y,
        slot_index,
    );
}

pub fn setup_active_skill_shrine_overwrite_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    shrine_overwrite: Res<ActiveSkillShrineOverwrite>,
    res: Res<ScreenResolution>,
    skills: Query<
        (
            &PlayerSkills,
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
    player_class: Res<PlayerClass>,
    unlocked_skills: Res<UnlockedSkills>,
    unlock_upgrades: Res<UnlockUpgrades>,
    cheat_settings: Res<CheatSettings>,
) {
    let overwrite_overlay = spawn_full_screen_ui_overlay_tuned(&mut commands, &res, 0.0, 0.95, 9.);
    commands
        .entity(overwrite_overlay)
        .insert(UIState::ActiveSkills);
    let Ok((
        skills,
        skill_power,
        blessings,
        max_mana,
        max_health,
        bonus_as,
        attack_speed,
        crit,
        spd,
        size,
    )) = skills.single()
    else {
        return;
    };
    let assignable_slots = shrine_assignable_slots(
        &player_class.class,
        &unlocked_skills,
        &unlock_upgrades,
        cheat_settings.bypass_class_unlocks,
    );
    let has_empty_target = assignable_slots
        .iter()
        .any(|&slot| skills.get_active_skill_in_slot(slot).is_none());
    let title = if has_empty_target {
        "Choose a Slot"
    } else {
        "Choose a Skill to Lose"
    };

    let _title_text = commands
        .spawn((
            gf::MENU_TITLE_LARGE
                .text(&asset_server, title.to_string(), WHITE)
                .with_transform(Transform {
                    translation: Vec3::new(0., res.game_height / 2. - 30., 21.),
                    scale: gf::MENU_TITLE_LARGE.transform_scale(),
                    ..Default::default()
                }),
            RenderLayers::from_layers(&[3]),
            UIState::BlessingChoice,
        ))
        .id();

    let overwrite_root = commands
        .spawn((
            (Transform::IDENTITY, Visibility::default()),
            RenderLayers::from_layers(&[3]),
            UIState::ActiveSkills,
            Name::new("Active Skill Shrine Overwrite UI"),
        ))
        .insert(ChildOf(overwrite_overlay))
        .id();

    let stats = shrine_skill_tooltip_stats((
        skill_power,
        blessings,
        max_mana,
        max_health,
        bonus_as,
        attack_speed,
        crit,
        spd,
        size,
    ));

    let slot_count = assignable_slots.len();
    let half_spacing = if slot_count > 1 {
        SHRINE_TOOLTIP_SPACING * 0.5
    } else {
        0.
    };

    if !has_empty_target {
        let icon_y = half_spacing + SKILL_TOOLTIP_SIZE.y * 0.5 + SHRINE_TOOLTIP_GAP + 16.;
        spawn_shrine_gain_skill_icon(
            &mut commands,
            &graphics,
            overwrite_root,
            icon_y,
            shrine_overwrite.skill_choice.active_skill.clone(),
        );
    }

    for (row, &slot_index) in assignable_slots.iter().enumerate() {
        let choice_option = match slot_index {
            1 => skills.active_skill_slot_1.clone(),
            2 => skills.active_skill_slot_2.clone(),
            _ => None,
        };
        let tooltip_y = half_spacing - row as f32 * SHRINE_TOOLTIP_SPACING;

        if let Some(choice) = choice_option {
            spawn_shrine_interactive_slot_tooltip(
                &mut commands,
                &graphics,
                &asset_server,
                overwrite_root,
                tooltip_y,
                ActiveSkillSlotChoiceUI {
                    index: slot_index,
                    skill_choice: Some(choice),
                    interaction_lock_timer: Timer::from_seconds(0.75, TimerMode::Once),
                },
                &stats,
                if slot_index == 1 {
                    "SKILL_SLOT_1_TOOLTIP_CONTAINER"
                } else {
                    "SKILL_SLOT_2_TOOLTIP_CONTAINER"
                },
                if slot_index == 1 {
                    "SKILL_SLOT_1_TOOLTIP_HIT"
                } else {
                    "SKILL_SLOT_2_TOOLTIP_HIT"
                },
                slot_index as u32,
            );
        } else {
            spawn_empty_shrine_slot_row(
                &mut commands,
                &graphics,
                &asset_server,
                overwrite_root,
                tooltip_y,
                slot_index,
            );
        }
    }

    let back_button = spawn_back_button(
        Vec3::new(
            res.game_width / 2. - 55.,
            -res.game_height / 2. + 38.,
            SHRINE_BACK_BUTTON_Z,
        ),
        &mut commands,
        &graphics,
        &asset_server,
    );
    commands
        .entity(back_button)
        .insert(UIState::ActiveSkills)
        .insert(Focusable {
            group: UIState::ActiveSkills,
            index: 20,
        });
}

pub fn handle_active_skill_shrine_overwrite_interaction(
    cursor_pos: Res<crate::cursor::CursorPos>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut skill_choices: Query<(Entity, &mut Interactable, &ActiveSkillSlotChoiceUI)>,
    parents: Query<&ChildOf>,
    mut containers: Query<(&mut Transform, &mut BounceOnHit)>,
    mut player_skills: Query<(
        Entity,
        &mut crate::player::skills::PlayerSkills,
        &GlobalTransform,
    )>,
    mut next_ui_state: ResMut<NextState<UIState>>,
    mut commands: Commands,
    mut att_event: MessageWriter<crate::attributes::AttributeChangeEvent>,
    mut shrine_query: Query<&mut crate::item::active_skill_shrine::ActiveSkillShrineState>,
    shrine_overwrite_res: Option<Res<ActiveSkillShrineOverwrite>>,
    shrine_focus: ShrineUiFocus,
    mut tutorial_trigger: ActiveSkillShrineTutorialTrigger,
) {
    // Only handle if this is a shrine overwrite, not heirloom limbo
    let overwrite = if let Some(overwrite_res) = shrine_overwrite_res.as_ref() {
        overwrite_res
    } else {
        return; // This is an heirloom limbo overwrite, handled elsewhere
    };

    let hit_test = super::ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None, None);
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);

    for (e, mut interactable, skill_ui) in skill_choices.iter_mut() {
        let is_hit = matches!(hit_test, Some(hit_ent) if hit_ent.0 == e);
        let is_focused = shrine_focus.is_focused(e, &cursor_pos);
        let confirm_pressed =
            (is_hit && left_mouse_pressed) || (is_focused && shrine_focus.confirm_just_pressed());

        if is_hit || is_focused {
            match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                    commands.spawn(crate::audio::SoundSpawner::new(
                        crate::audio::AudioSoundEffect::UISkillHover,
                        0.2,
                    ));
                    if let Ok(parent) = parents.get(e) {
                        if let Ok((mut transform, mut bounce)) = containers.get_mut(parent.parent())
                        {
                            super::ui_helpers::apply_ui_hover_scale(
                                &mut transform,
                                Some(&mut bounce),
                                true,
                            );
                            bounce.activate();
                        }
                    }
                }
                Interaction::Hovering => {
                    if confirm_pressed && skill_ui.interaction_lock_timer.is_finished() {
                        let Ok((player_e, mut skills, _t)) = player_skills.single_mut() else {
                            return;
                        };
                        let new_skill = overwrite.skill_choice.clone();

                        assign_shrine_skill_to_slot(&mut skills, skill_ui.index, new_skill.clone());
                        new_skill
                            .active_skill
                            .add_skill_components(player_e, &mut commands);

                        // Mark shrine as used
                        if let Ok(mut shrine_state) = shrine_query.get_mut(overwrite.shrine_entity)
                        {
                            shrine_state.is_used = true;
                        }

                        // Remove resources
                        commands.remove_resource::<ActiveSkillShrineOverwrite>();
                        commands.remove_resource::<ActiveSkillShrineSelection>();
                        next_ui_state.set(UIState::Closed);
                        att_event.write(crate::attributes::AttributeChangeEvent);
                        tutorial_trigger.try_trigger();
                    }
                }
                _ => (),
            }
        } else {
            let Interaction::Hovering = interactable.current() else {
                continue;
            };
            interactable.change(Interaction::None);
            if let Ok(parent) = parents.get(e) {
                if let Ok((mut transform, mut bounce)) = containers.get_mut(parent.parent()) {
                    super::ui_helpers::apply_ui_hover_scale(
                        &mut transform,
                        Some(&mut bounce),
                        false,
                    );
                }
            }
        }
    }
}
