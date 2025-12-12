use bevy::{ecs::system::ParamSet, prelude::*, render::view::RenderLayers};
use bevy_aseprite::aseprite;
use rand::{seq::SliceRandom, Rng};
use strum::IntoEnumIterator;

use crate::{
    assets::Graphics,
    attributes::AttributeChangeEvent,
    inventory::ItemStack,
    item::WorldObject,
    player::{
        currency::{CoinCurrency, TimeFragmentCurrency},
        skills::{Heirloom, HeirloomRarity, HeirloomWithRarity, PlayerSkills},
        ModifyCurencyEvent, Player,
    },
    proto::proto_param::ProtoParam,
    ui::key_input_guide::InteractionGuideTrigger,
    GameParam, ScreenResolution, GAME_HEIGHT,
};

/// Resource to track how many purchases the player has made at blacksmith merchants
/// This determines price scaling (25% increase per purchase)
#[derive(Resource, Default, Debug, Clone)]
pub struct BlacksmithPurchaseTracker {
    pub purchases_made: u32,
}

impl BlacksmithPurchaseTracker {
    /// Get the price multiplier based on purchases made
    /// Each purchase increases prices by 25%
    pub fn get_price_multiplier(&self) -> f32 {
        1.75_f32.powi(self.purchases_made as i32)
    }
}

/// Reset the blacksmith purchase tracker on new run
pub fn reset_blacksmith_tracker(mut tracker: ResMut<BlacksmithPurchaseTracker>) {
    info!(
        "Resetting blacksmith purchase tracker from {} purchases",
        tracker.purchases_made
    );
    tracker.purchases_made = 0;
}

use super::skill_choice_ui::spawn_heirloom_tooltip_card;

use super::{
    spawn_item_stack_icon, ui_helpers::spawn_ui_overlay, Interactable, UIElement, UIState,
    ESSENCE_UI_SIZE,
};

#[derive(Component)]
pub struct EssenceUI;

#[derive(Component)]
pub struct EssenceTooltipCard;

#[derive(Component, Clone, Debug, Resource, Default)]
pub struct EssenceOption {
    pub heirloom: Heirloom,
    pub rarity: HeirloomRarity,
    pub time_fragment_cost: u32,
    pub coin_cost: u32,
}

impl EssenceOption {
    fn get_heirloom(&self) -> Heirloom {
        self.heirloom.clone()
    }

    fn get_rarity(&self) -> HeirloomRarity {
        self.rarity.clone()
    }
}
#[derive(Debug)]
pub struct SubmitEssenceChoice {
    pub choice: EssenceOption,
}

#[derive(Resource, Component, Clone, Default)]
pub struct EssenceShopChoices {
    pub choices: Vec<EssenceOption>,
    pub owner_entity: Option<Entity>,
    pub tile_pos: Option<crate::world::TileMapPosition>,
}
aseprite!(pub BlacksmithMerchant, "textures/blacksmith.ase");

/// System to handle spawning/despawning tooltip cards when hovering over heirlooms
pub fn handle_essence_heirloom_tooltip(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    essence_options: Query<(&EssenceOption, &super::interactions::Interactable)>,
    existing_tooltips: Query<Entity, With<EssenceTooltipCard>>,
    mut last_hovered: Local<Option<Heirloom>>,
) {
    use super::interactions::Interaction;

    // Find the currently hovered heirloom
    let currently_hovered = essence_options
        .iter()
        .find(|(_, interactable)| matches!(interactable.current(), Interaction::Hovering))
        .map(|(option, _)| option.get_heirloom());

    // Only update if the hover state changed
    if *last_hovered == currently_hovered {
        return;
    }

    // Despawn all existing tooltips
    for tooltip_e in existing_tooltips.iter() {
        commands.entity(tooltip_e).despawn_recursive();
    }

    // Spawn new tooltip if hovering
    if let Some(hovered_heirloom) = &currently_hovered {
        // Find the essence option to get the rarity
        for (essence_option, interactable) in essence_options.iter() {
            if matches!(interactable.current(), Interaction::Hovering)
                && essence_option.get_heirloom() == *hovered_heirloom
            {
                let tooltip_e = spawn_heirloom_tooltip_card(
                    &graphics,
                    &mut commands,
                    &asset_server,
                    essence_option.get_heirloom(),
                    essence_option.get_rarity(),
                    Vec3::new(-130., 0., 15.), // Left side of the essence shop
                    None,                      // No scaling text for shop tooltips
                );

                commands
                    .entity(tooltip_e)
                    .insert(EssenceTooltipCard)
                    .insert(UIState::Essence);

                break;
            }
        }
    }

    *last_hovered = currently_hovered;
}

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

        // icon - use the same method as player_hud.rs
        let icon = commands
            .spawn(SpriteSheetBundle {
                sprite: graphics.get_heirloom_icon(essence_option.get_heirloom()),
                texture_atlas: graphics.texture_atlas.as_ref().unwrap().clone(),
                transform: Transform {
                    translation: Vec3::new(0., 0., 1.),
                    ..Default::default()
                },
                ..Default::default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .id();
        commands.entity(icon).set_parent(slot_entity);

        // Get heirloom name and rarity color
        let heirloom_name = essence_option.get_heirloom().get_title();
        let rarity_color = essence_option.get_rarity().get_color();

        let _name = commands
            .spawn((
                Text2dBundle {
                    text: Text::from_section(
                        heirloom_name,
                        TextStyle {
                            font: asset_server.load("fonts/4x5.ttf"),
                            font_size: 5.0,
                            color: rarity_color,
                        },
                    )
                    .with_alignment(TextAlignment::Left),
                    text_anchor: bevy::sprite::Anchor::CenterLeft,
                    transform: Transform::from_translation(Vec3::new(-30.5, y_offset, 1.)),
                    ..Default::default()
                },
                RenderLayers::from_layers(&[3]),
                Name::new("Heirloom Name"),
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
    mut attribute_event: EventWriter<AttributeChangeEvent>,
    mut params: ParamSet<(GameParam, Query<(Entity, &mut PlayerSkills), With<Player>>)>,
    shop: Res<EssenceShopChoices>,
    mut purchase_tracker: ResMut<BlacksmithPurchaseTracker>,
) {
    for choice in ev.iter() {
        // Access coins and time_fragments through GameParam (set 0)
        let time_fragments = params.p0().get_time_fragments();
        let coins = params.p0().get_coins();

        if time_fragments >= choice.choice.time_fragment_cost as i32
            && coins >= choice.choice.coin_cost
        {
            currency_event.send(ModifyCurencyEvent {
                delta: -(choice.choice.time_fragment_cost as i32),
                obj: WorldObject::TimeFragment,
            });
            currency_event.send(ModifyCurencyEvent {
                delta: -(choice.choice.coin_cost as i32),
                obj: WorldObject::Coin,
            });

            // Add heirloom to player's heirloom pool (set 1 - mutable access)
            if let Ok((player_entity, mut player_skills)) = params.p1().get_single_mut() {
                let heirloom_with_rarity = HeirloomWithRarity {
                    heirloom: choice.choice.heirloom.clone(),
                    rarity: choice.choice.rarity.clone(),
                };

                player_skills.heirlooms.push(heirloom_with_rarity.clone());

                // Add skill components to the player entity
                heirloom_with_rarity.heirloom.add_skill_components(
                    player_entity,
                    &mut commands,
                    player_skills.clone(),
                );

                // Trigger attribute recalculation
                attribute_event.send(AttributeChangeEvent);
            }

            // Increment purchase counter to increase prices for next shop
            purchase_tracker.purchases_made += 1;
            info!(
                "Blacksmith purchase made! Total purchases: {}, Next price multiplier: {:.2}x",
                purchase_tracker.purchases_made,
                purchase_tracker.get_price_multiplier()
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

                // Update the chunk cache so the merchant stays "Done" when chunk respawns (set 0)
                if let Some(tile_pos) = shop.tile_pos {
                    info!(
                        "Updating blacksmith merchant in chunk cache at {:?} to Done state",
                        tile_pos
                    );
                    params
                        .p0()
                        .add_object_to_chunk_cache(tile_pos, WorldObject::BlacksmithMerchantDone);
                }
            }
        }
    }
}

pub fn handle_populate_essence_shop_on_new_spawn(
    mut new_spawns: Query<
        (Entity, &mut EssenceShopChoices, &GlobalTransform),
        Added<EssenceShopChoices>,
    >,
    player_atts: Query<&crate::attributes::LootRateBonus, With<crate::player::Player>>,
    heirloom_queue: Res<crate::player::skills::HeirloomChoiceQueue>,
    purchase_tracker: Res<BlacksmithPurchaseTracker>,
) {
    for (entity, mut shop, transform) in new_spawns.iter_mut() {
        let mut shop_choices = vec![];
        let mut rng = rand::thread_rng();

        // Store tile position for chunk cache updates
        // Account for sprite anchor offset (y: 12.0) to get the correct base tile position
        let world_pos = transform.translation().truncate() - bevy::math::Vec2::new(0., 12.);
        let tile_pos = crate::world::world_helpers::world_pos_to_tile_pos(world_pos);
        shop.tile_pos = Some(tile_pos);

        // Get the price multiplier based on previous purchases
        let purchase_multiplier = purchase_tracker.get_price_multiplier();

        while shop_choices.len() < 3 {
            // Determine rarity based on luck/randomness
            let loot_bonus = player_atts.get_single().map(|a| a.0).unwrap_or(0);
            let rarity =
                crate::player::skills::HeirloomChoiceQueue::gen_rarity(&mut rng, loot_bonus);

            // Pick a heirloom from the pool that matches this rarity
            let picked_heirloom_choice =
                heirloom_queue.get_skill_of_rarity(rarity.clone(), &mut rng, &|_| true);

            let Some(heirloom_choice) = picked_heirloom_choice else {
                // If no heirloom of this rarity is available, try a different rarity
                continue;
            };

            // Calculate cost based on rarity
            let rarity_cost_inc = match heirloom_choice.rarity {
                HeirloomRarity::Common => 1.,
                HeirloomRarity::Uncommon => 1.2,
                HeirloomRarity::Rare => 1.6,
                HeirloomRarity::Legendary => 2.5,
            };
            let time_frag_cost = match heirloom_choice.rarity {
                HeirloomRarity::Rare => 1,
                HeirloomRarity::Legendary => 3,
                _ => 0,
            };

            // Base cost (level 1 equivalent: 1 * 4 + 3 = 7)
            let base_cost = 7.;
            let random_adjustment = rand::thread_rng().gen_range(2.0..7.0) * rarity_cost_inc * 2.;

            // Apply purchase multiplier to scale prices
            let final_cost =
                (base_cost * rarity_cost_inc + random_adjustment) * purchase_multiplier;

            shop_choices.push(EssenceOption {
                heirloom: heirloom_choice.heirloom,
                rarity: heirloom_choice.rarity,
                coin_cost: final_cost.trunc() as u32,
                time_fragment_cost: time_frag_cost,
            });
        }
        shop.choices = shop_choices;
        shop.owner_entity = Some(entity);
    }
}
