use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy_aseprite::{anim::AsepriteAnimation, AsepriteBundle};
use strum::IntoEnumIterator;

use crate::{
    animations::player_sprite::PlayerSpriteHandles,
    assets::Graphics,
    attributes::ItemAttributes,
    audio::{AudioSoundEffect, SoundSpawner},
    colors::{DARK_WOOD_BROWN, GREY, WHITE},
    inputs::CursorPos,
    inventory::ItemStack,
    item::ItemDisplayMetaData,
    player::{
        achievements::{is_pet_unlocked, Achievements},
        class_rank::ClassRankSystem,
        score::HighScores,
        skills::SkillClass,
        unlocks::{persist_unlock_data, UnlockUpgrades},
        ClassUnlockData, UnlockCurrency, UnlockedClasses,
    },
    ui::{spawn_back_button, spawn_back_button_texture_only, MenuButton, UIElement, UIState},
    FairyPetSprite, Pet, RenderLayers, ScreenResolution, SlimePetSprite, GAME_HEIGHT,
};

use super::{
    interactions::{Interactable, Interaction},
    inventory_ui::spawn_item_stack_icon,
    ui_helpers::spawn_ui_overlay,
};

#[derive(Component)]
pub struct ClassSelectionUI;

#[derive(Component)]
pub struct ClassOption;
#[derive(Component)]
pub struct PetOption;

#[derive(Component)]
pub struct ClassIcon {
    pub class: SkillClass,
}

#[derive(Component)]
pub struct PlayerSelectSlot {
    pub is_hovered: bool,
    pub is_selected: bool,
    pub class: SkillClass,
    pub is_locked: bool,
}

#[derive(Component)]
pub struct PetSelectSlot {
    pub is_hovered: bool,
    pub is_selected: bool,
    pub pet: Pet,
}

#[derive(Component)]
pub struct ClassUnlockInfoPanel;

#[derive(Component)]
pub enum ClassUnlockInfoTextKind {
    Title,
    Achievement(usize),
    Cost,
}

#[derive(Component)]
pub struct ClassUnlockInfoText;

#[derive(Component)]
pub struct ClassUnlockCurrencyText;

#[derive(Resource, Default, Debug, Clone)]
pub struct ClassUnlockHoverState {
    pub hovered_class: Option<SkillClass>,
    pub slot_position: Vec3,
}

#[derive(Resource, Default, Debug, Clone)]
pub struct ClassUnlockConfirmState {
    pub active: bool,
    pub class: Option<SkillClass>,
    pub cost: u32,
    pub anchor_position: Vec3,
}

#[derive(Component)]
pub struct ClassUnlockConfirmPanel;

#[derive(Component)]
pub struct ClassUnlockConfirmText;

#[derive(Component)]
pub enum ClassUnlockConfirmButton {
    Yes,
    No,
}

#[derive(Component)]
pub struct ClassInfoCard;

#[derive(Component)]
pub struct ConfirmButton;

#[derive(Component)]
pub struct ClassPreviewSprite;

#[derive(Component)]
pub struct PetPreviewSprite;

#[derive(Resource)]
pub struct ClassSelectionState {
    pub selected_class: Option<SkillClass>,
    pub selected_pet: Option<Pet>,
}
impl Default for ClassSelectionState {
    fn default() -> Self {
        Self {
            selected_class: Some(SkillClass::Warrior),
            selected_pet: None, // No pet selected by default
        }
    }
}

pub fn setup_class_selection_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    sprite_handles: Res<PlayerSpriteHandles>,
    res: Res<ScreenResolution>,
    class_ranks: Res<ClassRankSystem>,
    high_scores: Option<Res<HighScores>>,
    achievements: Option<Res<Achievements>>,
    unlocked_classes: Res<UnlockedClasses>,
    unlock_currency: Option<Res<UnlockCurrency>>,
    _class_unlocks: Option<Res<ClassUnlockData>>,
) {
    let overlay = spawn_ui_overlay(
        &mut commands,
        Vec2::new(res.game_width + 10., GAME_HEIGHT + 100.),
        1.,
        9.,
    );

    commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    format!(
                        "Currency: {}",
                        unlock_currency.as_ref().map(|c| c.amount).unwrap_or(0)
                    ),
                    TextStyle {
                        font: asset_server.load("fonts/4x5.ttf"),
                        font_size: 10.0,
                        color: Color::WHITE,
                    },
                )
                .with_alignment(TextAlignment::Left),
                text_anchor: Anchor::TopLeft,
                transform: Transform::from_translation(Vec3::new(-190., 115., 11.)),
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            ClassSelectionUI,
            UIState::ClassSelection,
            ClassUnlockCurrencyText,
            Name::new("CLASS UNLOCK CURRENCY TEXT"),
        ))
        .set_parent(overlay);

    spawn_class_unlock_info_ui(&mut commands, &asset_server);
    spawn_class_unlock_confirm_ui(&mut commands, &graphics, &asset_server);

    // Title
    let title_text = commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                "Choose Your Class",
                TextStyle {
                    font: asset_server.load("fonts/alagard.ttf"),
                    font_size: 30.0,
                    color: DARK_WOOD_BROWN,
                },
            ),
            text_anchor: Anchor::Center,
            transform: Transform {
                translation: Vec3::new(0., 90., 10.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("CLASS TITLE"))
        .id();
    commands
        .entity(title_text)
        .insert(UIState::ClassSelection)
        .insert(ClassSelectionUI);
    // Class options from RON data
    let class_options = SkillClass::iter()
        .filter(|class| *class != SkillClass::None)
        .collect::<Vec<_>>();

    let achievements_ref = achievements.as_ref().map(|a| a.as_ref());
    let default_class = class_options
        .iter()
        .find(|class| unlocked_classes.contains(class))
        .cloned()
        .unwrap_or(SkillClass::Warrior);

    // Initialize the selection state with first unlocked class
    commands.insert_resource(ClassSelectionState {
        selected_class: Some(default_class.clone()),
        selected_pet: None,
    });

    let _class_select_bg = commands
        .spawn(SpriteBundle {
            texture: graphics
                .get_ui_element_texture(UIElement::PlayerSelect)
                .clone(),
            sprite: Sprite {
                custom_size: Some(Vec2::new(370.5, 142.)),
                ..Default::default()
            },
            transform: Transform {
                translation: Vec3::new(0., 0., 10.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(UIState::ClassSelection)
        .insert(ClassSelectionUI)
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("CLASS UI"))
        .id();

    for (i, class) in class_options.iter().enumerate() {
        let x_offset = (i % 6) as f32 * 27.0 - 166.; // Center the options
        let y_offset = ((i / 6) as f32).trunc() * -29.0; // every 6 options, go to next row

        let class_unlocked = unlocked_classes.contains(class);
        let class_selected = class == &default_class;

        // Class slot background
        let icon_slot = commands
            .spawn((
                SpriteBundle {
                    texture: graphics
                        .get_ui_element_texture(UIElement::PlayerSelectSlot)
                        .clone(),
                    sprite: Sprite {
                        custom_size: Some(Vec2::new(22., 22.)),
                        ..Default::default()
                    },
                    transform: Transform {
                        translation: Vec3::new(x_offset, 50. + y_offset, 11.),
                        scale: Vec3::new(1., 1., 1.),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                UIState::ClassSelection,
                ClassSelectionUI,
                ClassOption,
                PlayerSelectSlot {
                    is_hovered: false,
                    is_selected: class_selected && class_unlocked, // First unlocked class is selected by default
                    class: class.clone(),
                    is_locked: !class_unlocked,
                },
                Interactable::default(),
                RenderLayers::from_layers(&[3]),
                Name::new("CLASS OPTION"),
            ))
            .id();

        // Class icon - show actual icon if unlocked, or grey locked version if locked
        let class_data = graphics.get_class_data(class.clone());
        let icon_handle = if class_unlocked {
            graphics.get_ui_element_texture(class_data.class_icon.clone())
        } else {
            graphics.get_ui_element_texture(UIElement::UnknownUnlockIcon)
        };

        let mut icon_entity_commands = commands.spawn((
            SpriteBundle {
                texture: icon_handle,
                sprite: Sprite {
                    custom_size: Some(Vec2::new(22., 22.)),
                    ..Default::default()
                },
                transform: Transform {
                    translation: Vec3::new(0., 0., 1.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..Default::default()
            },
            UIState::ClassSelection,
            ClassSelectionUI,
            ClassOption,
            ClassIcon {
                class: class.clone(),
            },
            RenderLayers::from_layers(&[3]),
            Name::new("CLASS OPTION"),
        ));

        icon_entity_commands.set_parent(icon_slot);
    }

    for (i, pet) in Pet::iter().enumerate() {
        // Check if pet is unlocked
        let pet_unlocked = achievements_ref
            .map(|a| is_pet_unlocked(&pet, a))
            .unwrap_or(false);

        // Pet option background
        let x_offset = (i as f32 - 1.0) * 27.0 - 139.; // Center the options
        let mut slot_entity_commands = commands.spawn(SpriteBundle {
            texture: graphics
                .get_ui_element_texture(UIElement::PlayerSelectSlot)
                .clone(),
            sprite: Sprite {
                custom_size: Some(Vec2::new(22., 22.)),
                ..Default::default()
            },
            transform: Transform {
                translation: Vec3::new(x_offset, -49., 11.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        });

        slot_entity_commands
            .insert(UIState::ClassSelection)
            .insert(ClassSelectionUI)
            .insert(PetOption)
            .insert(PetSelectSlot {
                is_hovered: false,
                is_selected: false,
                pet: pet.clone(),
            })
            .insert(RenderLayers::from_layers(&[3]))
            .insert(Name::new("PET OPTION"));

        // Only add Interactable component for unlocked pets
        if pet_unlocked {
            slot_entity_commands.insert(super::Interactable::default());
        }

        let icon_slot = slot_entity_commands.id();

        // Pet icon - show actual icon if unlocked, or UnknownUnlockIcon if locked
        let pet_data = graphics.get_pet_data(pet.clone());
        let icon_texture = if pet_unlocked {
            graphics
                .get_ui_element_texture(pet_data.pet_icon.clone())
                .clone()
        } else {
            graphics
                .get_ui_element_texture(UIElement::UnknownUnlockIcon)
                .clone()
        };

        let mut icon_entity_commands = commands.spawn(SpriteBundle {
            texture: icon_texture,
            sprite: Sprite {
                custom_size: Some(Vec2::new(22., 22.)),
                color: if pet_unlocked { Color::WHITE } else { GREY },
                ..Default::default()
            },
            transform: Transform {
                translation: Vec3::new(0., 0., 1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        });

        icon_entity_commands
            .insert(UIState::ClassSelection)
            .insert(ClassSelectionUI)
            .insert(PetOption)
            .insert(RenderLayers::from_layers(&[3]))
            .insert(Name::new("PET OPTION ICON"));

        if pet_unlocked {
            icon_entity_commands.insert(super::Interactable::default());
        }

        icon_entity_commands.set_parent(icon_slot);
    }

    // Confirm button
    let confirm_button =
        spawn_back_button_texture_only(Vec3::new(80., -90., 11.), &mut commands, &graphics);

    commands
        .entity(confirm_button)
        .insert(UIState::ClassSelection)
        .insert(ClassSelectionUI)
        .insert(MenuButton::Begin)
        .insert(ConfirmButton);

    // Confirm button text
    let _confirm_text = commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                "BEGIN",
                TextStyle {
                    font: asset_server.load("fonts/alagard.ttf"),
                    font_size: 15.0,
                    color: WHITE,
                },
            ),
            text_anchor: Anchor::Center,
            transform: Transform {
                translation: Vec3::new(0.5, -1., 1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .set_parent(confirm_button);

    // Back Button (parent sprite + child text)
    let back_button_e = spawn_back_button(
        Vec3::new(-152.5, -90., 11.),
        &mut commands,
        &graphics,
        &asset_server,
    );

    commands
        .entity(back_button_e)
        .insert(UIState::ClassSelection)
        .insert(ClassSelectionUI);

    // Class preview sprite (shows default selected class)
    let _class_preview = spawn_player_preview(
        &mut commands,
        &sprite_handles,
        &SkillClass::Warrior, // Default to Melee
        &asset_server,
        &graphics,
        &class_ranks,
        high_scores.as_ref(),
    );

    // No pet preview by default - wait for player selection
}

fn spawn_class_unlock_info_ui(commands: &mut Commands, asset_server: &AssetServer) {
    let panel_entity = commands
        .spawn((
            SpriteBundle {
                sprite: Sprite {
                    color: Color::rgba(0.08, 0.08, 0.08, 0.92),
                    custom_size: Some(Vec2::new(110., 110.)),
                    ..Default::default()
                },
                transform: Transform::from_translation(Vec3::new(0., 0., 30.)),
                visibility: Visibility::Hidden,
                ..Default::default()
            },
            UIState::ClassSelection,
            ClassSelectionUI,
            ClassUnlockInfoPanel,
            RenderLayers::from_layers(&[3]),
            Name::new("Class Unlock Info Panel"),
        ))
        .id();

    let title_font = asset_server.load("fonts/alagard.ttf");
    let body_font = asset_server.load("fonts/4x5.ttf");

    let text_entries = [
        (
            ClassUnlockInfoTextKind::Title,
            Vec3::new(-48., 40., 1.),
            15.0,
            title_font.clone(),
        ),
        (
            ClassUnlockInfoTextKind::Achievement(0),
            Vec3::new(-48., 16., 1.),
            5.0,
            body_font.clone(),
        ),
        (
            ClassUnlockInfoTextKind::Achievement(1),
            Vec3::new(-48., 0., 1.),
            5.0,
            body_font.clone(),
        ),
        (
            ClassUnlockInfoTextKind::Achievement(2),
            Vec3::new(-48., -16., 1.),
            5.0,
            body_font.clone(),
        ),
        (
            ClassUnlockInfoTextKind::Cost,
            Vec3::new(-48., -38., 1.),
            5.0,
            body_font.clone(),
        ),
    ];

    for (kind, offset, size, font) in text_entries.into_iter() {
        commands
            .spawn((
                Text2dBundle {
                    text: Text::from_section(
                        "",
                        TextStyle {
                            font,
                            font_size: size,
                            color: Color::WHITE,
                        },
                    )
                    .with_alignment(TextAlignment::Left),
                    text_anchor: Anchor::CenterLeft,
                    transform: Transform::from_translation(offset),
                    ..Default::default()
                },
                RenderLayers::from_layers(&[3]),
                ClassUnlockInfoText,
                kind,
                UIState::ClassSelection,
                ClassSelectionUI,
                Name::new("Class Unlock Info Text"),
            ))
            .set_parent(panel_entity);
    }
}

fn spawn_class_unlock_confirm_ui(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
) {
    let panel_entity = commands
        .spawn((
            SpriteBundle {
                // texture: graphics.get_ui_element_texture(UIElement::MenuButton),
                sprite: Sprite {
                    color: Color::rgba(0.05, 0.05, 0.05, 0.93),
                    custom_size: Some(Vec2::new(140., 70.)),
                    ..Default::default()
                },
                transform: Transform::from_translation(Vec3::new(0., 0., 30.)),
                visibility: Visibility::Hidden,
                ..Default::default()
            },
            UIState::ClassSelection,
            ClassSelectionUI,
            ClassUnlockConfirmPanel,
            RenderLayers::from_layers(&[3]),
            Name::new("Class Unlock Confirm Panel"),
        ))
        .id();

    let text_entity = commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    "",
                    TextStyle {
                        font: asset_server.load("fonts/4x5.ttf"),
                        font_size: 5.0,
                        color: Color::WHITE,
                    },
                )
                .with_alignment(TextAlignment::Center),
                text_anchor: Anchor::Center,
                transform: Transform::from_translation(Vec3::new(0., 10., 1.)),
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            ClassUnlockConfirmText,
            UIState::ClassSelection,
            ClassSelectionUI,
            Name::new("Class Unlock Confirm Text"),
        ))
        .id();

    commands.entity(text_entity).set_parent(panel_entity);

    let yes_button = commands
        .spawn((
            SpriteBundle {
                texture: graphics.get_ui_element_texture(UIElement::BackButton),
                sprite: Sprite {
                    custom_size: Some(Vec2::new(48., 18.)),
                    ..Default::default()
                },
                transform: Transform::from_translation(Vec3::new(28., -20., 1.)),
                visibility: Visibility::Hidden,
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            ClassUnlockConfirmButton::Yes,
            Interactable::default(),
            MenuButton::ClassUnlockYes,
            UIState::ClassSelection,
            ClassSelectionUI,
            Name::new("CLASS UNLOCK YES BUTTON"),
        ))
        .id();

    let no_button = commands
        .spawn((
            SpriteBundle {
                texture: graphics.get_ui_element_texture(UIElement::BackButton),
                sprite: Sprite {
                    custom_size: Some(Vec2::new(48., 18.)),
                    ..Default::default()
                },
                transform: Transform::from_translation(Vec3::new(-30., -20., 1.)),
                visibility: Visibility::Hidden,
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            ClassUnlockConfirmButton::No,
            Interactable::default(),
            MenuButton::ClassUnlockNo,
            UIState::ClassSelection,
            ClassSelectionUI,
            Name::new("CLASS UNLOCK NO BUTTON"),
        ))
        .id();

    commands.entity(yes_button).set_parent(panel_entity);
    commands.entity(no_button).set_parent(panel_entity);

    let button_font = asset_server.load("fonts/alagard.ttf");

    commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    "Yes",
                    TextStyle {
                        font: button_font.clone(),
                        font_size: 15.0,
                        color: Color::WHITE,
                    },
                )
                .with_alignment(TextAlignment::Center),
                text_anchor: Anchor::Center,
                transform: Transform::from_translation(Vec3::new(0., -1., 1.)),
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            UIState::ClassSelection,
            ClassSelectionUI,
        ))
        .set_parent(yes_button);

    commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    "No",
                    TextStyle {
                        font: button_font,
                        font_size: 15.0,
                        color: Color::WHITE,
                    },
                )
                .with_alignment(TextAlignment::Center),
                text_anchor: Anchor::Center,
                transform: Transform::from_translation(Vec3::new(0., -1., 1.)),
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            UIState::ClassSelection,
            ClassSelectionUI,
        ))
        .set_parent(no_button);
}

pub fn handle_class_selection(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut class_options: Query<(
        Entity,
        Option<&mut Interactable>,
        Option<&mut PlayerSelectSlot>,
        Option<&mut PetSelectSlot>,
        Option<&GlobalTransform>,
    )>,

    mut commands: Commands,
    mut selection_state: ResMut<ClassSelectionState>,
    unlocked_classes: Res<UnlockedClasses>,
    achievements: Option<Res<Achievements>>,
    class_unlocks: Option<Res<ClassUnlockData>>,
    unlock_currency: Option<Res<UnlockCurrency>>,
    mut hover_state: ResMut<ClassUnlockHoverState>,
    mut confirm_state: ResMut<ClassUnlockConfirmState>,
) {
    let hit_test = super::ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);

    hover_state.hovered_class = None;
    hover_state.slot_position = Vec3::ZERO;

    for (entity, interactable_opt, mut class_option, mut pet_option, global_transform) in
        class_options.iter_mut()
    {
        let Some(mut interactable) = interactable_opt else {
            continue;
        };

        match hit_test {
            Some(hit_ent) if hit_ent.0 == entity => {
                if let Some(slot) = class_option.as_mut() {
                    let class_id = slot.class.clone();
                    let is_locked = !unlocked_classes.contains(&class_id);
                    slot.is_locked = is_locked;
                    let slot_pos = global_transform
                        .map(|t| t.translation())
                        .unwrap_or(Vec3::ZERO);
                    if !confirm_state.active {
                        hover_state.hovered_class = if is_locked {
                            hover_state.slot_position = slot_pos;
                            Some(class_id.clone())
                        } else {
                            None
                        };
                    }

                    match interactable.current() {
                        Interaction::None => {
                            interactable.change(Interaction::Hovering);
                            if !is_locked {
                                slot.is_hovered = true;
                                commands
                                    .spawn(SoundSpawner::new(AudioSoundEffect::ButtonHover, 0.15));
                            }
                        }
                        Interaction::Hovering => {
                            if left_mouse_pressed {
                                if is_locked {
                                    if let (Some(unlock_data), Some(achievements_res)) =
                                        (class_unlocks.as_ref(), achievements.as_ref())
                                    {
                                        if let Some(entry) = unlock_data.entry(&class_id) {
                                            let requirements_met = entry
                                                .achievements
                                                .iter()
                                                .all(|req| achievements_res.has(*req));
                                            let can_afford = unlock_currency
                                                .as_ref()
                                                .map(|currency| currency.can_spend(entry.cost))
                                                .unwrap_or(false);
                                            if requirements_met && can_afford {
                                                confirm_state.active = true;
                                                confirm_state.class = Some(class_id.clone());
                                                confirm_state.cost = entry.cost;
                                                confirm_state.anchor_position = slot_pos;
                                            }
                                        }
                                    }
                                } else {
                                    selection_state.selected_class = Some(class_id.clone());
                                    slot.is_selected = true;
                                    commands.spawn(SoundSpawner::new(
                                        AudioSoundEffect::ButtonClick,
                                        0.2,
                                    ));
                                }
                            }
                        }
                        _ => {}
                    }
                } else if let Some(pet_state) = pet_option.as_mut() {
                    match interactable.current() {
                        Interaction::None => {
                            interactable.change(Interaction::Hovering);
                            pet_state.is_hovered = true;
                            commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonHover, 0.15));
                        }
                        Interaction::Hovering => {
                            if left_mouse_pressed {
                                selection_state.selected_pet = Some(pet_state.pet.clone());
                                pet_state.is_selected = true;
                                commands
                                    .spawn(SoundSpawner::new(AudioSoundEffect::ButtonClick, 0.2));
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {
                if let Some(slot) = class_option.as_mut() {
                    if slot.is_hovered {
                        interactable.change(Interaction::None);
                        slot.is_hovered = false;
                    }
                }
                if let Some(slot) = pet_option.as_mut() {
                    if slot.is_hovered {
                        interactable.change(Interaction::None);
                        slot.is_hovered = false;
                    }
                }
            }
        }
    }
}

pub fn handle_slot_deselection(
    selection_state: Res<ClassSelectionState>,
    mut class_slots: Query<(Entity, &mut PlayerSelectSlot), With<ClassOption>>,
    mut pet_slots: Query<(Entity, &mut PetSelectSlot), With<PetOption>>,
) {
    if !selection_state.is_changed() {
        return;
    }

    // Deselect all class slots first
    for (_, mut slot) in class_slots.iter_mut() {
        slot.is_selected = false;
    }

    // Deselect all pet slots first
    for (_, mut slot) in pet_slots.iter_mut() {
        slot.is_selected = false;
    }

    // Then mark the selected ones as selected
    if let Some(selected_class) = &selection_state.selected_class {
        for (_, mut slot) in class_slots.iter_mut() {
            if slot.class == *selected_class {
                slot.is_selected = true;
            }
        }
    }

    if let Some(selected_pet) = &selection_state.selected_pet {
        for (_, mut slot) in pet_slots.iter_mut() {
            if slot.pet == *selected_pet {
                slot.is_selected = true;
            }
        }
    }
}

pub fn update_class_unlock_panel(
    hover_state: Res<ClassUnlockHoverState>,
    achievements: Option<Res<Achievements>>,
    class_unlocks: Option<Res<ClassUnlockData>>,
    unlocked_classes: Res<UnlockedClasses>,
    unlock_currency: Option<Res<UnlockCurrency>>,
    graphics: Res<Graphics>,
    mut panel_query: Query<(&mut Visibility, &mut Transform), With<ClassUnlockInfoPanel>>,
    mut text_query: Query<(&ClassUnlockInfoTextKind, &mut Text), With<ClassUnlockInfoText>>,
) {
    let Some(class) = hover_state.hovered_class.clone() else {
        if let Ok((mut vis, _)) = panel_query.get_single_mut() {
            *vis = Visibility::Hidden;
        }
        return;
    };

    if unlocked_classes.contains(&class) {
        if let Ok((mut vis, _)) = panel_query.get_single_mut() {
            *vis = Visibility::Hidden;
        }
        return;
    }

    let achievements_ref = achievements.as_ref().map(|a| a.as_ref());
    let unlock_entry = class_unlocks.as_ref().and_then(|data| data.entry(&class));
    let class_data = graphics.get_class_data(class.clone());
    let currency_ref = unlock_currency.as_ref().map(|c| &**c);

    let mut achievement_rows: Vec<(String, bool)> = Vec::new();
    let mut cost = 0_u32;
    if let Some(entry) = unlock_entry {
        cost = entry.cost;
        achievement_rows = entry
            .achievements
            .iter()
            .map(|req| {
                let done = achievements_ref.map_or(false, |a| a.has(*req));
                (
                    format!("{} {}", if done { "[x]" } else { "[ ]" }, req.get_name()),
                    done,
                )
            })
            .collect();
    }

    if let Ok((mut visibility, mut transform)) = panel_query.get_single_mut() {
        *visibility = Visibility::Visible;
        let offset = Vec3::new(70., -20.5, 0.);
        transform.translation = Vec3::new(
            hover_state.slot_position.x + offset.x,
            hover_state.slot_position.y + offset.y,
            30.,
        );
    }

    for (kind, mut text) in text_query.iter_mut() {
        match kind {
            ClassUnlockInfoTextKind::Title => {
                text.sections[0].value = format!("{}", class_data.name.clone());
                text.sections[0].style.color = Color::WHITE;
            }
            ClassUnlockInfoTextKind::Achievement(idx) => {
                if let Some((line, done)) = achievement_rows.get(*idx) {
                    text.sections[0].value = line.clone();
                    text.sections[0].style.color = if *done {
                        Color::rgb(0.4, 0.9, 0.4)
                    } else {
                        Color::rgb(1.0, 0.4, 0.4)
                    };
                } else {
                    text.sections[0].value.clear();
                    text.sections[0].style.color = Color::WHITE;
                }
            }
            ClassUnlockInfoTextKind::Cost => {
                if let Some(_entry) = unlock_entry {
                    text.sections[0].value = format!("Cost: {}", cost);
                    text.sections[0].style.color =
                        if currency_ref.map(|c| c.can_spend(cost)).unwrap_or(false) {
                            Color::WHITE
                        } else {
                            Color::rgb(1.0, 0.4, 0.4)
                        };
                } else {
                    text.sections[0].value.clear();
                    text.sections[0].style.color = Color::WHITE;
                }
            }
        }
    }
}

pub fn update_unlock_currency_text(
    unlock_currency: Option<Res<UnlockCurrency>>,
    mut query: Query<&mut Text, With<ClassUnlockCurrencyText>>,
) {
    let Some(unlock_currency) = unlock_currency else {
        return;
    };

    if !unlock_currency.is_changed() {
        return;
    }

    for mut text in query.iter_mut() {
        text.sections[0].value = format!("Currency: {}", unlock_currency.amount);
    }
}

pub fn update_class_unlock_confirm_panel(
    confirm_state: Res<ClassUnlockConfirmState>,
    graphics: Res<Graphics>,
    mut param_set: ParamSet<(
        Query<&mut Visibility, With<ClassUnlockConfirmPanel>>,
        Query<(
            &ClassUnlockConfirmButton,
            &mut Visibility,
            &mut Interactable,
        )>,
    )>,
    mut text_query: Query<&mut Text, With<ClassUnlockConfirmText>>,
) {
    let active = confirm_state.active;

    {
        let mut panel_query = param_set.p0();
        if let Ok(mut panel_vis) = panel_query.get_single_mut() {
            if !active {
                *panel_vis = Visibility::Hidden;
            } else {
                *panel_vis = Visibility::Visible;
            }
        }
    }

    let mut button_query = param_set.p1();
    if !active {
        for (_, mut vis, mut interactable) in button_query.iter_mut() {
            *vis = Visibility::Hidden;
            if matches!(interactable.current(), Interaction::Hovering) {
                interactable.change(Interaction::None);
            }
        }
        return;
    }

    if let Ok(mut text) = text_query.get_single_mut() {
        if let Some(class) = confirm_state.class.clone() {
            let class_data = graphics.get_class_data(class.clone());
            let cost = confirm_state.cost;
            text.sections[0].value = if cost > 0 {
                format!("Unlock {}\n\n{} currency?", class_data.name, cost)
            } else {
                format!("Unlock {} for free?", class_data.name)
            };
        } else {
            text.sections[0].value.clear();
        }
    }

    for (_, mut vis, _) in button_query.iter_mut() {
        *vis = Visibility::Visible;
    }
}

pub fn persist_class_unlock_state(
    unlock_currency: Option<&UnlockCurrency>,
    unlocked_classes: &UnlockedClasses,
    achievements: Option<&Achievements>,
    unlock_upgrades: Option<&UnlockUpgrades>,
) {
    persist_unlock_data(
        unlock_currency,
        Some(unlocked_classes),
        achievements,
        unlock_upgrades,
    );
}

fn spawn_player_preview(
    commands: &mut Commands,
    sprite_handles: &PlayerSpriteHandles,
    selected_class: &SkillClass,
    asset_server: &Res<AssetServer>,
    graphics: &Res<Graphics>,
    class_ranks: &Res<ClassRankSystem>,
    high_scores: Option<&Res<HighScores>>,
) -> Entity {
    let ICONS_X_OFFSET = 22.;
    let DESC_TEXT_X = ICONS_X_OFFSET + 12.;
    let (aseprite_path, animation_tag) = selected_class.get_anim_data(&sprite_handles);

    let class_data = graphics.get_class_data(selected_class.clone());
    let power_icon = graphics.get_ui_element_texture(class_data.skill_icon.clone());
    let class_name = &class_data.name;
    let weapon_description = class_data.weapon_description.join(" ");
    let stat_description = class_data.stat_description.join(" ");

    // Get class rank information
    let class_rank = class_ranks.get_class_rank(selected_class);
    let rank_text = format!("Rank {}", class_rank.rank);
    // let exp_to_next = class_rank.get_experience_to_next_rank();
    // let progress_text = if exp_to_next > 0 {
    //     format!("Next rank in {} exp", exp_to_next)
    // } else {
    //     "Max rank reached!".to_string()
    // };

    // Spawn the player preview container
    let player_container = commands
        .spawn((
            UIState::ClassSelection,
            ClassSelectionUI,
            ClassPreviewSprite,
            RenderLayers::from_layers(&[3]),
            Name::new("PLAYER PREVIEW CONTAINER"),
        ))
        .insert(SpatialBundle::from_transform(Transform {
            translation: Vec3::new(28., 35., 12.),
            scale: Vec3::new(1., 1., 1.),
            ..Default::default()
        }))
        .id();

    // Spawn class title text above the player
    let _title_text = commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                class_name,
                TextStyle {
                    font: asset_server.load("fonts/alagard.ttf"),
                    font_size: 15.0,
                    color: DARK_WOOD_BROWN,
                },
            ),
            text_anchor: Anchor::Center,
            transform: Transform {
                translation: Vec3::new(62., 16., 1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("CLASS TITLE"))
        .set_parent(player_container)
        .id();

    // Spawn rank information
    let _rank_text = commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                rank_text,
                TextStyle {
                    font: asset_server.load("fonts/4x5.ttf"),
                    font_size: 5.0,
                    color: DARK_WOOD_BROWN,
                },
            ),
            text_anchor: Anchor::CenterLeft,
            transform: Transform {
                translation: Vec3::new(-19., 16., 1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("CLASS RANK"))
        .set_parent(player_container)
        .id();

    // Highest score for this class (top-right corner display)
    let class_high_score = high_scores
        .and_then(|hs| hs.class_high_scores.get(selected_class).copied())
        .unwrap_or(0);
    let high_score_text = format!("Best: {}", class_high_score);
    let _high_score = commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                high_score_text,
                TextStyle {
                    font: asset_server.load("fonts/4x5.ttf"),
                    font_size: 5.0,
                    color: DARK_WOOD_BROWN,
                },
            ),
            text_anchor: Anchor::CenterLeft,
            transform: Transform {
                translation: Vec3::new(103., 16., 1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("CLASS HIGH SCORE"))
        .set_parent(player_container)
        .id();

    // Spawn the player sprite
    let _player_sprite = commands
        .spawn(AsepriteBundle {
            aseprite: aseprite_path,
            animation: AsepriteAnimation::from(animation_tag),
            transform: Transform {
                translation: Vec3::new(-2., -7., 1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(UIState::ClassSelection)
        .insert(ClassSelectionUI)
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("CLASS PREVIEW"))
        .set_parent(player_container)
        .id();

    // Spawn starting weapon to the right of the player
    let starting_weapon = selected_class.get_starting_wep();

    // Get the starting weapon rarity based on class rank
    let weapon_rarity = class_rank.get_starting_weapon_rarity();

    let weapon_item_stack = ItemStack {
        obj_type: starting_weapon,
        count: 1,
        rarity: weapon_rarity.clone(),
        attributes: ItemAttributes::default(),
        metadata: ItemDisplayMetaData::default(),
    };

    let weapon_sprite = spawn_item_stack_icon(
        commands,
        graphics,
        &weapon_item_stack,
        asset_server,
        Vec2::new(ICONS_X_OFFSET, 2.),
        Vec2::ZERO,
        3,
    );
    // Note: spawn_item_stack_icon automatically adds rarity glows based on item_stack.rarity
    // power icon
    let _wep_icon_slot = commands
        .spawn(SpriteBundle {
            texture: graphics
                .get_ui_element_texture(UIElement::ClassWeaponSlot)
                .clone(),
            sprite: Sprite {
                custom_size: Some(Vec2::new(18., 18.)),
                ..Default::default()
            },
            transform: Transform {
                translation: Vec3::new(0., 0., -1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("POWER ICON"))
        .set_parent(weapon_sprite)
        .id();

    // Set the weapon as a child of the player container
    commands.entity(weapon_sprite).set_parent(player_container);

    // power icon
    let _icon_slot = commands
        .spawn(SpriteBundle {
            texture: power_icon,
            sprite: Sprite {
                custom_size: Some(Vec2::new(18., 18.)),
                ..Default::default()
            },
            transform: Transform {
                translation: Vec3::new(ICONS_X_OFFSET, -18., 11.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("POWER ICON"))
        .set_parent(player_container)
        .id();
    // Spawn weapon description text
    let _weapon_description_text = commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                weapon_description,
                TextStyle {
                    font: asset_server.load("fonts/4x5.ttf"),
                    font_size: 5.0,
                    color: DARK_WOOD_BROWN,
                },
            )
            .with_alignment(TextAlignment::Left),
            text_anchor: Anchor::TopLeft,
            transform: Transform {
                translation: Vec3::new(DESC_TEXT_X, 7., 1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("WEAPON DESCRIPTION"))
        .set_parent(player_container)
        .id();

    // Spawn stat description text
    let _stat_description_text = commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                stat_description,
                TextStyle {
                    font: asset_server.load("fonts/4x5.ttf"),
                    font_size: 5.0,
                    color: DARK_WOOD_BROWN,
                },
            )
            .with_alignment(TextAlignment::Left),
            text_anchor: Anchor::TopLeft,
            transform: Transform {
                translation: Vec3::new(DESC_TEXT_X, -14., 1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("STAT DESCRIPTION"))
        .set_parent(player_container)
        .id();

    player_container
}

fn spawn_pet_preview(
    commands: &mut Commands,
    sprite_handles: &PlayerSpriteHandles,
    selected_pet: &Pet,
    asset_server: &Res<AssetServer>,
    graphics: &Res<Graphics>,
) -> Entity {
    let (aseprite_path, animation_tag) = match selected_pet {
        Pet::Fairy => (sprite_handles.fairy_pet.clone(), FairyPetSprite::tags::IDLE),
        Pet::Slime => (sprite_handles.slime_pet.clone(), SlimePetSprite::tags::IDLE),
    };

    let pet_data = graphics.get_pet_data(selected_pet.clone());
    let pet_name = &pet_data.name;
    let power_icon = graphics.get_ui_element_texture(pet_data.skill_icon.clone());
    let pet_description = pet_data.description.join(" ");

    // Spawn the pet preview container
    let pet_container = commands
        .spawn((
            UIState::ClassSelection,
            ClassSelectionUI,
            PetPreviewSprite,
            RenderLayers::from_layers(&[3]),
            Name::new("PET PREVIEW CONTAINER"),
        ))
        .insert(SpatialBundle::from_transform(Transform {
            translation: Vec3::new(28., -26., 12.),
            scale: Vec3::new(1., 1., 1.),
            ..Default::default()
        }))
        .id();

    // Spawn pet title text above the pet
    let _title_text = commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                pet_name,
                TextStyle {
                    font: asset_server.load("fonts/alagard.ttf"),
                    font_size: 15.0,
                    color: DARK_WOOD_BROWN,
                },
            ),
            text_anchor: Anchor::Center,
            transform: Transform {
                translation: Vec3::new(62., 10., 1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("PET TITLE"))
        .set_parent(pet_container)
        .id();

    // Spawn the pet sprite
    let _pet_sprite = commands
        .spawn(AsepriteBundle {
            aseprite: aseprite_path,
            animation: AsepriteAnimation::from(animation_tag),
            transform: Transform {
                translation: Vec3::new(-3., -9., 1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(UIState::ClassSelection)
        .insert(ClassSelectionUI)
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("PET PREVIEW"))
        .set_parent(pet_container)
        .id();

    // power icon
    let _icon_slot = commands
        .spawn(SpriteBundle {
            texture: power_icon,
            sprite: Sprite {
                custom_size: Some(Vec2::new(18., 18.)),
                ..Default::default()
            },
            transform: Transform {
                translation: Vec3::new(22., -13., 11.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("POWER ICON"))
        .set_parent(pet_container)
        .id();

    // Spawn pet description text to the right
    let _description_text = commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                pet_description,
                TextStyle {
                    font: asset_server.load("fonts/4x5.ttf"),
                    font_size: 5.0,
                    color: DARK_WOOD_BROWN,
                },
            )
            .with_alignment(TextAlignment::Left),
            text_anchor: Anchor::TopLeft,
            transform: Transform {
                translation: Vec3::new(34., 0., 1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("PET DESCRIPTION"))
        .set_parent(pet_container)
        .id();

    pet_container
}

pub fn update_preview_sprites(
    selection_state: Res<ClassSelectionState>,
    class_previews: Query<Entity, With<ClassPreviewSprite>>,
    pet_previews: Query<Entity, With<PetPreviewSprite>>,
    sprite_handles: Res<PlayerSpriteHandles>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    graphics: Res<Graphics>,
    class_ranks: Res<ClassRankSystem>,
    high_scores: Option<Res<HighScores>>,
) {
    if !selection_state.is_changed() {
        return;
    }

    // Despawn existing class previews (including their containers)
    for entity in class_previews.iter() {
        commands.entity(entity).despawn_recursive();
    }

    // Despawn existing pet previews
    for entity in pet_previews.iter() {
        commands.entity(entity).despawn_recursive();
    }

    // Recreate class preview using helper function
    if let Some(selected_class) = &selection_state.selected_class {
        info!("Recreating class preview for {:?}", selected_class);
        let _player_container = spawn_player_preview(
            &mut commands,
            &sprite_handles,
            selected_class,
            &asset_server,
            &graphics,
            &class_ranks,
            high_scores.as_ref(),
        );
    }

    // Recreate pet preview using helper function
    if let Some(selected_pet) = &selection_state.selected_pet {
        info!("Recreating pet preview for {:?}", selected_pet);
        let _pet_container = spawn_pet_preview(
            &mut commands,
            &sprite_handles,
            selected_pet,
            &asset_server,
            &graphics,
        );
    }
}

pub fn update_slot_visuals(
    mut class_slots: Query<
        (&mut Handle<Image>, &PlayerSelectSlot),
        (
            With<ClassOption>,
            Without<PetOption>,
            Without<ConfirmButton>,
        ),
    >,
    mut pet_slots: Query<
        (&mut Handle<Image>, &PetSelectSlot),
        (
            With<PetOption>,
            Without<ClassOption>,
            Without<ConfirmButton>,
        ),
    >,
    graphics: Res<Graphics>,
) {
    // Update class slot visuals
    for (mut texture, slot) in class_slots.iter_mut() {
        if slot.is_selected {
            // Use selected texture (could be the same as hover for now)
            *texture = graphics
                .get_ui_element_texture(UIElement::PlayerSelectSlotHover)
                .clone();
        } else if slot.is_hovered {
            *texture = graphics
                .get_ui_element_texture(UIElement::PlayerSelectSlotHover)
                .clone();
        } else {
            *texture = graphics
                .get_ui_element_texture(UIElement::PlayerSelectSlot)
                .clone();
        }
    }

    // Update pet slot visuals
    for (mut texture, slot) in pet_slots.iter_mut() {
        if slot.is_selected {
            // Use selected texture (could be the same as hover for now)
            *texture = graphics
                .get_ui_element_texture(UIElement::PlayerSelectSlotHover)
                .clone();
        } else if slot.is_hovered {
            *texture = graphics
                .get_ui_element_texture(UIElement::PlayerSelectSlotHover)
                .clone();
        } else {
            *texture = graphics
                .get_ui_element_texture(UIElement::PlayerSelectSlot)
                .clone();
        }
    }
}

pub fn update_class_option_icons(
    unlocked_classes: Res<UnlockedClasses>,
    graphics: Res<Graphics>,
    mut class_icons: Query<(&ClassIcon, &mut Handle<Image>)>,
) {
    if !unlocked_classes.is_changed() {
        return;
    }

    for (class_icon, mut texture) in class_icons.iter_mut() {
        let class_data = graphics.get_class_data(class_icon.class.clone());
        if unlocked_classes.contains(&class_icon.class) {
            *texture = graphics
                .get_ui_element_texture(class_data.class_icon.clone())
                .clone();
        } else {
            *texture = graphics
                .get_ui_element_texture(UIElement::UnknownUnlockIcon)
                .clone();
        }
    }
}

pub fn update_info_card(
    _selection_state: Res<ClassSelectionState>,
    _info_cards: Query<&mut Children, With<ClassInfoCard>>,
    _commands: Commands,
    _asset_server: Res<AssetServer>,
) {
    // TODO: Implement info card updates
    // For now, this is a placeholder to avoid compilation errors
}
