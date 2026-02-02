use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy_aseprite::{anim::AsepriteAnimation, AsepriteBundle};
use strum::IntoEnumIterator;

use crate::{
    ai::pathfinding::PathfindingCache,
    animations::{
        enemy_sprites::spawn_attack_warning_aseprite, player_sprite::PlayerSpriteHandles,
    },
    assets::Graphics,
    attributes::{ItemAttributes, ItemRarity},
    audio::{AudioSoundEffect, SoundSpawner},
    chaos::ChaosTracker,
    colors::{DARK_WOOD_BROWN, GREY, WHITE},
    container::ContainerRegistry,
    cursor::CursorPos,
    inventory::ItemStack,
    item::{CraftingTracker, ItemDisplayMetaData, WorldObject},
    night::NightTracker,
    player::{
        achievements::{is_pet_unlocked, Achievements},
        class_rank::ClassRankSystem,
        currency::TimeFragmentCurrency,
        score::HighScores,
        skills::{HeirloomChoiceQueue, PlayerClass, SkillClass},
        unlocks::{persist_unlock_data, RunUnlockState, UnlockUpgrades},
        ClassUnlockData, UnlockedClasses,
    },
    ui::{
        main_menu::GameStartFadein, spawn_back_button, spawn_back_button_texture_only,
        CheatSettings, MenuButton, UIElement, UIState,
    },
    world::{dimension::EraManager, portal::UIPortal},
    FairyPetSprite, Pet, RenderLayers, ScreenResolution, SlimePetSprite, GAME_HEIGHT,
};

use super::{
    interactions::{Interactable, Interaction},
    inventory_ui::spawn_item_stack_icon,
    player_hud::spawn_skill_tooltip_content,
    ui_helpers::spawn_ui_overlay,
};

const BODY_FONT: &str = "fonts/slkscr.ttf";
const TITLE_FONT: &str = "fonts/slkscrbold.ttf";
const BODY_FONT_SIZE: f32 = 8.4;

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

#[derive(Component)]
pub struct ClassUnlockWarningAnimation;

#[derive(Component)]
pub struct PortalAnimationState {
    pub state: PortalAnimState,
    pub pending_game_start: Option<PendingGameStart>,
}

#[derive(Debug, Clone)]
pub enum PortalAnimState {
    Idle,
    Transition,
    Era1,
}

#[derive(Debug, Clone, Resource)]
pub struct PendingGameStart {
    pub class: SkillClass,
    pub pets: Vec<Pet>,
}

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
    unlock_currency: Option<Res<TimeFragmentCurrency>>,
    _class_unlocks: Option<Res<ClassUnlockData>>,
    cheat_settings: Res<CheatSettings>,
) {
    let overlay = spawn_ui_overlay(
        &mut commands,
        Vec2::new(res.game_width + 10., GAME_HEIGHT + 100.),
        1.,
        9.,
    );

    let currency_text = commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    format!(
                        "{}",
                        unlock_currency
                            .as_ref()
                            .map(|c| c.time_fragments.max(0))
                            .unwrap_or(0)
                    ),
                    TextStyle {
                        font: asset_server.load(BODY_FONT),
                        font_size: BODY_FONT_SIZE,
                        color: Color::WHITE,
                    },
                )
                .with_alignment(TextAlignment::Left),
                text_anchor: Anchor::CenterLeft,
                transform: Transform::from_translation(Vec3::new(-170., 100., 11.)),
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            ClassSelectionUI,
            UIState::ClassSelection,
            ClassUnlockCurrencyText,
            Name::new("CLASS UNLOCK CURRENCY TEXT"),
        ))
        .set_parent(overlay)
        .id();
    let currency_stack = spawn_item_stack_icon(
        &mut commands,
        &graphics,
        &ItemStack::crate_icon_stack(WorldObject::TimeFragment),
        &asset_server,
        Vec2::new(-9., 1.),
        Vec2::new(0., 0.),
        3,
    );
    commands.entity(currency_stack).set_parent(currency_text);

    //===== PORTAL =====
    let _portal = commands
        .spawn(AsepriteBundle {
            aseprite: graphics.ui_portal_ase.as_ref().unwrap().clone(),
            animation: AsepriteAnimation::from(UIPortal::tags::IDLE),
            transform: Transform {
                translation: Vec3::new(30., -30., 10.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(UIState::ClassSelection)
        .insert(ClassSelectionUI)
        .insert(RenderLayers::from_layers(&[3]))
        .insert(PortalAnimationState {
            state: PortalAnimState::Idle,
            pending_game_start: None,
        })
        .insert(Name::new("PORTAL"))
        .id();
    spawn_class_unlock_info_ui(&mut commands, &asset_server);
    spawn_class_unlock_confirm_ui(&mut commands, &asset_server);

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
                translation: Vec3::new(0., res.game_height / 2. - 24., 10.),
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
                custom_size: Some(Vec2::new(576., 360.)),
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
        let x_offset = (i % 6) as f32 * 29.0 - 240.; // Center the options
        let y_offset = ((i / 6) as f32).trunc() * -29.0; // every 6 options, go to next row

        let class_unlocked =
            cheat_settings.bypass_class_unlocks || unlocked_classes.contains(class);
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
                        translation: Vec3::new(x_offset, 116. + y_offset, 11.),
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

        // Spawn warning animation for locked classes that have all achievements met
        if !class_unlocked {
            if let (Some(unlock_data), Some(achievements_res)) =
                (_class_unlocks.as_ref(), achievements.as_ref())
            {
                if let Some(entry) = unlock_data.entry(class) {
                    let requirements_met = entry
                        .achievements
                        .iter()
                        .all(|req| achievements_res.has(*req));
                    if requirements_met {
                        // Spawn warning animation above and center of the slot
                        let warning_y = 50. + y_offset + 11.; // 15 pixels above the slot center
                        let warning_pos = Vec3::new(x_offset, warning_y, 20.5);
                        let warning_entity = spawn_attack_warning_aseprite(
                            &mut commands,
                            &asset_server,
                            warning_pos,
                            overlay,
                            999999.0, // Very long duration so it persists
                        );
                        commands.entity(warning_entity).insert((
                            ClassSelectionUI,
                            UIState::ClassSelection,
                            ClassUnlockWarningAnimation,
                            ClassIcon {
                                class: class.clone(),
                            },
                            RenderLayers::from_layers(&[3]),
                            Name::new("Class Unlock Warning Animation"),
                        ));
                    }
                }
            }
        }
    }

    for (i, pet) in Pet::iter().enumerate() {
        // Check if pet is unlocked
        let pet_unlocked = achievements_ref
            .map(|a| is_pet_unlocked(&pet, a))
            .unwrap_or(false);

        // Pet option background
        let x_offset = (i as f32 - 1.0) * 29.0 + 149.; // Center the options
        let mut slot_entity_commands = commands.spawn(SpriteBundle {
            texture: graphics
                .get_ui_element_texture(UIElement::PlayerSelectSlot)
                .clone(),
            sprite: Sprite {
                custom_size: Some(Vec2::new(22., 22.)),
                ..Default::default()
            },
            transform: Transform {
                translation: Vec3::new(x_offset, 81., 11.),
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

    let button_y = -res.game_height / 2. + 38.;
    let button_x = res.game_width / 2. - 55.;
    // Confirm button
    let confirm_button = spawn_back_button_texture_only(
        Vec3::new(button_x, button_y, 11.),
        &mut commands,
        &graphics,
    );

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
                translation: Vec3::new(0.5, -0.5, 1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .set_parent(confirm_button);

    // Back Button (parent sprite + child text)
    let back_button_e = spawn_back_button(
        Vec3::new(button_x - 80., button_y, 11.),
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

pub fn update_class_unlock_warnings(
    mut commands: Commands,
    achievements: Option<Res<Achievements>>,
    class_unlocks: Option<Res<ClassUnlockData>>,
    unlocked_classes: Res<UnlockedClasses>,
    existing_warnings: Query<(Entity, &ClassIcon), With<ClassUnlockWarningAnimation>>,
    overlay_query: Query<Entity, (With<ClassSelectionUI>, With<UIState>)>,
    cheat_settings: Res<CheatSettings>,
) {
    // Get overlay entity for parenting
    let overlay = overlay_query.iter().next();
    if overlay.is_none() {
        return;
    }

    // Get existing warnings by class
    let existing_warnings_by_class: std::collections::HashMap<SkillClass, Entity> =
        existing_warnings
            .iter()
            .map(|(entity, icon)| (icon.class.clone(), entity))
            .collect();

    // Despawn warnings for classes that no longer need them (unlocked or requirements not met)
    for (class, entity) in existing_warnings_by_class.iter() {
        let should_have_warning =
            if cheat_settings.bypass_class_unlocks || unlocked_classes.contains(class) {
                false // Class is unlocked (or cheat enabled), no warning needed
            } else if let (Some(unlock_data), Some(achievements_res)) =
                (class_unlocks.as_ref(), achievements.as_ref())
            {
                if let Some(entry) = unlock_data.entry(class) {
                    entry
                        .achievements
                        .iter()
                        .all(|req| achievements_res.has(*req))
                } else {
                    false
                }
            } else {
                false
            };

        if !should_have_warning {
            commands.entity(*entity).despawn_recursive();
        }
    }
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
    let body_font = asset_server.load(BODY_FONT);

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
            BODY_FONT_SIZE,
            body_font.clone(),
        ),
        (
            ClassUnlockInfoTextKind::Achievement(1),
            Vec3::new(-48., 0., 1.),
            BODY_FONT_SIZE,
            body_font.clone(),
        ),
        (
            ClassUnlockInfoTextKind::Achievement(2),
            Vec3::new(-48., -16., 1.),
            BODY_FONT_SIZE,
            body_font.clone(),
        ),
        (
            ClassUnlockInfoTextKind::Cost,
            Vec3::new(-48., -38., 1.),
            BODY_FONT_SIZE,
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

fn spawn_class_unlock_confirm_ui(commands: &mut Commands, asset_server: &AssetServer) {
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
                        font: asset_server.load(BODY_FONT),
                        font_size: BODY_FONT_SIZE,
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

    // Don't spawn buttons here - they'll be spawned/despawned dynamically in update_class_unlock_confirm_panel
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
    unlock_currency: Option<Res<TimeFragmentCurrency>>,
    mut hover_state: ResMut<ClassUnlockHoverState>,
    mut confirm_state: ResMut<ClassUnlockConfirmState>,
    cheat_settings: Res<CheatSettings>,
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
                    let is_locked = !cheat_settings.bypass_class_unlocks
                        && !unlocked_classes.contains(&class_id);
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
                                    .spawn(SoundSpawner::new(AudioSoundEffect::ButtonHover, 0.05));
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
                            commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonHover, 0.05));
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
    unlock_currency: Option<Res<TimeFragmentCurrency>>,
    graphics: Res<Graphics>,
    mut panel_query: Query<(&mut Visibility, &mut Transform), With<ClassUnlockInfoPanel>>,
    mut text_query: Query<(&ClassUnlockInfoTextKind, &mut Text), With<ClassUnlockInfoText>>,
    cheat_settings: Res<CheatSettings>,
) {
    let Some(class) = hover_state.hovered_class.clone() else {
        if let Ok((mut vis, _)) = panel_query.get_single_mut() {
            *vis = Visibility::Hidden;
        }
        return;
    };

    if cheat_settings.bypass_class_unlocks || unlocked_classes.contains(&class) {
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
    unlock_currency: Option<Res<TimeFragmentCurrency>>,
    mut query: Query<&mut Text, With<ClassUnlockCurrencyText>>,
) {
    let Some(unlock_currency) = unlock_currency else {
        return;
    };

    if !unlock_currency.is_changed() {
        return;
    }

    for mut text in query.iter_mut() {
        text.sections[0].value = format!("{}", unlock_currency.time_fragments.max(0));
    }
}

pub fn update_class_unlock_confirm_panel(
    confirm_state: Res<ClassUnlockConfirmState>,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    mut param_set: ParamSet<(
        Query<Entity, With<ClassUnlockConfirmPanel>>,
        Query<Entity, With<ClassUnlockConfirmButton>>,
    )>,
    mut text_query: Query<&mut Text, With<ClassUnlockConfirmText>>,
    mut panel_vis_query: Query<&mut Visibility, With<ClassUnlockConfirmPanel>>,
) {
    let active = confirm_state.active;

    // Update panel visibility
    if let Ok(mut panel_vis) = panel_vis_query.get_single_mut() {
        *panel_vis = if active {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }

    // Get panel entity for parenting buttons
    let panel_entity = param_set.p0().get_single().ok();

    // Despawn buttons when not active
    if !active {
        let button_query = param_set.p1();
        for button_entity in button_query.iter() {
            commands.entity(button_entity).despawn_recursive();
        }
        return;
    }

    // Check if buttons already exist
    let button_query = param_set.p1();
    let button_count = button_query.iter().count();

    // Spawn buttons if they don't exist
    if button_count == 0 {
        if let Some(panel) = panel_entity {
            let button_font = asset_server.load("fonts/alagard.ttf");

            let yes_button = commands
                .spawn((
                    SpriteBundle {
                        texture: graphics.get_ui_element_texture(UIElement::BackButton),
                        sprite: Sprite {
                            custom_size: Some(Vec2::new(48., 18.)),
                            ..Default::default()
                        },
                        transform: Transform::from_translation(Vec3::new(28., -20., 1.)),
                        ..Default::default()
                    },
                    RenderLayers::from_layers(&[3]),
                    ClassUnlockConfirmButton::Yes,
                    Interactable::default(),
                    MenuButton::ClassUnlockYes,
                    UIElement::BackButton,
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
                        ..Default::default()
                    },
                    RenderLayers::from_layers(&[3]),
                    ClassUnlockConfirmButton::No,
                    Interactable::default(),
                    MenuButton::ClassUnlockNo,
                    UIElement::BackButton,
                    UIState::ClassSelection,
                    ClassSelectionUI,
                    Name::new("CLASS UNLOCK NO BUTTON"),
                ))
                .id();

            commands.entity(yes_button).set_parent(panel);
            commands.entity(no_button).set_parent(panel);

            // Add button text
            commands
                .spawn(Text2dBundle {
                    text: Text::from_section(
                        "Yes",
                        TextStyle {
                            font: button_font.clone(),
                            font_size: 15.0,
                            color: WHITE,
                        },
                    )
                    .with_alignment(TextAlignment::Center),
                    text_anchor: Anchor::Center,
                    transform: Transform::from_translation(Vec3::new(0., -1., 1.)),
                    ..Default::default()
                })
                .insert(RenderLayers::from_layers(&[3]))
                .set_parent(yes_button);

            commands
                .spawn(Text2dBundle {
                    text: Text::from_section(
                        "No",
                        TextStyle {
                            font: button_font,
                            font_size: 15.0,
                            color: WHITE,
                        },
                    )
                    .with_alignment(TextAlignment::Center),
                    text_anchor: Anchor::Center,
                    transform: Transform::from_translation(Vec3::new(0., -1., 1.)),
                    ..Default::default()
                })
                .insert(RenderLayers::from_layers(&[3]))
                .set_parent(no_button);
        }
    }

    // Update text
    if let Ok(mut text) = text_query.get_single_mut() {
        if let Some(class) = confirm_state.class.clone() {
            let class_data = graphics.get_class_data(class.clone());
            let cost = confirm_state.cost;
            text.sections[0].value = if cost > 0 {
                format!("Unlock {}\n\n\n{} currency?", class_data.name, cost)
            } else {
                format!("Unlock {} for free?", class_data.name)
            };
        } else {
            text.sections[0].value.clear();
        }
    }
}

pub fn persist_class_unlock_state(
    time_fragment_currency: Option<&TimeFragmentCurrency>,
    unlocked_classes: &UnlockedClasses,
    achievements: Option<&Achievements>,
    unlock_upgrades: Option<&UnlockUpgrades>,
) {
    persist_unlock_data(
        time_fragment_currency,
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
    let ICONS_X_OFFSET = -24.;
    // let SKILL_X_OFFSET = -20.;
    let ICONS_Y_OFFSET = -20.;
    let ICON_Y_SPACING = -33.;
    let SKILL_Y_OFFSET = -16.;
    let TEXT_Y_OFFSET = 12.;
    let TITLE_Y_OFFSET = 5.;
    let TITLE_X_OFFSET = 72.;
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
            translation: Vec3::new(-230., 79., 12.),
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
                translation: Vec3::new(TITLE_X_OFFSET, TITLE_Y_OFFSET, 1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("CLASS TITLE"))
        .set_parent(player_container)
        .id();
    // Spawn class title text above the player
    let _skill_title_text = commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                "Skills".to_string(),
                TextStyle {
                    font: asset_server.load("fonts/alagard.ttf"),
                    font_size: 15.0,
                    color: DARK_WOOD_BROWN,
                },
            ),
            text_anchor: Anchor::Center,
            transform: Transform {
                translation: Vec3::new(TITLE_X_OFFSET, ICONS_Y_OFFSET + ICON_Y_SPACING - 27., 1.),
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
                    font: asset_server.load(BODY_FONT),
                    font_size: BODY_FONT_SIZE,
                    color: DARK_WOOD_BROWN,
                },
            ),
            text_anchor: Anchor::CenterLeft,
            transform: Transform {
                translation: Vec3::new(-19., TITLE_Y_OFFSET, 1.),
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
                    font: asset_server.load(BODY_FONT),
                    font_size: BODY_FONT_SIZE,
                    color: DARK_WOOD_BROWN,
                },
            ),
            text_anchor: Anchor::CenterLeft,
            transform: Transform {
                translation: Vec3::new(123., TITLE_Y_OFFSET, 1.),
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
                translation: Vec3::new(268., -97., 1.),
                scale: Vec3::new(2., 2., 2.),
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
        Vec2::new(ICONS_X_OFFSET, ICONS_Y_OFFSET),
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

    // Spawn weapon description text
    let _weapon_description_text = commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                format!("{} {}", weapon_rarity, weapon_description),
                TextStyle {
                    font: asset_server.load(BODY_FONT),
                    font_size: BODY_FONT_SIZE,
                    color: if weapon_rarity == ItemRarity::Common {
                        DARK_WOOD_BROWN
                    } else {
                        weapon_rarity.get_color()
                    },
                },
            )
            .with_alignment(TextAlignment::Left),
            text_anchor: Anchor::TopLeft,
            transform: Transform {
                translation: Vec3::new(DESC_TEXT_X, ICONS_Y_OFFSET + TEXT_Y_OFFSET - 8., 1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("WEAPON DESCRIPTION"))
        .set_parent(player_container)
        .id();

    // power icon
    let _power_icon_slot = commands
        .spawn(SpriteBundle {
            texture: power_icon,
            sprite: Sprite {
                custom_size: Some(Vec2::new(18., 18.)),
                ..Default::default()
            },
            transform: Transform {
                translation: Vec3::new(ICONS_X_OFFSET, ICONS_Y_OFFSET + ICON_Y_SPACING + 11., 11.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("POWER ICON"))
        .set_parent(player_container)
        .id();

    // Spawn stat description text
    let _power_description_text = commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                stat_description,
                TextStyle {
                    font: asset_server.load(BODY_FONT),
                    font_size: BODY_FONT_SIZE,
                    color: DARK_WOOD_BROWN,
                },
            )
            .with_alignment(TextAlignment::Left),
            text_anchor: Anchor::TopLeft,
            transform: Transform {
                translation: Vec3::new(
                    DESC_TEXT_X,
                    ICONS_Y_OFFSET + ICON_Y_SPACING + TEXT_Y_OFFSET + 4.,
                    1.,
                ),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("STAT DESCRIPTION"))
        .set_parent(player_container)
        .id();

    for (skill_index, active_skill) in class_data.active_skills.iter().enumerate() {
        let skill_y_offset = SKILL_Y_OFFSET
            + ICONS_Y_OFFSET
            + ICON_Y_SPACING * 2.
            + ((ICON_Y_SPACING - 4.) * skill_index as f32);

        let skill_container = commands
            .spawn(SpatialBundle::from_transform(Transform {
                translation: Vec3::new(0., skill_y_offset, 0.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            }))
            .insert(RenderLayers::from_layers(&[3]))
            .insert(Name::new(format!("SKILL CONTAINER {}", skill_index)))
            .set_parent(player_container)
            .id();

        // Use the shared helper function to spawn skill content
        spawn_skill_tooltip_content(
            commands,
            graphics,
            asset_server,
            active_skill.clone(),
            skill_container,
            1.,
        );
    }

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
    let pet_skill_desc = pet_data.skill_description.join(" ");
    let pet_skill_name = pet_data.skill_name.clone();
    let pet_passive = pet_data.passive_description.join(" ");
    const Y_OFFSET: f32 = -4.;
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
            translation: Vec3::new(120., 20., 12.),
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
                translation: Vec3::new(-100., -69., 1.),
                scale: Vec3::new(2., 2., 2.),
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
                translation: Vec3::new(0., -18. + Y_OFFSET, 11.),
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
    let _power_title_text = commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                pet_skill_name,
                TextStyle {
                    font: asset_server.load(TITLE_FONT),
                    font_size: BODY_FONT_SIZE,
                    color: DARK_WOOD_BROWN,
                },
            )
            .with_alignment(TextAlignment::Left),
            text_anchor: Anchor::TopLeft,
            transform: Transform {
                translation: Vec3::new(12., Y_OFFSET, 1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("PET DESCRIPTION"))
        .set_parent(pet_container)
        .id();
    let _skill_description_text = commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                pet_skill_desc,
                TextStyle {
                    font: asset_server.load(BODY_FONT),
                    font_size: BODY_FONT_SIZE,
                    color: DARK_WOOD_BROWN,
                },
            )
            .with_alignment(TextAlignment::Left),
            text_anchor: Anchor::TopLeft,
            transform: Transform {
                translation: Vec3::new(12., -9. + Y_OFFSET, 1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("PET DESCRIPTION"))
        .set_parent(pet_container)
        .id();
    let _passive_title_text = commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                "Passive Buff",
                TextStyle {
                    font: asset_server.load(TITLE_FONT),
                    font_size: BODY_FONT_SIZE,
                    color: DARK_WOOD_BROWN,
                },
            )
            .with_alignment(TextAlignment::Left),
            text_anchor: Anchor::TopLeft,
            transform: Transform {
                translation: Vec3::new(12., -38. + Y_OFFSET, 1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("PET DESCRIPTION"))
        .set_parent(pet_container)
        .id();
    let _passive_text = commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                pet_passive,
                TextStyle {
                    font: asset_server.load(BODY_FONT),
                    font_size: BODY_FONT_SIZE,
                    color: DARK_WOOD_BROWN,
                },
            )
            .with_alignment(TextAlignment::Left),
            text_anchor: Anchor::TopLeft,
            transform: Transform {
                translation: Vec3::new(12., -48. + Y_OFFSET, 1.),
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
    cheat_settings: Res<CheatSettings>,
) {
    if !unlocked_classes.is_changed() && !cheat_settings.is_changed() {
        return;
    }

    for (class_icon, mut texture) in class_icons.iter_mut() {
        let class_data = graphics.get_class_data(class_icon.class.clone());
        if cheat_settings.bypass_class_unlocks || unlocked_classes.contains(&class_icon.class) {
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

pub fn handle_portal_animation(
    mut portal_query: Query<(
        Entity,
        &mut PortalAnimationState,
        &mut bevy_aseprite::anim::AsepriteAnimation,
    )>,
    pending_game_start: Option<ResMut<PendingGameStart>>,
    mut next_state: ResMut<NextState<crate::GameState>>,
    mut next_ui_state: ResMut<NextState<UIState>>,
    mut commands: Commands,
    unlock_upgrades: Res<UnlockUpgrades>,
    mut run_unlock_state: ResMut<RunUnlockState>,
    screen_res: Res<ScreenResolution>,
) {
    for (_portal_entity, mut anim_state, mut anim) in portal_query.iter_mut() {
        // Check if we have a pending game start resource and are still in Idle state
        if let Some(pending) = pending_game_start.as_deref() {
            if matches!(anim_state.state, PortalAnimState::Idle) && anim.current_frame() == 0 {
                // Start the TRANSITION animation
                anim_state.state = PortalAnimState::Transition;
                anim_state.pending_game_start = Some(pending.clone());
                *anim = bevy_aseprite::anim::AsepriteAnimation::from(UIPortal::tags::TRANSITION);
                // Remove the resource so we don't trigger this again
                commands.remove_resource::<PendingGameStart>();
            }
        }

        match anim_state.state {
            PortalAnimState::Transition => {
                // Check if we've reached frame 17, then switch to ERA1
                if anim.current_frame() >= 17 {
                    anim_state.state = PortalAnimState::Era1;
                    *anim = bevy_aseprite::anim::AsepriteAnimation::from(UIPortal::tags::ERA1);
                }
            }
            PortalAnimState::Era1 => {
                // Check if we've reached frame 26, then transition to game
                if anim.current_frame() >= 26 {
                    // Now we can actually start the game
                    if let Some(pending) = anim_state.pending_game_start.take() {
                        // Close the class selection UI and transition to loading state
                        next_ui_state.set(UIState::Closed);
                        next_state.set(crate::GameState::Initializing);

                        // Add PlayerClass component to the game
                        commands.insert_resource(PlayerClass {
                            class: pending.class,
                            pets: pending.pets,
                        });

                        // Initialize game resources
                        commands.init_resource::<crate::Game>();
                        commands.init_resource::<NightTracker>();
                        commands.init_resource::<ChaosTracker>();
                        commands.insert_resource(HeirloomChoiceQueue::default());
                        commands.init_resource::<ContainerRegistry>();
                        commands.init_resource::<PathfindingCache>();
                        commands.init_resource::<CraftingTracker>();
                        commands.init_resource::<EraManager>();

                        run_unlock_state.reset_for_run(&*unlock_upgrades);

                        // Start the game with fade-in overlay
                        commands
                            .spawn(SpriteBundle {
                                sprite: Sprite {
                                    color: Color::rgba(0., 0., 0., 0.),
                                    custom_size: Some(Vec2::new(
                                        screen_res.game_width + 10.,
                                        crate::GAME_HEIGHT + 20.,
                                    )),
                                    ..default()
                                },
                                transform: Transform {
                                    translation: Vec3::new(0., 0., 10.),
                                    scale: Vec3::new(1., 1., 1.),
                                    ..Default::default()
                                },
                                ..default()
                            })
                            .insert(RenderLayers::from_layers(&[3]))
                            .insert(Name::new("overlay"))
                            .insert(GameStartFadein(Timer::from_seconds(3.0, TimerMode::Once)));
                    }
                }
            }
            PortalAnimState::Idle => {
                // Do nothing, waiting for Begin button
            }
        }
    }
}
