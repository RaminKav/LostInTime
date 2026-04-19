use bevy::prelude::*;

use crate::{
    assets::Graphics,
    cursor::CursorPos,
    inventory::Inventory,
    item::{
        item_actions::{ItemAction, ItemActions},
        EquipmentType, MainHand, RequiredEquipmentType, WorldObject,
    },
    player::Player,
    proto::proto_param::ProtoParam,
    ui::CheatSettings,
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
    cheat_settings: Res<CheatSettings>,
    tile_hover_check: Query<(Entity, &TileHover)>,
    proto_param: ProtoParam,
    mut game: GameParam,
    main_hand: Query<&WorldObject, With<MainHand>>,
    player_inv: Query<&Inventory, With<Player>>,
    tool_req_query: Query<&RequiredEquipmentType>,
) {
    if !cheat_settings.show_tile_hover {
        if let Ok((e, _)) = tile_hover_check.get_single() {
            commands.entity(e).despawn();
        }
        return;
    }

    let tile_pos = world_pos_to_tile_pos(cursor.world_coords.truncate());
    if let Ok((e, tile_hover)) = tile_hover_check.get_single() {
        if tile_hover.pos == tile_pos && !game.world_obj_cache.is_changed() {
            return;
        }
        commands.entity(e).despawn();
    }
    let main_hand_obj = main_hand.get_single();
    let mut hover = UIElement::TileHover;

    if let Ok(main_hand) = main_hand_obj {
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
    }

    // Tool requirement: match combat — any inventory slot with the tool, or main hand item type.
    if let Some((obj_e, _)) = game.get_obj_entity_at_tile(tile_pos, &proto_param) {
        if let Ok(req) = tool_req_query.get(obj_e) {
            let inv_ok = player_inv
                .get_single()
                .map(|inv| inv.has_equipment_type(&req.0, &proto_param))
                .unwrap_or(false);
            let main_ok = main_hand_obj
                .ok()
                .and_then(|mh| proto_param.get_component::<EquipmentType, _>(*mh))
                .map(|et| et == &req.0)
                .unwrap_or(false);
            if !inv_ok && !main_ok {
                hover = UIElement::BlockedTileHover;
            }
        }
    }

    commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(hover),
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
