use bevy::{
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
    sprite_render::{AlphaMode2d, Material2d, Material2dPlugin},
};
use bevy_rapier2d::prelude::{Collider, CollisionGroups, Group};
use seldom_state::{
    prelude::{always, IntoTrigger, StateMachine},
    set::StateSet,
};
use serde::Deserialize;
use serde::Serialize;
use std::time::Duration;
use strum_macros::{Display, EnumIter, IntoStaticStr};

use crate::{
    ai::{
        cached_attack_distance, cached_line_of_sight, hurt_by_player, night_time_aggro,
        FollowState, IdleState, LeapAttackState, ProjectileAttackState,
    },
    attributes::{add_current_health_with_max_health, Attack, CurrentHealth, MaxHealth},
    chaos::{hp_multiplier_for_total_chaos, ChaosTracker},
    client::is_not_paused,
    colors::{BLACK, DARK_GREEN, GREY, LIGHT_BROWN, LIGHT_GREEN, PINK, RED},
    inputs::FacingDirection,
    item::{
        boss_shrine::{BossSummonIndex, BossSummonTracker},
        projectile::Projectile,
        Loot, LootTable, Wall, WorldObject,
    },
    night::{InfiniteMode, InfiniteModeMob, NightTracker},
    player::{
        beastiary::mob_display_name,
        levels::{ExperienceReward, PlayerLevel},
        Player,
    },
    ui::minimap::UpdateMiniMapEvent,
    world::{dungeon::Dungeon, TileMapPosition},
    AppExt, GameParam, GameState,
};

pub mod aseprite_enemy;
pub mod fairy;
pub mod red_mushking;
pub mod red_mushling;
pub mod scorpion;
pub mod spawn_helpers;
pub mod spawner;
pub mod stone_golem;
pub mod void_worm;
use self::spawner::SpawnerPlugin;
use aseprite_enemy::*;
use fairy::*;
use red_mushking::*;
use red_mushling::*;
// use stone_golem::*;

pub struct EnemyPlugin;

impl Plugin for EnemyPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(Material2dPlugin::<EnemyMaterial>::default())
            .with_default_schedule(FixedUpdate, |app| {
                app.add_message::<EnemySpawnEvent>();
            })
            .add_systems(Update, handle_boss_health_threshold)
            .add_systems(
                Update,
                (
                    handle_new_red_mushling_state_machine,
                    handle_new_red_mushking_state_machine,
                    scorpion::handle_new_scorpion_state_machine,
                    stone_golem::handle_new_stone_golem_state_machine,
                    handle_new_fairy_state_machine,
                    handle_new_mob_state_machine,
                    aseprite_enemy_setup,
                    handle_new_aseprite_enemy_state_machine,
                    red_mushling::handle_mushling_rush_warnings.run_if(is_not_paused),
                    juice_up_spawned_elite_mobs
                        .after(crate::defs::spawn::apply_pending_sprite_sheets)
                        .before(add_current_health_with_max_health),
                    juice_up_spawned_mobs_per_day.before(add_current_health_with_max_health),
                    juice_up_world_object_max_health_by_chaos
                        .before(add_current_health_with_max_health),
                    scale_boss_summon_stats
                        .after(juice_up_spawned_mobs_per_day)
                        .before(add_current_health_with_max_health),
                    enhance_infinite_mode_mobs.before(add_current_health_with_max_health),
                    enhance_infinite_mode_leap_attack_startup,
                )
                    .run_if(in_state(GameState::Main)),
            )
            .add_systems(
                Update,
                (
                    red_mushking::tick_attack_rotation.run_if(is_not_paused),
                    red_mushking::handle_aoe_attack.run_if(is_not_paused),
                    stone_golem::tick_spike_attack_timer
                        .run_if(is_not_paused)
                        .before(StateSet::Transition),
                    stone_golem::initialize_spike_attack_state.run_if(is_not_paused),
                    stone_golem::handle_spike_attack.run_if(is_not_paused),
                    stone_golem::initialize_wave_attack_state.run_if(is_not_paused),
                    stone_golem::spawn_delayed_wave_warnings.run_if(is_not_paused),
                    stone_golem::maintain_wave_attack.run_if(is_not_paused),
                    stone_golem::handle_spike_warnings
                        .run_if(is_not_paused)
                        .after(stone_golem::handle_spike_attack)
                        .after(stone_golem::initialize_wave_attack_state),
                    stone_golem::check_spike_attack_completion
                        .run_if(is_not_paused)
                        .after(stone_golem::handle_spike_warnings),
                    stone_golem::check_wave_attack_completion
                        .run_if(is_not_paused)
                        .after(stone_golem::handle_spike_warnings),
                    stone_golem::stone_golem_follow
                        .run_if(is_not_paused)
                        .after(stone_golem::maintain_wave_attack),
                    stone_golem::update_stone_golem_walk_animation.run_if(is_not_paused),
                    stone_golem::handle_stone_golem_death.run_if(is_not_paused),
                    red_mushling::handle_mushling_wakeup_timers.run_if(is_not_paused),
                )
                    .run_if(in_state(GameState::Main)),
            )
            .add_systems(
                Update,
                (
                    void_worm::void_worm_laser_attack.run_if(is_not_paused),
                    void_worm::cleanup_orphan_void_lasers.run_if(is_not_paused),
                )
                    .run_if(in_state(GameState::Main)),
            )
            .add_systems(
                Update,
                (
                    scorpion::scorpion_queue_next_attack.run_if(is_not_paused),
                    scorpion::scorpion_follow.run_if(is_not_paused),
                    scorpion::tick_scorpion_timers.run_if(is_not_paused),
                    scorpion::handle_claw_attack.run_if(is_not_paused),
                    scorpion::handle_tail_attack.run_if(is_not_paused),
                    scorpion::tick_tornado_timer.run_if(is_not_paused),
                    scorpion::handle_scorpion_death.run_if(is_not_paused),
                )
                    .run_if(in_state(GameState::Main)),
            )
            .add_plugins(SpawnerPlugin)
            .add_systems(Update, apply_pending_tint.run_if(in_state(GameState::Main)));
    }
}

#[derive(
    Component,
    Default,
    Debug,
    Clone,
    Hash,
    Display,
    Eq,
    PartialEq,
    Reflect,
    IntoStaticStr,
    EnumIter,
    Serialize,
    Deserialize,
)]
pub enum Mob {
    #[default]
    None,
    Slime,
    SpikeSlime,
    FurDevil,
    Hog,
    StingFly,
    Bushling,
    Fairy,
    RedMushling,
    RedMushking,
    StoneGolem,
    Crow,
    SmallCactus,
    BigCactus,
    Bull,
    Scorpion,
    Lizard,
    VoidCrawler,
    VoidWorm,
}

impl Mob {
    pub fn get_mob_color(&self) -> Color {
        match self {
            Mob::None => BLACK,
            Mob::Slime => LIGHT_GREEN,
            Mob::SpikeSlime => LIGHT_GREEN,
            Mob::Bushling => DARK_GREEN,
            Mob::StingFly => LIGHT_GREEN,
            Mob::Fairy => PINK,
            Mob::FurDevil => PINK,
            Mob::RedMushling => RED,
            Mob::RedMushking => RED,
            Mob::Hog => LIGHT_BROWN,
            Mob::StoneGolem => GREY,
            Mob::Crow => BLACK,
            Mob::SmallCactus => DARK_GREEN,
            Mob::BigCactus => DARK_GREEN,
            Mob::Bull => LIGHT_BROWN,
            Mob::Scorpion => LIGHT_BROWN,
            Mob::Lizard => DARK_GREEN,
            Mob::VoidCrawler => PINK,
            Mob::VoidWorm => PINK,
        }
    }
    pub fn get_base_kb(&self) -> f32 {
        match self {
            Mob::None => 0.,
            Mob::Slime => 0.,
            Mob::SpikeSlime => 80.,
            Mob::Bushling => 65.,
            Mob::StingFly => 100.,
            Mob::Fairy => 50.,
            Mob::FurDevil => 80.,
            Mob::Crow => 100.,
            Mob::RedMushling => 0.,
            Mob::RedMushking => 0.,
            Mob::StoneGolem => 10.,
            Mob::Hog => 50.,
            Mob::SmallCactus => 60.,
            Mob::BigCactus => 50.,
            Mob::Bull => 30.,
            Mob::Scorpion => 0.,
            Mob::Lizard => 100.,
            Mob::VoidCrawler => 70.,
            Mob::VoidWorm => 0.,
        }
    }
    pub fn is_boss(&self) -> bool {
        match self {
            Mob::RedMushking => true,
            Mob::StoneGolem => true,
            Mob::Scorpion => true,
            _ => false,
        }
    }
    pub fn get_boss_name(&self) -> Option<&'static str> {
        match self {
            _ => Some(mob_display_name(self)),
        }
    }

    /// Label for run stat UI (boss lore names, otherwise enum display).
    pub fn stat_tracker_display_name(&self) -> String {
        if let Some(name) = self.get_boss_name() {
            name.to_string()
        } else if *self == Mob::None {
            "Unknown".to_string()
        } else {
            self.to_string()
        }
    }
}
#[derive(Component, Default, Deserialize, Debug, Clone, Reflect, PartialEq, Eq)]
pub enum CombatAlignment {
    #[default]
    Passive,
    Neutral,
    Hostile,
}

#[derive(Component, Default, Deserialize, Debug, Clone, Reflect)]
pub struct EliteMob;

/// Applied after [`juice_up_spawned_elite_mobs`] so juice can retry if the
/// first `Added<EliteMob>` frame missed optional components.
#[derive(Component, Default, Debug, Clone)]
pub struct EliteMobJuiced;

#[derive(Component, Default, Deserialize, Debug, Clone, Reflect)]
pub struct FollowSpeed(pub f32);

#[derive(Message)]
pub struct EnemySpawnEvent {
    pub enemy: Mob,
    pub pos: TileMapPosition,
}

#[derive(Reflect, Default, Component, Clone, Debug, Copy)]
#[reflect(Component)]
pub struct MobLevel(pub u8);

#[derive(Debug, Default, Reflect, Clone, Component)]
#[reflect(Component, Default)]
pub struct LeapAttack {
    pub activation_distance: f32,
    pub duration: f32,
    pub cooldown: f32,
    pub startup: f32,
    pub speed: f32,
}

/// Inserted on a mob for the short window between "attack started" and
/// "attack ended" so other systems can detect the in-progress attack. Every
/// mob cycles through this on every attack; stored `SparseSet` so that
/// attacking does not move the mob between archetypes each time.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct MobIsAttacking(pub Mob);

/// Small Cactus attack config: spawns a circle hitbox in front of itself.
#[derive(Debug, Default, Reflect, Clone, Component, Deserialize)]
#[reflect(Component, Default)]
pub struct CircleAttack {
    pub activation_distance: f32,
    pub cooldown: f32,
    pub startup: f32,
    /// How far in front of the mob the hitbox spawns (pixels).
    pub hitbox_offset: f32,
    /// Delay after attack anim starts before hitbox spawns.
    pub hitbox_delay: f32,
    /// Radius of the circle collider.
    pub hitbox_radius: f32,
}

/// Big Cactus attack config: triple-hit leap.
#[derive(Debug, Reflect, Clone, Component, Deserialize)]
#[reflect(Component, Default)]
#[serde(default)]
pub struct MultiLeapAttack {
    pub activation_distance: f32,
    pub duration_per_hit: f32,
    pub cooldown: f32,
    pub startup: f32,
    pub speed: f32,
    pub num_hits: u8,
    /// Pause between successive lunges.
    pub pause_between_hits: f32,
    /// Per-lunge windup delay before movement begins (attack anim plays during this).
    pub lunge_delay: f32,
    /// Total wall-clock duration of the full attack tag (sum of frame delays). Ends the attack state when elapsed so the clip does not loop past once.
    pub attack_anim_duration: f32,
}

impl Default for MultiLeapAttack {
    fn default() -> Self {
        Self {
            activation_distance: 0.,
            duration_per_hit: 0.,
            cooldown: 0.,
            startup: 0.,
            speed: 0.,
            num_hits: 0,
            pause_between_hits: 0.,
            lunge_delay: 0.,
            attack_anim_duration: 1.6,
        }
    }
}

/// Bull charge attack config.
#[derive(Debug, Default, Reflect, Clone, Component, Deserialize)]
#[reflect(Component, Default)]
pub struct BullChargeAttack {
    pub activation_distance: f32,
    pub charge_speed: f32,
    pub startup: f32,
    pub cooldown: f32,
    /// Extra distance past the player position.
    pub overshoot: f32,
    /// How long the deceleration/stop phase lasts.
    pub stop_duration: f32,
}

#[derive(Reflect, Clone, Component, Deserialize)]
#[reflect(Component, Default)]
#[serde(default)]
pub struct ProjectileAttack {
    pub activation_distance: f32,
    pub cooldown: f32,
    pub projectile: Projectile,
    /// Delay before the attack animation starts (enemy stays in walk anim; attack warning shows).
    pub attack_startup: f32,
    /// Delay after the attack animation starts before the projectile actually spawns.
    pub projectile_delay: f32,
}

impl Default for ProjectileAttack {
    fn default() -> Self {
        Self {
            activation_distance: 0.,
            cooldown: 0.,
            projectile: Projectile::default(),
            attack_startup: 0.3,
            projectile_delay: 0.,
        }
    }
}

/// Void Worm laser attack config. The worm walks toward the player, then stops at
/// a random point between `min_stop_distance` and `max_stop_distance` and fires a
/// stationary laser (a separate aseprite) in a random cardinal direction for
/// `laser_duration` seconds, then walks for `walk_duration` seconds before
/// repeating. See [`crate::enemy::void_worm`].
#[derive(Debug, Reflect, Clone, Component, Deserialize)]
#[reflect(Component, Default)]
#[serde(default)]
pub struct LaserAttack {
    /// Closest the worm will stop before firing.
    pub min_stop_distance: f32,
    /// Farthest the worm will stop before firing (also the follow->laser trigger range).
    pub max_stop_distance: f32,
    /// How long the laser stays active each cycle (seconds).
    pub laser_duration: f32,
    /// How long the worm walks between laser cycles (seconds).
    pub walk_duration: f32,
}

impl Default for LaserAttack {
    fn default() -> Self {
        Self {
            min_stop_distance: 100.,
            max_stop_distance: 200.,
            laser_duration: 4.,
            walk_duration: 3.,
        }
    }
}
pub fn handle_new_mob_state_machine(
    mut commands: Commands,
    game: GameParam,
    spawn_events: Query<
        (
            Entity,
            &Mob,
            &CombatAlignment,
            &FollowSpeed,
            Option<&LeapAttack>,
            Option<&ProjectileAttack>,
        ),
        Or<(Added<Mob>, Added<CombatAlignment>, Changed<CombatAlignment>)>,
    >,
    dungeon_check: Query<&Dungeon>,
) {
    for (e, mob, alignment, follow_speed, leap_attack_option, proj_attack_option) in
        spawn_events.iter()
    {
        let mut alignment = alignment.clone();
        commands
            .entity(e)
            .insert(CollisionGroups::new(Group::GROUP_1, Group::GROUP_1));
        if dungeon_check.single().is_ok() {
            alignment = CombatAlignment::Hostile;
        }
        // Skip enemies handled by dedicated state machine systems
        if mob == &Mob::RedMushling
            || mob.is_boss()
            || mob == &Mob::Fairy
            || aseprite_enemy::is_aseprite_basic_mob(mob)
        {
            continue;
        }
        let mut e_cmds = commands.entity(e);
        let mut state_machine = StateMachine::default().set_trans_logging(false);
        match alignment {
            CombatAlignment::Neutral => {
                state_machine = state_machine
                    .trans::<IdleState, _>(
                        hurt_by_player,
                        FollowState {
                            target: game.game.player,
                            curr_delta: None,
                            curr_path: None,
                            speed: follow_speed.0,
                        },
                    )
                    .trans::<FollowState, _>(
                        cached_line_of_sight(130. * 130.).not(),
                        IdleState {
                            walk_timer: Timer::from_seconds(2., TimerMode::Repeating),
                            direction: FacingDirection::new_rand_dir(rand::thread_rng()),
                            speed: 0.5,
                            is_stopped: false,
                        },
                    );
            }
            CombatAlignment::Hostile => {
                // Hostiles chase immediately — no aggro/LoS distance gate.
                state_machine = state_machine.trans::<IdleState, _>(
                    always,
                    FollowState {
                        target: game.game.player,
                        curr_delta: None,
                        curr_path: None,
                        speed: follow_speed.0,
                    },
                );
            }
            CombatAlignment::Passive => {
                //TODO: impl run away
            }
        }
        if let Some(leap_attack) = leap_attack_option {
            // Once LeapAttack starts, `leap_attack` owns the full cycle (startup → lunge →
            // cooldown). Do NOT cancel back to Follow when the player leaves range: on 0.19
            // the `.not()` distance trigger also fires on cache misses, which aborted the
            // lunge and left `EnemyAnimationState::Attack` looping forever.
            state_machine = state_machine.trans::<FollowState, _>(
                cached_attack_distance(
                    leap_attack.activation_distance * leap_attack.activation_distance,
                ),
                LeapAttackState {
                    target: game.game.player,
                    attack_startup_timer: Timer::from_seconds(leap_attack.startup, TimerMode::Once),
                    attack_duration_timer: Timer::from_seconds(
                        leap_attack.duration,
                        TimerMode::Once,
                    ),
                    attack_cooldown_timer: Timer::from_seconds(
                        leap_attack.cooldown,
                        TimerMode::Once,
                    ),
                    dir: None,
                    speed: leap_attack.speed,
                    attack_preview_entity: None,
                },
            );
        }
        if let Some(proj_attack) = proj_attack_option {
            state_machine = state_machine
                .trans::<FollowState, _>(
                    cached_attack_distance(
                        proj_attack.activation_distance * proj_attack.activation_distance,
                    ),
                    ProjectileAttackState {
                        target: game.game.player,
                        attack_startup_timer: Timer::from_seconds(
                            proj_attack.attack_startup,
                            TimerMode::Once,
                        ),
                        attack_cooldown_timer: Timer::from_seconds(
                            proj_attack.cooldown,
                            TimerMode::Once,
                        ),
                        projectile_delay_timer: Timer::from_seconds(
                            proj_attack.projectile_delay,
                            TimerMode::Once,
                        ),
                        dir: None,
                        projectile: proj_attack.projectile.clone(),
                    },
                )
                .trans::<ProjectileAttackState, _>(
                    cached_attack_distance((proj_attack.activation_distance + 30.).powi(2)).not(),
                    FollowState {
                        target: game.game.player,
                        curr_delta: None,
                        curr_path: None,
                        speed: follow_speed.0,
                    },
                );
            if let Some(leap_attack) = leap_attack_option {
                state_machine = state_machine.trans::<ProjectileAttackState, _>(
                    cached_attack_distance(
                        leap_attack.activation_distance * leap_attack.activation_distance,
                    ),
                    FollowState {
                        target: game.game.player,
                        curr_delta: None,
                        curr_path: None,
                        speed: follow_speed.0,
                    },
                );
            }
        }
        if alignment != CombatAlignment::Passive {
            state_machine = state_machine.trans::<IdleState, _>(
                night_time_aggro,
                FollowState {
                    target: game.game.player,
                    curr_delta: None,
                    curr_path: None,
                    speed: follow_speed.0,
                },
            );
        }
        e_cmds.insert(state_machine);
    }
}
fn handle_mob_move_minimap_update(
    _moving_enemies: Query<(Entity, &GlobalTransform), (With<Mob>, Changed<GlobalTransform>)>,
    mut _minimap_event: MessageWriter<UpdateMiniMapEvent>,
) {
    return;
    // if moving_enemies.iter().count() > 0 {
    //     minimap_event.write(UpdateMiniMapEvent {
    //         pos: None,
    //         new_tile: None,
    //     });
    // }
}
fn juice_up_spawned_elite_mobs(
    mut elites: Query<
        (
            Entity,
            &Mob,
            &mut MaxHealth,
            &mut Attack,
            &mut ExperienceReward,
            &mut LootTable,
            &mut Sprite,
            Option<&mut CurrentHealth>,
        ),
        (With<EliteMob>, Without<EliteMobJuiced>),
    >,
    mut commands: Commands,
    defs: Res<crate::defs::GameDefs>,
) {
    // Sheet mobs spawn with `PendingSpriteSheet`; requiring `Sprite` waits until that resolves.
    // Visual size must match 0.10: ONLY `custom_size = 48` on 32px sheets (1.5×). Do NOT also
    // set `Transform.scale` — that compounds to ~2.25×, and `BounceOnHit` then snaps scale back
    // to `rest_scale` (1.0) after the first hit, which looked like "starts huge, then shrinks".
    const COLLIDER_AND_STAT_SCALE: f32 = 1.5;
    const ELITE_SPRITE_SIZE: Vec2 = Vec2::new(48., 48.);
    for (e, mob, mut hp, mut att, mut exp, mut loot, mut sprite, maybe_current_hp) in
        elites.iter_mut()
    {
        hp.0 = (hp.0 as f32 * 5.) as i32;
        att.0 = (att.0 as f32 * COLLIDER_AND_STAT_SCALE) as i32;
        exp.0 = (exp.0 as f32 * 3.) as u32;
        loot.drops = loot
            .drops
            .iter()
            .map(|l| Loot {
                item: l.item,
                min: l.min,
                max: l.max,
                rate: l.rate * 3.,
            })
            .collect();
        if let Some(collider) = defs
            .get_mob_def(mob.clone())
            .and_then(|d| d.scaled_capsule_collider(COLLIDER_AND_STAT_SCALE))
        {
            commands.entity(e).insert(collider);
        }
        sprite.custom_size = Some(ELITE_SPRITE_SIZE);
        if let Some(mut current_hp) = maybe_current_hp {
            current_hp.0 = hp.0;
        } else {
            commands.entity(e).insert(CurrentHealth(hp.0));
        }
        commands.entity(e).insert(EliteMobJuiced);
        info!(
            "Elite juiced: {mob:?} e={e:?} hp={} atk={} sprite_size={ELITE_SPRITE_SIZE:?}",
            hp.0, att.0
        );
    }
}

fn juice_up_spawned_mobs_per_day(
    mut elites: Query<
        (
            Entity,
            &mut MaxHealth,
            &mut Attack,
            &Mob,
            Option<&mut CurrentHealth>,
        ),
        Added<Mob>,
    >,
    night_tracker: Res<NightTracker>,
    chaos_tracker: Option<Res<ChaosTracker>>,
    infinite_mode: Res<InfiniteMode>,
    player_level: Query<&PlayerLevel>,
    in_dungeon: Query<&Dungeon, With<crate::world::dimension::ActiveDimension>>,
    mut commands: Commands,
) {
    let dungeon_chaos_multiplier = if in_dungeon.single().is_ok() { 2. } else { 1. };
    for (e, mut hp, mut att, mob, maybe_current_hp) in elites.iter_mut() {
        let global_chaos = chaos_tracker.as_ref().map(|c| c.get_chaos()).unwrap_or(0.0);
        let (hp_multiplier, attack_multiplier, total_chaos, infinite_chaos) =
            if infinite_mode.active {
                let infinite_chaos = infinite_mode.get_chaos_bonus();
                let total_chaos = (global_chaos + infinite_chaos) * dungeon_chaos_multiplier;
                (
                    InfiniteMode::endless_mob_hp_multiplier(total_chaos),
                    InfiniteMode::endless_mob_attack_multiplier(total_chaos),
                    total_chaos,
                    infinite_chaos,
                )
            } else {
                let total_chaos = global_chaos * dungeon_chaos_multiplier;
                let chaos_factor = 1. + total_chaos;
                (
                    hp_multiplier_for_total_chaos(total_chaos),
                    chaos_factor.powf(0.56),
                    total_chaos,
                    0.0,
                )
            };

        hp.0 = (hp.0 as f32 * hp_multiplier) as i32;
        att.0 = (att.0 as f32 * attack_multiplier) as i32;
        // Keep current HP filled after spawn juicing (same as elites). Otherwise
        // `add_current_health_with_max_health` may have already stamped CurrentHealth
        // from the pre-juice max, leaving bosses at ~base/max (e.g. 20%).
        if let Some(mut current_hp) = maybe_current_hp {
            current_hp.0 = hp.0;
        } else {
            commands.entity(e).insert(CurrentHealth(hp.0));
        }
        debug!(
            "[{}] chaos_factor: {} (days: {}, level: {}, global_chaos: {:.1}, infinite_chaos: {:.1}, endless: {}) |||| {:?} {:?}",
            mob,
            1. + total_chaos,
            night_tracker.days,
            player_level.single().map(|level| level.level as f32 * 0.2).unwrap_or(0.),
            global_chaos,
            infinite_chaos,
            infinite_mode.active,
            hp.0,
            att.0
        );
        commands.entity(e).insert(MobLevel(night_tracker.days + 1));
    }
}

/// Scale breakable world objects' max HP (trees, rocks, stumps, etc.) with chaos, same formula as mobs.
fn juice_up_world_object_max_health_by_chaos(
    mut q: Query<
        &mut MaxHealth,
        (
            Added<WorldObject>,
            Without<Mob>,
            Without<Player>,
            Without<Wall>,
        ),
    >,
    chaos_tracker: Option<Res<ChaosTracker>>,
    infinite_mode: Res<InfiniteMode>,
) {
    let global_chaos = chaos_tracker.as_ref().map(|c| c.get_chaos()).unwrap_or(0.0);
    let total_chaos = global_chaos + infinite_mode.get_chaos_bonus();
    let hp_multiplier = hp_multiplier_for_total_chaos(total_chaos);
    for mut hp in q.iter_mut() {
        hp.0 = (hp.0 as f32 * hp_multiplier) as i32;
    }
}

/// Pending tint to apply once [`Sprite`] is available (sheet mobs resolve a frame late).
#[derive(Component)]
pub struct PendingTint(pub Color);

/// Extra HP/attack scaling for bosses summoned multiple times per era.
/// The first summon (index 0) gets no bonus; each subsequent one scales up.
fn scale_boss_summon_stats(
    mut bosses: Query<
        (
            Entity,
            &mut MaxHealth,
            &mut Attack,
            &BossSummonIndex,
            &Mob,
            Option<&mut CurrentHealth>,
        ),
        Added<BossSummonIndex>,
    >,
    tracker: Res<BossSummonTracker>,
    mut commands: Commands,
) {
    for (e, mut hp, mut att, summon_idx, mob, maybe_current_hp) in bosses.iter_mut() {
        let idx = summon_idx.0;
        if idx == 0 {
            continue;
        }
        let hp_scale = summon_idx.health_scale();
        let att_scale = summon_idx.damage_scale();
        hp.0 = (hp.0 as f32 * hp_scale) as i32;
        att.0 = (att.0 as f32 * att_scale) as i32;
        if let Some(mut current_hp) = maybe_current_hp {
            current_hp.0 = hp.0;
        } else {
            commands.entity(e).insert(CurrentHealth(hp.0));
        }
        commands
            .entity(e)
            .insert(PendingTint(tracker.get_boss_tint()));
        info!(
            "[{}] Boss summon #{}: hp_scale={:.2} att_scale={:.2} -> hp={} att={} tint={:?}",
            mob,
            idx,
            hp_scale,
            att_scale,
            hp.0,
            att.0,
            tracker.get_boss_tint()
        );
    }
}

/// Applies a PendingTint once the [`Sprite`] becomes available (after pending
/// sheet resolution). Catches both: sprite added after tint, or tint after sprite.
fn apply_pending_tint(
    mut query: Query<(Entity, &mut Sprite, &PendingTint)>,
    mut commands: Commands,
) {
    for (entity, mut sprite, pending) in query.iter_mut() {
        sprite.color = pending.0;
        commands.entity(entity).remove::<PendingTint>();
    }
}

impl Material2d for EnemyMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/enemy_attack.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

#[derive(Asset, AsBindGroup, Reflect, Default, Debug, Clone)]
#[reflect(Default, Debug)]
pub struct EnemyMaterial {
    #[uniform(0)]
    pub is_attacking: f32,
    #[texture(1)]
    #[sampler(2)]
    pub source_texture: Option<Handle<Image>>,
}

/// Enhance mobs spawned during infinite mode with red tint and speed boost based on difficulty level
fn enhance_infinite_mode_mobs(
    mut mobs: Query<(Entity, &mut FollowSpeed, Option<&mut Sprite>), Added<InfiniteModeMob>>,
    infinite_mode: Res<InfiniteMode>,
    mut commands: Commands,
) {
    for (entity, mut follow_speed, maybe_sprite) in mobs.iter_mut() {
        // Apply speed boost based on current difficulty level
        let speed_multiplier = infinite_mode.get_speed_multiplier();
        follow_speed.0 *= speed_multiplier;

        let tint_alpha = infinite_mode.get_tint_alpha();
        let tier = infinite_mode.get_tier();
        let tint_color = if tier == 1 {
            let r = 1.0 - (tint_alpha * 0.2);
            let g = 1.0 - (tint_alpha * 0.6);
            Color::srgba(r, g, 1.0, 1.0)
        } else {
            let green_blue = 1.0 - (tint_alpha * 0.5);
            Color::srgba(1.0, green_blue, green_blue, 1.0)
        };
        if let Some(mut sprite) = maybe_sprite {
            sprite.color = tint_color;
        } else {
            commands.entity(entity).insert(PendingTint(tint_color));
        }

        // Store the difficulty level this mob was spawned at for reference
        commands.entity(entity).insert(InfiniteMobTint {
            difficulty_level: infinite_mode.difficulty_level,
        });

        let tier = infinite_mode.get_tier();
        let tier_name = if tier == 1 { "Purple" } else { "Red" };
        debug!(
            "Enhanced infinite mode mob: speed {:.1}x, tint alpha {:.1} (difficulty {}, Tier {} - {})",
            speed_multiplier, tint_alpha, infinite_mode.difficulty_level, tier, tier_name
        );
    }
}

/// Marker component for infinite mode mob tint (stores the difficulty level when spawned)
#[derive(Component, Debug, Clone)]
pub struct InfiniteMobTint {
    pub difficulty_level: u8,
}

/// Scale up LeapAttack startup speed for infinite mode mobs
/// This runs when LeapAttackState is added to ensure the startup timer is scaled correctly
fn enhance_infinite_mode_leap_attack_startup(
    mut leap_attack_states: Query<
        &mut LeapAttackState,
        (Added<LeapAttackState>, With<InfiniteModeMob>),
    >,
    infinite_mode: Res<InfiniteMode>,
) {
    if !infinite_mode.active {
        return;
    }

    let speed_multiplier = infinite_mode.get_speed_multiplier();
    let speed_multiplier = speed_multiplier.max(0.001); // avoid division by zero / negative
    for mut leap_state in leap_attack_states.iter_mut() {
        // Scale down the startup timer duration (faster startup = shorter duration)
        // Divide by speed multiplier to make it faster
        let original_duration = leap_state.attack_startup_timer.duration();
        let original_elapsed = leap_state.attack_startup_timer.elapsed();
        let orig_dur_secs = original_duration.as_secs_f32();
        if orig_dur_secs <= 0.0 {
            continue; // skip invalid timer
        }
        let scaled_duration_secs = orig_dur_secs / speed_multiplier;

        // Create new timer with scaled duration
        let mut new_timer = Timer::from_seconds(scaled_duration_secs, TimerMode::Once);
        // Preserve the elapsed time proportionally (scale elapsed by the same factor)
        let elapsed_ratio = (original_elapsed.as_secs_f32() / orig_dur_secs).clamp(0.0, 1.0);
        let scaled_elapsed_secs = (scaled_duration_secs * elapsed_ratio).min(scaled_duration_secs);
        let tick_secs = scaled_elapsed_secs.max(0.0); // Duration::from_secs_f32 panics on negative
        new_timer.tick(Duration::from_secs_f32(tick_secs));

        leap_state.attack_startup_timer = new_timer;

        debug!(
            "Scaled LeapAttack startup for infinite mode mob: {:.2}s -> {:.2}s ({}x speed)",
            original_duration.as_secs_f32(),
            scaled_duration_secs,
            speed_multiplier
        );
    }
}
