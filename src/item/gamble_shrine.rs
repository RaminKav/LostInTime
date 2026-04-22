use bevy::prelude::*;
use bevy_aseprite::{anim::AsepriteAnimation, aseprite, AsepriteBundle};
use bevy_proto::prelude::ProtoCommands;
use rand::{seq::IteratorRandom, Rng};

use crate::{
    assets::{Graphics, SpriteAnchor},
    combat::pickup_radius::BeingPulledToPlayer,
    custom_commands::CommandsExt,
    inventory::{player_can_accept_ground_item_pickup, Inventory, ItemStack},
    pets::state::Pet,
    item::{object_actions::ObjectAction, ItemDrop},
    player::Player,
    proto::proto_param::ProtoParam,
    ui::{
        key_input_guide::InteractionGuideTrigger, minimap::UpdateMiniMapEvent, BlacksmithMerchant,
        EssenceShopChoices,
    },
    world::{world_helpers::world_pos_to_tile_pos, TileMapPosition},
    GameParam,
};

use super::WorldObject;

#[derive(Component)]
pub struct CombatShrineMob {
    pub parent_shrine: Entity,
}

#[derive(Component)]
pub struct GambleShrine {
    pub success: bool,
    pub tile_pos: TileMapPosition,
}

pub struct GambleShrineEvent {
    pub entity: Entity,
    pub success: bool,
}

pub fn handle_gamble_shrine_rewards(
    mut shrines: Query<(
        Entity,
        &GlobalTransform,
        &GambleShrine,
        &mut AsepriteAnimation,
    )>,
    // mut proto_commands: ProtoCommands,
    proto: ProtoParam,
    mut commands: Commands,
    mut game: GameParam,
    mut minimap_event: EventWriter<UpdateMiniMapEvent>,
    item_drop_query: Query<
        (Entity, &ItemStack),
        (With<ItemDrop>, Without<BeingPulledToPlayer>),
    >,
    inv: Query<&Inventory, With<Player>>,
    pets: Query<(), With<Pet>>,
) {
    for (e, t, shrine, mut anim) in shrines.iter_mut() {
        if shrine.success {
            if anim.current_frame() == 53 {
                *anim = AsepriteAnimation::from(GambleShrineAnim::tags::DONE);
                commands.entity(e).remove::<GambleShrine>();

                // let drop_list = [WorldObject::ChestBlock, WorldObject::Coin];
                // // give rewards
                // let picked_drop = *drop_list.iter().choose(&mut rand::thread_rng()).unwrap();
                // let mut rng = rand::thread_rng();
                // let count = match picked_drop {
                //     WorldObject::Coin => rng.gen_range(34..53),
                //     _ => 1,
                // };
                // proto_commands.spawn_item_from_proto(
                //     picked_drop,
                //     &proto,
                //     t.translation().truncate() + Vec2::new(0., -78.), // offset so it doesn't spawn on the shrine
                //     count,
                //     Some(game.get_player_level()),
                // );

                // Pull items on the map by adding BeingPulledToPlayer (skip if inv can't accept)
                if let Ok(inv) = inv.get_single() {
                    let player_has_pet = pets.iter().next().is_some();
                    for (item_entity, item_stack) in item_drop_query.iter() {
                        if !player_can_accept_ground_item_pickup(item_stack, inv, player_has_pet) {
                            continue;
                        }
                        commands
                            .entity(item_entity)
                            .insert(BeingPulledToPlayer::default());
                    }
                }

                commands
                    .entity(e)
                    .insert(WorldObject::GambleShrineDone)
                    .remove::<ObjectAction>();
                // Use the stored tile position instead of recalculating
                game.add_object_to_chunk_cache(shrine.tile_pos, WorldObject::GambleShrineDone);

                // Update minimap to reflect the shrine is now "Done"
                minimap_event.send(UpdateMiniMapEvent {
                    pos: Some(shrine.tile_pos),
                    new_tile: Some(WorldObject::GambleShrineDone),
                });
            }
        } else if anim.current_frame() == 92 {
            *anim = AsepriteAnimation::from(GambleShrineAnim::tags::IDLE);
            let obj_action = proto
                .get_component::<ObjectAction, _>(WorldObject::GambleShrine)
                .expect("Gamble shrine missing ObjectAction");
            commands
                .entity(e)
                .remove::<GambleShrine>()
                .insert(InteractionGuideTrigger {
                    key: Some("F".to_string()),
                    text: Some("Interact".to_string()),
                    activation_distance: 32.,
                    icon_stack: Some(ItemStack::crate_icon_stack(WorldObject::TimeFragment)),
                })
                .insert(obj_action.clone());
        }
    }
}

aseprite!(pub GambleShrineAnim, "textures/gamble_shrine/gamble_shrine.ase");

pub fn add_gamble_visuals_on_spawn(
    mut commands: Commands,
    new_shrines: Query<
        (Entity, &WorldObject, &Transform),
        Or<(Added<WorldObject>, Changed<WorldObject>)>,
    >,
    graphics: Res<Graphics>,
) {
    for (e, obj, t) in new_shrines.iter() {
        if obj == &WorldObject::GambleShrine {
            commands
                .entity(e)
                .insert(AsepriteBundle {
                    transform: *t,
                    animation: AsepriteAnimation::from(GambleShrineAnim::tags::IDLE),
                    aseprite: graphics.gamble_shrine_anim.as_ref().unwrap().clone(),
                    ..default()
                })
                .insert(Name::new("GAMBLE"));
        } else if obj == &WorldObject::GambleShrineDone {
            commands
                .entity(e)
                .insert(AsepriteBundle {
                    transform: *t,
                    animation: AsepriteAnimation::from(GambleShrineAnim::tags::DONE),
                    aseprite: graphics.gamble_shrine_anim.as_ref().unwrap().clone(),
                    ..default()
                })
                .insert(Name::new("GAMBLE_DONE"));
        } else if obj == &WorldObject::BlacksmithMerchant {
            commands
                .entity(e)
                .insert(AsepriteBundle {
                    transform: *t,
                    animation: AsepriteAnimation::from(BlacksmithMerchant::tags::IDLE_WORKING_1),
                    aseprite: graphics.blacksmith_merchant.as_ref().unwrap().clone(),
                    ..default()
                })
                .insert(EssenceShopChoices::default())
                .insert(Name::new("BLACKSMITH"));
        } else if obj == &WorldObject::BlacksmithMerchantDone {
            commands
                .entity(e)
                .insert(AsepriteBundle {
                    transform: *t,
                    animation: AsepriteAnimation::from(BlacksmithMerchant::tags::DONE),
                    aseprite: graphics.blacksmith_merchant.as_ref().unwrap().clone(),
                    ..default()
                })
                .insert(Name::new("BLACKSMITH"));
        }
    }
}
