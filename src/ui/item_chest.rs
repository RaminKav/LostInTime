use bevy::{prelude::*, render::view::RenderLayers};
use bevy_aseprite::aseprite;
use itertools::Itertools;
use rand::seq::SliceRandom;
use strum::IntoEnumIterator;

use crate::{
    assets::Graphics,
    attributes::{
        attribute_helpers::{create_new_random_item_stack_with_attributes, spawn_rarity_animation},
        ItemRarity,
    },
    inventory::ItemStack,
    item::WorldObject,
    juice::bounce::BounceOnHit,
    player::skills::SkillChoiceQueue,
    proto::proto_param::ProtoParam,
    GameParam, ScreenResolution, GAME_HEIGHT,
};

use super::{
    ui_helpers::spawn_ui_overlay, Interactable, UIElement, UIState, SKILLS_CHOICE_UI_SIZE,
};

aseprite!(pub SkillChoiceFlash, "ui/SkillChoiceFlash.aseprite");

#[derive(Resource, Debug, Clone)]
pub struct ItemChestState {
    pub shuffle_timer: Timer,
    pub shuffle_duration_timer: Timer,
    pub picked_item: Option<ItemStack>,
    pub current_entity: Option<Entity>,
    pub current_item: Option<WorldObject>,
    pub state: ItemChestAnimState,
    pub current_ui_rarity: ItemRarity,
    // pub owner_chest_entity: Entity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemChestAnimState {
    Closed,
    Opening,
    Done,
}

#[derive(Component)]
pub struct ItemChest;

#[derive(Component)]
pub struct ItemChestButton;

pub struct ItemChestAnimChangeEvent {
    pub state: ItemChestAnimState,
    pub set_ui_rarity: Option<ItemRarity>,
}

pub fn setup_item_chest_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    choices_queue: Res<SkillChoiceQueue>,
    mut next_ui_state: ResMut<NextState<UIState>>,
    res: Res<ScreenResolution>,
) {
    // // title bar
    // let title_sprite = commands
    //     .spawn(SpriteBundle {
    //         texture: graphics.get_ui_element_texture(UIElement::TitleBar).clone(),
    //         sprite: Sprite {
    //             custom_size: Some(Vec2::new(168., 16.)),
    //             ..Default::default()
    //         },
    //         transform: Transform {
    //             translation: Vec3::new(0., 80., 10.),
    //             scale: Vec3::new(1., 1., 1.),
    //             ..Default::default()
    //         },
    //         ..Default::default()
    //     })
    //     .insert(RenderLayers::from_layers(&[3]))
    //     .insert(UIElement::TitleBar)
    //     .insert(UIState::Skills)
    //     .insert(Name::new("SKILL ICON!!"))
    //     .id();

    // let title_text = spawn_text(
    //     &mut commands,
    //     &asset_server,
    //     Vec3::new(0., 0., 1.),
    //     BLACK,
    //     "Choose a new skill".to_string(),
    //     Anchor::Center,
    //     2.,
    //     3,
    // );
    // commands
    //     .entity(title_text)
    //     .insert(UIState::Skills)
    //     .set_parent(title_sprite);

    spawn_ui_overlay(
        &mut commands,
        Vec2::new(res.game_width + 10., GAME_HEIGHT + 100.),
        0.8,
        9.,
    );

    let ui_element = UIElement::SkillChoice;
    let size = SKILLS_CHOICE_UI_SIZE;

    let item_chest_ui = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(ui_element.clone()),
            sprite: Sprite {
                custom_size: Some(size),
                ..Default::default()
            },
            transform: Transform {
                translation: Vec3::new(0., 0., 10.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(ui_element)
        .insert(UIState::ItemChest)
        // .insert(Interactable::default())
        .insert(Name::new("ITEM CHEST"))
        .insert(RenderLayers::from_layers(&[3]))
        .id();

    // icon
    commands
        .spawn(SpriteSheetBundle {
            sprite: graphics
                .spritesheet_map
                .as_ref()
                .unwrap()
                .get(&WorldObject::Chest)
                .unwrap()
                .clone(),
            texture_atlas: graphics.texture_atlas.as_ref().unwrap().clone(),

            transform: Transform {
                translation: Vec2::new(0., 18.).extend(4.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(ItemChest)
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("SKILL ICON!!"))
        .set_parent(item_chest_ui);

    let upgrade_button = commands
        .spawn(SpriteBundle {
            texture: graphics
                .get_ui_element_texture(UIElement::UpgradeButton)
                .clone(),
            sprite: Sprite {
                custom_size: Some(Vec2::new(13., 13.)),
                ..Default::default()
            },
            transform: Transform {
                translation: Vec3::new(0.5, -36.5, 10.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIElement::UpgradeButton)
        .insert(Interactable::default())
        .insert(ItemChestButton)
        .insert(UIState::Inventory)
        .insert(Name::new("UPGRADE BUTTON"))
        .id();
    commands
        .entity(item_chest_ui)
        .push_children(&[upgrade_button]);
}

pub fn toggle_item_chest_visibility(
    mut next_inv_state: ResMut<NextState<UIState>>,
    curr_ui_state: Res<State<UIState>>,
    key_input: ResMut<Input<KeyCode>>,
    mut commands: Commands,
) {
    if curr_ui_state.0 == UIState::ActiveSkills {
        return;
    }
    next_inv_state.set(UIState::ItemChest);
}

pub fn shuffle_items(
    mut item_chest_state: ResMut<ItemChestState>,
    time: Res<Time>,
    mut commands: Commands,
    graphics: Res<Graphics>,
    mut events: EventWriter<ItemChestAnimChangeEvent>,
) {
    if item_chest_state.state != ItemChestAnimState::Opening {
        return;
    }
    item_chest_state.shuffle_timer.tick(time.delta());
    item_chest_state.shuffle_duration_timer.tick(time.delta());
    if item_chest_state.shuffle_duration_timer.just_finished() {
        // events.send(ItemChestAnimChangeEvent(ItemChestAnimState::Done));
        if let Some(current_entity) = item_chest_state.current_entity {
            commands.entity(current_entity).despawn();
            item_chest_state.current_entity = None;
            events.send(ItemChestAnimChangeEvent {
                state: ItemChestAnimState::Done,
                set_ui_rarity: None,
            });
        }
        return;
    }
    let picked_rarity = item_chest_state.picked_item.clone().unwrap().rarity;
    if item_chest_state.shuffle_duration_timer.percent() >= 0.25
        && item_chest_state.current_ui_rarity == ItemRarity::Common
        && picked_rarity != ItemRarity::Common
    {
        item_chest_state.current_ui_rarity = ItemRarity::Uncommon;

        events.send(ItemChestAnimChangeEvent {
            state: ItemChestAnimState::Opening,
            set_ui_rarity: Some(ItemRarity::Uncommon),
        });
    } else if item_chest_state.shuffle_duration_timer.percent() >= 0.48
        && item_chest_state.current_ui_rarity == ItemRarity::Uncommon
        && picked_rarity != ItemRarity::Uncommon
    {
        item_chest_state.current_ui_rarity = ItemRarity::Rare;
        events.send(ItemChestAnimChangeEvent {
            state: ItemChestAnimState::Opening,
            set_ui_rarity: Some(ItemRarity::Rare),
        });
    } else if item_chest_state.shuffle_duration_timer.percent() >= 0.7
        && item_chest_state.current_ui_rarity == ItemRarity::Rare
        && picked_rarity != ItemRarity::Rare
    {
        item_chest_state.current_ui_rarity = ItemRarity::Legendary;
        events.send(ItemChestAnimChangeEvent {
            state: ItemChestAnimState::Opening,
            set_ui_rarity: Some(ItemRarity::Legendary),
        });
    }
    if item_chest_state.shuffle_timer.finished()
        && !item_chest_state.shuffle_duration_timer.finished()
    {
        if let Some(current_entity) = item_chest_state.current_entity {
            commands.entity(current_entity).despawn();
        }
        let mut rng = rand::thread_rng();
        let filtered_items = WorldObject::iter()
            .filter(|obj| {
                item_chest_state
                    .current_item
                    .map(|old_item| old_item != *obj)
                    .unwrap_or(true)
                    && (obj.is_weapon() || obj.is_armor() || obj.is_accessory())
            })
            .collect_vec();
        let pick_new_item = filtered_items.choose(&mut rng).expect("No items found");
        let icon = commands
            .spawn(SpriteSheetBundle {
                sprite: graphics
                    .spritesheet_map
                    .as_ref()
                    .unwrap()
                    .get(&pick_new_item.clone())
                    .unwrap()
                    .clone(),
                texture_atlas: graphics.texture_atlas.as_ref().unwrap().clone(),

                transform: Transform {
                    translation: Vec3::new(0., 25., 15.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..Default::default()
            })
            .insert(UIState::ItemChest)
            .insert(RenderLayers::from_layers(&[3]))
            .insert(Name::new("Chest Icon!!"))
            .id();
        item_chest_state.current_entity = Some(icon);
        item_chest_state.shuffle_timer.reset();
        item_chest_state.current_item = Some(pick_new_item.clone());
    }
}

pub fn handle_anim_events(
    mut commands: Commands,
    mut events: EventReader<ItemChestAnimChangeEvent>,
    mut item_chest_state: ResMut<ItemChestState>,
    query: Query<Entity, With<ItemChest>>,
    graphics: Res<Graphics>,
    proto: ProtoParam,
    mut game: GameParam,
    asset_server: Res<AssetServer>,
) {
    for event in events.iter() {
        match event.state {
            ItemChestAnimState::Opening => {
                //pick random item stack
                if item_chest_state.picked_item.is_none() {
                    let mut rng = rand::thread_rng();
                    let filtered_items = WorldObject::iter()
                        .filter(|obj| obj.is_weapon() || obj.is_armor() || obj.is_accessory())
                        .collect_vec();
                    let pick_new_item = filtered_items.choose(&mut rng).expect("No items found");
                    let mut stack = proto.get_item_data(pick_new_item.clone()).unwrap().clone();
                    stack.metadata.level = Some(game.get_player_level());

                    item_chest_state.picked_item = Some(
                        create_new_random_item_stack_with_attributes(&stack, &proto, &mut commands),
                    );
                }
                // handle opening animation
                item_chest_state.state = ItemChestAnimState::Opening;
                for entity in query.iter() {
                    commands.entity(entity).despawn_recursive();
                }
                let rarity = event.set_ui_rarity.clone().unwrap_or(ItemRarity::Common);
                spawn_rarity_animation(
                    rarity.clone(),
                    &mut commands,
                    &asset_server,
                    Vec3::new(0., 25., 16.),
                );
                let ui_element = match rarity {
                    ItemRarity::Common => UIElement::ItemChestOpeningCommon,
                    ItemRarity::Uncommon => UIElement::ItemChestOpeningUncommon,
                    ItemRarity::Rare => UIElement::ItemChestOpeningRare,
                    ItemRarity::Legendary => UIElement::ItemChestOpeningLegendary,
                };
                commands
                    .spawn(SpriteBundle {
                        texture: graphics.get_ui_element_texture(ui_element.clone()),
                        sprite: Sprite {
                            custom_size: Some(Vec2::splat(32.)),
                            ..Default::default()
                        },
                        transform: Transform {
                            translation: Vec3::new(0., 26., 14.),
                            scale: Vec3::new(1., 1., 1.),
                            ..Default::default()
                        },
                        ..Default::default()
                    })
                    .insert(ui_element)
                    .insert(ItemChest)
                    .insert(UIState::ItemChest)
                    .insert(BounceOnHit::new())
                    // .insert(Interactable::default())
                    .insert(Name::new("ITEM CHEST"))
                    .insert(RenderLayers::from_layers(&[3]));
            }
            ItemChestAnimState::Done => {
                item_chest_state.state = ItemChestAnimState::Done;

                if let Some(current_entity) = item_chest_state.current_entity {
                    commands.entity(current_entity).despawn();
                    item_chest_state.current_entity = None;
                }
                commands
                    .spawn(SpriteSheetBundle {
                        sprite: graphics
                            .spritesheet_map
                            .as_ref()
                            .unwrap()
                            .get(&item_chest_state.picked_item.clone().unwrap().obj_type)
                            .unwrap()
                            .clone(),
                        texture_atlas: graphics.texture_atlas.as_ref().unwrap().clone(),

                        transform: Transform {
                            translation: Vec3::new(0., 25., 15.),
                            scale: Vec3::new(1., 1., 1.),
                            ..Default::default()
                        },
                        ..Default::default()
                    })
                    .insert(UIState::ItemChest)
                    .insert(RenderLayers::from_layers(&[3]))
                    .insert(Name::new("Chest Icon!!"));
                // handle done animation
            }
            ItemChestAnimState::Closed => {

                // handle closed state
            }
        }
    }
}
// pub fn spawn_skill_choice_flash(
//     commands: &mut Commands,
//     asset_server: &AssetServer,
//     pos: Vec3,
//     slot: usize,
// ) {
//     commands
//         .spawn(AsepriteBundle {
//             animation: AsepriteAnimation::from(SkillChoiceFlash::tags::FLASH),
//             aseprite: asset_server.load(SkillChoiceFlash::PATH),
//             transform: Transform {
//                 translation: pos,
//                 ..Default::default()
//             },
//             ..Default::default()
//         })
//         .insert(RenderLayers::from_layers(&[3]))
//         .insert(VisibilityBundle::default())
//         .insert(RerollDice(slot))
//         .insert(DoneAnimation);
// }
