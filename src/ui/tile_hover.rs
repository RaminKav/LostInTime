use bevy::prelude::*;

use crate::{
    assets::Graphics,
    inputs::CursorPos,
    item::{
        item_actions::{ItemAction, ItemActions},
        EquipmentType, MainHand, RequiredEquipmentType, WorldObject,
    },
    proto::proto_param::ProtoParam,
    world::{
        world_helpers::{can_object_be_placed_here, tile_pos_to_world_pos, world_pos_to_tile_pos},
        y_sort::YSort,
        TileMapPosition,
    },
    GameParam,
};

use super::UIElement;

#[derive(Component)]
pub struct TileHover {
    pos: TileMapPosition,
}

pub fn spawn_tile_hover_on_cursor_move(
    mut commands: Commands,
    cursor: Res<CursorPos>,
    graphics: Res<Graphics>,
    tile_hover_check: Query<(Entity, &TileHover)>,
    proto_param: ProtoParam,
    mut game: GameParam,
    main_hand: Query<&WorldObject, With<MainHand>>,
    tool_req_query: Query<&RequiredEquipmentType>,
) {
    let tile_pos = world_pos_to_tile_pos(cursor.world_coords.truncate());
    if let Ok((e, tile_hover)) = tile_hover_check.get_single() {
        if tile_hover.pos == tile_pos && !game.world_obj_cache.is_changed() {
            return;
        }
        commands.entity(e).despawn();
    }
    let main_hand_obj = main_hand.get_single();
    let hover_type = if let Ok(main_hand) = main_hand_obj {
        let mut hover = UIElement::TileHover;
        // check space for placing
        if let Some(actions) = proto_param.get_component::<ItemActions, _>(*main_hand) {
            for action in actions.actions.clone() {
                hover = match action {
                    ItemAction::PlacesInto(obj) => {
                        if !can_object_be_placed_here(tile_pos, &mut game, obj, &proto_param) {
                            UIElement::BlockedTileHover
                        } else {
                            UIElement::TileHover
                        }
                    }
                    _ => UIElement::TileHover,
                };
            }
        }
        // check tool type
        if let Some((obj_e, _)) = game.get_obj_entity_at_tile(tile_pos, &proto_param) {
            if let Ok(req) = tool_req_query.get(obj_e) {
                if let Ok(main_hand) = main_hand_obj {
                    if req.0
                        != *proto_param
                            .get_component::<EquipmentType, _>(*main_hand)
                            .unwrap_or(&EquipmentType::None)
                    {
                        hover = UIElement::BlockedTileHover;
                    }
                } else {
                    hover = UIElement::BlockedTileHover;
                }
            }
        }

        hover
    } else {
        UIElement::TileHover
    };
    commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(hover_type),
            transform: Transform {
                translation: tile_pos_to_world_pos(tile_pos, false).extend(1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            sprite: Sprite {
                custom_size: Some(Vec2::new(16., 16.)),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(TileHover { pos: tile_pos })
        .insert(YSort(-0.2));
}
