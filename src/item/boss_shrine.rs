use bevy::prelude::*;
use bevy_proto::prelude::ProtoCommands;
use rand::Rng;

use crate::{
    colors::{LIGHT_RED, RED},
    custom_commands::CommandsExt,
    enemy::Mob,
    inventory::ItemStack,
    juice::{FlashEffect, ShakeEffect},
    player::{ModifyCurencyEvent, Player},
    proto::proto_param::ProtoParam,
    ui::{global_text_message::GlobalTextMessageEvent, key_input_guide::InteractionGuideTrigger},
    world::{dimension::Era, dungeon::Dungeon, world_helpers::tile_pos_to_world_pos},
    GameParam, InputMappings, TextureCamera,
};

/// Per-era boss selection used by the boss shrine.
fn boss_for_era(era: &Era) -> Mob {
    match era {
        Era::Second => Mob::Scorpion,
        _ => Mob::RedMushking,
    }
}

use super::WorldObject;

pub const BOSS_SUMMON_BASE_COST: i32 = 50;
pub const BOSS_SUMMON_COST_INCREMENT: i32 = 50;

#[derive(Resource, Default)]
pub struct BossSummonTracker {
    pub summon_count: u32,
}

impl BossSummonTracker {
    pub fn current_cost(&self) -> i32 {
        BOSS_SUMMON_BASE_COST + (self.summon_count as i32 * BOSS_SUMMON_COST_INCREMENT)
    }

    pub fn reset(&mut self) {
        self.summon_count = 0;
    }
    pub fn get_boss_tint(&self) -> Color {
        match self.summon_count - 1 {
            0 => Color::rgba(1., 1., 1., 1.0),
            1 => Color::rgba(0.2, 0.6, 1., 1.0),  // blue
            2 => Color::rgba(0.8, 0.4, 1.0, 1.0), // purple
            3 => Color::rgba(1.0, 0.4, 0.0, 1.0), // orange
            _ => Color::rgba(1.0, 0.0, 0.0, 1.0),
        }
    }
    pub fn get_health_scale(&self) -> f32 {
        match self.summon_count {
            0 => 1.0,
            1 => 2.0,
            2 => 4.0,
            3 => 10.0,
            _ => 10.0,
        }
    }
    pub fn get_damage_scale(&self) -> f32 {
        match self.summon_count {
            0 => 1.0,
            1 => 1.5,
            2 => 2.0,
            3 => 4.0,
            _ => 4.0,
        }
    }
}

#[derive(Resource)]
pub struct DelayedSpawn {
    timer: Timer,
    mob: Mob,
    pos: Vec2,
    pub summon_index: u32,
}

pub fn handle_pay_shrine_cost(
    mut commands: Commands,
    key_input: Res<Input<KeyCode>>,
    mouse_input: Res<Input<MouseButton>>,
    keybinds: Res<InputMappings>,
    player_query: Query<&GlobalTransform, With<Player>>,
    game: GameParam,
    mut game_camera: Query<Entity, With<TextureCamera>>,
    mut currency_event: EventWriter<ModifyCurencyEvent>,
    mut global_text_events: EventWriter<GlobalTextMessageEvent>,
    dungeon_check: Query<&Dungeon>,
    delayed_spawn: Option<Res<DelayedSpawn>>,
    mut summon_tracker: ResMut<BossSummonTracker>,
) {
    if dungeon_check.get_single().is_ok() {
        return;
    }
    if delayed_spawn.is_some() {
        return;
    }
    if keybinds.check_interact_input(&key_input, &mouse_input) {
        let player_t = player_query.single();
        let Some(shrine) = game
            .world_obj_cache
            .unique_objs
            .get(&WorldObject::BossShrine)
        else {
            return;
        };
        let shrine_pos = tile_pos_to_world_pos(*shrine, false);
        let cost = summon_tracker.current_cost();

        if shrine_pos.distance(player_t.translation().truncate()) < 32.
            && game.get_coins() as i32 >= cost
        {
            currency_event.send(ModifyCurencyEvent {
                delta: -cost,
                obj: WorldObject::Coin,
            });
            global_text_events
                .send(GlobalTextMessageEvent::new("WARNING!", LIGHT_RED).with_panel_width(164.0));
            let summon_index = summon_tracker.summon_count;
            summon_tracker.summon_count += 1;
            commands.insert_resource(DelayedSpawn {
                timer: Timer::from_seconds(3., TimerMode::Once),
                mob: boss_for_era(&game.era.current_era),
                pos: shrine_pos,
                summon_index,
            });

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
/// Marker component for boss summon scaling, attached to bosses spawned from the shrine.
#[derive(Component)]
pub struct BossSummonIndex(pub u32);
impl BossSummonIndex {
    pub fn num_spawns(&self) -> usize {
        match self.0 {
            0 => 8,
            1 => 12,
            2 => 18,
            3 => 24,
            _ => 24,
        }
    }
    pub fn num_poison_bombs(&self) -> usize {
        match self.0 {
            0 => 2,
            1 => 3,
            2 => 5,
            3 => 8,
            _ => 8,
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
        let summon_index = delayed_spawns.summon_index;
        commands.remove_resource::<DelayedSpawn>();
        if let Some(entity) = proto_commands.spawn_from_proto(
            delayed_spawns.mob.clone(),
            &proto.prototypes,
            delayed_spawns.pos,
        ) {
            commands
                .entity(entity)
                .insert(BossSummonIndex(summon_index));
        }
        commands.insert_resource(FlashEffect {
            timer: Timer::from_seconds(0.5, TimerMode::Once),
            color: Color::rgba(1., 1., 1., 1.),
        });
    }
}

pub fn update_boss_shrine_guide_cost(
    mut guides: Query<(&WorldObject, &mut InteractionGuideTrigger)>,
    summon_tracker: Res<BossSummonTracker>,
) {
    if !summon_tracker.is_changed() {
        return;
    }
    for (obj, mut guide) in guides.iter_mut() {
        if matches!(obj, WorldObject::BossShrine) {
            guide.icon_stack = Some(
                ItemStack::crate_icon_stack(WorldObject::Coin)
                    .copy_with_count(summon_tracker.current_cost() as usize),
            );
        }
    }
}
