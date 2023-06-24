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
        AttackDistance, AttackState, FollowState, HurtByPlayer, IdleState, LineOfSight,
        MoveDirection,
    },
    ui::minimap::UpdateMiniMapEvent,
    world::TileMapPositionData,
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
    Neutral(NeutralMob),
    Hostile(HostileMob),
    Passive(PassiveMob),
}
#[derive(
    Component,
    Default,
    Deserialize,
    Debug,
    Clone,
    Hash,
    Display,
    IntoStaticStr,
    Eq,
    PartialEq,
    Schematic,
    Reflect,
    FromReflect,
)]
#[reflect(Schematic)]
pub enum NeutralMob {
    #[default]
    Slime,
}
#[derive(
    Component,
    Default,
    Deserialize,
    Debug,
    Clone,
    Hash,
    Display,
    IntoStaticStr,
    Eq,
    PartialEq,
    Schematic,
    Reflect,
    FromReflect,
)]
#[reflect(Schematic)]
pub enum HostileMob {
    #[default]
    Slime,
}
#[derive(
    Component,
    Default,
    Deserialize,
    Debug,
    Clone,
    Hash,
    Display,
    IntoStaticStr,
    Eq,
    PartialEq,
    Schematic,
    Reflect,
    FromReflect,
)]
#[reflect(Schematic)]
pub enum PassiveMob {
    #[default]
    Slime,
}

pub struct EnemySpawnEvent {
    pub enemy: Mob,
    pub pos: TileMapPositionData,
}
impl EnemyPlugin {
    pub fn handle_new_mob_state_machine(
        mut commands: Commands,
        game: GameParam,
        spawn_events: Query<(Entity, &Mob), Added<Mob>>,
    ) {
        for (e, mob) in spawn_events.iter() {
            let mut e_cmds = commands.entity(e);
            match mob {
                Mob::Neutral(_) => {
                    e_cmds.insert(
                        StateMachine::default()
                            .set_trans_logging(false)
                            .trans::<IdleState>(
                                HurtByPlayer,
                                FollowState {
                                    target: game.game.player,
                                    speed: 0.7,
                                },
                            )
                            .trans::<FollowState>(
                                AttackDistance {
                                    target: game.game.player,
                                    range: 50.,
                                },
                                AttackState {
                                    target: game.game.player,
                                    attack_startup_timer: Timer::from_seconds(0.3, TimerMode::Once),
                                    attack_duration_timer: Timer::from_seconds(
                                        0.3,
                                        TimerMode::Once,
                                    ),
                                    attack_cooldown_timer: Timer::from_seconds(1., TimerMode::Once),
                                    dir: None,
                                    speed: 1.4,
                                    damage: 10,
                                },
                            )
                            .trans::<FollowState>(
                                Trigger::not(LineOfSight {
                                    target: game.game.player,
                                    range: 130.,
                                }),
                                IdleState {
                                    walk_timer: Timer::from_seconds(2., TimerMode::Repeating),
                                    direction: MoveDirection::new_rand_dir(rand::thread_rng()),
                                    speed: 0.5,
                                },
                            )
                            .trans::<AttackState>(
                                Trigger::not(AttackDistance {
                                    target: game.game.player,
                                    range: 20.,
                                }),
                                FollowState {
                                    target: game.game.player,
                                    speed: 0.7,
                                },
                            ),
                    );
                }
                Mob::Hostile(_) => {
                    e_cmds.insert(
                        StateMachine::default()
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
                                AttackDistance {
                                    target: game.game.player,
                                    range: 50.,
                                },
                                AttackState {
                                    target: game.game.player,
                                    attack_startup_timer: Timer::from_seconds(0.3, TimerMode::Once),
                                    attack_duration_timer: Timer::from_seconds(
                                        0.3,
                                        TimerMode::Once,
                                    ),
                                    attack_cooldown_timer: Timer::from_seconds(1., TimerMode::Once),
                                    dir: None,
                                    speed: 1.4,
                                    damage: 10,
                                },
                            )
                            .trans::<FollowState>(
                                Trigger::not(LineOfSight {
                                    target: game.game.player,
                                    range: 130.,
                                }),
                                IdleState {
                                    walk_timer: Timer::from_seconds(2., TimerMode::Repeating),
                                    direction: MoveDirection::new_rand_dir(rand::thread_rng()),
                                    speed: 0.5,
                                },
                            )
                            .trans::<AttackState>(
                                Trigger::not(AttackDistance {
                                    target: game.game.player,
                                    range: 50.,
                                }),
                                FollowState {
                                    target: game.game.player,
                                    speed: 0.7,
                                },
                            ),
                    );
                }
                Mob::Passive(_) => {
                    //TODO: impl run away
                }
                _ => {}
            }
        }
    }
    fn handle_mob_move_minimap_update(
        moving_enemies: Query<Entity, (With<Mob>, Changed<GlobalTransform>)>,
        mut minimap_event: EventWriter<UpdateMiniMapEvent>,
    ) {
        if moving_enemies.iter().count() > 0 {
            minimap_event.send(UpdateMiniMapEvent);
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
