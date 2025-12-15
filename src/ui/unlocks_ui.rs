use bevy::prelude::*;
use bevy::render::view::RenderLayers;
use bevy::sprite::Anchor;

use crate::{
    assets::Graphics,
    audio::{AudioSoundEffect, SoundSpawner},
    inputs::CursorPos,
    inventory::ItemStack,
    item::WorldObject,
    player::{
        achievements::Achievements,
        currency::TimeFragmentCurrency,
        unlocks::{persist_unlock_data, UnlockUpgradeKind, UnlockUpgrades, UnlockedClasses},
    },
    ui::{
        interactions::Interaction, spawn_back_button, spawn_item_stack_icon, ui_helpers,
        Interactable, UIElement, UIState,
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

const UNLOCK_ROWS: [UnlockUpgradeKind; 7] = [
    UnlockUpgradeKind::Reroll,
    UnlockUpgradeKind::Banish,
    UnlockUpgradeKind::StartFood,
    UnlockUpgradeKind::StartTome,
    UnlockUpgradeKind::StartOrb,
    UnlockUpgradeKind::StartingTools,
    UnlockUpgradeKind::ThirdActiveSkillSlot,
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
        UnlockUpgradeKind::StartFood => {
            let count = upgrades.food_count();
            if count == 0 {
                format!("Tier {}: No bonus food yet", tier)
            } else {
                format!("Tier {}: Start with {} random food items.", tier, count,)
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
        UnlockUpgradeKind::ThirdActiveSkillSlot => {
            if upgrades.third_active_skill_slot_unlocked {
                "Unlocked: Gain a third active skill slot.".to_string()
            } else {
                "Gain a third active skill slot.".to_string()
            }
        }
    }
}

pub fn handle_unlocks_clicks(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut buttons: Query<(Entity, &mut Interactable, &UnlockPurchaseButton)>,
    mut commands: Commands,
    graphics: Res<Graphics>,
    mut currency: ResMut<TimeFragmentCurrency>,
    mut upgrades: ResMut<UnlockUpgrades>,
    unlocked_classes: Res<UnlockedClasses>,
    achievements: Res<Achievements>,
) {
    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    let left_mouse_released = mouse_input.just_released(MouseButton::Left);

    for (entity, mut interactable, button) in buttons.iter_mut() {
        // Skip if unlock is maxed
        if upgrades.is_maxed(button.kind) {
            continue;
        }

        let cost = upgrades.next_cost(button.kind);
        let affordable = currency.time_fragments.max(0) as u32 >= cost;
        match hit_test {
            Some(hit) if hit.0 == entity => match interactable.current() {
                Interaction::None => {
                    if !affordable {
                        continue;
                    }
                    interactable.change(Interaction::Hovering);
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonHover, 0.05));
                    commands
                        .entity(entity)
                        .insert(UIElement::BackButtonHover)
                        .insert(graphics.get_ui_element_texture(UIElement::BackButtonHover));
                }
                Interaction::Hovering => {
                    if left_mouse_released && affordable {
                        if currency.spend(cost) {
                            upgrades.increment(button.kind);
                            commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonClick, 0.2));

                            persist_unlock_data(
                                Some(&*currency),
                                Some(&*unlocked_classes),
                                Some(&*achievements),
                                Some(&*upgrades),
                            );
                        }
                        interactable.change(Interaction::None);
                        commands
                            .entity(entity)
                            .insert(UIElement::BackButton)
                            .insert(graphics.get_ui_element_texture(UIElement::BackButton));
                    }
                }
                _ => {}
            },
            _ => {
                // reset hovering states if we stop hovering
                let Interaction::Hovering = interactable.current() else {
                    continue;
                };
                interactable.change(Interaction::None);
                commands
                    .entity(entity)
                    .insert(UIElement::BackButton)
                    .insert(graphics.get_ui_element_texture(UIElement::BackButton));
            }
        }
    }
}

pub fn update_unlocks_currency_text(
    currency: Res<TimeFragmentCurrency>,
    mut query: Query<&mut Text, With<UnlocksCurrencyText>>,
) {
    if !currency.is_changed() {
        return;
    }

    for mut text in query.iter_mut() {
        text.sections[0].value = format!("{}", currency.time_fragments.max(0));
    }
}

pub fn refresh_unlock_button_states(
    currency: Res<TimeFragmentCurrency>,
    upgrades: Res<UnlockUpgrades>,
    mut buttons: Query<(&UnlockPurchaseButton, &mut Sprite, &Children)>,
    mut text_queries: ParamSet<(
        Query<&mut Text, With<UnlockButtonLabel>>,
        Query<(&UnlockCostText, &mut Text)>,
        Query<(&UnlockInfoText, &mut Text)>,
    )>,
) {
    if !currency.is_changed() && !upgrades.is_changed() {
        return;
    }

    for (button, mut sprite, children) in buttons.iter_mut() {
        // Hide button if unlock is maxed
        let is_maxed = upgrades.is_maxed(button.kind);
        if is_maxed {
            // Hide the button sprite
            sprite.color = Color::NONE;
            // Also hide the button label text
            {
                let mut labels = text_queries.p0();
                for child in children.iter() {
                    if let Ok(mut text) = labels.get_mut(*child) {
                        text.sections[0].style.color = Color::NONE;
                    }
                }
            }
            continue;
        }

        let cost = upgrades.next_cost(button.kind);
        let affordable = currency.time_fragments.max(0) as u32 >= cost;
        sprite.color = if affordable {
            Color::WHITE
        } else {
            Color::rgb(0.55, 0.55, 0.55)
        };

        {
            let mut labels = text_queries.p0();
            for child in children.iter() {
                if let Ok(mut text) = labels.get_mut(*child) {
                    text.sections[0].style.color = if affordable {
                        crate::colors::WHITE
                    } else {
                        Color::rgb(0.7, 0.7, 0.7)
                    };
                }
            }
        }
    }

    {
        let mut cost_texts = text_queries.p1();
        for (cost, mut text) in cost_texts.iter_mut() {
            let is_maxed = upgrades.is_maxed(cost.kind);
            if is_maxed {
                text.sections[0].value = "Maxed".to_string();
            } else {
                text.sections[0].value = format!("Cost: {}", upgrades.next_cost(cost.kind));
            }
        }
    }

    {
        let mut info_texts = text_queries.p2();
        for (info, mut text) in info_texts.iter_mut() {
            text.sections[0].value = unlock_effect_summary(info.kind, &upgrades);
        }
    }
}

pub fn cleanup_unlocks_ui(mut commands: Commands, query: Query<Entity, With<UnlocksUI>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn_recursive();
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
    let overlay = ui_helpers::spawn_ui_overlay(
        &mut commands,
        Vec2::new(resolution.game_width, resolution.game_height),
        1.,
        10.,
    );
    commands
        .entity(overlay)
        .insert(UnlocksUI)
        .insert(UIState::Unlocks);

    // Title
    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                "Unlocks",
                TextStyle {
                    font: asset_server.load("fonts/alagard.ttf"),
                    font_size: 30.0,
                    color: crate::colors::YELLOW_2,
                },
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: bevy::sprite::Anchor::Center,
            transform: Transform::from_translation(Vec3::new(0., 104., 11.)),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        UnlocksUI,
        Name::new("Unlocks Title"),
    ));

    // Currency text
    let currency_text = commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    format!("{}", currency.time_fragments.max(0)),
                    TextStyle {
                        font: asset_server.load("fonts/4x5.ttf"),
                        font_size: 10.0,
                        color: crate::colors::WHITE,
                    },
                )
                .with_alignment(TextAlignment::Left),
                text_anchor: bevy::sprite::Anchor::CenterLeft,
                transform: Transform::from_translation(Vec3::new(-160., 104.5, 11.)),
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            UnlocksUI,
            UnlocksCurrencyText,
            Name::new("Unlocks Currency Text"),
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
    commands.entity(currency_stack).set_parent(currency_text);

    let start_y = 86.5;
    let row_spacing = -27.0;

    for (index, kind) in UNLOCK_ROWS.iter().enumerate() {
        let y = start_y + row_spacing * index as f32;
        spawn_unlock_row(
            &mut commands,
            &graphics,
            &asset_server,
            *kind,
            Vec3::new(-140., y, 11.),
            Vec3::new(110.5, y - 6.5, 11.),
            upgrades.as_ref(),
        );
    }

    // Back Button
    let back_button = spawn_back_button(
        Vec3::new(0., -108., 11.),
        &mut commands,
        &graphics,
        &asset_server,
    );
    commands.entity(back_button).insert(UnlocksUI);
}

fn spawn_unlock_row(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    kind: UnlockUpgradeKind,
    info_pos: Vec3,
    button_pos: Vec3,
    upgrades: &UnlockUpgrades,
) {
    let title_name = format!("Unlock Row Title {}", kind.display_name());
    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                kind.display_name(),
                TextStyle {
                    font: asset_server.load("fonts/alagard.ttf"),
                    font_size: 15.0,
                    color: crate::colors::DARK_WOOD_BROWN,
                },
            )
            .with_alignment(TextAlignment::Left),
            text_anchor: bevy::sprite::Anchor::CenterLeft,
            transform: Transform::from_translation(info_pos),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        UnlocksUI,
        UIState::Unlocks,
        Name::new(title_name),
    ));

    let info_text_pos = Vec3::new(info_pos.x, info_pos.y - 11., info_pos.z);
    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                unlock_effect_summary(kind, upgrades),
                TextStyle {
                    font: asset_server.load("fonts/4x5.ttf"),
                    font_size: 5.,
                    color: crate::colors::WHITE,
                },
            )
            .with_alignment(TextAlignment::Left),
            text_anchor: bevy::sprite::Anchor::CenterLeft,
            transform: Transform::from_translation(info_text_pos),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        UnlocksUI,
        UIState::Unlocks,
        UnlockInfoText { kind },
        Name::new(format!("Unlock Row Info {}", kind.display_name())),
    ));

    let cost_pos = Vec3::new(35., info_pos.y, info_pos.z);
    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                format!("Cost: {}", upgrades.next_cost(kind)),
                TextStyle {
                    font: asset_server.load("fonts/4x5.ttf"),
                    font_size: 5.0,
                    color: crate::colors::WHITE,
                },
            )
            .with_alignment(TextAlignment::Left),
            text_anchor: bevy::sprite::Anchor::CenterLeft,
            transform: Transform::from_translation(cost_pos),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        UnlocksUI,
        UIState::Unlocks,
        UnlockCostText { kind },
        Name::new(format!("Unlock Row Cost {}", kind.display_name())),
    ));

    // Only show purchase button if unlock is not maxed
    let is_maxed = upgrades.is_maxed(kind);
    let button_entity = commands
        .spawn(SpriteBundle {
            texture: graphics
                .get_ui_element_texture(UIElement::BackButton)
                .clone(),
            sprite: Sprite {
                custom_size: Some(Vec2::new(53., 18.)),
                ..Default::default()
            },
            transform: Transform::from_translation(button_pos),
            visibility: if is_maxed {
                Visibility::Hidden
            } else {
                Visibility::Visible
            },
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIState::Unlocks)
        .insert(UIElement::BackButton)
        .insert(UnlocksUI)
        .insert(UnlockPurchaseButton { kind })
        .insert(Interactable::default())
        .insert(Name::new(format!(
            "Unlock Purchase Button {}",
            kind.display_name()
        )))
        .id();

    commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    "Purchase",
                    TextStyle {
                        font: asset_server.load("fonts/4x5.ttf"),
                        font_size: 5.0,
                        color: crate::colors::WHITE,
                    },
                )
                .with_alignment(TextAlignment::Center),
                text_anchor: Anchor::Center,
                transform: Transform::from_translation(Vec3::new(0.5, 0.5, 1.)),
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            UIState::Unlocks,
            UnlockButtonLabel,
            Name::new(format!("Unlock Purchase Label {}", kind.display_name())),
        ))
        .set_parent(button_entity);
}
