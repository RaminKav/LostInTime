use bevy::{
    prelude::*,
    reflect::TypeUuid,
    render::render_resource::{AsBindGroup, ShaderRef},
    sprite::{Material2d, Material2dPlugin},
    utils::Duration,
};
use bevy_proto::prelude::{ReflectSchematic, Schematic};
use bevy_rapier2d::prelude::{Collider, CollisionGroups, Group};
use seldom_state::prelude::{StateMachine, Trigger};
use serde::Deserialize;
use serde::Serialize;
use strum_macros::{Display, EnumIter, IntoStaticStr};

use crate::{
    ai::{
        AttackDistance, FollowState, HurtByPlayer, IdleState, LeapAttackState, LineOfSight,
        NightTimeAggro, ProjectileAttackState,
    },
    attributes::{add_current_health_with_max_health, Attack, MaxHealth},
    chaos::ChaosTracker,
    colors::{BLACK, DARK_GREEN, GREY, LIGHT_BROWN, LIGHT_GREEN, PINK, RED},
    inputs::FacingDirection,
    item::{projectile::Projectile, Loot, LootTable},
    night::{InfiniteModeMob, NightTracker},
    player::{
        levels::{ExperienceReward, PlayerLevel},
        skills::PlayerSkills,
    },
    proto::{proto_param::ProtoParam, ColliderCapsulProto},
    ui::minimap::UpdateMiniMapEvent,
    world::{dimension::EraManager, dungeon::Dungeon, TileMapPosition},
    AppExt, GameParam, GameState,
};

pub mod fairy;
pub mod red_mushking;
pub mod red_mushling;
pub mod spawn_helpers;
pub mod spawner;
pub mod stone_golem;
use self::spawner::SpawnerPlugin;
use fairy::*;
use red_mushking::*;
use red_mushling::*;
// use stone_golem::*;

pub struct EnemyPlugin;

impl Plugin for EnemyPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(Material2dPlugin::<EnemyMaterial>::default())
            .with_default_schedule(CoreSchedule::FixedUpdate, |app| {
                app.add_event::<EnemySpawnEvent>();
            })
            .add_system(handle_boss_health_threshold.in_base_set(CoreSet::PreUpdate))
            .add_systems(
                (
                    handle_new_red_mushling_state_machine,
                    handle_new_red_mushking_state_machine,
                    stone_golem::handle_new_stone_golem_state_machine,
                    handle_new_fairy_state_machine,
                    handle_new_mob_state_machine,
                    red_mushling::handle_mushling_rush_warnings,
                    handle_mob_move_minimap_update,
                    juice_up_spawned_elite_mobs.before(add_current_health_with_max_health),
                    juice_up_spawned_mobs_per_day.before(add_current_health_with_max_health),
                    enhance_infinite_mode_mobs.before(add_current_health_with_max_health),
                    enhance_infinite_mode_leap_attack_startup,
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_systems(
                (
                    red_mushking::tick_aoe_attack_timer,
                    red_mushking::handle_aoe_attack,
                    stone_golem::tick_spike_attack_timer,
                    stone_golem::handle_spike_attack,
                    stone_golem::update_stone_golem_walk_animation,
                    stone_golem::handle_stone_golem_death,
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_plugin(SpawnerPlugin);
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
    Schematic,
    Reflect,
    FromReflect,
    IntoStaticStr,
    EnumIter,
    Serialize,
    Deserialize,
)]
#[reflect(Schematic)]
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
        }
    }
    pub fn get_base_kb(&self) -> f32 {
        match self {
            Mob::None => 0.,
            Mob::Slime => 0.,
            Mob::SpikeSlime => 40.,
            Mob::Bushling => 25.,
            Mob::StingFly => 50.,
            Mob::Fairy => 50.,
            Mob::FurDevil => 50.,
            Mob::RedMushling => 0.,
            Mob::RedMushking => 0.,
            Mob::StoneGolem => 0.,
            Mob::Hog => 50.,
        }
    }
    pub fn is_boss(&self) -> bool {
        match self {
            Mob::RedMushking => true,
            Mob::StoneGolem => true,
            _ => false,
        }
    }
    pub fn get_boss_name(&self) -> Option<&'static str> {
        match self {
            Mob::RedMushking => Some("Red Mushking"),
            Mob::StoneGolem => Some("Blake Boulder"),
            _ => None,
        }
    }
}
#[derive(
    Component, Default, Deserialize, Debug, Clone, Schematic, Reflect, FromReflect, PartialEq, Eq,
)]
#[reflect(Schematic)]
pub enum CombatAlignment {
    #[default]
    Passive,
    Neutral,
    Hostile,
}

#[derive(Component, Default, Deserialize, Debug, Clone, FromReflect, Schematic, Reflect)]
#[reflect(Schematic)]
pub struct EliteMob;

#[derive(Component, Default, Deserialize, Debug, Clone, Schematic, Reflect, FromReflect)]
#[reflect(Schematic)]
pub struct FollowSpeed(pub f32);

pub struct EnemySpawnEvent {
    pub enemy: Mob,
    pub pos: TileMapPosition,
}

#[derive(Reflect, FromReflect, Default, Schematic, Component, Clone, Debug, Copy)]
#[reflect(Component, Schematic)]
pub struct MobLevel(pub u8);

#[derive(FromReflect, Debug, Default, Reflect, Clone, Component, Schematic)]
#[reflect(Component, Schematic, Default)]
pub struct LeapAttack {
    pub activation_distance: f32,
    pub duration: f32,
    pub cooldown: f32,
    pub startup: f32,
    pub speed: f32,
}

#[derive(Component)]
pub struct MobIsAttacking(pub Mob);

#[derive(FromReflect, Default, Reflect, Clone, Component, Schematic)]
#[reflect(Component, Schematic, Default)]
pub struct ProjectileAttack {
    pub activation_distance: f32,
    pub cooldown: f32,
    pub projectile: Projectile,
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
        if dungeon_check.get_single().is_ok() {
            alignment = CombatAlignment::Hostile;
        }
        if mob == &Mob::RedMushling || mob.is_boss() || mob == &Mob::Fairy {
            continue;
        }
        let mut e_cmds = commands.entity(e);
        let mut state_machine = StateMachine::default().set_trans_logging(false);
        match alignment {
            CombatAlignment::Neutral => {
                state_machine = state_machine
                    .trans::<IdleState>(
                        HurtByPlayer,
                        FollowState {
                            target: game.game.player,
                            curr_delta: None,
                            curr_path: None,
                            speed: follow_speed.0,
                        },
                    )
                    .trans::<FollowState>(
                        Trigger::not(LineOfSight {
                            target: game.game.player,
                            range: 130.,
                        }),
                        IdleState {
                            walk_timer: Timer::from_seconds(2., TimerMode::Repeating),
                            direction: FacingDirection::new_rand_dir(rand::thread_rng()),
                            speed: 0.5,
                            is_stopped: false,
                        },
                    );
            }
            CombatAlignment::Hostile => {
                state_machine = state_machine.trans::<IdleState>(
                    LineOfSight {
                        target: game.game.player,
                        range: 130.,
                    },
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
            state_machine = state_machine
                .trans::<FollowState>(
                    AttackDistance {
                        target: game.game.player,
                        range: leap_attack.activation_distance,
                    },
                    LeapAttackState {
                        target: game.game.player,
                        attack_startup_timer: Timer::from_seconds(
                            leap_attack.startup,
                            TimerMode::Once,
                        ),
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
                    },
                )
                .trans::<LeapAttackState>(
                    Trigger::not(AttackDistance {
                        target: game.game.player,
                        range: leap_attack.activation_distance + 32.,
                    }),
                    FollowState {
                        target: game.game.player,
                        curr_delta: None,
                        curr_path: None,
                        speed: follow_speed.0,
                    },
                );
        }
        if let Some(proj_attack) = proj_attack_option {
            state_machine = state_machine
                .trans::<FollowState>(
                    AttackDistance {
                        target: game.game.player,
                        range: proj_attack.activation_distance,
                    },
                    ProjectileAttackState {
                        target: game.game.player,
                        attack_startup_timer: Timer::from_seconds(0.3, TimerMode::Once),
                        attack_cooldown_timer: Timer::from_seconds(
                            proj_attack.cooldown,
                            TimerMode::Once,
                        ),
                        dir: None,
                        projectile: proj_attack.projectile.clone(),
                    },
                )
                .trans::<ProjectileAttackState>(
                    Trigger::not(AttackDistance {
                        target: game.game.player,
                        range: proj_attack.activation_distance + 30.,
                    }),
                    FollowState {
                        target: game.game.player,
                        curr_delta: None,
                        curr_path: None,
                        speed: follow_speed.0,
                    },
                );
            if let Some(leap_attack) = leap_attack_option {
                state_machine = state_machine.trans::<ProjectileAttackState>(
                    AttackDistance {
                        target: game.game.player,
                        range: leap_attack.activation_distance,
                    },
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
            state_machine = state_machine.trans::<IdleState>(
                NightTimeAggro,
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
    moving_enemies: Query<(Entity, &GlobalTransform), (With<Mob>, Changed<GlobalTransform>)>,
    mut minimap_event: EventWriter<UpdateMiniMapEvent>,
) {
    if moving_enemies.iter().count() > 0 {
        minimap_event.send(UpdateMiniMapEvent {
            pos: None,
            new_tile: None,
        });
    }
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
            &mut TextureAtlasSprite,
        ),
        Added<EliteMob>,
    >,
    mut commands: Commands,
    proto: ProtoParam,
) {
    for (e, mob, mut hp, mut att, mut exp, mut loot, mut sprite) in elites.iter_mut() {
        hp.0 = (hp.0 as f32 * 5.) as i32;
        att.0 = (att.0 as f32 * 1.5) as i32;
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
        let collider_scale_up = 1.5;
        let mut collider_proto = proto
            .get_component::<ColliderCapsulProto, _>(mob.clone())
            .expect("mob should have collider")
            .clone();
        collider_proto.scale(collider_scale_up);
        let collider: Collider = collider_proto.clone().into();
        commands.entity(e).insert(collider);
        sprite.custom_size = Some(Vec2::new(48., 48.));
    }
}

fn juice_up_spawned_mobs_per_day(
    mut elites: Query<
        (
            Entity,
            &mut MaxHealth,
            &mut Attack,
            &mut ExperienceReward,
            &Mob,
        ),
        Added<Mob>,
    >,
    night_tracker: Res<NightTracker>,
    chaos_tracker: Option<Res<ChaosTracker>>,
    infinite_mode: Res<crate::night::InfiniteMode>,
    player_level: Query<&PlayerLevel>,
    mut commands: Commands,
) {
    // Get total chaos from tracker (all sources now increment the tracker)
    let global_chaos = chaos_tracker.as_ref().map(|c| c.get_chaos()).unwrap_or(0.0) * 1.5;
    // Get infinite mode chaos bonus (only applies during infinite mode, not carried to next era)
    let infinite_chaos = infinite_mode.get_chaos_bonus() * 1.5;
    let total_chaos = global_chaos + infinite_chaos;

    for (e, mut hp, mut att, mut exp, _mob) in elites.iter_mut() {
        // 1. per day, 0.2 per level, 1 per heirloom, 1 per totem,
        let chaos_factor = 1.
            + night_tracker.days as f32
            + (player_level.single().level as f32 * 0.2)
            + total_chaos;
        info!(
            "chaos_factor: {} (days: {}, level: {}, global_chaos: {:.1}, infinite_chaos: {:.1})",
            chaos_factor,
            night_tracker.days,
            player_level.single().level,
            global_chaos,
            infinite_chaos
        );
        hp.0 = (hp.0 as f32 * (chaos_factor.powf(1.1))) as i32;
        att.0 = (att.0 as f32 * (chaos_factor.powf(0.7))) as i32;
        exp.0 = (exp.0 as f32 * (1. + chaos_factor * 0.175)) as u32;
        commands.entity(e).insert(MobLevel(night_tracker.days + 1));
    }
}

impl Material2d for EnemyMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/enemy_attack.wgsl".into()
    }
}

#[derive(AsBindGroup, TypeUuid, Reflect, FromReflect, Default, Debug, Clone)]
#[reflect(Default, Debug)]
#[uuid = "a04064b6-dcdd-11ed-afa1-0242ac120002"]
pub struct EnemyMaterial {
    #[uniform(0)]
    pub is_attacking: f32,
    #[texture(1)]
    #[sampler(2)]
    pub source_texture: Option<Handle<Image>>,
}

/// Enhance mobs spawned during infinite mode with red tint and speed boost based on difficulty level
fn enhance_infinite_mode_mobs(
    mut mobs: Query<
        (Entity, &mut FollowSpeed, Option<&mut TextureAtlasSprite>),
        Added<InfiniteModeMob>,
    >,
    infinite_mode: Res<crate::night::InfiniteMode>,
    mut commands: Commands,
) {
    for (entity, mut follow_speed, maybe_sprite) in mobs.iter_mut() {
        // Apply speed boost based on current difficulty level
        let speed_multiplier = infinite_mode.get_speed_multiplier();
        follow_speed.0 *= speed_multiplier;

        // Apply tint based on tier: Tier 1 = Purple, Tier 2 = Red
        let tint_alpha = infinite_mode.get_tint_alpha();
        let tier = infinite_mode.get_tier();
        if let Some(mut sprite) = maybe_sprite {
            if tier == 1 {
                // Tier 1: Purple tint (interpolate from white to purple based on alpha)
                // Purple RGB: (0.8, 0.4, 1.0) - bright purple
                let r = 1.0 - (tint_alpha * 0.2); // 1.0 to 0.8
                let g = 1.0 - (tint_alpha * 0.6); // 1.0 to 0.4
                let b = 1.0; // Always 1.0 for purple
                sprite.color = Color::rgba(r, g, b, 1.0);
            } else {
                // Tier 2: Red tint (always full red, alpha is always 1.0)
                // Interpolate from white (1.0, 1.0, 1.0) to red (1.0, 0.5, 0.5)
                let green_blue = 1.0 - (tint_alpha * 0.5); // Goes from 1.0 to 0.5
                sprite.color = Color::rgba(1.0, green_blue, green_blue, 1.0);
            }
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
    infinite_mode: Res<crate::night::InfiniteMode>,
) {
    if !infinite_mode.active {
        return;
    }

    let speed_multiplier = infinite_mode.get_speed_multiplier();
    for mut leap_state in leap_attack_states.iter_mut() {
        // Scale down the startup timer duration (faster startup = shorter duration)
        // Divide by speed multiplier to make it faster
        let original_duration = leap_state.attack_startup_timer.duration();
        let original_elapsed = leap_state.attack_startup_timer.elapsed();
        let scaled_duration_secs = original_duration.as_secs_f32() / speed_multiplier;

        // Create new timer with scaled duration
        let mut new_timer = Timer::from_seconds(scaled_duration_secs, TimerMode::Once);
        // Preserve the elapsed time proportionally (scale elapsed by the same factor)
        let elapsed_ratio = original_elapsed.as_secs_f32() / original_duration.as_secs_f32();
        let scaled_elapsed_secs = scaled_duration_secs * elapsed_ratio;
        // Tick the new timer by the scaled elapsed time
        new_timer.tick(Duration::from_secs_f32(scaled_elapsed_secs));

        leap_state.attack_startup_timer = new_timer;

        debug!(
            "Scaled LeapAttack startup for infinite mode mob: {:.2}s -> {:.2}s ({}x speed)",
            original_duration.as_secs_f32(),
            scaled_duration_secs,
            speed_multiplier
        );
    }
}
