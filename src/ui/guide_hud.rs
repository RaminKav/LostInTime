use bevy::{prelude::*, render::view::RenderLayers};

use crate::{
    assets::Graphics,
    inventory::{Inventory, ItemStack},
    item::{Recipes, WorldObject},
};

use super::{spawn_item_stack_icon, UIElement, UI_SLOT_SIZE};

// pub enum ProgressionGoal {
//     Axe(WorldObject::WoodAxe),
//     CraftingTable(WorldObject::CraftingTableBlock),
//     Walls(WorldObject::WoodWallBlock),
//     Furnace(WorldObject::FurnaceBlock),
//     Cauldron(WorldObject::CauldronBlock),
//     Anvil(WorldObject::AnvilBlock),
// }

#[derive(Resource, Debug)]
pub struct CurrentGoal {
    pub goal: WorldObject,
}

pub fn init_starting_goal(_commands: Commands) {
    // commands.insert_resource(CurrentGoal {
    //     goal: WorldObject::WoodAxe,
    // });
}

#[derive(Component)]
pub struct GoalIcons;

/// runs on new CurrentGoal resource added
pub fn handle_display_new_goal(
    curr_goal: Res<CurrentGoal>,
    recipes: Res<Recipes>,
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
) {
    let curr_goal_obj = curr_goal.goal;
    let curr_goal_ingredients = recipes
        .crafting_list
        .get(&curr_goal_obj)
        .unwrap()
        .0
        .iter()
        .map(|r| r.item)
        .collect::<Vec<WorldObject>>();

    let goal_icon = spawn_item_stack_icon(
        &mut commands,
        &graphics,
        &ItemStack::crate_icon_stack(curr_goal_obj),
        &asset_server,
        Vec2::ZERO,
        Vec2::new(0., 0.),
        3,
    );

    let _slot_entity = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::ScreenIconSlot),
            transform: Transform::from_translation(Vec3::new(-170., -100., 0.)),
            sprite: Sprite {
                custom_size: Some(Vec2::new(UI_SLOT_SIZE, UI_SLOT_SIZE)),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(GoalIcons)
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("SCREEN ICON"))
        .push_children(&[goal_icon]);

    for (i, ingredient) in curr_goal_ingredients.iter().enumerate() {
        let ingredient_icon = spawn_item_stack_icon(
            &mut commands,
            &graphics,
            &ItemStack::crate_icon_stack(*ingredient),
            &asset_server,
            Vec2::ZERO,
            Vec2::new(0., 0.),
            3,
        );
        let _slot_entity = commands
            .spawn(SpriteBundle {
                texture: graphics.get_ui_element_texture(UIElement::ScreenIconSlot),
                transform: Transform::from_translation(Vec3::new(
                    -170. + 20. * (i) as f32,
                    -100. + 22_f32,
                    0.,
                )),
                sprite: Sprite {
                    custom_size: Some(Vec2::new(UI_SLOT_SIZE, UI_SLOT_SIZE)),
                    ..Default::default()
                },
                ..Default::default()
            })
            .insert(GoalIcons)
            .insert(RenderLayers::from_layers(&[3]))
            .insert(Name::new("SCREEN ICON"))
            .push_children(&[ingredient_icon]);
    }
}

pub fn handle_update_goal_progress(
    inv: Query<&Inventory, Changed<Inventory>>,
    mut commands: Commands,
    _recipes: Res<Recipes>,
    curr_goal: Res<CurrentGoal>,
    icons: Query<Entity, With<GoalIcons>>,
) {
    let curr_goal_obj = curr_goal.goal;
    // let curr_goal_ingredients = recipes
    //     .crafting_list
    //     .get(&curr_goal_obj)
    //     .unwrap()
    //     .0
    //     .iter()
    //     .map(|r| r.item.clone())
    //     .collect::<Vec<WorldObject>>();
    // println!("Inv changed! {:?}", inv.single().items.items);
    if let Ok(inv) = inv.get_single() {
        if inv
            .items
            .items
            .iter()
            .flatten()
            .any(|i| i.get_obj() == &curr_goal_obj)
        {
            debug!("got goal item");
            commands.remove_resource::<CurrentGoal>();
            for e in icons.iter() {
                debug!("DELETE!!");
                commands.entity(e).despawn_recursive();
            }
        }
    }
}
