use bevy::prelude::*;
use bevy_proto::prelude::ProtoCommands;
use rand::Rng;

use crate::{
    custom_commands::CommandsExt,
    enemy::Mob,
    inventory::ItemStack,
    juice::{FlashEffect, ShakeEffect},
    player::{ModifyTimeFragmentsEvent, TimeFragmentCurrency},
    proto::proto_param::ProtoParam,
    world::{dimension::ActiveDimension, dungeon::Dungeon, world_helpers::tile_pos_to_world_pos},
    GameParam, TextureCamera,
};

use super::WorldObject;

#[derive(Resource)]
pub struct DelayedSpawn {
    timer: Timer,
    mob: Mob,
    pos: Vec2,
}

pub fn check_for_items_on_shrine(
    dungeon_check: Query<&Dungeon, With<ActiveDimension>>,
    dropped_items: Query<(Entity, &Transform, &ItemStack), With<WorldObject>>,
    mut game_camera: Query<Entity, With<TextureCamera>>,
    game: GameParam,
    mut commands: Commands,
) {
    if dungeon_check.get_single().is_ok() {
        return;
    }
    let Some(shrine) = game
        .world_obj_cache
        .unique_objs
        .get(&WorldObject::BossShrine)
    else {
        return;
    };
    let shrine_pos = tile_pos_to_world_pos(*shrine, false);

    for (e, tfxm, item_stack) in dropped_items.iter() {
        if shrine_pos.distance(tfxm.translation.truncate()) < 32. {
            if item_stack.count >= 10 && item_stack.obj_type == WorldObject::RedMushroomBlock {
                // proto_commands.spawn_from_proto(Mob::RedMushking, &proto.prototypes, shrine_pos);
                commands.insert_resource(DelayedSpawn {
                    timer: Timer::from_seconds(3., TimerMode::Once),
                    mob: Mob::RedMushking,
                    pos: shrine_pos,
                });
                commands.entity(e).despawn_recursive();

                // Boss Effects
                // Screen Shake
                let mut rng = rand::thread_rng();
                let seed = rng.gen_range(0..100000);
                let speed = 10.;
                let max_mag = 120.;
                let noise = 0.5;
                let dir = Vec2::new(1., 1.);
                for e in game_camera.iter_mut() {
                    commands.entity(e).insert(ShakeEffect {
                        timer: Timer::from_seconds(3.5, TimerMode::Once),
                        speed,
                        seed,
                        max_mag,
                        noise,
                        dir,
                    });
                }
            }
        }
    }
}
pub fn handle_pay_shrine_cost(
    mut commands: Commands,
    key_input: ResMut<Input<KeyCode>>,
    player_query: Query<(&GlobalTransform, &TimeFragmentCurrency)>,
    game: GameParam,
    mut game_camera: Query<Entity, With<TextureCamera>>,
    mut currency_event: EventWriter<ModifyTimeFragmentsEvent>,
    dungeon_check: Query<&Dungeon>,
) {
    if dungeon_check.get_single().is_ok() {
        return;
    }
    if key_input.just_pressed(KeyCode::F) {
        let (player_t, currency) = player_query.single();
        let Some(shrine) = game
            .world_obj_cache
            .unique_objs
            .get(&WorldObject::BossShrine)
        else {
            return;
        };
        let shrine_pos = tile_pos_to_world_pos(*shrine, false);

        if shrine_pos.distance(player_t.translation().truncate()) < 32. {
            if currency.time_fragments >= 10 {
                currency_event.send(ModifyTimeFragmentsEvent { delta: -10 });
                // proto_commands.spawn_from_proto(Mob::RedMushking, &proto.prototypes, shrine_pos);
                commands.insert_resource(DelayedSpawn {
                    timer: Timer::from_seconds(3., TimerMode::Once),
                    mob: Mob::RedMushking,
                    pos: shrine_pos,
                });

                // Boss Effects
                // Screen Shake
                let mut rng = rand::thread_rng();
                let seed = rng.gen_range(0..100000);
                let speed = 10.;
                let max_mag = 120.;
                let noise = 0.5;
                let dir = Vec2::new(1., 1.);
                for e in game_camera.iter_mut() {
                    commands.entity(e).insert(ShakeEffect {
                        timer: Timer::from_seconds(3.5, TimerMode::Once),
                        speed,
                        seed,
                        max_mag,
                        noise,
                        dir,
                    });
                }
            }
        }
    }
}
pub fn handle_delayed_spawns(
    mut delayed_spawns: ResMut<DelayedSpawn>,
    mut commands: Commands,
    time: Res<Time>,
    mut proto_commands: ProtoCommands,
    proto: ProtoParam,
) {
    delayed_spawns.timer.tick(time.delta());
    if delayed_spawns.timer.finished() {
        commands.remove_resource::<DelayedSpawn>();
        proto_commands.spawn_from_proto(
            delayed_spawns.mob.clone(),
            &proto.prototypes,
            delayed_spawns.pos,
        );
        //Flash
        commands.insert_resource(FlashEffect {
            timer: Timer::from_seconds(0.5, TimerMode::Once),
            color: Color::rgba(1., 1., 1., 1.),
        });
    }
}
