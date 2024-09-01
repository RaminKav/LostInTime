use std::ops::{Range, RangeInclusive};

use bevy::{
    prelude::*,
    reflect::{FromReflect, Reflect},
    sprite::MaterialMesh2dBundle,
    time::{Timer, TimerMode},
    utils::HashMap,
};
use bevy_proto::{
    backend::schematics::FromSchematicInput,
    prelude::{Prototypes, PrototypesMut, ReflectSchematic, Schematic, SchematicContext},
};
use bevy_rapier2d::prelude::{Collider, KinematicCharacterController, QueryFilterFlags, Sensor};

pub mod proto_param;
use crate::{
    ai::IdleState,
    animations::{
        enemy_sprites::{
            CharacterAnimationSpriteSheetData, EnemyAnimationState, LeftFacingSideProfile,
        },
        AnimationFrameTracker, AnimationPosTracker, AnimationTimer, DoneAnimation, FadeOpacity,
    },
    assets::{SpriteAnchor, SpriteSize},
    attributes::{
        Attack, AttributeQuality, AttributeValue, ItemAttributes, ItemRarity, MaxHealth,
        RawItemBaseAttributes, RawItemBonusAttributes,
    },
    enemy::{
        CombatAlignment, EnemyMaterial, FollowSpeed, LeapAttack, Mob, MobLevel, ProjectileAttack,
    },
    inputs::FacingDirection,
    inventory::ItemStack,
    item::{
        item_actions::{ConsumableItem, ItemAction, ItemActions, ManaCost},
        item_upgrades::ClawUpgradeMultiThrow,
        melee::MeleeAttack,
        object_actions::{ObjectAction, ObjectActionCost},
        projectile::{ArcProjectileData, Projectile, ProjectileState, RangedAttack},
        Block, BreaksWith, EquipmentType, FoliageSize, ItemDisplayMetaData, Loot, LootTable,
        PlacesInto, RequiredEquipmentType, Wall, WorldObject,
    },
    player::levels::ExperienceReward,
    sappling::{GrowsInto, Sappling},
    schematic::{loot_chests::LootChestType, SchematicType},
    status_effects::{StatusEffectState, StatusEffectTracker},
    ui::{
        crafting_ui::CraftingContainerType,
        scrapper_ui::{Scrap, ScrapsInto},
        EssenceOption, EssenceShopChoices,
    },
    world::{ForestGenerationParams, WallTextureData},
    CustomFlush, GameState, YSort,
};
pub struct ProtoPlugin;

impl Plugin for ProtoPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.register_type::<Mob>()
            .register_type::<SensorProto>()
            .register_type::<FadeOpacity>()
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
            .register_type::<WallTextureData>()
            .register_type::<ItemAttributes>()
            .register_type::<RawItemBaseAttributes>()
            .register_type::<RawItemBonusAttributes>()
            .register_type::<ExperienceReward>()
            .register_type::<GrowsInto>()
            .register_type::<SapplingProto>()
            .register_type::<ItemDisplayMetaData>()
            .register_type::<YSort>()
            .register_type::<IdleStateProto>()
            .register_type::<EnemyMaterialMesh2DProto>()
            .register_type::<SpriteSheetProto>()
            .register_type::<KCC>()
            .register_type::<MobLevel>()
            .register_type::<LootChestType>()
            .register_type::<SpriteSize>()
            .register_type::<SpriteAnchor>()
            .register_type::<ItemAction>()
            .register_type::<ItemActions>()
            .register_type::<AttributeValue>()
            .register_type::<AttributeQuality>()
            .register_type::<SchematicType>()
            .register_type::<ObjectAction>()
            .register_type::<ConsumableItem>()
            .register_type::<FoliageSize>()
            .register_type::<ArcProjectileData>()
            .register_type::<ColliderProto>()
            .register_type::<FollowSpeed>()
            .register_type::<ColliderCapsulProto>()
            .register_type::<EquipmentType>()
            .register_type::<ItemRarity>()
            .register_type::<AnimationTimerProto>()
            .register_type::<LeftFacingSideProfile>()
            .register_type::<RequiredEquipmentType>()
            .register_type::<ClawUpgradeMultiThrow>()
            .register_type::<ObjectActionCost>()
            .register_type::<ManaCost>()
            .register_type::<FacingDirection>()
            .register_type::<ForestGenerationParams>()
            .register_type::<CraftingContainerType>()
            .register_type::<LeapAttack>()
            .register_type::<ProjectileAttack>()
            .register_type::<Scrap>()
            .register_type::<ScrapsInto>()
            .register_type::<Vec<Scrap>>()
            .register_type::<StatusEffectTracker>()
            .register_type::<EssenceOption>()
            .register_type::<Vec<EssenceOption>>()
            .register_type::<EssenceShopChoices>()
            .register_type::<StatusEffectState>()
            .register_type::<CharacterAnimationSpriteSheetData>()
            .register_type::<AnimationPosTracker>()
            .register_type::<HashMap<WorldObject, Vec<WorldObject>>>()
            .register_type::<HashMap<WorldObject, f64>>()
            .register_type::<HashMap<SchematicType, f64>>()
            .register_type::<HashMap<WorldObject, f32>>()
            .register_type::<Vec<WorldObject>>()
            .register_type::<Vec<u8>>()
            .register_type::<Vec<StatusEffectState>>()
            .register_type::<Vec<f32>>()
            .register_type::<Vec<String>>()
            .register_type::<Vec<ItemAction>>()
            .register_type::<Option<Range<i32>>>()
            .register_type::<Option<RangeInclusive<i32>>>()
            .register_type::<Option<u8>>()
            .register_type::<Range<i32>>()
            .register_type::<RangeInclusive<i32>>()
            .register_type_data::<Range<i32>, ReflectDeserialize>()
            .register_type_data::<RangeInclusive<i32>, ReflectDeserialize>()
            .add_plugin(bevy_proto::prelude::ProtoPlugin::new())
            .add_system(apply_system_buffers.in_set(CustomFlush))
            .add_system(Self::load_prototypes.in_set(Update(GameState::LoadingProtos)))
            .add_system(
                Self::check_proto_ready
                    .run_if(resource_exists::<AllProtos>())
                    .after(Self::load_prototypes)
                    .in_set(Update(GameState::LoadingProtos)),
            );
    }
}

#[derive(Resource)]
struct AllProtos(Vec<HandleUntyped>);

impl ProtoPlugin {
    fn load_prototypes(mut prototypes: PrototypesMut, mut commands: Commands) {
        info!("Loading prototypes...");
        let handles = prototypes.load_folder("proto").unwrap();
        commands.insert_resource(AllProtos(handles));
    }
    fn check_proto_ready(
        prototypes: Prototypes,
        handles: Res<AllProtos>,
        mut next_state: ResMut<NextState<GameState>>,
    ) {
        for p in &handles.0 {
            if !prototypes.is_ready_handle(p) {
                println!("Proto not ready yet {p:?}",);
                return;
            }
        }
        let num = handles.0.len();
        info!("All {num} prototypes ready. Moving to main menu!");
        next_state.set(GameState::MainMenu);
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
            is_stopped: false,
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
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    r: f32,
}

impl From<ColliderCapsulProto> for Collider {
    fn from(c: ColliderCapsulProto) -> Collider {
        Collider::capsule(Vec2::new(c.x1, c.y1), Vec2::new(c.x2, c.y2), c.r)
    }
}

#[derive(Schematic, Reflect, FromReflect)]
#[reflect(Schematic)]
#[schematic(into = Sappling)]
pub struct SapplingProto(f32);

impl From<SapplingProto> for Sappling {
    fn from(c: SapplingProto) -> Sappling {
        Sappling(Timer::from_seconds(c.0, TimerMode::Once))
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
            mesh: meshes.add(Rectangle::new(input.size.x, input.size.y)),
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
