use bevy::{prelude::*, render::view::RenderLayers, sprite::Anchor};

use crate::{
    assets::Graphics,
    attributes::{hunger::Hunger, CurrentHealth, MaxHealth},
    colors::{RED, YELLOW},
    inventory::Inventory,
    player::Player,
    GAME_HEIGHT, GAME_WIDTH,
};

use super::{
    interactions::Interaction, spawn_inv_slot, InventorySlotType, InventoryState, InventoryUI,
    InventoryUIState, UIElement,
};

#[derive(Component)]
pub struct HealthBar;
#[derive(Component)]
pub struct FoodBar;

pub fn setup_healthbar_ui(mut commands: Commands, graphics: Res<Graphics>) {
    let inner_health = commands
        .spawn(SpriteBundle {
            sprite: Sprite {
                color: RED,
                custom_size: Some(Vec2::new(62.0, 7.0)),
                anchor: Anchor::CenterLeft,
                ..default()
            },
            transform: Transform {
                translation: Vec3::new(-62. / 2., 0., 10.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(HealthBar)
        .insert(Name::new("inner health bar"))
        .id();
    let health_bar_frame = commands
        .spawn(SpriteBundle {
            texture: graphics
                .ui_image_handles
                .as_ref()
                .unwrap()
                .get(&UIElement::HealthBarFrame)
                .unwrap()
                .clone(),
            sprite: Sprite {
                custom_size: Some(Vec2::new(64., 9.)),
                ..Default::default()
            },
            transform: Transform {
                translation: Vec3::new(
                    (-GAME_WIDTH + 68.) / 2.,
                    (GAME_HEIGHT - 11.) / 2. - 2.,
                    10.,
                ),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(Name::new("HEALTH BAR"))
        .insert(RenderLayers::from_layers(&[3]))
        .id();
    commands
        .entity(health_bar_frame)
        .push_children(&[inner_health]);
}
pub fn setup_foodbar_ui(mut commands: Commands, graphics: Res<Graphics>) {
    let inner_food = commands
        .spawn(SpriteBundle {
            sprite: Sprite {
                color: YELLOW,
                custom_size: Some(Vec2::new(62.0, 7.0)),
                anchor: Anchor::CenterLeft,
                ..default()
            },
            transform: Transform {
                translation: Vec3::new(-62. / 2., 0., 10.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(FoodBar)
        .insert(Name::new("inner food bar"))
        .id();
    let food_bar_frame = commands
        .spawn(SpriteBundle {
            texture: graphics
                .ui_image_handles
                .as_ref()
                .unwrap()
                .get(&UIElement::HealthBarFrame)
                .unwrap()
                .clone(),
            sprite: Sprite {
                custom_size: Some(Vec2::new(64., 9.)),
                ..Default::default()
            },
            transform: Transform {
                translation: Vec3::new(
                    (-GAME_WIDTH + 68.) / 2.,
                    (GAME_HEIGHT - 11.) / 2. - 12.,
                    10.,
                ),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(Name::new("FOOD BAR"))
        .insert(RenderLayers::from_layers(&[3]))
        .id();
    commands.entity(food_bar_frame).push_children(&[inner_food]);
}
pub fn update_healthbar(
    player_health_query: Query<
        (&CurrentHealth, &MaxHealth),
        (
            Or<(Changed<CurrentHealth>, Changed<MaxHealth>)>,
            With<Player>,
        ),
    >,
    mut health_bar_query: Query<&mut Sprite, With<HealthBar>>,
) {
    let Ok((player_health, player_max_health)) = player_health_query.get_single() else {return};
    health_bar_query.single_mut().custom_size = Some(Vec2 {
        x: 62. * player_health.0 as f32 / player_max_health.0 as f32,
        y: 7.,
    });
}

pub fn update_foodbar(
    player_hunger_query: Query<&Hunger, (With<Player>, Changed<Hunger>)>,
    mut food_bar_query: Query<&mut Sprite, With<FoodBar>>,
) {
    let Ok(hunger) = player_hunger_query.get_single() else {return};
    food_bar_query.single_mut().custom_size = Some(Vec2 {
        x: 62. * hunger.current as f32 / hunger.max as f32,
        y: 7.,
    });
}

pub fn setup_hotbar_hud(
    mut commands: Commands,
    graphics: Res<Graphics>,
    inv_query: Query<Entity, With<InventoryUI>>,
    inv_state: Res<InventoryState>,
    asset_server: Res<AssetServer>,
    mut inv: Query<&mut Inventory>,
    inv_ui_state: Res<State<InventoryUIState>>,
) {
    for (slot_index, item) in inv.single_mut().items.items.iter().enumerate() {
        // hotbar slots
        if slot_index <= 5 {
            spawn_inv_slot(
                &mut commands,
                &inv_ui_state,
                &graphics,
                slot_index,
                Interaction::None,
                &inv_state,
                &inv_query,
                &asset_server,
                InventorySlotType::Hotbar,
                item.clone(),
            );
        }
    }
}
