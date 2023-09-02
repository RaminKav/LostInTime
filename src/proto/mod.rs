use std::ops::Range;

use bevy::{
    prelude::*,
    reflect::{FromReflect, Reflect},
    sprite::MaterialMesh2dBundle,
    time::{Timer, TimerMode},
    utils::HashMap,
};
use bevy_proto::{
    backend::schematics::FromSchematicInput,
    prelude::{
        prototype_ready, ProtoCommands, PrototypesMut, ReflectSchematic, Schematic,
        SchematicContext,
    },
};
use bevy_rapier2d::prelude::{Collider, KinematicCharacterController, QueryFilterFlags, Sensor};

pub mod proto_param;
use crate::{
    ai::IdleState,
    animations::{
        enemy_sprites::{CharacterAnimationSpriteSheetData, EnemyAnimationState},
        AnimationFrameTracker, AnimationPosTracker, AnimationTimer, DoneAnimation,
    },
    assets::{SpriteAnchor, SpriteSize},
    attributes::{
        Attack, ItemAttributes, ItemRarity, MaxHealth, RawItemBaseAttributes,
        RawItemBonusAttributes,
    },
    enemy::{CombatAlignment, EnemyMaterial, LeapAttack, Mob, ProjectileAttack},
    inputs::FacingDirection,
    inventory::ItemStack,
    item::{
        item_actions::{ConsumableItem, ItemAction},
        item_upgrades::ClawUpgradeMultiThrow,
        melee::MeleeAttack,
        object_actions::ObjectAction,
        projectile::{ArcProjectileData, Projectile, ProjectileState, RangedAttack},
        Block, BreaksWith, EquipmentType, ItemDisplayMetaData, Loot, LootTable, PlacesInto,
        RequiredEquipmentType, Wall, WorldObject,
    },
    player::levels::ExperienceReward,
    schematic::{loot_chests::LootChestType, SchematicType},
    world::WorldObjectEntityData,
    CustomFlush, GameState, YSort,
};
pub struct ProtoPlugin;

impl Plugin for ProtoPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.register_type::<Mob>()
            .register_type::<SensorProto>()
            .register_type::<CombatAlignment>()
            .register_type::<AnimationFrameTracker>()
            .register_type::<EnemyAnimationState>()
            .register_type::<MaxHealth>()
            .register_type::<LootTable>()
            .register_type::<Loot>()
            .register_type::<Vec<Loot>>()
            .register_type::<WorldObject>()
            .register_type::<Option<WorldObject>>()
            .register_type::<PlacesInto>()
            .register_type::<BreaksWith>()
            .register_type::<Block>()
            .register_type::<DoneAnimation>()
            .register_type::<Wall>()
            .register_type::<Projectile>()
            .register_type::<ProjectileState>()
            .register_type::<RangedAttack>()
            .register_type::<Attack>()
            .register_type::<MeleeAttack>()
            .register_type::<ItemStack>()
            .register_type::<WorldObjectEntityData>()
            .register_type::<ItemAttributes>()
            .register_type::<RawItemBaseAttributes>()
            .register_type::<RawItemBonusAttributes>()
            .register_type::<ExperienceReward>()
            .register_type::<ItemDisplayMetaData>()
            .register_type::<YSort>()
            .register_type::<IdleStateProto>()
            .register_type::<EnemyMaterialMesh2DProto>()
            .register_type::<SpriteSheetProto>()
            .register_type::<KCC>()
            .register_type::<LootChestType>()
            .register_type::<SpriteSize>()
            .register_type::<SpriteAnchor>()
            .register_type::<ItemAction>()
            .register_type::<SchematicType>()
            .register_type::<ObjectAction>()
            .register_type::<ConsumableItem>()
            .register_type::<ArcProjectileData>()
            .register_type::<ColliderProto>()
            .register_type::<ColliderCapsulProto>()
            .register_type::<EquipmentType>()
            .register_type::<ItemRarity>()
            .register_type::<AnimationTimerProto>()
            .register_type::<RequiredEquipmentType>()
            .register_type::<ClawUpgradeMultiThrow>()
            .register_type::<FacingDirection>()
            .register_type::<LeapAttack>()
            .register_type::<ProjectileAttack>()
            .register_type::<CharacterAnimationSpriteSheetData>()
            .register_type::<AnimationPosTracker>()
            .register_type::<HashMap<WorldObject, Vec<WorldObject>>>()
            .register_type::<HashMap<WorldObject, f64>>()
            .register_type::<HashMap<SchematicType, f64>>()
            .register_type::<Vec<WorldObject>>()
            .register_type::<Vec<u8>>()
            .register_type::<Vec<f32>>()
            .register_type::<Option<Range<i32>>>()
            .register_type::<Range<i32>>()
            .register_type_data::<Range<i32>, ReflectDeserialize>()
            .add_plugin(bevy_proto::prelude::ProtoPlugin::new())
            .add_system(apply_system_buffers.in_set(CustomFlush))
            .add_startup_system(Self::load_prototypes.before(CustomFlush))
            .add_system(
                Self::spawn_proto_resources
                    .in_schedule(OnEnter(GameState::Main))
                    .run_if(prototype_ready("WorldGenerationParams").and_then(run_once())),
            );
    }
}

impl ProtoPlugin {
    fn load_prototypes(mut prototypes: PrototypesMut) {
        println!("Loading prototypes...");
        //TODO: automate this
        prototypes.load("proto/tree.prototype.ron");
        prototypes.load("proto/WorldGenerationParams.prototype.ron");
        prototypes.load("proto/DungeonWorldGenerationParams.prototype.ron");
        prototypes.load("proto/stonewall.prototype.ron");
        prototypes.load("proto/stonewallblock.prototype.ron");
        prototypes.load("proto/projectile.prototype.ron");
        prototypes.load("proto/rock.prototype.ron");
        prototypes.load("proto/fireball.prototype.ron");
        prototypes.load("proto/electricity.prototype.ron");
        prototypes.load("proto/world_object.prototype.ron");
        prototypes.load("proto/sword.prototype.ron");
        prototypes.load("proto/chestplate.prototype.ron");
        prototypes.load("proto/pants.prototype.ron");
        prototypes.load("proto/dagger.prototype.ron");
        prototypes.load("proto/basicstaff.prototype.ron");
        prototypes.load("proto/dualstaff.prototype.ron");
        prototypes.load("proto/firestaff.prototype.ron");
        prototypes.load("proto/ring.prototype.ron");
        prototypes.load("proto/pendant.prototype.ron");
        prototypes.load("proto/flint.prototype.ron");
        prototypes.load("proto/smallpotion.prototype.ron");
        prototypes.load("proto/largepotion.prototype.ron");
        prototypes.load("proto/log.prototype.ron");
        prototypes.load("proto/mob_basic.prototype.ron");
        prototypes.load("proto/slime.prototype.ron");
        prototypes.load("proto/spikeslime.prototype.ron");
        prototypes.load("proto/furdevil.prototype.ron");
        prototypes.load("proto/chest.prototype.ron");
        prototypes.load("proto/pebble.prototype.ron");
        prototypes.load("proto/chestblock.prototype.ron");
        prototypes.load("proto/dungeonentrance.prototype.ron");
        prototypes.load("proto/dungeonentranceblock.prototype.ron");
        prototypes.load("proto/grass.prototype.ron");
        prototypes.load("proto/grassblock.prototype.ron");
        prototypes.load("proto/boulder.prototype.ron");
        prototypes.load("proto/stick.prototype.ron");
        prototypes.load("proto/woodplank.prototype.ron");
        prototypes.load("proto/woodaxe.prototype.ron");
        prototypes.load("proto/plantfibre.prototype.ron");
        prototypes.load("proto/slimegoo.prototype.ron");
        prototypes.load("proto/bandage.prototype.ron");
        prototypes.load("proto/string.prototype.ron");
        prototypes.load("proto/deadsapling.prototype.ron");
        prototypes.load("proto/apple.prototype.ron");
        prototypes.load("proto/magicwhip.prototype.ron");
        prototypes.load("proto/woodbow.prototype.ron");
        prototypes.load("proto/arrow.prototype.ron");
        prototypes.load("proto/greenwhip.prototype.ron");
        prototypes.load("proto/claw.prototype.ron");
        prototypes.load("proto/throwingstar.prototype.ron");
        prototypes.load("proto/fireexplosionaoe.prototype.ron");
        prototypes.load("proto/crate.prototype.ron");
        prototypes.load("proto/crateblock.prototype.ron");
        prototypes.load("proto/coal.prototype.ron");
        prototypes.load("proto/metalshard.prototype.ron");
        prototypes.load("proto/coalboulder.prototype.ron");
        prototypes.load("proto/metalboulder.prototype.ron");
        prototypes.load("proto/slimegooprojectile.prototype.ron");
        prototypes.load("proto/stonechunk.prototype.ron");
        prototypes.load("proto/woodsword.prototype.ron");
        prototypes.load("proto/pebbleblock.prototype.ron");
    }
    fn spawn_proto_resources(mut commands: ProtoCommands) {
        commands.apply("WorldGenerationParams");
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
            direction: FacingDirection::new_rand_dir(rand::thread_rng()),
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
pub struct ColliderProto {
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
#[schematic(into = Collider)]
pub struct ColliderCapsulProto {
    x: f32,
    y: f32,
    r: f32,
}

impl From<ColliderCapsulProto> for Collider {
    fn from(c: ColliderCapsulProto) -> Collider {
        Collider::capsule(Vec2::new(0., c.x), Vec2::new(0., c.y), c.r)
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

#[derive(Schematic, Reflect, Debug, FromReflect)]
#[reflect(Schematic)]
#[schematic(into = SpriteSheetBundle)]
pub struct SpriteSheetProto {
    pub asset: String,
    pub size: Vec2,
    pub cols: usize,
    pub rows: usize,
}

impl FromSchematicInput<SpriteSheetProto> for SpriteSheetBundle {
    fn from_input(input: SpriteSheetProto, context: &mut SchematicContext) -> SpriteSheetBundle {
        let world = context.world_mut();

        let asset_server = world.resource::<AssetServer>();
        let texture_handle = asset_server.load(input.asset);

        let mut texture_atlases = world.resource_mut::<Assets<TextureAtlas>>();

        let texture_atlas = TextureAtlas::from_grid(
            texture_handle,
            input.size,
            input.cols,
            input.rows,
            None,
            None,
        );
        let texture_atlas_handle = texture_atlases.add(texture_atlas);
        SpriteSheetBundle {
            texture_atlas: texture_atlas_handle,
            ..default()
        }
    }
}
