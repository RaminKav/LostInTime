use bevy::{prelude::*, render::view::RenderLayers, sprite::Anchor};

use crate::{
    assets::Graphics,
    attributes::{
        attribute_helpers::skill_power_multiplier, AttackSpeed, BonusAttackSpeed, CritChance,
        MaxHealth, MaxMana, ProjectileSize, SkillPower, Speed,
    },
    audio::{AudioSoundEffect, SoundSpawner},
    blessings::{
        build_ancestor_blessing_offer, Ancestor, AncestorBlessing, AncestorBlessingIcon,
        AncestorBlessingOffer, OwnedBlessings, PendingRunStartBlessing, ResolvedAncestorBlessing,
    },
    colors::{LIGHT_RED, SHIELD_BLUE, WHITE, YELLOW_2},
    cursor::CursorPos,
    inventory::ItemStack,
    item::WorldObject,
    player::{
        class_rank::ClassRankSystem,
        levels::PlayerLevel,
        skills::{
            active_skill_scaling::METEOR_SHOWER_BASE_COUNT,
            effective_player_attack_speed_multiplier, ActiveSkill, Heirloom, HeirloomChoiceQueue,
            HeirloomRarity, HeirloomWithRarity, PlayerClass, PlayerSkills,
        },
        Player,
    },
    proto::proto_param::ProtoParam,
    ui::{
        clamp_tooltip_center_x, clamp_tooltip_center_y,
        damage_numbers::spawn_floating_text_with_shadow,
        game_fonts::{self as gf, FLOATING_TEXT, HEIRLOOM_CARD_DESC_LINE_STEP},
        spawn_skill_tooltip_content,
        ui_helpers::{self, spawn_full_screen_ui_overlay},
        HeirloomDynamicTooltip, HeirloomTooltipRequest, HeirloomTooltipShow, Interactable,
        Interaction, ItemOrRecipeTooltip, ToolTipUpdateEvent, UIElement, UIState,
        ITEM_TOOLTIP_LARGE_CARD_SIZE, SKILLS_CHOICE_UI_SIZE,
    },
    GameState, ScreenResolution,
};

#[derive(Component, Clone)]
pub struct BlessingChoiceUI {
    pub selected: bool,
    pub ancestor: Ancestor,
    pub choice: ResolvedAncestorBlessing,
}

pub struct AncestorBlessingSelectEvent {
    pub ancestor: Ancestor,
    pub choice: ResolvedAncestorBlessing,
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

fn blessing_choice_tooltip_target(
    choice: &ResolvedAncestorBlessing,
) -> Option<BlessingChoiceTooltipTarget> {
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
    class_ranks: Option<&ClassRankSystem>,
    player_class: Option<&PlayerClass>,
) -> ItemStack {
    let mut stack = proto
        .get_item_data(item)
        .cloned()
        .unwrap_or_else(|| ItemStack::crate_icon_stack(item));
    stack.count = 1;

    if choice.blessing == AncestorBlessing::UpgradeStartingWeapon && item.is_weapon() {
        if let (Some(ranks), Some(pc)) = (class_ranks, player_class) {
            stack.rarity = ranks
                .get_starting_weapon_rarity(&pc.class)
                .get_next_rarity();
        }
    }

    stack
}

fn fade_blessing_card_descendants(
    entity: Entity,
    alpha: f32,
    children: &Query<&Children>,
    sprites: &mut Query<&mut Sprite>,
    atlas_sprites: &mut Query<&mut TextureAtlasSprite>,
    texts: &mut Query<&mut Visibility, With<Text>>,
) {
    if let Ok(mut sprite) = sprites.get_mut(entity) {
        sprite.color.set_a(alpha);
    }
    if let Ok(mut atlas) = atlas_sprites.get_mut(entity) {
        atlas.color.set_a(alpha);
    }
    if let Ok(mut visibility) = texts.get_mut(entity) {
        *visibility = Visibility::Hidden;
    }
    if let Ok(kids) = children.get(entity) {
        for child in kids.iter() {
            fade_blessing_card_descendants(*child, alpha, children, sprites, atlas_sprites, texts);
        }
    }
}

fn blessing_choice_card_ui(choice: &ResolvedAncestorBlessing) -> (UIElement, Vec2) {
    if let Some(rarity) = choice.blessing.display_card_rarity() {
        return Heirloom::None.get_ui_element(rarity);
    }
    if let Some(heirloom) = choice.resolved_heirloom.as_ref() {
        return heirloom.heirloom.get_ui_element(heirloom.rarity);
    }
    (UIElement::SkillChoice, SKILLS_CHOICE_UI_SIZE)
}

fn blessing_choice_card_hover_ui(choice: &ResolvedAncestorBlessing) -> (UIElement, UIElement) {
    if let Some(rarity) = choice.blessing.display_card_rarity() {
        return (
            Heirloom::None.get_ui_element(rarity).0,
            Heirloom::None.get_ui_element_hover(rarity),
        );
    }
    if let Some(heirloom) = choice.resolved_heirloom.as_ref() {
        return (
            heirloom.heirloom.get_ui_element(heirloom.rarity).0,
            heirloom.heirloom.get_ui_element_hover(heirloom.rarity),
        );
    }
    (UIElement::SkillChoice, UIElement::SkillChoiceHover)
}

fn spawn_ancestor_blessing_card(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    choice: &ResolvedAncestorBlessing,
    position: Vec3,
) -> Entity {
    let (ui_element, size) = blessing_choice_card_ui(choice);

    let card_e = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(ui_element.clone()),
            sprite: Sprite {
                custom_size: Some(size),
                ..Default::default()
            },
            transform: Transform {
                translation: position,
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(ui_element)
        .insert(Name::new("BLESSING CHOICE UI"))
        .insert(RenderLayers::from_layers(&[3]))
        .id();

    if let Some(icon) = &choice.display_icon {
        spawn_blessing_card_icon(commands, graphics, asset_server, card_e, icon);
    }

    let mut text_title = commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                choice.title.clone(),
                gf::HEIRLOOM_CARD_TITLE.text_style(asset_server, WHITE),
            ),
            text_anchor: Anchor::Center,
            transform: Transform {
                translation: Vec3::new(0., 24. + BLESSING_CARD_TITLE_Y_OFFSET, 1.),
                ..Default::default()
            },
            ..default()
        },
        Name::new("Blessing Title"),
        RenderLayers::from_layers(&[3]),
    ));
    text_title.set_parent(card_e);

    let desc_lines: Vec<&str> = choice.description.iter().map(String::as_str).collect();
    let has_chaos = choice.blessing.max_hp_penalty_pct() > 0.0;
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

    for (line_index, desc) in desc_lines.iter().enumerate() {
        let y = block_line_ys[line_index] + block_center_offset;
        let mut text_desc = commands.spawn((
            Text2dBundle {
                text: Text::from_section(
                    *desc,
                    gf::HEIRLOOM_CARD_BODY.text_style(asset_server, YELLOW_2),
                ),
                text_anchor: Anchor::Center,
                transform: Transform {
                    translation: Vec3::new(2., y, 1.),
                    ..Default::default()
                },
                ..default()
            },
            Name::new("Blessing Desc"),
            RenderLayers::from_layers(&[3]),
        ));
        text_desc.set_parent(card_e);
    }

    if has_chaos {
        let chaos_lines = [
            format!(
                "-{}% Max HP",
                (choice.blessing.max_hp_penalty_pct() * 100.0) as i32
            ),
            format!("+{} Chaos", choice.blessing.starting_chaos() as i32),
        ];

        for (line_index, line) in chaos_lines.iter().enumerate() {
            let y = block_line_ys[desc_lines.len() + line_index] + block_center_offset;
            let mut text_chaos = commands.spawn((
                Text2dBundle {
                    text: Text::from_section(
                        line.clone(),
                        gf::HEIRLOOM_CARD_BODY.text_style(asset_server, LIGHT_RED),
                    ),
                    text_anchor: Anchor::Center,
                    transform: Transform {
                        translation: Vec3::new(2., y, 1.),
                        ..Default::default()
                    },
                    ..default()
                },
                Name::new("Blessing Chaos Desc"),
                RenderLayers::from_layers(&[3]),
            ));
            text_chaos.set_parent(card_e);
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
                    Text2dBundle {
                        text: Text::from_section(
                            "?",
                            gf::GLOBAL_MESSAGE.text_style(asset_server, SHIELD_BLUE),
                        ),
                        text_anchor: Anchor::Center,
                        transform: Transform {
                            translation: Vec3::new(2., 52., 4.),
                            ..Default::default()
                        },
                        ..default()
                    },
                    RenderLayers::from_layers(&[3]),
                    Name::new("BLESSING MYSTERY ICON"),
                ))
                .set_parent(card_e);
        }
        AncestorBlessingIcon::Heirloom(heirloom, rarity) => {
            let skill_icon = commands
                .spawn((
                    SpriteSheetBundle {
                        sprite: graphics.get_heirloom_icon(heirloom.clone()),
                        texture_atlas: graphics.texture_atlas.as_ref().unwrap().clone(),
                        transform: Transform {
                            translation: Vec2::new(2., 52.).extend(4.),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    Sprite {
                        custom_size: Some(Vec2::new(32., 32.)),
                        ..Default::default()
                    },
                    RenderLayers::from_layers(&[3]),
                    Name::new("BLESSING HEIRLOOM ICON"),
                ))
                .id();
            commands.entity(skill_icon).set_parent(card_e);

            if let Some(glow) = rarity.get_item_glow() {
                commands
                    .spawn(SpriteBundle {
                        texture: graphics.get_item_glow(glow),
                        sprite: Sprite {
                            custom_size: Some(Vec2::new(32., 32.)),
                            ..Default::default()
                        },
                        transform: Transform {
                            translation: Vec2::new(0., 0.).extend(-1.),
                            ..Default::default()
                        },
                        ..Default::default()
                    })
                    .insert(RenderLayers::from_layers(&[3]))
                    .set_parent(skill_icon);
            }
        }
        AncestorBlessingIcon::Skill(active_skill) => {
            commands
                .spawn((
                    SpriteBundle {
                        texture: graphics.get_active_skill_icon(active_skill.clone()),
                        sprite: Sprite {
                            custom_size: Some(Vec2::new(32., 32.)),
                            ..Default::default()
                        },
                        transform: Transform {
                            translation: Vec3::new(2., 52., 4.),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    RenderLayers::from_layers(&[3]),
                    Name::new("BLESSING SKILL ICON"),
                ))
                .set_parent(card_e);
        }
        AncestorBlessingIcon::Item(item) => {
            let sprite = graphics
                .icons
                .as_ref()
                .and_then(|icons| icons.get(item).cloned())
                .or_else(|| {
                    graphics
                        .spritesheet_map
                        .as_ref()
                        .and_then(|map| map.get(item).cloned())
                });
            if let Some(sprite) = sprite {
                commands
                    .spawn(SpriteSheetBundle {
                        sprite,
                        texture_atlas: graphics.texture_atlas.as_ref().unwrap().clone(),
                        transform: Transform {
                            translation: Vec2::new(2., 52.).extend(4.),
                            ..Default::default()
                        },
                        ..Default::default()
                    })
                    .insert(RenderLayers::from_layers(&[3]))
                    .insert(Name::new("BLESSING ITEM ICON"))
                    .set_parent(card_e);
            }
        }
    }
}

pub fn setup_blessing_choice_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    res: Res<ScreenResolution>,
    heirloom_queue: Res<HeirloomChoiceQueue>,
    player_skills: Query<&PlayerSkills>,
    player_level: Query<&PlayerLevel>,
    player_class: Option<Res<PlayerClass>>,
) {
    let player_level = player_level.single().level;
    let player_skills = player_skills.single();
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

    let t_offset = Vec2::new(4., 4.);

    let _title_text = commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    offer.ancestor.title().to_string(),
                    gf::GLOBAL_MESSAGE.text_style(&asset_server, offer.ancestor.title_color()),
                ),
                transform: Transform {
                    translation: Vec3::new(0., 144., 20.),
                    ..Default::default()
                },
                ..default()
            },
            RenderLayers::from_layers(&[3]),
            UIState::BlessingChoice,
        ))
        .id();

    let _subtitle = commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    "Back again...? You hear the voice of your distant ancestor...".to_string(),
                    TextStyle {
                        font: asset_server.load("fonts/slkscr.ttf"),
                        font_size: 8.4,
                        color: WHITE,
                    },
                ),
                transform: Transform {
                    translation: Vec3::new(0., 110., 20.),
                    ..Default::default()
                },
                ..default()
            },
            RenderLayers::from_layers(&[3]),
            UIState::BlessingChoice,
        ))
        .id();
    let _subtitle2 = commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    "\n\nChoose a blessing".to_string(),
                    TextStyle {
                        font: asset_server.load("fonts/slkscrbold.ttf"),
                        font_size: 8.4,
                        color: WHITE,
                    },
                ),
                transform: Transform {
                    translation: Vec3::new(0., 90., 20.),
                    ..Default::default()
                },
                ..default()
            },
            RenderLayers::from_layers(&[3]),
            UIState::BlessingChoice,
        ))
        .id();

    spawn_full_screen_ui_overlay(&mut commands, &res, 1., 9.);

    let count = offer.choices.len();
    for i in -1i32..(offer.choices.len() as i32 - 1) {
        let choice = offer.choices[(i + 1) as usize].clone();
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
        let card_e = spawn_ancestor_blessing_card(
            &mut commands,
            &graphics,
            &asset_server,
            &choice,
            position,
        );
        commands
            .entity(card_e)
            .insert(BlessingChoiceUI {
                selected: false,
                ancestor: offer.ancestor,
                choice: choice.clone(),
            })
            .insert(UIState::BlessingChoice)
            .insert(Interactable::default());
    }
}

pub fn handle_blessing_choice_card_interactions(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut blessing_choices: Query<(Entity, &mut Interactable, &mut BlessingChoiceUI)>,
    mut commands: Commands,
    graphics: Res<Graphics>,
    mut blessing_event: EventWriter<AncestorBlessingSelectEvent>,
) {
    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);

    for (e, mut interactable, mut state) in blessing_choices.iter_mut() {
        let (default_ui, hover_ui) = blessing_choice_card_hover_ui(&state.choice);

        match hit_test {
            Some(hit_ent) if hit_ent.0 == e => match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::UISkillHover, 0.2));

                    commands
                        .entity(e)
                        .insert(hover_ui.clone())
                        .insert(graphics.get_ui_element_texture(hover_ui));
                }
                Interaction::Hovering => {
                    if left_mouse_pressed {
                        info!(
                            "CLICKED ANCESTOR BLESSING: {:?} from {:?}",
                            state.choice.blessing, state.ancestor
                        );
                        state.selected = true;
                        blessing_event.send(AncestorBlessingSelectEvent {
                            ancestor: state.ancestor,
                            choice: state.choice.clone(),
                        });

                        let delay_sec = if state.choice.blessing.needs_heirloom_reveal_delay() {
                            2.
                        } else {
                            0.75
                        };
                        commands.insert_resource(BlessingTransitionState {
                            timer: Timer::from_seconds(delay_sec, TimerMode::Once),
                            heirlooms: None,
                        });
                        commands.remove_resource::<PendingRunStartBlessing>();
                    }
                }
                _ => (),
            },
            _ => {
                let Interaction::Hovering = interactable.current() else {
                    continue;
                };
                let ui_element = default_ui;

                interactable.change(Interaction::None);
                commands
                    .entity(e)
                    .insert(ui_element.clone())
                    .insert(graphics.get_ui_element_texture(ui_element));
            }
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
    cards: Query<(&BlessingChoiceUI, &Interactable, &GlobalTransform)>,
    existing_heirloom_tooltips: Query<Entity, With<HeirloomDynamicTooltip>>,
    existing_skill_tooltips: Query<Entity, With<BlessingChoiceSkillTooltip>>,
    existing_item_tooltips: Query<Entity, With<ItemOrRecipeTooltip>>,
    mut tooltip_requests: EventWriter<HeirloomTooltipRequest>,
    mut item_tooltip_events: EventWriter<ToolTipUpdateEvent>,
    player_class: Option<Res<PlayerClass>>,
    class_ranks: Option<Res<ClassRankSystem>>,
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
    let currently_hovered = cards
        .iter()
        .find(|(_, interactable, _)| matches!(interactable.current(), Interaction::Hovering))
        .and_then(|(ui, _, transform)| {
            blessing_choice_tooltip_target(&ui.choice).map(|target| {
                (
                    ui.choice.clone(),
                    target,
                    transform.translation() + BLESSING_CARD_ICON_OFFSET,
                )
            })
        });

    let hover_state = currently_hovered
        .as_ref()
        .map(|(_, target, _)| match target {
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
        commands.entity(tooltip_e).despawn_recursive();
    }
    for tooltip_e in existing_skill_tooltips.iter() {
        commands.entity(tooltip_e).despawn_recursive();
    }
    for tooltip_e in existing_item_tooltips.iter() {
        commands.entity(tooltip_e).despawn_recursive();
    }

    match &currently_hovered {
        None => {
            tooltip_requests.send(HeirloomTooltipRequest::Clear);
        }
        Some((_choice, BlessingChoiceTooltipTarget::Heirloom(heirloom, rarity), icon_pos)) => {
            const TOOLTIP_EDGE_PAD: f32 = 8.;
            let (_, card_size) = heirloom.get_ui_element(*rarity);
            let clamped_x = clamp_tooltip_center_x(
                icon_pos.x + 90.,
                card_size.x * 0.5,
                res.game_width,
                TOOLTIP_EDGE_PAD,
            );
            let tooltip_pos = Vec3::new(clamped_x, icon_pos.y, icon_pos.z + 10.);
            tooltip_requests.send(HeirloomTooltipRequest::Show(HeirloomTooltipShow {
                heirloom: heirloom.clone(),
                rarity: *rarity,
                position: tooltip_pos,
                scaling_text: None,
                trigger_count: 0,
                ui_state: Some(UIState::BlessingChoice),
            }));
        }
        Some((_, BlessingChoiceTooltipTarget::Skill(skill), icon_pos)) => {
            tooltip_requests.send(HeirloomTooltipRequest::Clear);
            // The visible skill panel background is offset +72 in x from the container and is
            // 246 wide, so clamp its *center* to the screen then back out the container x.
            const TOOLTIP_EDGE_PAD: f32 = 8.;
            const SKILL_PANEL_BG_X_OFFSET: f32 = 72.;
            const SKILL_PANEL_HALF_WIDTH: f32 = 123.;
            let panel_center_x = clamp_tooltip_center_x(
                icon_pos.x - 30. + SKILL_PANEL_BG_X_OFFSET,
                SKILL_PANEL_HALF_WIDTH,
                res.game_width,
                TOOLTIP_EDGE_PAD,
            );
            let tooltip_pos = Vec3::new(
                crate::ui::snap_world_to_pixel_grid(
                    panel_center_x - SKILL_PANEL_BG_X_OFFSET,
                    res.scale,
                ),
                crate::ui::snap_world_to_pixel_grid(icon_pos.y + 56., res.scale),
                icon_pos.z + 10.,
            );
            let container = commands
                .spawn(RenderLayers::from_layers(&[3]))
                .insert(BlessingChoiceSkillTooltip)
                .insert(UIState::BlessingChoice)
                .insert(SpatialBundle::from_transform(Transform {
                    translation: tooltip_pos,
                    ..Default::default()
                }))
                .id();

            let _tooltip_bg = commands
                .spawn(SpriteBundle {
                    texture: graphics.get_ui_element_texture(UIElement::SkillTooltip),
                    sprite: Sprite {
                        custom_size: Some(Vec2::new(246., 71.)),
                        ..Default::default()
                    },
                    transform: Transform::from_translation(Vec3::new(72., -3., 1.)),
                    ..Default::default()
                })
                .insert(RenderLayers::from_layers(&[3]))
                .insert(Name::new("BLESSING SKILL TOOLTIP"))
                .set_parent(container)
                .id();

            let (skill_power_val, max_mana, max_health, bonus_as, crit, spd, size) = skill_power
                .get_single()
                .map(|(sp, b, mm, mh, bas, as_, c, s, sz)| {
                    (
                        skill_power_multiplier(sp, b.get_skill_power_bonus()),
                        mm.0,
                        mh.0,
                        effective_player_attack_speed_multiplier(
                            as_.map(|a| a.0).unwrap_or(0),
                            bas.map(|b| b.get_multiplier()).unwrap_or(1.0),
                        ),
                        c.0,
                        s.0,
                        sz.0,
                    )
                })
                .unwrap_or((1., 100, 100, 1.0, 10, 0, 0));

            spawn_skill_tooltip_content(
                &mut commands,
                &graphics,
                &asset_server,
                skill.clone(),
                None,
                container,
                skill_power_val,
                max_mana,
                max_health,
                bonus_as,
                crit,
                spd,
                size,
                meteor_shower_state
                    .get_single()
                    .map(|s| s.meteor_count)
                    .unwrap_or(METEOR_SHOWER_BASE_COUNT),
            );
        }
        Some((choice, BlessingChoiceTooltipTarget::Item(item), icon_pos)) => {
            tooltip_requests.send(HeirloomTooltipRequest::Clear);
            let item_stack = blessing_item_stack_for_tooltip(
                *item,
                choice,
                &proto,
                class_ranks.as_deref(),
                player_class.as_deref(),
            );
            const TOOLTIP_EDGE_PAD: f32 = 8.;
            let half_w = ITEM_TOOLTIP_LARGE_CARD_SIZE.x * 0.5;
            let half_h = ITEM_TOOLTIP_LARGE_CARD_SIZE.y * 0.5;
            let panel_center = Vec3::new(
                clamp_tooltip_center_x(icon_pos.x + 90., half_w, res.game_width, TOOLTIP_EDGE_PAD),
                clamp_tooltip_center_y(icon_pos.y, half_h, res.game_height, TOOLTIP_EDGE_PAD),
                icon_pos.z + 10.,
            );
            // Reuse the real inventory item tooltip renderer (`handle_spawn_inv_item_tooltip`),
            // placing the card unparented in world space at `panel_center`.
            item_tooltip_events.send(ToolTipUpdateEvent {
                item_stack,
                is_recipe: false,
                show_range: false,
                world_anchor: Some(panel_center),
                ui_state_tag: Some(UIState::BlessingChoice),
                ..Default::default()
            });
        }
    }

    *last_hovered = hover_state;
}

pub fn transition_to_main_after_blessing(
    mut timer: ResMut<BlessingTransitionState>,
    mut next_game_state: ResMut<NextState<GameState>>,
    mut next_ui_state: ResMut<NextState<UIState>>,
    time: Res<Time>,
    mut commands: Commands,
) {
    timer.timer.tick(time.delta());
    if timer.timer.just_finished() {
        next_game_state.set(GameState::Main);
        next_ui_state.set(UIState::Closed);
        commands.remove_resource::<BlessingTransitionState>();
        commands.remove_resource::<AncestorBlessingOffer>();
    }
}

pub fn transition_blessing_ui_after_choice(
    mut state: ResMut<BlessingTransitionState>,
    mut blessing_cards: Query<(Entity, &BlessingChoiceUI, &GlobalTransform)>,
    mut sprites: Query<&mut Sprite>,
    mut atlas_sprites: Query<&mut TextureAtlasSprite>,
    mut texts: Query<&mut Visibility, With<Text>>,
    child_hierarchy: Query<&Children>,
    graphics: Res<Graphics>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    let timer_percent = state.timer.percent();
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
                    .spawn(SpriteSheetBundle {
                        sprite: graphics.get_heirloom_icon(heirloom.heirloom.clone()),
                        texture_atlas: graphics.texture_atlas.as_ref().unwrap().clone(),
                        transform: Transform {
                            translation: Vec2::new(12., 0.).extend(4.),
                            scale: Vec3::new(1., 1., 1.),
                            ..Default::default()
                        },
                        ..Default::default()
                    })
                    .insert(RenderLayers::from_layers(&[3]))
                    .insert(Name::new("HEIRLOOM ICON"))
                    .id();
                commands.entity(icon).set_parent(floating_text);

                if let Some(glow) = heirloom.rarity.get_item_glow() {
                    commands
                        .spawn(SpriteBundle {
                            texture: graphics.get_item_glow(glow),
                            sprite: Sprite {
                                custom_size: Some(Vec2::new(32., 32.)),
                                ..Default::default()
                            },
                            transform: Transform {
                                translation: Vec2::new(0., 0.).extend(-1.),
                                scale: Vec3::new(1., 1., 1.),
                                ..Default::default()
                            },
                            ..Default::default()
                        })
                        .insert(RenderLayers::from_layers(&[3]))
                        .set_parent(icon);
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
                &mut atlas_sprites,
                &mut texts,
            );
        }
    }
}
