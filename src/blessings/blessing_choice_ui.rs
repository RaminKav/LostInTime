use bevy::text::Justify;
use bevy::{camera::visibility::RenderLayers, prelude::*, sprite::Anchor};

use crate::{
    assets::Graphics,
    attributes::{
        AttackSpeed, BonusAttackSpeed, CritChance, ItemRarity, MaxHealth, MaxMana, ProjectileSize,
        SkillPower, Speed,
    },
    audio::{AudioSoundEffect, SoundSpawner},
    blessings::{
        build_ancestor_blessing_offer, build_major_blessing_offer, Ancestor, AncestorBlessing,
        AncestorBlessingIcon, AncestorBlessingOffer, BlessingTier, CurrentBlessingTier,
        DeferredEraSwap, MajorBlessingOffer, OwnedBlessings, OwnedMajorBlessings,
        PendingMajorHeirloomPick, PendingRunStartBlessing, ResolvedAncestorBlessing,
        ResolvedMajorBlessing,
    },
    colors::{LIGHT_RED, SHIELD_BLUE, WHITE, YELLOW_2},
    cursor::CursorPos,
    inventory::ItemStack,
    item::{
        item_drop_outline::{HeirloomIconOutline, UiShadowChild},
        WorldObject,
    },
    juice::bounce::BounceOnHit,
    player::{
        levels::PlayerLevel,
        skills::{
            ActiveSkill, Heirloom, HeirloomChoiceQueue, HeirloomRarity, HeirloomWithRarity,
            PlayerClass, PlayerSkills,
        },
        unlocks::RunUnlockState,
        Player,
    },
    proto::proto_param::ProtoParam,
    ui::{
        clamp_tooltip_center_x, clamp_tooltip_center_y,
        damage_numbers::spawn_floating_text_with_shadow,
        desc_spans::{blessing_desc_line, skill_desc_line, spawn_desc_line},
        game_fonts::{self as gf, FLOATING_TEXT, HEIRLOOM_CARD_DESC_LINE_STEP},
        player_hud::{
            active_skill_tooltip_params_from_player, spawn_skill_tooltip_content,
            spawn_skill_tooltip_shell, SKILL_TOOLTIP_ICON_SIZE,
        },
        set_sprite_image,
        ui_helpers::{self, spawn_full_screen_ui_overlay_tuned},
        CheatSettings, Focusable, HeirloomDynamicTooltip, HeirloomTooltipRequest,
        HeirloomTooltipShow, Interactable, Interaction, ItemOrRecipeTooltip, ToolTipUpdateEvent,
        UIElement, UIState, ITEM_TOOLTIP_LARGE_CARD_SIZE, SKILLS_CHOICE_UI_SIZE,
    },
    GameState, ScreenResolution, DEBUG,
};

/// Card payload — minor (run-start) or major (mid-run) blessing.
#[derive(Clone, Debug)]
pub enum BlessingChoiceKind {
    Minor(ResolvedAncestorBlessing),
    Major(ResolvedMajorBlessing),
}

impl BlessingChoiceKind {
    fn display_card_rarity(&self) -> Option<HeirloomRarity> {
        match self {
            BlessingChoiceKind::Minor(c) => c.blessing.display_card_rarity(),
            BlessingChoiceKind::Major(c) => c.display_card_rarity(),
        }
    }

    fn title(&self) -> &str {
        match self {
            BlessingChoiceKind::Minor(c) => &c.title,
            BlessingChoiceKind::Major(c) => &c.title,
        }
    }

    fn description(&self) -> &[String] {
        match self {
            BlessingChoiceKind::Minor(c) => &c.description,
            BlessingChoiceKind::Major(c) => &c.description,
        }
    }

    /// Minor chaos blessings append generic `-Max HP` / `+Chaos` lines.
    /// Majors bake any tradeoff into their own description text — do not append.
    fn appends_chaos_tradeoff_lines(&self) -> bool {
        match self {
            BlessingChoiceKind::Minor(c) => c.blessing.max_hp_penalty_pct() > 0.0,
            BlessingChoiceKind::Major(_) => false,
        }
    }

    fn max_hp_penalty_pct(&self) -> f32 {
        match self {
            BlessingChoiceKind::Minor(c) => c.blessing.max_hp_penalty_pct(),
            BlessingChoiceKind::Major(c) => c.max_hp_penalty_pct(),
        }
    }

    fn starting_chaos(&self) -> f32 {
        match self {
            BlessingChoiceKind::Minor(c) => c.blessing.starting_chaos(),
            BlessingChoiceKind::Major(c) => c.starting_chaos(),
        }
    }

    fn needs_heirloom_reveal_delay(&self) -> bool {
        match self {
            BlessingChoiceKind::Minor(c) => c.blessing.needs_heirloom_reveal_delay(),
            BlessingChoiceKind::Major(c) => c.blessing.needs_heirloom_pick_ui(),
        }
    }

    pub fn as_minor(&self) -> Option<&ResolvedAncestorBlessing> {
        match self {
            BlessingChoiceKind::Minor(c) => Some(c),
            _ => None,
        }
    }
}

#[derive(Component, Clone)]
pub struct BlessingChoiceUI {
    pub selected: bool,
    pub ancestor: Ancestor,
    pub choice: BlessingChoiceKind,
}

#[derive(Message)]
pub struct AncestorBlessingSelectEvent {
    pub ancestor: Ancestor,
    pub choice: ResolvedAncestorBlessing,
}

#[derive(Message)]
pub struct MajorBlessingSelectEvent {
    pub ancestor: Ancestor,
    pub choice: ResolvedMajorBlessing,
}

#[derive(Resource)]
pub struct BlessingTransitionState {
    pub timer: Timer,
    pub heirlooms: Option<Vec<HeirloomWithRarity>>,
}

#[derive(Clone)]
pub enum BlessingChoiceTooltipTarget {
    Heirloom(Heirloom, HeirloomRarity),
    Skill(ActiveSkill),
    Item(WorldObject),
}

#[derive(Component)]
pub(crate) struct BlessingChoiceSkillTooltip;

pub fn enter_blessing_ui(mut next_ui_state: ResMut<NextState<UIState>>) {
    next_ui_state.set(UIState::BlessingChoice);
}

const BLESSING_CARD_TITLE_Y_OFFSET: f32 = -4.;
const BLESSING_CARD_DESC_Y_OFFSET: f32 = -2.;
const BLESSING_CHAOS_DESC_GAP: f32 = 4.0;
const BLESSING_HEIRLOOM_REVEAL_TEXT_Y: f32 = -102.;
const BLESSING_CARD_ICON_OFFSET: Vec3 = Vec3::new(2., 52., 4.);
/// Bounce strength for blessing choice cards on hover (fraction of default mob bounce).
const BLESSING_CARD_BOUNCE_STRENGTH: f32 = 0.4;
/// Layer-3 z for blessing hover tooltips — matches [`HEIRLOOM_TOOLTIP_CARD_Z`] so skill/item
/// cards render above the screen title copy (z ≈ 20) and choice cards (z ≈ 10).
const BLESSING_CHOICE_TOOLTIP_Z: f32 = 140.0;
/// Gap between the bottom edge of a blessing card and its ancestor name label.
const BLESSING_ANCESTOR_LABEL_GAP: f32 = 16.0;
/// Horizontal gap left between a card's edge and the tooltip docked beside it.
const BLESSING_TOOLTIP_CARD_GAP: f32 = 8.0;

/// Docks a tooltip beside its card rather than a fixed offset, so cards of any width never
/// overlap their own tooltip: cards in the "left half" of the row dock their tooltip to the
/// right, cards in the "right half" dock to the left. Returns the *unclamped* desired tooltip
/// center x (screen-edge clamping still happens afterward).
fn blessing_tooltip_dock_x(
    card_center_x: f32,
    card_half_width: f32,
    tooltip_half_width: f32,
    card_index: u32,
    card_count: u32,
) -> f32 {
    let dock_right = card_count <= 1 || card_index as f32 <= (card_count as f32 - 1.0) / 2.0;
    if dock_right {
        card_center_x + card_half_width + BLESSING_TOOLTIP_CARD_GAP + tooltip_half_width
    } else {
        card_center_x - card_half_width - BLESSING_TOOLTIP_CARD_GAP - tooltip_half_width
    }
}

fn blessing_choice_tooltip_target(
    choice: &BlessingChoiceKind,
) -> Option<BlessingChoiceTooltipTarget> {
    let choice = choice.as_minor()?;
    if choice.blessing.hides_resolved_reward_from_player() {
        return None;
    }
    match &choice.display_icon {
        Some(AncestorBlessingIcon::Heirloom(heirloom, rarity)) => Some(
            BlessingChoiceTooltipTarget::Heirloom(heirloom.clone(), *rarity),
        ),
        Some(AncestorBlessingIcon::Skill(skill)) => {
            Some(BlessingChoiceTooltipTarget::Skill(skill.clone()))
        }
        Some(AncestorBlessingIcon::Item(item)) => Some(BlessingChoiceTooltipTarget::Item(*item)),
        Some(AncestorBlessingIcon::Mystery) => None,
        None => choice
            .resolved_heirloom
            .as_ref()
            .map(|h| BlessingChoiceTooltipTarget::Heirloom(h.heirloom.clone(), h.rarity))
            .or_else(|| {
                choice
                    .resolved_skill
                    .map(BlessingChoiceTooltipTarget::Skill)
            })
            .or_else(|| {
                choice
                    .resolved_item
                    .or(choice.resolved_weapon)
                    .map(BlessingChoiceTooltipTarget::Item)
            }),
    }
}

fn blessing_item_stack_for_tooltip(
    item: WorldObject,
    choice: &ResolvedAncestorBlessing,
    proto: &ProtoParam,
) -> ItemStack {
    let mut stack = proto
        .get_item_data(item)
        .cloned()
        .unwrap_or_else(|| ItemStack::crate_icon_stack(item));
    stack.count = 1;

    if choice.blessing == AncestorBlessing::UpgradeStartingWeapon && item.is_weapon() {
        stack.rarity = ItemRarity::Common.get_next_rarity();
    }

    stack
}

fn fade_blessing_card_descendants(
    entity: Entity,
    alpha: f32,
    children: &Query<&Children>,
    sprites: &mut Query<&mut Sprite>,
    visibility_set: &mut ParamSet<(
        Query<'_, '_, &mut Visibility, With<Text2d>>,
        Query<'_, '_, &mut Visibility, With<UiShadowChild>>,
    )>,
) {
    if let Ok(mut sprite) = sprites.get_mut(entity) {
        sprite.color = sprite.color.with_alpha(alpha);
    }
    {
        let mut texts = visibility_set.p0();
        if let Ok(mut visibility) = texts.get_mut(entity) {
            *visibility = Visibility::Hidden;
        }
    }
    // Shadow children use a mesh material, not Sprite — hide them as soon as the card fades.
    {
        let mut shadow_visibilities = visibility_set.p1();
        if let Ok(mut visibility) = shadow_visibilities.get_mut(entity) {
            *visibility = if alpha < 1.0 {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            };
        }
    }
    if let Ok(kids) = children.get(entity) {
        for child in kids.iter() {
            fade_blessing_card_descendants(child, alpha, children, sprites, visibility_set);
        }
    }
}

fn blessing_choice_card_ui(choice: &BlessingChoiceKind) -> (UIElement, Vec2) {
    if let Some(rarity) = choice.display_card_rarity() {
        return Heirloom::None.get_ui_element(rarity);
    }
    if let Some(minor) = choice.as_minor() {
        if let Some(heirloom) = minor.resolved_heirloom.as_ref() {
            return heirloom.heirloom.get_ui_element(heirloom.rarity);
        }
    }
    (UIElement::SkillChoice, SKILLS_CHOICE_UI_SIZE)
}

fn blessing_choice_card_hover_ui(choice: &BlessingChoiceKind) -> (UIElement, UIElement) {
    if let Some(rarity) = choice.display_card_rarity() {
        return (
            Heirloom::None.get_ui_element(rarity).0,
            Heirloom::None.get_ui_element_hover(rarity),
        );
    }
    if let Some(minor) = choice.as_minor() {
        if let Some(heirloom) = minor.resolved_heirloom.as_ref() {
            return (
                heirloom.heirloom.get_ui_element(heirloom.rarity).0,
                heirloom.heirloom.get_ui_element_hover(heirloom.rarity),
            );
        }
    }
    (UIElement::SkillChoice, UIElement::SkillChoiceHover)
}

fn spawn_blessing_choice_card(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    ancestor: Ancestor,
    choice: &BlessingChoiceKind,
    position: Vec3,
) -> Entity {
    let (ui_element, size) = blessing_choice_card_ui(choice);

    let card_e = commands
        .spawn((
            Sprite {
                image: graphics.get_ui_element_texture(ui_element.clone()),
                custom_size: Some(size),
                ..default()
            },
            Transform {
                translation: position,
                ..Default::default()
            },
        ))
        .insert(ui_element)
        .insert(Name::new("BLESSING CHOICE UI"))
        .insert(RenderLayers::from_layers(&[3]))
        .insert(crate::item::item_drop_outline::UiShadow::container())
        .id();

    if let Some(minor) = choice.as_minor() {
        if let Some(icon) = &minor.display_icon {
            spawn_blessing_card_icon(commands, graphics, asset_server, card_e, icon);
        }
    }

    let mut text_title = commands.spawn((
        gf::HEIRLOOM_CARD_TITLE
            .text(&asset_server, choice.title().to_string(), WHITE)
            .anchor(Anchor::CENTER)
            .with_transform(Transform {
                translation: Vec3::new(0., 24. + BLESSING_CARD_TITLE_Y_OFFSET, 1.),
                scale: gf::HEIRLOOM_CARD_TITLE.transform_scale(),
                ..Default::default()
            }),
        Name::new("Blessing Title"),
        RenderLayers::from_layers(&[3]),
    ));
    text_title.insert(ChildOf(card_e));

    // Ancestor name sits just below the card (Alagard DISPLAY / 15pt).
    let ancestor_label_y = -(size.y * 0.5) - BLESSING_ANCESTOR_LABEL_GAP;
    let mut ancestor_label = commands.spawn((
        gf::DISPLAY
            .text(
                asset_server,
                ancestor.title().to_string(),
                ancestor.title_color(),
            )
            .anchor(Anchor::CENTER)
            .with_transform(Transform {
                translation: Vec3::new(0., ancestor_label_y, 1.),
                scale: gf::DISPLAY.transform_scale(),
                ..Default::default()
            }),
        Name::new("Blessing Ancestor Label"),
        RenderLayers::from_layers(&[3]),
    ));
    ancestor_label.insert(ChildOf(card_e));

    let desc_lines: Vec<&str> = choice.description().iter().map(String::as_str).collect();
    let has_chaos = choice.appends_chaos_tradeoff_lines();
    let chaos_line_count = if has_chaos { 2 } else { 0 };

    let mut block_line_ys: Vec<f32> = (0..desc_lines.len())
        .map(|i| gf::heirloom_desc_first_line_y() - i as f32 * HEIRLOOM_CARD_DESC_LINE_STEP)
        .collect();
    if has_chaos {
        let chaos_first_y = gf::heirloom_desc_first_line_y()
            - desc_lines.len() as f32 * HEIRLOOM_CARD_DESC_LINE_STEP
            - BLESSING_CHAOS_DESC_GAP;
        for i in 0..chaos_line_count {
            block_line_ys.push(chaos_first_y - i as f32 * HEIRLOOM_CARD_DESC_LINE_STEP);
        }
    }

    let block_center_offset = block_line_ys
        .first()
        .zip(block_line_ys.last())
        .map(|(top, bottom)| {
            gf::heirloom_desc_text_center_y() - (top + bottom) * 0.5 + BLESSING_CARD_DESC_Y_OFFSET
        })
        .unwrap_or(BLESSING_CARD_DESC_Y_OFFSET);

    let highlight_phrases = choice
        .as_minor()
        .map(|c| c.description_highlight_phrases())
        .unwrap_or_default();
    for (line_index, desc) in desc_lines.iter().enumerate() {
        let y = block_line_ys[line_index] + block_center_offset;
        spawn_desc_line(
            commands,
            asset_server,
            gf::HEIRLOOM_CARD_BODY,
            &blessing_desc_line(*desc, &highlight_phrases),
            YELLOW_2,
            Vec3::new(2., y, 1.),
            Anchor::CENTER,
            Justify::Center,
            3,
            card_e,
        );
    }

    if has_chaos {
        let chaos_lines = [
            format!("-{}% Max HP", (choice.max_hp_penalty_pct() * 100.0) as i32),
            format!("+{} Chaos", choice.starting_chaos() as i32),
        ];

        for (line_index, line) in chaos_lines.iter().enumerate() {
            let y = block_line_ys[desc_lines.len() + line_index] + block_center_offset;
            spawn_desc_line(
                commands,
                asset_server,
                gf::HEIRLOOM_CARD_BODY,
                &skill_desc_line(line),
                LIGHT_RED,
                Vec3::new(2., y, 1.),
                Anchor::CENTER,
                Justify::Center,
                3,
                card_e,
            );
        }
    }

    card_e
}

fn spawn_blessing_card_icon(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    card_e: Entity,
    icon: &AncestorBlessingIcon,
) {
    match icon {
        AncestorBlessingIcon::Mystery => {
            commands
                .spawn((
                    gf::GLOBAL_MESSAGE
                        .text(&asset_server, "?", SHIELD_BLUE)
                        .anchor(Anchor::CENTER)
                        .with_transform(Transform {
                            translation: Vec3::new(2., 52., 4.),
                            scale: gf::GLOBAL_MESSAGE.transform_scale(),
                            ..Default::default()
                        }),
                    RenderLayers::from_layers(&[3]),
                    Name::new("BLESSING MYSTERY ICON"),
                ))
                .insert(ChildOf(card_e));
        }
        AncestorBlessingIcon::Heirloom(heirloom, rarity) => {
            let mut icon_sprite = graphics.get_heirloom_icon(heirloom.clone());
            icon_sprite.custom_size = Some(Vec2::new(32., 32.));
            let skill_icon = commands
                .spawn((
                    icon_sprite,
                    Transform {
                        translation: Vec2::new(2., 52.).extend(4.),
                        ..Default::default()
                    },
                    RenderLayers::from_layers(&[3]),
                    HeirloomIconOutline::new(*rarity, Default::default()),
                    Name::new("BLESSING HEIRLOOM ICON"),
                ))
                .id();
            commands.entity(skill_icon).insert(ChildOf(card_e));

            if let Some(glow) = rarity.get_item_glow() {
                commands
                    .spawn((
                        Sprite {
                            image: graphics.get_item_glow(glow),
                            custom_size: Some(Vec2::new(32., 32.)),
                            ..default()
                        },
                        Transform {
                            translation: Vec2::new(0., 0.).extend(-1.),
                            ..Default::default()
                        },
                    ))
                    .insert(RenderLayers::from_layers(&[3]))
                    .insert(ChildOf(skill_icon));
            }
        }
        AncestorBlessingIcon::Skill(active_skill) => {
            commands
                .spawn((
                    (
                        Sprite {
                            image: graphics.get_active_skill_icon(active_skill.clone()),
                            custom_size: Some(Vec2::new(32., 32.)),
                            ..default()
                        },
                        Transform {
                            translation: Vec3::new(2., 52., 4.),
                            ..Default::default()
                        },
                    ),
                    RenderLayers::from_layers(&[3]),
                    Name::new("BLESSING SKILL ICON"),
                ))
                .insert(ChildOf(card_e));
        }
        AncestorBlessingIcon::Item(item) => {
            let mut sprite = graphics
                .icons
                .as_ref()
                .and_then(|icons| icons.get(item).cloned())
                .or_else(|| {
                    graphics
                        .spritesheet_map
                        .as_ref()
                        .and_then(|map| map.get(item).cloned())
                });
            if let Some(ref mut sprite) = sprite {
                // Inventory / sheet item size. Skill & heirloom card icons use 32×32; leaving
                // world-object icons at that size 2×-upscales the 16px frames.
                sprite.custom_size = Some(Vec2::splat(16.));
                commands
                    .spawn((
                        sprite.clone(),
                        Transform {
                            translation: Vec2::new(2., 52.).extend(4.),
                            ..Default::default()
                        },
                    ))
                    .insert(RenderLayers::from_layers(&[3]))
                    .insert(Name::new("BLESSING ITEM ICON"))
                    .insert(ChildOf(card_e));
            }
        }
    }
}

pub fn setup_minor_blessing_choice_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    res: Res<ScreenResolution>,
    heirloom_queue: Res<HeirloomChoiceQueue>,
    player_skills: Query<&PlayerSkills>,
    player_level: Query<&PlayerLevel>,
    player_class: Option<Res<PlayerClass>>,
    existing_offer: Option<Res<AncestorBlessingOffer>>,
    mut current_tier: ResMut<CurrentBlessingTier>,
) {
    current_tier.0 = BlessingTier::Minor;
    // Reuse a pending offer after temporary UI leave (inventory/map/options), like Skills.
    let offer = if let Some(existing) = existing_offer {
        existing.clone()
    } else {
        let Ok(player_level) = player_level.single() else {
            return;
        };
        let player_level = player_level.level;
        let Ok(player_skills) = player_skills.single() else {
            return;
        };
        let starting_weapon = player_class
            .as_ref()
            .map(|pc| pc.class.get_starting_wep())
            .unwrap_or(WorldObject::Sword);
        let offer = build_ancestor_blessing_offer(
            heirloom_queue.as_ref(),
            Some(player_skills),
            player_level,
            starting_weapon,
        );
        commands.insert_resource(offer.clone());
        offer
    };

    let choices: Vec<(Ancestor, BlessingChoiceKind)> = offer
        .choices
        .into_iter()
        .map(|(a, c)| (a, BlessingChoiceKind::Minor(c)))
        .collect();
    spawn_blessing_choice_screen(
        &mut commands,
        &graphics,
        &asset_server,
        &res,
        UIState::BlessingChoice,
        "Choose a Blessing",
        "Back again...? You hear the voice of your distant ancestor...",
        &choices,
    );
}

pub fn setup_major_blessing_choice_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    res: Res<ScreenResolution>,
    player_skills: Query<&PlayerSkills>,
    owned_majors: Query<&OwnedMajorBlessings>,
    run_unlocks: Option<Res<RunUnlockState>>,
    existing_offer: Option<Res<MajorBlessingOffer>>,
    mut current_tier: ResMut<CurrentBlessingTier>,
) {
    current_tier.0 = BlessingTier::Major;
    // Reuse a pending offer after temporary UI leave (inventory/map/options), like Skills.
    let offer = if let Some(existing) = existing_offer {
        existing.clone()
    } else {
        let Ok(player_skills) = player_skills.single() else {
            return;
        };
        let owned = owned_majors.single().ok().cloned().unwrap_or_default();
        let offer = build_major_blessing_offer(&owned, Some(player_skills), run_unlocks.as_deref());
        commands.insert_resource(offer.clone());
        offer
    };

    let choices: Vec<(Ancestor, BlessingChoiceKind)> = offer
        .choices
        .into_iter()
        .map(|(a, c)| (a, BlessingChoiceKind::Major(c)))
        .collect();
    spawn_blessing_choice_screen(
        &mut commands,
        &graphics,
        &asset_server,
        &res,
        UIState::MajorBlessingChoice,
        "Choose a Major Blessing",
        "Your ancestor's power surges after the fallen titan...",
        &choices,
    );
}

/// Match skill-choice / shrine overlays: clear centre, dark radial edge.
const BLESSING_OVERLAY_CENTER_ALPHA: f32 = 0.0;
const BLESSING_OVERLAY_EDGE_ALPHA: f32 = 0.988;

fn spawn_blessing_choice_screen(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    res: &ScreenResolution,
    ui_state: UIState,
    title: &str,
    subtitle: &str,
    choices: &[(Ancestor, BlessingChoiceKind)],
) {
    let t_offset = Vec2::new(4., 4.);

    commands.spawn((
        gf::GLOBAL_MESSAGE
            .text(asset_server, title.to_string(), WHITE)
            .with_transform(Transform {
                translation: Vec3::new(0., 144., 20.),
                scale: gf::GLOBAL_MESSAGE.transform_scale(),
                ..Default::default()
            }),
        RenderLayers::from_layers(&[3]),
        ui_state.clone(),
    ));

    commands.spawn((
        gf::BODY
            .text(asset_server, subtitle.to_string(), WHITE)
            .with_transform(Transform {
                translation: Vec3::new(0., 110., 20.),
                scale: gf::BODY.transform_scale(),
                ..Default::default()
            }),
        RenderLayers::from_layers(&[3]),
        ui_state.clone(),
    ));

    let overlay = spawn_full_screen_ui_overlay_tuned(
        commands,
        res,
        BLESSING_OVERLAY_CENTER_ALPHA,
        BLESSING_OVERLAY_EDGE_ALPHA,
        9.,
    );
    commands.entity(overlay).insert(ui_state.clone());

    spawn_blessing_choice_cards(
        commands,
        graphics,
        asset_server,
        choices,
        t_offset,
        ui_state,
    );
}

fn spawn_blessing_choice_cards(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    choices: &[(Ancestor, BlessingChoiceKind)],
    t_offset: Vec2,
    ui_state: UIState,
) {
    let count = choices.len();
    for i in -1i32..(choices.len() as i32 - 1) {
        let (ancestor, choice) = choices[(i + 1) as usize].clone();
        let card_index = (i + 1) as u32;
        let (_, size) = blessing_choice_card_ui(&choice);
        let translation = Vec2::new(
            i as f32 * (size.x + 8.) + if count == 2 { size.x / 2. } else { 0. } + 0.1,
            -20.,
        );
        let position = Vec3::new(
            (translation.x + t_offset.x).round(),
            (translation.y + t_offset.y).round(),
            10.,
        );
        let card_e = spawn_blessing_choice_card(
            commands,
            graphics,
            asset_server,
            ancestor,
            &choice,
            position,
        );
        commands
            .entity(card_e)
            .insert(BlessingChoiceUI {
                selected: false,
                ancestor,
                choice: choice.clone(),
            })
            .insert(ui_state.clone())
            .insert(Interactable::default())
            .insert(Focusable {
                group: ui_state.clone(),
                index: card_index,
            })
            .insert(BounceOnHit::with_strength_fraction(
                BLESSING_CARD_BOUNCE_STRENGTH,
            ));
    }
}

#[derive(bevy::ecs::system::SystemParam)]
pub struct DebugBlessingRerollParam<'w, 's> {
    key_input: Res<'w, ButtonInput<KeyCode>>,
    cheat_settings: Option<Res<'w, CheatSettings>>,
    old_cards: Query<'w, 's, Entity, With<BlessingChoiceUI>>,
    skill_tooltips: Query<'w, 's, Entity, With<BlessingChoiceSkillTooltip>>,
    heirloom_tooltips: Query<'w, 's, Entity, With<HeirloomDynamicTooltip>>,
    item_tooltips: Query<'w, 's, Entity, With<ItemOrRecipeTooltip>>,
    tooltip_requests: MessageWriter<'w, HeirloomTooltipRequest>,
    commands: Commands<'w, 's>,
    graphics: Res<'w, Graphics>,
    asset_server: Res<'w, AssetServer>,
    heirloom_queue: Res<'w, HeirloomChoiceQueue>,
    player_skills: Query<'w, 's, &'static PlayerSkills>,
    player_level: Query<'w, 's, &'static PlayerLevel>,
    player_class: Option<Res<'w, PlayerClass>>,
    owned_majors: Query<'w, 's, &'static OwnedMajorBlessings>,
    run_unlocks: Option<Res<'w, RunUnlockState>>,
    current_tier: Res<'w, CurrentBlessingTier>,
    ui_state: Res<'w, State<UIState>>,
}

/// Debug/dev-mode: press B while the game UI is closed to open the major blessing picker
/// without queueing a portal era swap (so picks can be tested in-place).
pub fn debug_open_major_blessing_ui(
    key_input: Res<ButtonInput<KeyCode>>,
    cheat_settings: Option<Res<CheatSettings>>,
    ui_state: Res<State<UIState>>,
    mut next_ui_state: ResMut<NextState<UIState>>,
    mut current_tier: ResMut<CurrentBlessingTier>,
    mut deferred_era_swap: ResMut<DeferredEraSwap>,
) {
    let dev_mode = cheat_settings.map(|c| c.dev_mode).unwrap_or(false);
    if !(*DEBUG || dev_mode) || !key_input.just_pressed(KeyCode::KeyB) {
        return;
    }
    if *ui_state.get() != UIState::Closed {
        return;
    }
    // Never couple the debug open path to the portal's deferred era swap.
    deferred_era_swap.era = None;
    current_tier.0 = BlessingTier::Major;
    next_ui_state.set(UIState::MajorBlessingChoice);
    info!("DEBUG: opened MajorBlessingChoice (no era swap queued)");
}

/// Debug/dev-mode: press N to reroll the three blessing options (same as skill choice UI).
pub fn debug_reroll_blessing_choices(mut p: DebugBlessingRerollParam) {
    let key_input = &p.key_input;
    let cheat_settings = p.cheat_settings.as_ref();
    let old_cards = &p.old_cards;
    let skill_tooltips = &p.skill_tooltips;
    let heirloom_tooltips = &p.heirloom_tooltips;
    let item_tooltips = &p.item_tooltips;
    let tooltip_requests = &mut p.tooltip_requests;
    let commands = &mut p.commands;
    let graphics = &p.graphics;
    let asset_server = &p.asset_server;
    let heirloom_queue = &p.heirloom_queue;
    let player_skills = &p.player_skills;
    let player_level = &p.player_level;
    let player_class = p.player_class.as_ref();
    let owned_majors = &p.owned_majors;
    let run_unlocks = p.run_unlocks.as_ref();
    let current_tier = &p.current_tier;
    let ui_state = &p.ui_state;
    let dev_mode = cheat_settings.map(|c| c.dev_mode).unwrap_or(false);
    if !(*DEBUG || dev_mode) || !key_input.just_pressed(KeyCode::KeyN) {
        return;
    }

    let Ok(player_skills) = player_skills.single() else {
        return;
    };

    for e in old_cards.iter() {
        commands.entity(e).despawn();
    }
    for e in skill_tooltips.iter() {
        commands.entity(e).despawn();
    }
    for e in heirloom_tooltips.iter() {
        commands.entity(e).despawn();
    }
    for e in item_tooltips.iter() {
        commands.entity(e).despawn();
    }
    tooltip_requests.write(HeirloomTooltipRequest::Clear);

    let active_ui = ui_state.get().clone();
    let choices = match current_tier.0 {
        BlessingTier::Minor => {
            let Ok(player_level) = player_level.single() else {
                return;
            };
            let starting_weapon = player_class
                .map(|pc| pc.class.get_starting_wep())
                .unwrap_or(WorldObject::Sword);
            let offer = build_ancestor_blessing_offer(
                heirloom_queue.as_ref(),
                Some(player_skills),
                player_level.level,
                starting_weapon,
            );
            commands.insert_resource(offer.clone());
            offer
                .choices
                .into_iter()
                .map(|(a, c)| (a, BlessingChoiceKind::Minor(c)))
                .collect::<Vec<_>>()
        }
        BlessingTier::Major => {
            let owned = owned_majors.single().ok().cloned().unwrap_or_default();
            let offer =
                build_major_blessing_offer(&owned, Some(player_skills), run_unlocks.map(|r| &**r));
            commands.insert_resource(offer.clone());
            offer
                .choices
                .into_iter()
                .map(|(a, c)| (a, BlessingChoiceKind::Major(c)))
                .collect::<Vec<_>>()
        }
    };

    spawn_blessing_choice_cards(
        commands,
        graphics,
        asset_server,
        &choices,
        Vec2::new(4., 4.),
        active_ui,
    );
}

pub fn handle_blessing_choice_card_interactions(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut blessing_choices: Query<(
        Entity,
        &mut Interactable,
        &mut BlessingChoiceUI,
        &mut Transform,
        &mut BounceOnHit,
    )>,
    mut commands: Commands,
    graphics: Res<Graphics>,
    mut minor_event: MessageWriter<AncestorBlessingSelectEvent>,
    mut major_event: MessageWriter<MajorBlessingSelectEvent>,
    ui_focus: Res<crate::ui::focus::UiFocus>,
) {
    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None, None);
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);

    for (e, mut interactable, mut state, mut transform, mut bounce) in blessing_choices.iter_mut() {
        let (default_ui, hover_ui) = blessing_choice_card_hover_ui(&state.choice);
        let is_hit = matches!(hit_test, Some(hit_ent) if hit_ent.0 == e);
        let is_focused = ui_focus.is_focused(e);
        let confirm_pressed =
            (is_hit && left_mouse_pressed) || (is_focused && ui_focus.confirm_just_pressed);

        if is_hit || is_focused {
            match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::UISkillHover, 0.2));

                    commands.entity(e).insert(hover_ui.clone());
                    set_sprite_image(&mut commands, e, graphics.get_ui_element_texture(hover_ui));
                    ui_helpers::apply_ui_hover_scale(&mut transform, Some(&mut bounce), true);
                    bounce.activate();
                }
                Interaction::Hovering => {
                    if confirm_pressed {
                        state.selected = true;
                        match &state.choice {
                            BlessingChoiceKind::Minor(choice) => {
                                info!(
                                    "CLICKED ANCESTOR BLESSING: {:?} from {:?}",
                                    choice.blessing, state.ancestor
                                );
                                minor_event.write(AncestorBlessingSelectEvent {
                                    ancestor: state.ancestor,
                                    choice: choice.clone(),
                                });
                                commands.remove_resource::<PendingRunStartBlessing>();
                            }
                            BlessingChoiceKind::Major(choice) => {
                                info!(
                                    "CLICKED MAJOR BLESSING: {:?} from {:?}",
                                    choice.blessing, state.ancestor
                                );
                                major_event.write(MajorBlessingSelectEvent {
                                    ancestor: state.ancestor,
                                    choice: choice.clone(),
                                });
                            }
                        }

                        let delay_sec = if state.choice.needs_heirloom_reveal_delay() {
                            2.
                        } else {
                            0.75
                        };
                        commands.insert_resource(BlessingTransitionState {
                            timer: Timer::from_seconds(delay_sec, TimerMode::Once),
                            heirlooms: None,
                        });
                    }
                }
                _ => (),
            }
        } else {
            let Interaction::Hovering = interactable.current() else {
                continue;
            };
            let ui_element = default_ui;

            interactable.change(Interaction::None);
            commands.entity(e).insert(ui_element.clone());
            set_sprite_image(
                &mut commands,
                e,
                graphics.get_ui_element_texture(ui_element),
            );
            ui_helpers::apply_ui_hover_scale(&mut transform, Some(&mut bounce), false);
        }
    }
}

#[derive(Clone, PartialEq)]
pub(crate) enum BlessingIconHoverState {
    Heirloom(Heirloom),
    Skill(ActiveSkill),
    Item(WorldObject),
}

pub fn handle_blessing_choice_icon_tooltips(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    res: Res<ScreenResolution>,
    proto: ProtoParam,
    cards: Query<(&BlessingChoiceUI, &Interactable, &Transform, &Focusable)>,
    existing_heirloom_tooltips: Query<Entity, With<HeirloomDynamicTooltip>>,
    existing_skill_tooltips: Query<Entity, With<BlessingChoiceSkillTooltip>>,
    existing_item_tooltips: Query<Entity, With<ItemOrRecipeTooltip>>,
    mut tooltip_requests: MessageWriter<HeirloomTooltipRequest>,
    mut item_tooltip_events: MessageWriter<ToolTipUpdateEvent>,
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
    mut last_hovered: Local<Option<BlessingIconHoverState>>,
) {
    let card_count = cards.iter().count() as u32;
    let currently_hovered = cards
        .iter()
        .find(|(_, interactable, _, _)| matches!(interactable.current(), Interaction::Hovering))
        .and_then(|(ui, _, transform, focusable)| {
            blessing_choice_tooltip_target(&ui.choice).map(|target| {
                let card_half_width = blessing_choice_card_ui(&ui.choice).1.x * 0.5;
                (
                    ui.choice.clone(),
                    target,
                    // Use the local `Transform`, not `GlobalTransform`: these cards are
                    // unparented root sprites, but on the very frame they're spawned (and the
                    // default focus first lands on one of them) `GlobalTransform` hasn't been
                    // propagated yet, so reading it here would place the very first tooltip at
                    // a stale/zeroed position until the player un-hovers and re-hovers.
                    transform.translation,
                    focusable.index,
                    card_half_width,
                )
            })
        });

    let hover_state = currently_hovered
        .as_ref()
        .map(|(_, target, _, _, _)| match target {
            BlessingChoiceTooltipTarget::Heirloom(heirloom, _) => {
                BlessingIconHoverState::Heirloom(heirloom.clone())
            }
            BlessingChoiceTooltipTarget::Skill(skill) => {
                BlessingIconHoverState::Skill(skill.clone())
            }
            BlessingChoiceTooltipTarget::Item(item) => BlessingIconHoverState::Item(*item),
        });

    if *last_hovered == hover_state {
        return;
    }

    for tooltip_e in existing_heirloom_tooltips.iter() {
        commands.entity(tooltip_e).despawn();
    }
    for tooltip_e in existing_skill_tooltips.iter() {
        commands.entity(tooltip_e).despawn();
    }
    for tooltip_e in existing_item_tooltips.iter() {
        commands.entity(tooltip_e).despawn();
    }

    match &currently_hovered {
        None => {
            tooltip_requests.write(HeirloomTooltipRequest::Clear);
        }
        Some((
            _choice,
            BlessingChoiceTooltipTarget::Heirloom(heirloom, rarity),
            card_pos,
            card_index,
            card_half_width,
        )) => {
            const TOOLTIP_EDGE_PAD: f32 = 8.;
            let (_, card_size) = heirloom.get_ui_element(*rarity);
            let tooltip_half_width = card_size.x * 0.5;
            let desired_x = blessing_tooltip_dock_x(
                card_pos.x,
                *card_half_width,
                tooltip_half_width,
                *card_index,
                card_count,
            );
            let clamped_x = clamp_tooltip_center_x(
                desired_x,
                tooltip_half_width,
                res.game_width,
                TOOLTIP_EDGE_PAD,
            );
            let icon_y = card_pos.y + BLESSING_CARD_ICON_OFFSET.y;
            // `spawn_heirloom_tooltip_card` adds [`HEIRLOOM_TOOLTIP_CARD_Z`] on top of this z.
            let tooltip_pos = Vec3::new(clamped_x, icon_y, 0.);
            tooltip_requests.write(HeirloomTooltipRequest::Show(HeirloomTooltipShow {
                heirloom: heirloom.clone(),
                rarity: *rarity,
                position: tooltip_pos,
                scaling_text: None,
                trigger_count: 0,
                ui_state: Some(UIState::BlessingChoice),
            }));
        }
        Some((
            _,
            BlessingChoiceTooltipTarget::Skill(skill),
            card_pos,
            card_index,
            card_half_width,
        )) => {
            tooltip_requests.write(HeirloomTooltipRequest::Clear);
            // The visible skill panel background is offset +72 in x from the container and is
            // 246 wide, so clamp its *center* to the screen then back out the container x.
            const TOOLTIP_EDGE_PAD: f32 = 8.;
            const SKILL_PANEL_BG_X_OFFSET: f32 = 72.;
            const SKILL_PANEL_HALF_WIDTH: f32 = 123.;
            let desired_x = blessing_tooltip_dock_x(
                card_pos.x,
                *card_half_width,
                SKILL_PANEL_HALF_WIDTH,
                *card_index,
                card_count,
            );
            let panel_center_x = clamp_tooltip_center_x(
                desired_x,
                SKILL_PANEL_HALF_WIDTH,
                res.game_width,
                TOOLTIP_EDGE_PAD,
            );
            let icon_y = card_pos.y + BLESSING_CARD_ICON_OFFSET.y;
            let tooltip_pos = Vec3::new(
                crate::ui::snap_world_to_pixel_grid(
                    panel_center_x - SKILL_PANEL_BG_X_OFFSET,
                    res.scale,
                ),
                crate::ui::snap_world_to_pixel_grid(icon_y + 56., res.scale),
                BLESSING_CHOICE_TOOLTIP_Z,
            );
            let (container, _) = spawn_skill_tooltip_shell(
                &mut commands,
                &graphics,
                tooltip_pos,
                "BLESSING SKILL TOOLTIP",
            );
            commands
                .entity(container)
                .insert(BlessingChoiceSkillTooltip)
                .insert(UIState::BlessingChoice);

            let params =
                active_skill_tooltip_params_from_player(&skill_power, &meteor_shower_state);
            spawn_skill_tooltip_content(
                &mut commands,
                &graphics,
                &asset_server,
                skill.clone(),
                None,
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
        Some((
            choice,
            BlessingChoiceTooltipTarget::Item(item),
            card_pos,
            card_index,
            card_half_width,
        )) => {
            tooltip_requests.write(HeirloomTooltipRequest::Clear);
            let Some(minor) = choice.as_minor() else {
                return;
            };
            let item_stack = blessing_item_stack_for_tooltip(*item, minor, &proto);
            const TOOLTIP_EDGE_PAD: f32 = 8.;
            let half_w = ITEM_TOOLTIP_LARGE_CARD_SIZE.x * 0.5;
            let half_h = ITEM_TOOLTIP_LARGE_CARD_SIZE.y * 0.5;
            let desired_x = blessing_tooltip_dock_x(
                card_pos.x,
                *card_half_width,
                half_w,
                *card_index,
                card_count,
            );
            let icon_y = card_pos.y + BLESSING_CARD_ICON_OFFSET.y;
            let panel_center = Vec3::new(
                clamp_tooltip_center_x(desired_x, half_w, res.game_width, TOOLTIP_EDGE_PAD),
                clamp_tooltip_center_y(icon_y, half_h, res.game_height, TOOLTIP_EDGE_PAD),
                BLESSING_CHOICE_TOOLTIP_Z,
            );
            // Reuse the real inventory item tooltip renderer (`handle_spawn_inv_item_tooltip`),
            // placing the card unparented in world space at `panel_center`.
            let ui_tag = match choice {
                BlessingChoiceKind::Minor(_) => UIState::BlessingChoice,
                BlessingChoiceKind::Major(_) => UIState::MajorBlessingChoice,
            };
            item_tooltip_events.write(ToolTipUpdateEvent {
                item_stack,
                is_recipe: false,
                show_range: false,
                world_anchor: Some(panel_center),
                ui_state_tag: Some(ui_tag),
                ..Default::default()
            });
        }
    }

    *last_hovered = hover_state;
}

pub fn transition_after_blessing_choice(
    mut timer: ResMut<BlessingTransitionState>,
    mut next_game_state: ResMut<NextState<GameState>>,
    mut next_ui_state: ResMut<NextState<UIState>>,
    current_tier: Res<CurrentBlessingTier>,
    pending_heirloom_pick: Option<Res<PendingMajorHeirloomPick>>,
    time: Res<Time>,
    mut commands: Commands,
) {
    timer.timer.tick(time.delta());
    if timer.timer.just_finished() {
        match current_tier.0 {
            BlessingTier::Minor => {
                next_game_state.set(GameState::Main);
                next_ui_state.set(UIState::Closed);
                commands.remove_resource::<AncestorBlessingOffer>();
            }
            BlessingTier::Major => {
                // GameState stays Main. If a secondary heirloom pick is pending (Singular Focus /
                // Collector's Bargain), open it directly — bouncing through Closed races era swap
                // into Initializing and can miss the one-shot heirloom-pick setup.
                commands.remove_resource::<MajorBlessingOffer>();
                if pending_heirloom_pick.is_some() {
                    next_ui_state.set(UIState::MajorHeirloomPick);
                } else {
                    next_ui_state.set(UIState::Closed);
                }
            }
        }
        commands.remove_resource::<BlessingTransitionState>();
    }
}

pub fn transition_blessing_ui_after_choice(
    mut state: ResMut<BlessingTransitionState>,
    mut blessing_cards: Query<(Entity, &BlessingChoiceUI, &GlobalTransform)>,
    mut sprites: Query<&mut Sprite>,
    mut visibility_set: ParamSet<(
        Query<'_, '_, &mut Visibility, With<Text2d>>,
        Query<'_, '_, &mut Visibility, With<UiShadowChild>>,
    )>,
    child_hierarchy: Query<&Children>,
    graphics: Res<Graphics>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    let timer_percent = state.timer.fraction();
    for (ent, blessing_choice_ui, transform) in blessing_cards.iter_mut() {
        if blessing_choice_ui.selected {
            for (i, heirloom) in state
                .heirlooms
                .clone()
                .unwrap_or(Vec::new())
                .iter()
                .enumerate()
            {
                let floating_text = spawn_floating_text_with_shadow(
                    &mut commands,
                    &asset_server,
                    transform.translation()
                        + Vec3::new(0., BLESSING_HEIRLOOM_REVEAL_TEXT_Y - 12. * i as f32, 1.),
                    heirloom.rarity.get_color(),
                    heirloom.heirloom.get_title(),
                    FLOATING_TEXT,
                );
                commands
                    .entity(floating_text)
                    .insert(RenderLayers::from_layers(&[3]));
                let icon = commands
                    .spawn((
                        graphics.get_heirloom_icon(heirloom.heirloom.clone()),
                        Transform {
                            translation: Vec2::new(12., 0.).extend(4.),
                            scale: Vec3::new(1., 1., 1.),
                            ..Default::default()
                        },
                    ))
                    .insert(RenderLayers::from_layers(&[3]))
                    .insert(HeirloomIconOutline::new(
                        heirloom.rarity,
                        Default::default(),
                    ))
                    .insert(Name::new("HEIRLOOM ICON"))
                    .id();
                commands.entity(icon).insert(ChildOf(floating_text));

                if let Some(glow) = heirloom.rarity.get_item_glow() {
                    commands
                        .spawn((
                            Sprite {
                                image: graphics.get_item_glow(glow),
                                custom_size: Some(Vec2::new(32., 32.)),
                                ..default()
                            },
                            Transform {
                                translation: Vec2::new(0., 0.).extend(-1.),
                                scale: Vec3::new(1., 1., 1.),
                                ..Default::default()
                            },
                        ))
                        .insert(RenderLayers::from_layers(&[3]))
                        .insert(ChildOf(icon));
                }
            }
            state.heirlooms = None;
        } else {
            let alpha = (1.0 - (timer_percent * 8.)).clamp(0.0, 1.0);
            fade_blessing_card_descendants(
                ent,
                alpha,
                &child_hierarchy,
                &mut sprites,
                &mut visibility_set,
            );
        }
    }
}
