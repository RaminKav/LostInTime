use bevy::{prelude::*, render::view::RenderLayers};
use bevy_aseprite::aseprite;
use bevy_proto::backend::schematics::{ReflectSchematic, Schematic};
use itertools::Itertools;
use rand::seq::SliceRandom;
use strum::IntoEnumIterator;

use crate::{
    assets::Graphics,
    attributes::{attribute_helpers::create_new_random_item_stack_with_attributes, ItemRarity},
    inventory::ItemStack,
    item::WorldObject,
    player::{ModifyTimeFragmentsEvent, Player, TimeFragmentCurrency},
    proto::proto_param::ProtoParam,
    ui::key_input_guide::InteractionGuideTrigger,
    GameParam, ScreenResolution, GAME_HEIGHT,
};

use super::{
    spawn_item_stack_icon, ui_helpers::spawn_ui_overlay, Interactable, UIElement, UIState,
    ESSENCE_UI_SIZE,
};

#[derive(Component)]
pub struct EssenceUI;

#[derive(Component, Clone, Debug, Resource, Reflect, FromReflect, Schematic, Default)]
#[reflect(Component, Schematic)]
pub struct EssenceOption {
    pub item: ItemStack,
    pub cost: u32,
}

impl EssenceOption {
    fn get_obj(&self) -> WorldObject {
        self.item.obj_type
    }
}
#[derive(Debug)]
pub struct SubmitEssenceChoice {
    pub choice: EssenceOption,
}

#[derive(Resource, Component, Clone, Reflect, FromReflect, Schematic, Default)]
#[reflect(Component, Schematic)]
pub struct EssenceShopChoices {
    pub choices: Vec<EssenceOption>,
    pub owner_entity: Option<Entity>,
}
aseprite!(pub BlacksmithMerchant, "textures/blacksmith.ase");

pub fn setup_essence_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    shop: Res<EssenceShopChoices>,
    resolution: Res<ScreenResolution>,
) {
    let (size, texture, t_offset) = (
        ESSENCE_UI_SIZE,
        graphics.get_ui_element_texture(UIElement::Essence),
        Vec2::new(3.5, 3.5),
    );

    let overlay = spawn_ui_overlay(
        &mut commands,
        Vec2::new(resolution.game_width + 10., GAME_HEIGHT + 20.),
        0.8,
        -1.,
    );

    let essence_ui_e = commands
        .spawn(SpriteBundle {
            texture,
            sprite: Sprite {
                custom_size: Some(size),
                ..Default::default()
            },
            transform: Transform {
                translation: Vec3::new(t_offset.x, t_offset.y, 10.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(EssenceUI)
        .insert(Name::new("SHOP UI"))
        .insert(UIState::Essence)
        .insert(RenderLayers::from_layers(&[3]))
        .id();

    for (i, essence_option) in shop.choices.iter().enumerate() {
        let translation = Vec3::new(24.5, 40.5 - (i as f32 * 29.), 1.);
        let slot_entity = commands
            .spawn((
                SpriteBundle {
                    texture: graphics.get_ui_element_texture(UIElement::EssenceButton),
                    transform: Transform {
                        translation,
                        scale: Vec3::new(1., 1., 1.),
                        ..Default::default()
                    },
                    sprite: Sprite {
                        custom_size: Some(Vec2::new(20., 20.)),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                Interactable::default(),
                UIElement::EssenceButton,
                essence_option.clone(),
                RenderLayers::from_layers(&[3]),
                Name::new("Essence Loot Button"),
            ))
            .set_parent(essence_ui_e)
            .id();

        // icon
        let icon = spawn_item_stack_icon(
            &mut commands,
            &graphics,
            &essence_option.item,
            &asset_server,
            Vec2::ZERO,
            Vec2::new(0., 0.),
            3,
        );
        commands.entity(icon).set_parent(slot_entity);

        let slot_entity = commands
            .spawn((
                SpriteBundle {
                    texture: graphics.get_ui_element_texture(UIElement::EssenceButton),
                    transform: Transform {
                        translation: translation + Vec3::new(-49., 0., 0.),
                        scale: Vec3::new(1., 1., 1.),
                        ..Default::default()
                    },
                    sprite: Sprite {
                        custom_size: Some(Vec2::new(20., 20.)),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                // Interactable::default(),
                UIElement::EssenceButton,
                RenderLayers::from_layers(&[3]),
                Name::new("Essence Cost Button"),
            ))
            .set_parent(essence_ui_e)
            .id();

        // cost icon
        let cost_icon = spawn_item_stack_icon(
            &mut commands,
            &graphics,
            &ItemStack::crate_icon_stack(WorldObject::TimeFragment)
                .copy_with_count(essence_option.cost as usize),
            &asset_server,
            Vec2::ZERO,
            Vec2::new(0., 0.),
            3,
        );
        commands.entity(cost_icon).set_parent(slot_entity);
    }

    commands.entity(essence_ui_e).push_children(&[overlay]);
}

pub fn handle_submit_essence_choice(
    mut commands: Commands,
    mut ev: EventReader<SubmitEssenceChoice>,
    mut next_inv_state: ResMut<NextState<UIState>>,
    essence_ui: Query<Entity, With<EssenceUI>>,
    currency: Query<&TimeFragmentCurrency>,
    mut currency_event: EventWriter<ModifyTimeFragmentsEvent>,
    mut game_param: GameParam,
    player_t: Query<&GlobalTransform, With<Player>>,
    shop: Res<EssenceShopChoices>,
) {
    for choice in ev.iter() {
        let cur = currency.single();
        if cur.time_fragments >= choice.choice.cost as i32 {
            currency_event.send(ModifyTimeFragmentsEvent {
                delta: -(choice.choice.cost as i32),
            });

            choice.choice.item.spawn_as_drop(
                &mut commands,
                &mut game_param,
                player_t.single().translation().truncate(),
            );

            next_inv_state.set(UIState::Closed);
            commands.remove_resource::<EssenceShopChoices>();
            if let Ok(e) = essence_ui.get_single() {
                commands.entity(e).despawn_recursive();
            }
            if let Some(owner_e) = shop.owner_entity {
                commands
                    .entity(owner_e)
                    .remove::<EssenceShopChoices>()
                    .remove::<InteractionGuideTrigger>();
            }
        }
    }
}

pub fn handle_populate_essence_shop_on_new_spawn(
    mut new_spawns: Query<(Entity, &mut EssenceShopChoices), Added<EssenceShopChoices>>,
    proto_param: ProtoParam,
    mut commands: Commands,
    game: GameParam,
) {
    for (entity, mut shop) in new_spawns.iter_mut() {
        let mut GENERIC_SHOP_OPTIONS = vec![
            EssenceOption {
                item: create_new_random_item_stack_with_attributes(
                    &ItemStack::crate_icon_stack(WorldObject::LargePotion).copy_with_count(3),
                    &proto_param,
                    &mut commands,
                ),
                cost: 5,
            },
            EssenceOption {
                item: create_new_random_item_stack_with_attributes(
                    &ItemStack::crate_icon_stack(WorldObject::OrbOfTransformation)
                        .copy_with_count(1),
                    &proto_param,
                    &mut commands,
                ),
                cost: 5,
            },
            EssenceOption {
                item: create_new_random_item_stack_with_attributes(
                    &ItemStack::crate_icon_stack(WorldObject::UpgradeTome).copy_with_count(1),
                    &proto_param,
                    &mut commands,
                ),
                cost: 3,
            },
            EssenceOption {
                item: create_new_random_item_stack_with_attributes(
                    &ItemStack::crate_icon_stack(WorldObject::Key).copy_with_count(1),
                    &proto_param,
                    &mut commands,
                ),
                cost: 10,
            },
        ];

        let mut shop_choices = vec![];
        let mut rng = rand::thread_rng();
        // Read the JSON contents of the file as an instance of `User`.

        // if let Some(seen_item) = data
        //     .seen_gear
        //     .iter()
        //     .filter(|i| i.metadata.level.unwrap_or(1) <= player_level)
        //     .choose(&mut rng)
        // {
        //     shop_choices.push(EssenceOption {
        //         item: seen_item.clone(),
        //         cost: seen_item.metadata.level.unwrap_or(1) as u32 + 2,
        //     });
        //     while shop_choices.len() < 4 {
        //         let pick = GENERIC_SHOP_OPTIONS
        //             .iter()
        //             .choose(&mut rng)
        //             .unwrap()
        //             .clone();
        //         shop_choices.push(pick.clone());
        //         GENERIC_SHOP_OPTIONS.retain(|x| x.get_obj() != pick.get_obj());
        //     }
        // } else {
        //     shop_choices = GENERIC_SHOP_OPTIONS;
        // }
        while shop_choices.len() < 4 {
            let filtered_items = WorldObject::iter()
                .filter(|obj| obj.is_weapon() || obj.is_armor() || obj.is_accessory())
                .collect_vec();
            let pick_new_item = filtered_items.choose(&mut rng).expect("No items found");
            let mut stack = proto_param
                .get_item_data(pick_new_item.clone())
                .unwrap()
                .clone();
            stack.metadata.level = Some(game.get_player_level());

            let random_item_stack =
                create_new_random_item_stack_with_attributes(&stack, &proto_param, &mut commands);
            let rarity_cost_inc = match random_item_stack.rarity {
                ItemRarity::Common => 0,
                ItemRarity::Uncommon => 2,
                ItemRarity::Rare => 3,
                ItemRarity::Legendary => 8,
            };
            shop_choices.push(EssenceOption {
                item: random_item_stack.clone(),
                cost: random_item_stack.metadata.level.unwrap_or(1) as u32 + 1 + rarity_cost_inc,
            });
        }
        shop.choices = shop_choices;
        shop.owner_entity = Some(entity);
    }
}
