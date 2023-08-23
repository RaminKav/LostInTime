use bevy::{
    prelude::*,
    reflect::TypeUuid,
    render::render_resource::{AsBindGroup, ShaderRef},
    sprite::{Material2d, Material2dPlugin},
};
use bevy_proto::prelude::{ReflectSchematic, Schematic};
use seldom_state::prelude::{StateMachine, Trigger};
use serde::Deserialize;
use strum_macros::{Display, IntoStaticStr};

use crate::{
    ai::{
        AttackDistance, FollowState, HurtByPlayer, IdleState, LeapAttackState, LineOfSight,
        ProjectileAttackState,
    },
    inputs::FacingDirection,
    item::projectile::Projectile,
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
            .add_system(Self::handle_new_mob_state_machine.in_set(OnUpdate(GameState::Main)))
            .add_system(Self::handle_mob_move_minimap_update.in_set(OnUpdate(GameState::Main)))
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
)]
#[reflect(Schematic)]
pub enum Mob {
    #[default]
    None,
    Slime,
    SpikeSlime,
    FurDevil,
}
#[derive(Component, Default, Deserialize, Debug, Clone, Schematic, Reflect, FromReflect)]
#[reflect(Schematic)]
pub enum CombatAlignment {
    #[default]
    Passive,
    Neutral,
    Hostile,
}

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
    pub speed: f32,
}

#[derive(FromReflect, Default, Reflect, Clone, Component, Schematic)]
#[reflect(Component, Schematic, Default)]
pub struct ProjectileAttack {
    pub activation_distance: f32,
    pub cooldown: f32,
    pub projectile: Projectile,
}
impl EnemyPlugin {
    pub fn handle_new_mob_state_machine(
        mut commands: Commands,
        game: GameParam,
        spawn_events: Query<
            (
                Entity,
                &Mob,
                &CombatAlignment,
                Option<&LeapAttack>,
                Option<&ProjectileAttack>,
            ),
            Added<Mob>,
        >,
    ) {
        for (e, _mob, alignment, leap_attack_option, proj_attack_option) in spawn_events.iter() {
            let mut e_cmds = commands.entity(e);
            let mut state_machine = StateMachine::default().set_trans_logging(false);
            match alignment {
                CombatAlignment::Neutral => {
                    state_machine = state_machine
                        .trans::<IdleState>(
                            HurtByPlayer,
                            FollowState {
                                target: game.game.player,
                                speed: 0.7,
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
                                speed: 0.7,
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
                            attack_startup_timer: Timer::from_seconds(0.3, TimerMode::Once),
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
                            range: leap_attack.activation_distance,
                        }),
                        FollowState {
                            target: game.game.player,
                            speed: 0.7,
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
                            speed: 1.,
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
                            speed: 0.7,
                        },
                    );
                }
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
