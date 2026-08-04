use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;
use rand::Rng;

use crate::{
    colors::{LIGHT_RED, RED},
    custom_commands::CommandsExt,
    enemy::Mob,
    gamepad_input::GamepadAction,
    inventory::ItemStack,
    juice::{FlashEffect, ShakeEffect},
    player::{ModifyCurencyEvent, Player},
    proto::proto_param::ProtoParam,
    ui::{
        global_text_message::GlobalTextMessageEvent,
        key_input_guide::{InteractionGuideTrigger, SHRINE_INTERACT_GUIDE_DISTANCE},
    },
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

#[derive(Resource, Default)]
pub struct BossSummonTracker {
    pub summon_count: u32,
}

impl BossSummonTracker {
    /// Doubles each summon: 50, 100, 200, 400, ...
    pub fn current_cost(&self) -> i32 {
        BOSS_SUMMON_BASE_COST.saturating_mul(1_i32 << self.summon_count.min(30))
    }

    pub fn reset(&mut self) {
        self.summon_count = 0;
    }
    pub fn get_boss_tint(&self) -> Color {
        match self.summon_count - 1 {
            0 => Color::srgba(1., 1., 1., 1.0),
            1 => Color::srgba(0.2, 0.6, 1., 1.0),  // blue
            2 => Color::srgba(0.8, 0.4, 1.0, 1.0), // purple
            3 => Color::srgba(1.0, 0.4, 0.0, 1.0), // orange
            _ => Color::srgba(1.0, 0.0, 0.0, 1.0),
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
    key_input: Res<ButtonInput<KeyCode>>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    keybinds: Res<InputMappings>,
    player_query: Query<&GlobalTransform, With<Player>>,
    game: GameParam,
    mut game_camera: Query<Entity, With<TextureCamera>>,
    mut currency_event: MessageWriter<ModifyCurencyEvent>,
    mut global_text_events: MessageWriter<GlobalTextMessageEvent>,
    dungeon_check: Query<&Dungeon>,
    delayed_spawn: Option<Res<DelayedSpawn>>,
    mut summon_tracker: ResMut<BossSummonTracker>,
    gamepad_action_q: Query<&ActionState<GamepadAction>, With<Player>>,
) {
    if dungeon_check.single().is_ok() {
        return;
    }
    if delayed_spawn.is_some() {
        return;
    }
    let gamepad_pressed = gamepad_action_q
        .single()
        .map(|a| a.just_pressed(&GamepadAction::Interact))
        .unwrap_or(false);
    if keybinds.check_interact_input(&key_input, &mouse_input) || gamepad_pressed {
        let Ok(player_t) = player_query.single() else {
            return;
        };
        let Some(shrine) = game
            .world_obj_cache
            .unique_objs
            .get(&WorldObject::BossShrine)
        else {
            return;
        };
        let shrine_pos = tile_pos_to_world_pos(*shrine, false);
        let cost = summon_tracker.current_cost();

        if shrine_pos.distance(player_t.translation().truncate()) < SHRINE_INTERACT_GUIDE_DISTANCE
            && game.get_coins() as i32 >= cost
        {
            currency_event.write(ModifyCurencyEvent {
                delta: -cost,
                obj: WorldObject::Coin,
            });
            global_text_events
                .write(GlobalTextMessageEvent::new("WARNING!", LIGHT_RED).with_panel_width(164.0));
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
#[derive(Component, Clone, Copy)]
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
    /// AoE explosion spawn radius multiplier; +25% per subsequent shrine summon.
    pub fn aoe_radius_scale(&self) -> f32 {
        1.0 + self.0 as f32 * 0.25
    }

    /// HP multiplier keyed to this boss's shrine summon tier (not the live tracker count).
    pub fn health_scale(&self) -> f32 {
        match self.0 {
            0 => 1.0,
            1 => 1.5,
            2 => 3.0,
            3 => 7.5,
            _ => 7.5,
        }
    }

    /// Attack multiplier keyed to this boss's shrine summon tier.
    pub fn damage_scale(&self) -> f32 {
        match self.0 {
            0 => 1.0,
            1 => 1.5,
            2 => 2.0,
            3 => 4.0,
            _ => 4.0,
        }
    }

    /// Extra tail-attack projectile waves per subsequent shrine summon (2 per tier).
    pub fn scorpion_extra_tail_waves(&self) -> u8 {
        self.0.saturating_mul(2) as u8
    }

    /// Tornado spawn-rate multiplier (+20% per tier); shorter interval = more frequent spawns.
    pub fn scorpion_tornado_frequency_scale(&self) -> f32 {
        1.0 + self.0 as f32 * 0.20
    }

    /// Tornado count per spawn tick (+20% per tier, rounded up).
    pub fn scorpion_tornado_spawns_per_tick(&self) -> u32 {
        (1.0 + self.0 as f32 * 0.20).ceil() as u32
    }

    /// Lunge reach multiplier for the scorpion claw attack (+25% per tier).
    pub fn scorpion_lunge_scale(&self) -> f32 {
        1.0 + self.0 as f32 * 0.25
    }
}

pub fn handle_delayed_spawns(
    mut delayed_spawns: ResMut<DelayedSpawn>,
    mut commands: Commands,
    time: Res<Time>,
    proto: ProtoParam,
) {
    delayed_spawns.timer.tick(time.delta());
    if delayed_spawns.timer.is_finished() {
        let summon_index = delayed_spawns.summon_index;
        commands.remove_resource::<DelayedSpawn>();
        if let Some(entity) =
            commands.spawn_from_proto(delayed_spawns.mob.clone(), &proto.defs, delayed_spawns.pos)
        {
            commands
                .entity(entity)
                .insert(BossSummonIndex(summon_index));
        }
        commands.insert_resource(FlashEffect {
            timer: Timer::from_seconds(0.5, TimerMode::Once),
            color: Color::srgba(1., 1., 1., 1.),
        });
    }
}

/// Asset path for the standalone boss shrine sprite.
pub const BOSS_SHRINE_TEXTURE_PATH: &str = "textures/BossShrine.png";

/// Boss shrine uses a standalone PNG (not the shared atlas). Reset visibility/sprite after spawn.
///
/// We re-insert the texture handle, sprite, and a full visibility bundle here as a failsafe:
/// Spawn applies the `SpriteBundle`/`Visibility` on a deferred schedule, and
/// the order relative to our manual inserts in `spawn_object_from_proto` is not guaranteed. Forcing
/// all render components here guarantees a consistent, visible result.
pub fn ensure_boss_shrine_sprite_on_spawn(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    new_shrines: Query<(Entity, &WorldObject), Added<WorldObject>>,
) {
    for (entity, obj) in new_shrines.iter() {
        if obj != &WorldObject::BossShrine {
            continue;
        }
        let image: Handle<Image> = asset_server.load(BOSS_SHRINE_TEXTURE_PATH);
        commands.entity(entity).insert((
            Sprite {
                image,
                custom_size: Some(Vec2::new(128., 128.)),
                ..default()
            },
            Visibility::Inherited,
        ));
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
