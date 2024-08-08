use bevy::prelude::*;

use crate::{
    inventory::ItemStack, item::WorldObject, player::Player,
    world::world_helpers::tile_pos_to_world_pos, GameParam,
};

use super::{damage_numbers::spawn_text, spawn_item_stack_icon, UIElement, UI_SLOT_SIZE};

#[derive(Component)]
pub struct ShrineInteractGuide;

pub fn spawn_shrine_interact_key_guide(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    player_query: Query<(Entity, &GlobalTransform), With<Player>>,
    game: GameParam,
    already_exists: Query<Entity, With<ShrineInteractGuide>>,
) {
    let (player_e, player_t) = player_query.single();
    let Some(shrine) = game
        .world_obj_cache
        .unique_objs
        .get(&WorldObject::BossShrine)
    else {
        return;
    };
    let shrine_pos = tile_pos_to_world_pos(*shrine, false);

    if already_exists.iter().count() == 0 {
        if shrine_pos.distance(player_t.translation().truncate()) < 32. {
            let key_entity = commands
                .spawn(SpriteBundle {
                    texture: asset_server.load("textures/FKey.png"),
                    transform: Transform::from_translation(Vec3::new(-29.5, 25.5, 1.)),
                    sprite: Sprite {
                        custom_size: Some(Vec2::new(11., 11.)),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .insert(ShrineInteractGuide)
                .set_parent(player_e)
                .id();

            let text_entity = spawn_text(
                &mut commands,
                &asset_server,
                Vec3::new(40., 0., 1.),
                Color::WHITE,
                "to activate".to_owned(),
            );
            commands.entity(text_entity).set_parent(key_entity);

            let item_icon = spawn_item_stack_icon(
                &mut commands,
                &game.graphics,
                &ItemStack::crate_icon_stack(WorldObject::TimeFragment).copy_with_count(10),
                &asset_server,
                Vec2::ZERO,
                0,
            );

            commands
                .spawn(SpriteBundle {
                    texture: game
                        .graphics
                        .get_ui_element_texture(UIElement::ScreenIconSlot),
                    transform: Transform::from_translation(Vec3::new(28.5, 18.5, 1.)),
                    sprite: Sprite {
                        custom_size: Some(Vec2::new(UI_SLOT_SIZE, UI_SLOT_SIZE)),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .set_parent(key_entity)
                .push_children(&[item_icon]);
        }
    } else if shrine_pos.distance(player_t.translation().truncate()) > 32. {
        for t in already_exists.iter() {
            commands.entity(t).despawn_recursive();
        }
    }
}
