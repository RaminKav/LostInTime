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
    player::{ModifyCurencyEvent, Player, TimeFragmentCurrency},
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
    pub time_fragment_cost: u32,
    pub coin_cost: u32,
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
        let y_offset = 38.5 - (i as f32 * 38.) + if i == 2 { -1. } else { 0. };
        let translation = Vec3::new(-46.5, y_offset, 1.);
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

        let name = commands
            .spawn((
                Text2dBundle {
                    text: Text::from_section(
                        format!("{}", essence_option.item.metadata.name),
                        TextStyle {
                            font: asset_server.load("fonts/4x5.ttf"),
                            font_size: 5.0,
                            color: essence_option.item.rarity.get_color(),
                        },
                    )
                    .with_alignment(TextAlignment::Left),
                    text_anchor: bevy::sprite::Anchor::CenterLeft,
                    transform: Transform::from_translation(Vec3::new(-30.5, y_offset, 1.)),
                    ..Default::default()
                },
                RenderLayers::from_layers(&[3]),
                Name::new("Unlocks Item Name"),
            ))
            .set_parent(essence_ui_e)
            .id();

        // let slot_entity = commands
        //     .spawn((
        //         SpriteBundle {
        //             texture: graphics.get_ui_element_texture(UIElement::EssenceButton),
        //             transform: Transform {
        //                 translation: translation + Vec3::new(-49., 0., 0.),
        //                 scale: Vec3::new(1., 1., 1.),
        //                 ..Default::default()
        //             },
        //             sprite: Sprite {
        //                 custom_size: Some(Vec2::new(20., 20.)),
        //                 ..Default::default()
        //             },
        //             ..Default::default()
        //         },
        //         // Interactable::default(),
        //         UIElement::EssenceButton,
        //         RenderLayers::from_layers(&[3]),
        //         Name::new("Essence Cost Button"),
        //     ))
        //     .set_parent(essence_ui_e)
        //     .id();

        // cost icon
        let coin_cost = spawn_item_stack_icon(
            &mut commands,
            &graphics,
            &ItemStack::crate_icon_stack(WorldObject::Coin)
                .copy_with_count(essence_option.coin_cost as usize),
            &asset_server,
            Vec2::new(50.5, y_offset - 6.),
            Vec2::new(0., 0.),
            3,
        );
        commands.entity(coin_cost).set_parent(essence_ui_e);
        if essence_option.time_fragment_cost > 0 {
            let time_fragment_cost = spawn_item_stack_icon(
                &mut commands,
                &graphics,
                &ItemStack::crate_icon_stack(WorldObject::TimeFragment)
                    .copy_with_count(essence_option.time_fragment_cost as usize),
                &asset_server,
                Vec2::new(50.5, y_offset + 7.),
                Vec2::new(0., 0.),
                3,
            );
            commands.entity(time_fragment_cost).set_parent(essence_ui_e);
        }
    }

    commands.entity(essence_ui_e).push_children(&[overlay]);
}

pub fn handle_submit_essence_choice(
    mut commands: Commands,
    mut ev: EventReader<SubmitEssenceChoice>,
    mut next_inv_state: ResMut<NextState<UIState>>,
    essence_ui: Query<Entity, With<EssenceUI>>,
    mut currency_event: EventWriter<ModifyCurencyEvent>,
    mut game_param: GameParam,
    player_t: Query<&GlobalTransform, With<Player>>,
    shop: Res<EssenceShopChoices>,
) {
    for choice in ev.iter() {
        let time_fragments = game_param.get_time_fragments();
        if time_fragments >= choice.choice.time_fragment_cost as i32
            && game_param.get_coins() >= choice.choice.coin_cost
        {
            currency_event.send(ModifyCurencyEvent {
                delta: -(choice.choice.time_fragment_cost as i32),
                obj: WorldObject::TimeFragment,
            });
            currency_event.send(ModifyCurencyEvent {
                delta: -(choice.choice.coin_cost as i32),
                obj: WorldObject::Coin,
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
                    .insert(WorldObject::BlacksmithMerchantDone)
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
        let mut shop_choices = vec![];
        let mut rng = rand::thread_rng();

        while shop_choices.len() < 3 {
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
                ItemRarity::Common => 1.,
                ItemRarity::Uncommon => 1.2,
                ItemRarity::Rare => 1.6,
                ItemRarity::Legendary => 2.5,
            };
            let time_frag_cost = if random_item_stack.rarity == ItemRarity::Rare {
                1
            } else if random_item_stack.rarity == ItemRarity::Legendary {
                3
            } else {
                0
            };

            shop_choices.push(EssenceOption {
                item: random_item_stack.clone(),
                coin_cost: ((random_item_stack.metadata.level.unwrap_or(1) as f32 * 5. + 7.)
                    * rarity_cost_inc)
                    .trunc() as u32,
                time_fragment_cost: time_frag_cost,
            });
        }
        shop.choices = shop_choices;
        shop.owner_entity = Some(entity);
    }
}
