use bevy::{
    prelude::*,
    reflect::{FromReflect, Reflect},
    sprite::MaterialMesh2dBundle,
    time::{Timer, TimerMode},
};
use bevy_proto::{
    backend::schematics::FromSchematicInput,
    prelude::{PrototypesMut, ReflectSchematic, Schematic, SchematicContext},
};
use bevy_rapier2d::prelude::{Collider, KinematicCharacterController, QueryFilterFlags, Sensor};

pub mod proto_param;
use crate::{
    ai::{IdleState, MoveDirection},
    animations::{AnimationFrameTracker, AnimationPosTracker, AnimationTimer},
    attributes::{Attack, ItemAttributes, MaxHealth},
    enemy::{EnemyMaterial, HostileMob, Mob, NeutralMob, PassiveMob},
    inventory::ItemStack,
    item::{
        melee::MeleeAttack,
        projectile::{Projectile, ProjectileState, RangedAttack},
        Block, Breakable, BreaksWith, ItemDisplayMetaData, Loot, LootTable, Wall, WorldObject,
    },
    world::WorldObjectEntityData,
    YSort,
};
pub struct ProtoPlugin;

impl Plugin for ProtoPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.register_type::<Mob>()
            .register_type::<NeutralMob>()
            .register_type::<SensorProto>()
            .register_type::<PassiveMob>()
            .register_type::<HostileMob>()
            .register_type::<AnimationFrameTracker>()
            .register_type::<MaxHealth>()
            .register_type::<LootTable>()
            .register_type::<Loot>()
            .register_type::<Vec<Loot>>()
            .register_type::<WorldObject>()
            .register_type::<Option<WorldObject>>()
            .register_type::<Breakable>()
            .register_type::<BreaksWith>()
            .register_type::<Block>()
            .register_type::<Wall>()
            .register_type::<Projectile>()
            .register_type::<ProjectileState>()
            .register_type::<RangedAttack>()
            .register_type::<Attack>()
            .register_type::<MeleeAttack>()
            .register_type::<ItemStack>()
            .register_type::<WorldObjectEntityData>()
            .register_type::<ItemAttributes>()
            .register_type::<ItemDisplayMetaData>()
            .register_type::<YSort>()
            .register_type::<IdleStateProto>()
            .register_type::<EnemyMaterialMesh2DProto>()
            .register_type::<KCC>()
            .register_type::<ColliderProto>()
            .register_type::<AnimationTimerProto>()
            .register_type::<AnimationPosTracker>()
            .add_plugin(bevy_proto::prelude::ProtoPlugin::new())
            .add_startup_system(Self::load_prototypes);
    }
}

impl ProtoPlugin {
    fn load_prototypes(mut prototypes: PrototypesMut) {
        println!("Loading prototypes...");
        //TODO: automate this
        prototypes.load("proto/tree.prototype.ron");
        prototypes.load("proto/obj_stonewall.prototype.ron");
        prototypes.load("proto/stonewall.prototype.ron");
        prototypes.load("proto/projectile.prototype.ron");
        prototypes.load("proto/rock.prototype.ron");
        prototypes.load("proto/fireball.prototype.ron");
        prototypes.load("proto/world_object.prototype.ron");
        prototypes.load("proto/sword.prototype.ron");
        prototypes.load("proto/dagger.prototype.ron");
        prototypes.load("proto/basicstaff.prototype.ron");
        prototypes.load("proto/dualstaff.prototype.ron");
        prototypes.load("proto/firestaff.prototype.ron");
        prototypes.load("proto/flint.prototype.ron");
        prototypes.load("proto/mob_basic.prototype.ron");
        prototypes.load("proto/slime_neutral.prototype.ron");
    }
}
#[derive(Schematic, Reflect, FromReflect)]
#[reflect(Schematic)]
#[schematic(into = KinematicCharacterController)]
struct KCC;

//TODO: do we need to do this? ask
#[derive(Schematic, Reflect, FromReflect)]
#[reflect(Schematic)]
#[schematic(into = Sensor)]
struct SensorProto;
impl From<SensorProto> for Sensor {
    fn from(_: SensorProto) -> Sensor {
        Sensor
    }
}

impl From<KCC> for KinematicCharacterController {
    fn from(_: KCC) -> KinematicCharacterController {
        KinematicCharacterController {
            filter_flags: QueryFilterFlags::EXCLUDE_SENSORS | QueryFilterFlags::EXCLUDE_KINEMATIC,
            ..default()
        }
    }
}

#[derive(Schematic, Reflect, FromReflect)]
#[reflect(Schematic)]
#[schematic(into = IdleState)]
struct IdleStateProto {
    walk_dir_change_time: f32,
    speed: f32,
}

impl From<IdleStateProto> for IdleState {
    fn from(idle_state: IdleStateProto) -> IdleState {
        IdleState {
            walk_timer: Timer::from_seconds(idle_state.walk_dir_change_time, TimerMode::Repeating),
            direction: MoveDirection::new_rand_dir(rand::thread_rng()),
            speed: idle_state.speed,
        }
    }
}

#[derive(Schematic, Reflect, FromReflect)]
#[reflect(Schematic)]
#[schematic(into = AnimationTimer)]
struct AnimationTimerProto {
    secs: f32,
}

impl From<AnimationTimerProto> for AnimationTimer {
    fn from(state: AnimationTimerProto) -> AnimationTimer {
        AnimationTimer(Timer::from_seconds(state.secs, TimerMode::Repeating))
    }
}

#[derive(Schematic, Reflect, FromReflect)]
#[reflect(Schematic)]
#[schematic(into = Collider)]
struct ColliderProto {
    x: f32,
    y: f32,
}

impl From<ColliderProto> for Collider {
    fn from(col_state: ColliderProto) -> Collider {
        Collider::cuboid(col_state.x, col_state.y)
    }
}

#[derive(Schematic, Reflect, FromReflect)]
#[reflect(Schematic)]
#[schematic(into = MaterialMesh2dBundle<EnemyMaterial>)]
struct EnemyMaterialMesh2DProto {
    asset: String,
    size: Vec2,
}

impl FromSchematicInput<EnemyMaterialMesh2DProto> for MaterialMesh2dBundle<EnemyMaterial> {
    fn from_input(
        input: EnemyMaterialMesh2DProto,
        context: &mut SchematicContext,
    ) -> MaterialMesh2dBundle<EnemyMaterial> {
        let world = context.world_mut();
        let asset_server = world.resource::<AssetServer>();
        let handle = asset_server.load(input.asset);
        let mut materials = world.resource_mut::<Assets<EnemyMaterial>>();
        let enemy_material = materials.add(EnemyMaterial {
            source_texture: Some(handle),
            is_attacking: 0.,
        });
        let mut meshes = world.resource_mut::<Assets<Mesh>>();
        MaterialMesh2dBundle {
            mesh: meshes
                .add(Mesh::from(shape::Quad {
                    size: input.size,
                    ..Default::default()
                }))
                .into(),
            material: enemy_material,
            ..default()
        }
    }
}
