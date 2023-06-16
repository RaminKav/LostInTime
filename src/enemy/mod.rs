use bevy::{
    prelude::*,
    reflect::TypeUuid,
    render::render_resource::{AsBindGroup, ShaderRef},
    sprite::{Material2d, Material2dPlugin, MaterialMesh2dBundle},
};
use bevy_proto::prelude::{prototype_ready, ProtoCommands, ReflectSchematic, Schematic};
use seldom_state::prelude::{StateMachine, Trigger};
use serde::Deserialize;
use strum_macros::Display;

use crate::{
    ai::{
        AttackDistance, AttackState, FollowState, HurtByPlayer, IdleState, LineOfSight,
        MoveDirection,
    },
    world::{TileMapPositionData, CHUNK_SIZE},
    CoreGameSet, GameParam,
};

pub mod spawner;
use self::spawner::SpawnerPlugin;

pub struct EnemyPlugin;

impl Plugin for EnemyPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(Material2dPlugin::<EnemyMaterial>::default())
            .add_event::<EnemySpawnEvent>()
            .add_system(
                Self::summon_enemies
                    .run_if(prototype_ready("EnemyBasicNeutral"))
                    .in_base_set(CoreGameSet::Main),
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
    pub fn summon_enemies(
        mut proto_commands: ProtoCommands,
        game: GameParam,
        asset_server: Res<AssetServer>,
        mut materials: ResMut<Assets<EnemyMaterial>>,
        mut meshes: ResMut<Assets<Mesh>>,
        mut spawn_events: EventReader<EnemySpawnEvent>,
    ) {
        for enemy in spawn_events.iter() {
            //TODO: Add helper fn for this
            let pos = Vec2::new(
                (enemy.pos.tile_pos.x as i32 * 32 + enemy.pos.chunk_pos.x * CHUNK_SIZE as i32 * 32)
                    as f32,
                (enemy.pos.tile_pos.y as i32 * 32 + enemy.pos.chunk_pos.y * CHUNK_SIZE as i32 * 32)
                    as f32,
            );
            let name = "slime"; //enemy.enemy.to_string();
            let handle = asset_server.load(format!(
                "textures/{}/{}-move-0.png",
                name.to_lowercase(),
                name.to_lowercase()
            ));
            let enemy_material = materials.add(EnemyMaterial {
                source_texture: Some(handle),
                is_attacking: 0.,
            });
            let mut enemy_e = proto_commands.spawn("EnemyBasicNeutral");
            let mut enemy_e = enemy_e.entity_commands();
            enemy_e.insert(MaterialMesh2dBundle {
                mesh: meshes
                    .add(Mesh::from(shape::Quad {
                        size: Vec2::new(32., 32.),
                        ..Default::default()
                    }))
                    .into(),
                transform: Transform::from_translation(pos.extend(0.)),
                material: enemy_material,
                ..default()
            });

            // .spawn((
            //     MaterialMesh2dBundle {
            //         mesh: meshes
            //             .add(Mesh::from(shape::Quad {
            //                 size: Vec2::new(32., 32.),
            //                 ..Default::default()
            //             }))
            //             .into(),
            //         transform: Transform::from_translation(pos.extend(0.)),
            //         material: enemy_material,
            //         ..default()
            //     },
            //     AnimationTimer(Timer::from_seconds(0.20, TimerMode::Repeating)),
            //     AnimationFrameTracker(0, 7),
            //     Health(100),
            //     KinematicCharacterController::default(),
            //     Collider::cuboid(10., 8.),
            //     YSort,
            //     enemy.enemy.clone(),
            //     IdleState {
            //         walk_timer: Timer::from_seconds(2., TimerMode::Repeating),
            //         direction: MoveDirection::new_rand_dir(rand::thread_rng()),
            //         speed: 0.5,
            //     },
            //     Name::new(name),
            // ));
            if let Some(loot_table) = game.loot_tables.table.get(&enemy.enemy) {
                enemy_e.insert(loot_table.clone());
            }
            match enemy.enemy {
                Mob::Neutral(_) => {
                    enemy_e.insert(
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
                    enemy_e.insert(
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
