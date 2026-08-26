use crate::ui::game_fonts as gf;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::text::Justify;

use crate::{
    assets::Graphics,
    audio::{AudioSoundEffect, SoundSpawner},
    cursor::CursorPos,
    inventory::ItemStack,
    item::WorldObject,
    player::{
        achievements::Achievements,
        currency::TimeFragmentCurrency,
        unlocks::{persist_unlock_data, UnlockUpgradeKind, UnlockUpgrades, UnlockedClasses},
    },
    ui::{
        interactions::{set_sprite_image, Interaction},
        main_menu::spawn_exit_icon_button,
        spawn_item_stack_icon, ui_helpers, Focusable, Interactable, UIElement, UIState,
    },
    ScreenResolution,
};

#[derive(Component)]
pub struct UnlocksUI;

#[derive(Component)]
pub struct UnlockPurchaseButton {
    kind: UnlockUpgradeKind,
}

#[derive(Component)]
pub struct UnlockInfoText {
    kind: UnlockUpgradeKind,
}

#[derive(Component)]
pub struct UnlockCostText {
    kind: UnlockUpgradeKind,
}

#[derive(Component)]
pub struct UnlockButtonLabel;

#[derive(Component)]
pub struct UnlocksCurrencyText;

/// Vertical offset applied to title, currency, and unlock rows (back button stays put).
const UNLOCKS_CONTENT_Y_OFFSET: f32 = 62.0;
const UNLOCK_BUTTON_DISABLED_SPRITE: Color = Color::srgb(0.55, 0.55, 0.55);
const UNLOCK_BUTTON_DISABLED_LABEL: Color = Color::srgb(0.7, 0.7, 0.7);

fn unlock_purchase_button_enabled(is_maxed: bool, affordable: bool) -> bool {
    !is_maxed && affordable
}

fn unlock_purchase_button_label(is_maxed: bool) -> &'static str {
    if is_maxed {
        "MAXED"
    } else {
        "Purchase"
    }
}

fn unlock_cost_label(kind: UnlockUpgradeKind, upgrades: &UnlockUpgrades) -> String {
    if upgrades.is_maxed(kind) {
        String::new()
    } else {
        format!("Cost: {}", upgrades.next_cost(kind))
    }
}

const UNLOCK_ROWS: [UnlockUpgradeKind; 9] = [
    UnlockUpgradeKind::Reroll,
    UnlockUpgradeKind::Banish,
    UnlockUpgradeKind::StartSupplies,
    UnlockUpgradeKind::StartStatBoosts,
    UnlockUpgradeKind::StartTome,
    UnlockUpgradeKind::StartOrb,
    UnlockUpgradeKind::StartingTools,
    UnlockUpgradeKind::MapMarkers,
    UnlockUpgradeKind::ThirdSkillSlot,
];

fn unlock_effect_summary(kind: UnlockUpgradeKind, upgrades: &UnlockUpgrades) -> String {
    let tier = upgrades.tier(kind);
    match kind {
        UnlockUpgradeKind::Reroll => format!(
            "Tier {}: Rerolls per run: {}",
            tier,
            upgrades.reroll_total()
        ),
        UnlockUpgradeKind::Banish => format!(
            "Tier {}: Banishes per run: {}",
            tier,
            upgrades.banish_total()
        ),
        UnlockUpgradeKind::StartSupplies => {
            let count = upgrades.supplies_count();
            if count == 0 {
                format!("Tier {}: No bonus supplies yet", tier)
            } else {
                format!(
                    "Tier {}: Start with {} random {}.",
                    tier,
                    count,
                    if count == 1 { "supply" } else { "supplies" }
                )
            }
        }
        UnlockUpgradeKind::StartStatBoosts => {
            let count = upgrades.stat_boost_count();
            if count == 0 {
                format!("Tier {}: No stat boost foods yet", tier)
            } else {
                format!(
                    "Tier {}: Start with {} random stat boost food{}.",
                    tier,
                    count,
                    if count == 1 { "" } else { "s" }
                )
            }
        }
        UnlockUpgradeKind::StartTome => {
            let count = upgrades.tome_count();
            if count == 0 {
                format!("Tier {}: Start with no tomes", tier)
            } else {
                let plural = if count == 1 { "tome" } else { "tomes" };
                format!("Tier {}: Start with {} Upgrade {}", tier, count, plural)
            }
        }
        UnlockUpgradeKind::StartOrb => {
            let count = upgrades.orb_count();
            if count == 0 {
                format!("Tier {}: No orbs yet", tier)
            } else {
                let phrase = if count == 1 { "orb" } else { "orbs" };
                format!("Tier {}: Start with {} {}", tier, count, phrase)
            }
        }
        UnlockUpgradeKind::StartingTools => match tier {
            0 => "Tier 1: Start with Wood Axe".to_string(),
            1 => "Tier 2: Start with Wood Axe and Pickaxe".to_string(),
            2 => "Tier 3: Start with Wood Axe and Pickaxe".to_string(),
            _ => format!("Unlocked: Start with Wood Axe and Pickaxe",),
        },
        UnlockUpgradeKind::MapMarkers => format!(
            "Tier {}: Place up to {} map marker{}",
            tier,
            upgrades.map_marker_count(),
            if upgrades.map_marker_count() == 1 {
                ""
            } else {
                "s"
            }
        ),
        UnlockUpgradeKind::ThirdSkillSlot => {
            if upgrades.third_skill_slot_unlocked {
                "Unlocked: 3rd skill slot for all classes".to_string()
            } else {
                "Unlock the 3rd skill slot for all classes".to_string()
            }
        }
    }
}

pub fn handle_unlocks_clicks(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut buttons: Query<(Entity, &mut Interactable, &UnlockPurchaseButton)>,
    mut commands: Commands,
    graphics: Res<Graphics>,
    mut currency: ResMut<TimeFragmentCurrency>,
    mut upgrades: ResMut<UnlockUpgrades>,
    unlocked_classes: Res<UnlockedClasses>,
    achievements: Res<Achievements>,
    focus_input: crate::ui::focus::FocusInput,
) {
    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None, None);
    let left_mouse_released = mouse_input.just_released(MouseButton::Left);

    for (entity, mut interactable, button) in buttons.iter_mut() {
        // Skip if unlock is maxed
        if upgrades.is_maxed(button.kind) {
            continue;
        }

        let cost = upgrades.next_cost(button.kind);
        let affordable = currency.time_fragments.max(0) as u32 >= cost;
        let is_hit = matches!(hit_test, Some(hit) if hit.0 == entity);
        let is_focused = focus_input.is_focused(entity);
        if is_hit || is_focused {
            match interactable.current() {
                Interaction::None => {
                    if !affordable {
                        continue;
                    }
                    interactable.change(Interaction::Hovering);
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonHover, 0.05));
                    commands.entity(entity).insert(UIElement::BackButtonHover);
                    set_sprite_image(
                        &mut commands,
                        entity,
                        graphics.get_ui_element_texture(UIElement::BackButtonHover),
                    );
                }
                Interaction::Hovering => {
                    if (left_mouse_released && is_hit && affordable)
                        || (is_focused && focus_input.confirm_just_pressed() && affordable)
                    {
                        if currency.spend(cost) {
                            upgrades.increment(button.kind);
                            commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonClick, 0.2));

                            persist_unlock_data(
                                Some(&*currency),
                                Some(&*unlocked_classes),
                                Some(&*achievements),
                                Some(&*upgrades),
                                None,
                            );
                        }
                        interactable.change(Interaction::None);
                        commands.entity(entity).insert(UIElement::BackButton);
                        set_sprite_image(
                            &mut commands,
                            entity,
                            graphics.get_ui_element_texture(UIElement::BackButton),
                        );
                    }
                }
                _ => {}
            }
        } else {
            // reset hovering states if we stop hovering
            let Interaction::Hovering = interactable.current() else {
                continue;
            };
            interactable.change(Interaction::None);
            commands.entity(entity).insert(UIElement::BackButton);
            set_sprite_image(
                &mut commands,
                entity,
                graphics.get_ui_element_texture(UIElement::BackButton),
            );
        }
    }
}

pub fn update_unlocks_currency_text(
    currency: Res<TimeFragmentCurrency>,
    mut query: Query<&mut Text2d, With<UnlocksCurrencyText>>,
) {
    if !currency.is_changed() {
        return;
    }

    for mut text in query.iter_mut() {
        text.0 = format!("{}", currency.time_fragments.max(0));
    }
}

pub fn refresh_unlock_button_states(
    mut commands: Commands,
    currency: Res<TimeFragmentCurrency>,
    upgrades: Res<UnlockUpgrades>,
    mut buttons: Query<(
        Entity,
        &UnlockPurchaseButton,
        &mut Sprite,
        &Children,
        Option<&Interactable>,
    )>,
    mut text_queries: ParamSet<(
        Query<(&mut Text2d, &mut TextColor), With<UnlockButtonLabel>>,
        Query<(&UnlockCostText, &mut Text2d)>,
        Query<(&UnlockInfoText, &mut Text2d)>,
    )>,
) {
    if !currency.is_changed() && !upgrades.is_changed() {
        return;
    }

    for (entity, button, mut sprite, children, interactable) in buttons.iter_mut() {
        let is_maxed = upgrades.is_maxed(button.kind);
        let cost = upgrades.next_cost(button.kind);
        let affordable = currency.time_fragments.max(0) as u32 >= cost;
        let enabled = unlock_purchase_button_enabled(is_maxed, affordable);

        sprite.color = if enabled {
            Color::WHITE
        } else {
            UNLOCK_BUTTON_DISABLED_SPRITE
        };

        if enabled && interactable.is_none() {
            commands.entity(entity).insert(Interactable::default());
        } else if !enabled && interactable.is_some() {
            commands.entity(entity).remove::<Interactable>();
        }

        {
            let mut labels = text_queries.p0();
            for child in children.iter() {
                if let Ok((mut text, mut text_color)) = labels.get_mut(child) {
                    text.0 = unlock_purchase_button_label(is_maxed).to_string();
                    text_color.0 = if enabled {
                        crate::colors::WHITE
                    } else {
                        UNLOCK_BUTTON_DISABLED_LABEL
                    };
                }
            }
        }
    }

    {
        let mut cost_texts = text_queries.p1();
        for (cost, mut text) in cost_texts.iter_mut() {
            text.0 = unlock_cost_label(cost.kind, &upgrades);
        }
    }

    {
        let mut info_texts = text_queries.p2();
        for (info, mut text) in info_texts.iter_mut() {
            text.0 = unlock_effect_summary(info.kind, &upgrades);
        }
    }
}

pub fn cleanup_unlocks_ui(mut commands: Commands, query: Query<Entity, With<UnlocksUI>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}

pub fn setup_unlocks_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    resolution: Res<ScreenResolution>,
    currency: Res<TimeFragmentCurrency>,
    upgrades: Res<UnlockUpgrades>,
) {
    let overlay = ui_helpers::spawn_full_screen_ui_overlay(&mut commands, &resolution, 1., 10.);
    commands
        .entity(overlay)
        .insert(UnlocksUI)
        .insert(UIState::Unlocks);

    // Title
    commands.spawn((
        gf::DISPLAY_LARGE
            .text(&asset_server, "Unlocks", crate::colors::WHITE)
            .justify(Justify::Center)
            .anchor(bevy::sprite::Anchor::CENTER)
            .with_transform(Transform {
                translation: Vec3::new(0., 104. + UNLOCKS_CONTENT_Y_OFFSET, 11.),
                scale: gf::DISPLAY_LARGE.transform_scale(),
                ..Default::default()
            }),
        RenderLayers::from_layers(&[3]),
        UnlocksUI,
        Name::new("Unlocks Title"),
    ));

    // Currency text
    let currency_text = commands
        .spawn((
            gf::DISPLAY
                .text(
                    &asset_server,
                    format!("{}", currency.time_fragments.max(0)),
                    crate::colors::WHITE,
                )
                .justify(Justify::Left)
                .anchor(bevy::sprite::Anchor::CENTER_LEFT)
                .with_transform(Transform {
                    translation: Vec3::new(-160., 104.5 + UNLOCKS_CONTENT_Y_OFFSET, 11.),
                    scale: gf::DISPLAY.transform_scale(),
                    ..Default::default()
                }),
            RenderLayers::from_layers(&[3]),
            UnlocksUI,
            UnlocksCurrencyText,
            Name::new("Unlocks Currency Text2d"),
        ))
        .id();
    let currency_stack = spawn_item_stack_icon(
        &mut commands,
        &graphics,
        &ItemStack::crate_icon_stack(WorldObject::TimeFragment),
        &asset_server,
        Vec2::new(-9., 1.5),
        Vec2::new(0., 0.),
        3,
    );
    commands
        .entity(currency_stack)
        .insert(ChildOf(currency_text));

    let start_y = 54.5 + UNLOCKS_CONTENT_Y_OFFSET;
    let row_spacing = -39.0;

    for (index, kind) in UNLOCK_ROWS
        .iter()
        .filter(|&&k| !k.is_disabled())
        .enumerate()
    {
        let y = start_y + row_spacing * index as f32;
        spawn_unlock_row(
            &mut commands,
            &graphics,
            &asset_server,
            *kind,
            Vec3::new(-140., y, 11.),
            Vec3::new(110.5, y - 6.5, 11.),
            upgrades.as_ref(),
            currency.as_ref(),
            index as u32,
        );
    }

    // Exit button
    let exit_button = spawn_exit_icon_button(Vec3::new(0., -158., 11.), &mut commands, &graphics);
    commands
        .entity(exit_button)
        .insert(UnlocksUI)
        .insert(Focusable {
            group: UIState::Unlocks,
            index: 100,
        });
}

fn spawn_unlock_row(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    kind: UnlockUpgradeKind,
    info_pos: Vec3,
    button_pos: Vec3,
    upgrades: &UnlockUpgrades,
    currency: &TimeFragmentCurrency,
    focus_index: u32,
) {
    let title_name = format!("Unlock Row Title {}", kind.display_name());
    commands.spawn((
        gf::DISPLAY
            .text(&asset_server, kind.display_name(), crate::colors::WHITE)
            .justify(Justify::Left)
            .anchor(bevy::sprite::Anchor::CENTER_LEFT)
            .with_transform(Transform {
                translation: info_pos,
                scale: gf::DISPLAY.transform_scale(),
                ..Default::default()
            }),
        RenderLayers::from_layers(&[3]),
        UnlocksUI,
        UIState::Unlocks,
        Name::new(title_name),
    ));

    let info_text_pos = Vec3::new(info_pos.x, info_pos.y - 13., info_pos.z);
    commands.spawn((
        gf::BODY
            .text(
                &asset_server,
                unlock_effect_summary(kind, upgrades),
                crate::colors::YELLOW_2,
            )
            .justify(Justify::Left)
            .anchor(bevy::sprite::Anchor::CENTER_LEFT)
            .with_transform(Transform {
                translation: info_text_pos,
                scale: gf::BODY.transform_scale(),
                ..Default::default()
            }),
        RenderLayers::from_layers(&[3]),
        UnlocksUI,
        UIState::Unlocks,
        UnlockInfoText { kind },
        Name::new(format!("Unlock Row Info {}", kind.display_name())),
    ));

    let cost_pos = Vec3::new(35., info_pos.y, info_pos.z);
    commands.spawn((
        gf::BODY
            .text(
                &asset_server,
                unlock_cost_label(kind, upgrades),
                crate::colors::WHITE,
            )
            .justify(Justify::Left)
            .anchor(bevy::sprite::Anchor::CENTER_LEFT)
            .with_transform(Transform {
                translation: cost_pos,
                scale: gf::BODY.transform_scale(),
                ..Default::default()
            }),
        RenderLayers::from_layers(&[3]),
        UnlocksUI,
        UIState::Unlocks,
        UnlockCostText { kind },
        Name::new(format!("Unlock Row Cost {}", kind.display_name())),
    ));

    // Purchase button — grayed out and non-interactive when maxed or unaffordable.
    let is_maxed = upgrades.is_maxed(kind);
    let cost = upgrades.next_cost(kind);
    let affordable = currency.time_fragments.max(0) as u32 >= cost;
    let enabled = unlock_purchase_button_enabled(is_maxed, affordable);
    let button_color = if enabled {
        Color::WHITE
    } else {
        UNLOCK_BUTTON_DISABLED_SPRITE
    };
    let label_color = if enabled {
        crate::colors::WHITE
    } else {
        UNLOCK_BUTTON_DISABLED_LABEL
    };
    let mut button_cmd = commands.spawn((
        Sprite {
            image: graphics
                .get_ui_element_texture(UIElement::BackButton)
                .clone(),
            custom_size: Some(Vec2::new(53., 18.)),
            color: button_color,
            ..Default::default()
        },
        Transform::from_translation(button_pos),
    ));
    button_cmd
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIState::Unlocks)
        .insert(UIElement::BackButton)
        .insert(UnlocksUI)
        .insert(UnlockPurchaseButton { kind })
        .insert(Name::new(format!(
            "Unlock Purchase Button {}",
            kind.display_name()
        )));

    if enabled {
        button_cmd
            .insert(Interactable::default())
            .insert(Focusable {
                group: UIState::Unlocks,
                index: focus_index,
            });
    }

    let button_entity = button_cmd.id();

    commands
        .spawn((
            gf::BODY
                .text(
                    &asset_server,
                    unlock_purchase_button_label(is_maxed),
                    label_color,
                )
                .justify(Justify::Center)
                .anchor(Anchor::CENTER)
                .with_transform(Transform {
                    translation: Vec3::new(0.5, 0.5, 1.),
                    scale: gf::BODY.transform_scale(),
                    ..Default::default()
                }),
            RenderLayers::from_layers(&[3]),
            UIState::Unlocks,
            UnlockButtonLabel,
            Name::new(format!("Unlock Purchase Label {}", kind.display_name())),
        ))
        .insert(ChildOf(button_entity));
}
