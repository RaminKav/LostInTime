use bevy::{
    prelude::*,
    reflect::TypeUuid,
    render::render_resource::{AsBindGroup, ShaderRef},
    sprite::{Material2d, Material2dPlugin},
};
use bevy_proto::prelude::{ReflectSchematic, Schematic};
use seldom_state::prelude::{StateMachine, Trigger};
use serde::Deserialize;
use strum_macros::{Display, EnumIter, IntoStaticStr};

use crate::{
    ai::{
        AttackDistance, FollowState, HurtByPlayer, IdleState, LeapAttackState, LineOfSight,
        NightTimeAggro, ProjectileAttackState,
    },
    attributes::{add_current_health_with_max_health, Attack, MaxHealth},
    colors::{BLACK, DARK_GREEN, LIGHT_BROWN, LIGHT_GREEN, PINK},
    inputs::FacingDirection,
    item::{projectile::Projectile, Loot, LootTable},
    player::levels::ExperienceReward,
    ui::minimap::UpdateMiniMapEvent,
    world::TileMapPosition,
    AppExt, GameParam, GameState,
};

pub mod spawner;
use self::spawner::SpawnerPlugin;

pub struct EnemyPlugin;

impl Plugin for EnemyPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(Material2dPlugin::<EnemyMaterial>::default())
            .with_default_schedule(CoreSchedule::FixedUpdate, |app| {
                app.add_event::<EnemySpawnEvent>();
            })
            .add_systems(
                (
                    handle_new_mob_state_machine,
                    handle_mob_move_minimap_update,
                    juice_up_spawned_elite_mobs.before(add_current_health_with_max_health),
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_plugin(SpawnerPlugin);
    }
}

#[derive(
    Component,
    Default,
    Deserialize,
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
}

impl Mob {
    pub fn get_mob_color(&self) -> Color {
        match self {
            Mob::None => BLACK,
            Mob::Slime => LIGHT_GREEN,
            Mob::SpikeSlime => LIGHT_GREEN,
            Mob::Bushling => DARK_GREEN,
            Mob::StingFly => LIGHT_GREEN,
            Mob::FurDevil => PINK,
            Mob::Hog => LIGHT_BROWN,
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

#[derive(FromReflect, Debug, Default, Reflect, Clone, Component, Schematic)]
#[reflect(Component, Schematic, Default)]
pub struct LeapAttack {
    pub activation_distance: f32,
    pub duration: f32,
    pub cooldown: f32,
    pub startup: f32,
    pub speed: f32,
}

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
        Added<Mob>,
    >,
) {
    for (e, _mob, alignment, follow_speed, leap_attack_option, proj_attack_option) in
        spawn_events.iter()
    {
        let mut e_cmds = commands.entity(e);
        let mut state_machine = StateMachine::default().set_trans_logging(false);
        match alignment {
            CombatAlignment::Neutral => {
                state_machine = state_machine
                    .trans::<IdleState>(
                        HurtByPlayer,
                        FollowState {
                            target: game.game.player,
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
                        },
                    );
            }
            CombatAlignment::Hostile => {
                state_machine = state_machine
                    .trans::<IdleState>(
                        LineOfSight {
                            target: game.game.player,
                            range: 130.,
                        },
                        FollowState {
                            target: game.game.player,
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
                        speed: follow_speed.0,
                    },
                );
            }
        }
        if alignment != &CombatAlignment::Passive {
            state_machine = state_machine.trans::<IdleState>(
                NightTimeAggro,
                FollowState {
                    target: game.game.player,
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
            &mut MaxHealth,
            &mut Attack,
            &mut ExperienceReward,
            &mut LootTable,
        ),
        Added<EliteMob>,
    >,
) {
    for (_e, mut hp, mut att, mut exp, mut loot) in elites.iter_mut() {
        hp.0 *= 2;
        att.0 *= 2;
        exp.0 *= 4;
        loot.drops = loot
            .drops
            .iter()
            .map(|l| Loot {
                item: l.item.clone(),
                min: l.min,
                max: l.max,
                rate: l.rate * 2.5,
            })
            .collect();
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
