mod enemy_hostile_basic;
pub mod pathfinding;

use crate::{
    client::is_not_paused,
    enemy::{
        fairy::{new_idle, trade_anim},
        red_mushking::{
            handle_death, new_follow, new_leap_attack, return_to_shrine, summon_attack,
        },
        red_mushling::{gas_attack, sprout},
    },
    GameState, Plugin,
};

use bevy::prelude::*;
pub use enemy_hostile_basic::*;
use pathfinding::{cache_ai_path_on_new_obj_spawn, spawn_new_debug_path, DebugPathResetEvent};
use seldom_state::StateMachinePlugin;

pub struct AIPlugin;

impl Plugin for AIPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<DebugPathResetEvent>()
            .add_plugin(StateMachinePlugin)
            .add_systems((spawn_new_debug_path,).in_set(OnUpdate(GameState::Main)))
            .add_systems(
                (
                    follow.run_if(is_not_paused),
                    new_follow.run_if(is_not_paused),
                    new_idle.run_if(is_not_paused),
                    handle_death.run_if(is_not_paused),
                    return_to_shrine.run_if(is_not_paused),
                    trade_anim.run_if(is_not_paused),
                    leap_attack.run_if(is_not_paused),
                    summon_attack.run_if(is_not_paused),
                    new_leap_attack.run_if(is_not_paused),
                    gas_attack.run_if(is_not_paused),
                    sprout.run_if(is_not_paused),
                    projectile_attack.run_if(is_not_paused),
                    tick_enemy_attack_cooldowns.run_if(is_not_paused),
                    idle.run_if(is_not_paused),
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                cache_ai_path_on_new_obj_spawn
                    .run_if(in_state(GameState::Main))
                    .in_base_set(CoreSet::PostUpdate),
            );
    }
}
