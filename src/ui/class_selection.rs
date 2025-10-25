use bevy::prelude::*;
use bevy_aseprite::{anim::AsepriteAnimation, AsepriteBundle};

use crate::{
    ai::pathfinding::PathfindingCache,
    animations::player_sprite::{
        PlayerBlueAseprite, PlayerGreenAseprite, PlayerGreyAseprite, PlayerRedAseprite,
    },
    assets::Graphics,
    colors::BLACK,
    container::ContainerRegistry,
    inputs::CursorPos,
    item::CraftingTracker,
    night::NightTracker,
    player::skills::{HeirloomChoiceQueue, PlayerClass, SkillClass},
    ui::{UIElement, UIState},
    EraManager, RenderLayers, ScreenResolution, GAME_HEIGHT,
};

use super::{
    damage_numbers::spawn_text,
    interactions::{Interactable, Interaction},
    ui_helpers::spawn_ui_overlay,
};

#[derive(Component)]
pub struct ClassSelectionUI;

#[derive(Component)]
pub struct ClassOption {
    pub class: SkillClass,
}

#[derive(Component)]
pub struct ClassPreview;

pub fn setup_class_selection_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    res: Res<ScreenResolution>,
) {
    // Background overlay
    spawn_ui_overlay(
        &mut commands,
        Vec2::new(res.game_width + 10., GAME_HEIGHT + 100.),
        0.9,
        9.,
    );

    // Title
    let title_text = spawn_text(
        &mut commands,
        &asset_server,
        Vec3::new(0., 80., 10.),
        BLACK,
        "Choose Your Class".to_string(),
        bevy::sprite::Anchor::Center,
        3.,
        3,
    );
    commands
        .entity(title_text)
        .insert(UIState::ClassSelection)
        .insert(ClassSelectionUI);

    // Class options
    let class_options = [
        (SkillClass::Melee, "Warrior", "High health and attack."),
        (SkillClass::Rogue, "Rogue", "High speed and dodge."),
        (SkillClass::Magic, "Mage", "Mana and Mana regen."),
        (SkillClass::Thief, "Thief", "Crit and Crit Chance."),
    ];

    for (i, (class, name, description)) in class_options.iter().enumerate() {
        let x_offset = (i as f32 - 1.0) * 120.0; // Center the options

        // Class option background`
        let option_bg = commands
            .spawn(SpriteBundle {
                texture: graphics
                    .get_ui_element_texture(UIElement::LargeTooltipCommon)
                    .clone(),
                sprite: Sprite {
                    custom_size: Some(Vec2::new(100., 120.)),
                    ..Default::default()
                },
                transform: Transform {
                    translation: Vec3::new(x_offset, 0., 10.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..Default::default()
            })
            .insert(UIState::ClassSelection)
            .insert(ClassSelectionUI)
            .insert(ClassOption {
                class: class.clone(),
            })
            .insert(super::Interactable::default())
            .insert(RenderLayers::from_layers(&[3]))
            .insert(Name::new("CLASS OPTION"))
            .id();

        // Class preview sprite
        let (aseprite_path, idle_tag) = match class {
            SkillClass::Melee => (PlayerRedAseprite::PATH, PlayerRedAseprite::tags::IDLE_FRONT),
            SkillClass::Rogue => (
                PlayerGreenAseprite::PATH,
                PlayerGreenAseprite::tags::IDLE_FRONT,
            ),
            SkillClass::Magic => (
                PlayerBlueAseprite::PATH,
                PlayerBlueAseprite::tags::IDLE_FRONT,
            ),
            SkillClass::Thief => (
                PlayerGreyAseprite::PATH,
                PlayerGreyAseprite::tags::IDLE_FRONT,
            ),
            _ => continue,
        };

        commands
            .spawn(AsepriteBundle {
                aseprite: asset_server.load(aseprite_path),
                animation: AsepriteAnimation::from(idle_tag),
                transform: Transform {
                    translation: Vec3::new(0., 20., 1.),
                    scale: Vec3::new(2., 2., 1.),
                    ..Default::default()
                },
                ..Default::default()
            })
            .insert(UIState::ClassSelection)
            .insert(ClassPreview)
            .insert(RenderLayers::from_layers(&[3]))
            .set_parent(option_bg);

        // Class name
        let name_text = spawn_text(
            &mut commands,
            &asset_server,
            Vec3::new(0., -20., 1.),
            BLACK,
            name.to_string(),
            bevy::sprite::Anchor::Center,
            2.,
            3,
        );
        commands.entity(name_text).set_parent(option_bg);

        // Class description
        let desc_text = spawn_text(
            &mut commands,
            &asset_server,
            Vec3::new(0., -40., 1.),
            BLACK,
            description.to_string(),
            bevy::sprite::Anchor::Center,
            1.,
            3,
        );
        commands.entity(desc_text).set_parent(option_bg);
    }
}

pub fn handle_class_selection(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut class_options: Query<(Entity, &mut Interactable, &ClassOption)>,
    mut commands: Commands,
    mut next_ui_state: ResMut<NextState<UIState>>,
    mut next_game_state: ResMut<NextState<crate::GameState>>,
    res: Res<crate::ScreenResolution>,
    _graphics: Res<Graphics>,
    _asset_server: Res<AssetServer>,
) {
    let hit_test = super::ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);

    for (e, mut interactable, class_option) in class_options.iter_mut() {
        match hit_test {
            Some(hit_ent) if hit_ent.0 == e => match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                    commands.spawn(crate::audio::SoundSpawner::new(
                        crate::audio::AudioSoundEffect::ButtonHover,
                        0.25,
                    ));
                }
                Interaction::Hovering => {
                    if left_mouse_pressed {
                        info!("Class selected: {:?}", class_option.class);

                        // Add PlayerClass component to the game
                        commands.insert_resource(PlayerClass {
                            class: class_option.class.clone(),
                        });

                        // Initialize game resources
                        commands.init_resource::<crate::Game>();
                        commands.init_resource::<NightTracker>();
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
                            .insert(crate::ui::main_menu::GameStartFadein(Timer::from_seconds(
                                3.0,
                                TimerMode::Once,
                            )));

                        // Close the class selection UI and start the game
                        next_ui_state.set(UIState::Closed);
                        next_game_state.set(crate::GameState::Main);

                        // Despawn the class selection UI
                        commands.entity(e).despawn_recursive();
                    }
                }
                _ => (),
            },
            _ => {
                // reset hovering states if we stop hovering
                let Interaction::Hovering = interactable.current() else {
                    continue;
                };

                interactable.change(Interaction::None);
            }
        }
    }
}
