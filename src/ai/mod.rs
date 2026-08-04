mod enemy_hostile_basic;
pub mod pathfinding;
pub mod steering;

use crate::{
    client::is_not_paused,
    enemy::{
        aseprite_enemy,
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
use seldom_state::{set::StateSet, StateMachinePlugin};
use steering::{build_enemy_spatial_grid, EnemySpatialGrid};

pub struct AIPlugin;

impl Plugin for AIPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EnemyAICacheMap>()
            .init_resource::<EnemySpatialGrid>()
            // seldom_state 0.17 runs transitions in PostUpdate by default. Keep the
            // LoS/attack-distance cache in that same schedule, immediately before
            // Transition, so triggers never see a stale/empty map.
            .add_plugins(StateMachinePlugin::default())
            .add_systems(
                PostUpdate,
                update_enemy_ai_cache
                    .before(StateSet::Transition)
                    .run_if(in_state(GameState::Main)),
            )
            .add_systems(
                Update,
                build_enemy_spatial_grid
                    .run_if(is_not_paused)
                    .run_if(in_state(GameState::Main)),
            )
            .add_systems(
                Update,
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
                    .after(build_enemy_spatial_grid)
                    .run_if(in_state(GameState::Main)),
            )
            .add_systems(
                Update,
                (
                    aseprite_enemy::aseprite_follow.run_if(is_not_paused),
                    aseprite_enemy::aseprite_idle.run_if(is_not_paused),
                    aseprite_enemy::aseprite_hit_react.run_if(is_not_paused),
                    aseprite_enemy::aseprite_leap_attack.run_if(is_not_paused),
                    aseprite_enemy::aseprite_projectile_attack.run_if(is_not_paused),
                    aseprite_enemy::aseprite_circle_attack.run_if(is_not_paused),
                    aseprite_enemy::aseprite_multi_leap_attack.run_if(is_not_paused),
                    aseprite_enemy::aseprite_bull_charge.run_if(is_not_paused),
                )
                    .after(build_enemy_spatial_grid)
                    .run_if(in_state(GameState::Main)),
            );
    }
}
