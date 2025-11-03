use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy_aseprite::{anim::AsepriteAnimation, AsepriteBundle};
use strum::IntoEnumIterator;

use crate::{
    ai::pathfinding::PathfindingCache,
    animations::player_sprite::PlayerSpriteHandles,
    assets::Graphics,
    attributes::ItemAttributes,
    chaos::ChaosTracker,
    colors::{BLACK, DARK_WOOD_BROWN, GREY},
    container::ContainerRegistry,
    inputs::CursorPos,
    inventory::ItemStack,
    item::{CraftingTracker, ItemDisplayMetaData},
    night::NightTracker,
    player::{
        achievements::{is_class_unlocked, is_pet_unlocked, Achievements},
        class_rank::ClassRankSystem,
        get_default_unlocked_classes,
        score::HighScores,
        skills::{HeirloomChoiceQueue, PlayerClass, SkillClass},
    },
    ui::{UIElement, UIState},
    EraManager, FairyPetSprite, Pet, RenderLayers, ScreenResolution, SlimePetSprite, GAME_HEIGHT,
};

use super::{
    damage_numbers::spawn_text,
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
) {
    // Background overlay
    spawn_ui_overlay(
        &mut commands,
        Vec2::new(res.game_width + 10., GAME_HEIGHT + 100.),
        1.,
        9.,
    );

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

    // Find first unlocked class for default selection
    let first_unlocked_class = class_options
        .iter()
        .find(|class| {
            achievements_ref
                .map(|a| is_class_unlocked(class, a))
                .unwrap_or_else(|| get_default_unlocked_classes().contains(class))
        })
        .cloned();

    // Initialize the selection state with first unlocked class
    commands.insert_resource(ClassSelectionState {
        selected_class: first_unlocked_class.clone(),
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

        // Check if class is unlocked
        let class_unlocked = achievements_ref
            .map(|a| is_class_unlocked(class, a))
            .unwrap_or_else(|| {
                // Default unlocked classes if no achievements resource
                use crate::player::achievements::get_default_unlocked_classes;
                get_default_unlocked_classes().contains(class)
            });

        // Class slot background
        let mut slot_entity_commands = commands.spawn((
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
                is_selected: first_unlocked_class.as_ref() == Some(class) && class_unlocked, // First unlocked class is selected by default
                class: class.clone(),
                is_locked: !class_unlocked,
            },
            RenderLayers::from_layers(&[3]),
            Name::new("CLASS OPTION"),
        ));

        // Only add Interactable component for unlocked classes
        if class_unlocked {
            slot_entity_commands.insert(super::Interactable::default());
        }

        let icon_slot = slot_entity_commands.id();

        // Class icon - show actual icon if unlocked, or grey locked version if locked
        let class_data = graphics.get_class_data(class.clone());
        let icon_texture = if class_unlocked {
            graphics
                .get_ui_element_texture(class_data.class_icon.clone())
                .clone()
        } else {
            // Use UnknownUnlockIcon for locked classes
            graphics
                .get_ui_element_texture(UIElement::UnknownUnlockIcon)
                .clone()
        };

        let mut icon_entity_commands = commands.spawn((
            SpriteBundle {
                texture: icon_texture,
                sprite: Sprite {
                    custom_size: Some(Vec2::new(22., 22.)),
                    // color: if class_unlocked { Color::WHITE } else { GREY },
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
            RenderLayers::from_layers(&[3]),
            Name::new("CLASS OPTION"),
        ));

        if class_unlocked {
            icon_entity_commands.insert(super::Interactable::default());
        }

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
    let confirm_button = commands
        .spawn(SpriteBundle {
            texture: graphics
                .get_ui_element_texture(UIElement::UpgradeButton)
                .clone(),
            sprite: Sprite {
                custom_size: Some(Vec2::new(60., 20.)),
                ..Default::default()
            },
            transform: Transform {
                translation: Vec3::new(80., -80., 11.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(UIState::ClassSelection)
        .insert(ClassSelectionUI)
        .insert(ConfirmButton)
        .insert(super::Interactable::default())
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("CONFIRM BUTTON"))
        .id();

    // Confirm button text
    let confirm_text = spawn_text(
        &mut commands,
        &asset_server,
        Vec3::new(0., 0., 1.),
        BLACK,
        "Confirm".to_string(),
        bevy::sprite::Anchor::Center,
        2.,
        3,
    );
    commands.entity(confirm_text).set_parent(confirm_button);

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

pub fn handle_class_selection(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut class_options: Query<(
        Entity,
        Option<&mut Interactable>,
        Option<&mut PlayerSelectSlot>,
        Option<&mut PetSelectSlot>,
        Option<&ConfirmButton>,
    )>,
    mut commands: Commands,
    mut next_ui_state: ResMut<NextState<UIState>>,
    mut next_game_state: ResMut<NextState<crate::GameState>>,
    mut selection_state: ResMut<ClassSelectionState>,
    res: Res<crate::ScreenResolution>,
    graphics: Res<Graphics>,
) {
    let hit_test = super::ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);

    // Handle class selection
    let mut _selected_class_entity: Option<Entity> = None;
    let mut _selected_pet_entity: Option<Entity> = None;

    for (e, interactable, mut class_option, mut pet_option, confirm_button_option) in
        class_options.iter_mut()
    {
        // Skip entities without Interactable component (locked classes)
        let Some(mut interactable) = interactable else {
            continue;
        };

        match hit_test {
            Some(hit_ent) if hit_ent.0 == e => match interactable.current() {
                Interaction::None => {
                    if let Some(slot) = class_option.as_mut() {
                        // Only allow hovering if class is not locked
                        if !slot.is_locked {
                            interactable.change(Interaction::Hovering);
                            slot.is_hovered = true;
                        }
                    } else if let Some(pet_state) = pet_option.as_mut() {
                        // Only allow hovering if pet is unlocked (has Interactable component)
                        interactable.change(Interaction::Hovering);
                        pet_state.is_hovered = true;
                    } else {
                        interactable.change(Interaction::Hovering);
                    }
                    if let Some(_) = confirm_button_option {
                        commands
                            .entity(e)
                            .insert(graphics.get_ui_element_texture(UIElement::UpgradeButtonHover));
                    }

                    commands.spawn(crate::audio::SoundSpawner::new(
                        crate::audio::AudioSoundEffect::ButtonHover,
                        0.15,
                    ));
                }
                Interaction::Hovering => {
                    if left_mouse_pressed {
                        if let Some(mut class_state) = class_option {
                            // Don't allow selection of locked classes
                            if class_state.is_locked {
                                info!("Class {:?} is locked!", class_state.class);
                                return;
                            }
                            info!("Class selected: {:?}", class_state.class);
                            // Update selection state
                            selection_state.selected_class = Some(class_state.class.clone());
                            class_state.is_selected = true;
                            _selected_class_entity = Some(e);
                        }
                        if let Some(mut pet_state) = pet_option {
                            // Pet selection is only allowed if pet is unlocked (has Interactable component)
                            info!("Pet selected: {:?}", pet_state.pet);
                            // Update selection state
                            selection_state.selected_pet = Some(pet_state.pet.clone());
                            pet_state.is_selected = true;
                            _selected_pet_entity = Some(e);
                        }
                        if let Some(_confirm_button) = confirm_button_option {
                            // Confirm button clicked
                            // Check if class is selected (pet is optional)
                            if let Some(class) = &selection_state.selected_class {
                                // Add PlayerClass component to the game
                                commands.insert_resource(PlayerClass {
                                    class: class.clone(),
                                    pets: selection_state.selected_pet.iter().cloned().collect(),
                                });

                                // Initialize game resources
                                commands.init_resource::<crate::Game>();
                                commands.init_resource::<NightTracker>();
                                commands.init_resource::<ChaosTracker>();
                                commands.init_resource::<HeirloomChoiceQueue>();
                                commands.init_resource::<ContainerRegistry>();
                                commands.init_resource::<PathfindingCache>();
                                commands.init_resource::<CraftingTracker>();
                                commands.init_resource::<EraManager>();

                                // Start the game with fade-in overlay
                                commands
                                    .spawn(SpriteBundle {
                                        sprite: Sprite {
                                            color: Color::rgba(0., 0., 0., 0.),
                                            custom_size: Some(Vec2::new(
                                                res.game_width + 10.,
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
                                    .insert(crate::ui::main_menu::GameStartFadein(
                                        Timer::from_seconds(3.0, TimerMode::Once),
                                    ));

                                // Close the class selection UI and transition to loading state
                                next_ui_state.set(UIState::Closed);
                                next_game_state.set(crate::GameState::Initializing);

                                // Despawn the class selection UI
                                commands.entity(e).despawn_recursive();
                            } else {
                                info!("Please select a class before confirming");
                            }
                        }
                    }
                }
                _ => (),
            },
            _ => {
                // Reset hovering state
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
                if let Some(_) = confirm_button_option {
                    commands
                        .entity(e)
                        .insert(graphics.get_ui_element_texture(UIElement::UpgradeButton));
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

pub fn update_info_card(
    _selection_state: Res<ClassSelectionState>,
    _info_cards: Query<&mut Children, With<ClassInfoCard>>,
    _commands: Commands,
    _asset_server: Res<AssetServer>,
) {
    // TODO: Implement info card updates
    // For now, this is a placeholder to avoid compilation errors
}
