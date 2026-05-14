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
use bevy_rapier2d::prelude::{
    Collider, CollisionGroups, Group, KinematicCharacterController, QueryFilterFlags, Sensor,
};

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
        scorpion::{ScorpionClawAttack, ScorpionTailAttack, ScorpionTornadoAttack},
        BullChargeAttack, CircleAttack, CombatAlignment, EnemyMaterial, FollowSpeed, LeapAttack,
        Mob, MobLevel, MultiLeapAttack, ProjectileAttack,
    },
    inputs::FacingDirection,
    inventory::ItemStack,
    item::{
        item_actions::{ConsumableItem, ItemAction, ItemActions, ManaCost},
        item_upgrades::ClawUpgradeMultiThrow,
        melee::MeleeAttack,
        object_actions::{ObjectAction, ObjectActionCost, TouchTriggerObjectAction},
        projectile::{ArcProjectileData, Projectile, ProjectileState, RangedAttack},
        Block, BreaksWith, EquipmentType, FoliageSize, ItemDisplayMetaData, Loot, LootTable,
        PlacesInto, RequiredEquipmentType, Wall, WorldObject,
    },
    player::levels::ExperienceReward,
    sapling::{GrowsInto, Sapling},
    status_effects::{StatusEffectState, StatusEffectTracker},
    ui::{
        crafting_ui::CraftingContainerType,
        scrapper_ui::{Scrap, ScrapsInto},
    },
    world::{ForestGenerationParams, ShrineCount, WallTextureData},
    CustomFlush, GameState, Pet, PetState, YSort,
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
            .register_type::<SaplingProto>()
            .register_type::<ItemDisplayMetaData>()
            .register_type::<YSort>()
            .register_type::<IdleStateProto>()
            .register_type::<EnemyMaterialMesh2DProto>()
            .register_type::<SpriteSheetProto>()
            .register_type::<KCC>()
            .register_type::<MobLevel>()
            .register_type::<SpriteSize>()
            .register_type::<SpriteAnchor>()
            .register_type::<ItemAction>()
            .register_type::<ItemActions>()
            .register_type::<AttributeValue>()
            .register_type::<AttributeQuality>()
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
            .register_type::<TouchTriggerObjectAction>()
            .register_type::<ClawUpgradeMultiThrow>()
            .register_type::<ObjectActionCost>()
            .register_type::<ManaCost>()
            .register_type::<Option<ItemStack>>()
            .register_type::<Pet>()
            .register_type::<PetState>()
            .register_type::<FacingDirection>()
            .register_type::<ForestGenerationParams>()
            .register_type::<ShrineCount>()
            .register_type::<HashMap<WorldObject, ShrineCount>>()
            .register_type::<CraftingContainerType>()
            .register_type::<LeapAttack>()
            .register_type::<ProjectileAttack>()
            .register_type::<Scrap>()
            .register_type::<ScrapsInto>()
            .register_type::<Vec<Scrap>>()
            .register_type::<StatusEffectTracker>()
            // EssenceOption and EssenceShopChoices no longer use Reflect (contain non-Reflect types)
            .register_type::<StatusEffectState>()
            .register_type::<CharacterAnimationSpriteSheetData>()
            .register_type::<AnimationPosTracker>()
            .register_type::<HashMap<WorldObject, Vec<WorldObject>>>()
            .register_type::<HashMap<WorldObject, f64>>()
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
            .register_type::<CircleAttack>()
            .register_type::<MultiLeapAttack>()
            .register_type::<BullChargeAttack>()
            .register_type::<ScorpionClawAttack>()
            .register_type::<ScorpionTailAttack>()
            .register_type::<ScorpionTornadoAttack>()
            .register_type::<RangeInclusive<i32>>()
            .register_type_data::<Range<i32>, ReflectDeserialize>()
            .register_type_data::<RangeInclusive<i32>, ReflectDeserialize>()
            .add_plugin(bevy_proto::prelude::ProtoPlugin::new())
            .add_system(apply_system_buffers.in_set(CustomFlush))
            .add_system(Self::load_base_templates.in_schedule(OnEnter(GameState::LoadingProtos)))
            .add_system(
                Self::check_base_templates_ready
                    .run_if(resource_exists::<BaseTemplateHandles>())
                    .in_set(OnUpdate(GameState::LoadingProtos)),
            )
            .add_system(
                Self::load_all_prototypes.in_schedule(OnEnter(GameState::LoadingProtosStage2)),
            )
            .add_system(
                Self::check_all_protos_ready
                    .run_if(resource_exists::<AllProtos>())
                    .in_set(OnUpdate(GameState::LoadingProtosStage2)),
            );
    }
}

#[derive(Resource)]
struct BaseTemplateHandles {
    handles: Vec<HandleUntyped>,
    start_time: std::time::Instant,
    check_count: u32,
}

#[derive(Resource)]
struct AllProtos {
    handles: Vec<HandleUntyped>,
    start_time: std::time::Instant,
    check_count: u32,
    last_not_ready: Option<HandleUntyped>,
}

impl ProtoPlugin {
    /// Stage 1: Load only base templates that other prototypes depend on
    fn load_base_templates(mut prototypes: PrototypesMut, mut commands: Commands) {
        info!("STAGE 1: Loading base template prototypes...");

        // Load base templates that other prototypes depend on
        let base_templates = vec![
            "proto/item_drop.prototype.ron",
            "proto/projectile.prototype.ron",
            "proto/world_object.prototype.ron",
            "proto/mob_basic.prototype.ron",
            "proto/mob_passive.prototype.ron",
        ];

        let mut handles = Vec::new();
        for template in base_templates {
            let handle = prototypes.load(template);
            handles.push(handle.clone_untyped());
        }

        info!("Queued {} base templates for loading", handles.len());

        commands.insert_resource(BaseTemplateHandles {
            handles,
            start_time: std::time::Instant::now(),
            check_count: 0,
        });
    }

    /// Check if base templates are ready, then proceed to stage 2
    fn check_base_templates_ready(
        prototypes: Prototypes,
        mut handles: ResMut<BaseTemplateHandles>,
        mut next_state: ResMut<NextState<GameState>>,
        asset_server: Res<AssetServer>,
    ) {
        handles.check_count += 1;
        let elapsed = handles.start_time.elapsed();

        // Log progress every 50 checks
        if handles.check_count % 50 == 0 {
            let ready_count = handles
                .handles
                .iter()
                .filter(|h| prototypes.is_ready_handle(*h))
                .count();
            info!(
                "STAGE 1: Waiting for base templates... ({}/{}  ready, {:.1}s elapsed)",
                ready_count,
                handles.handles.len(),
                elapsed.as_secs_f32()
            );
        }

        // Check if all base templates are ready
        for h in &handles.handles {
            if !prototypes.is_ready_handle(h) {
                return; // Still waiting
            }
        }

        // All base templates are ready!
        info!(
            "STAGE 1 COMPLETE: All {} base templates ready in {:.2}s. Proceeding to load all prototypes...",
            handles.handles.len(),
            elapsed.as_secs_f32()
        );
        next_state.set(GameState::LoadingProtosStage2);
    }

    /// Stage 2: Load all prototypes (base templates are already loaded and ready)
    fn load_all_prototypes(mut prototypes: PrototypesMut, mut commands: Commands) {
        info!("STAGE 2: Loading all prototypes...");

        // Load all prototypes (including base templates again, which is fine)
        let all_handles = prototypes.load_folder("proto").unwrap();

        commands.insert_resource(AllProtos {
            handles: all_handles,
            start_time: std::time::Instant::now(),
            check_count: 0,
            last_not_ready: None,
        });
    }
    /// Stage 2: Check if all prototypes are ready
    fn check_all_protos_ready(
        prototypes: Prototypes,
        mut handles: ResMut<AllProtos>,
        mut next_state: ResMut<NextState<GameState>>,
        asset_server: Res<AssetServer>,
    ) {
        handles.check_count += 1;
        let elapsed = handles.start_time.elapsed();

        // After 3 seconds, warn about potential dependency issues
        if elapsed.as_secs() >= 3 && handles.check_count < 100 {
            warn!(
                "Prototype loading is taking longer than usual. This may be due to template dependency ordering. Please wait..."
            );
        }

        // Timeout after 15 seconds (increased to give bevy_proto more time)
        if elapsed.as_secs() > 15 {
            // Get path info for the stuck prototype
            let path_info = handles.last_not_ready.as_ref().and_then(|h| {
                asset_server
                    .get_handle_path(h.clone())
                    .map(|p| p.path().to_string_lossy().to_string())
            });

            // Count how many are actually ready
            let ready_count = handles
                .handles
                .iter()
                .filter(|h| prototypes.is_ready_handle(*h))
                .count();

            error!(
                "Prototype loading timed out after {} seconds.\nLast proto not ready: {:?}\nPath: {:?}\n\nThis is likely a bevy_proto dependency ordering issue.\nReady: {}/{} prototypes",
                elapsed.as_secs(),
                handles.last_not_ready,
                path_info,
                ready_count,
                handles.handles.len()
            );
            warn!("Proceeding to main menu anyway - some features may not work correctly.");
            next_state.set(GameState::MainMenu);
            return;
        }

        // Log progress every 100 checks (roughly every 1.5 seconds)
        if handles.check_count % 100 == 0 {
            let path_info = handles.last_not_ready.as_ref().and_then(|h| {
                asset_server
                    .get_handle_path(h.clone())
                    .map(|p| p.path().to_string_lossy().to_string())
            });

            // Count how many are ready
            let ready_count = handles
                .handles
                .iter()
                .filter(|h| prototypes.is_ready_handle(*h))
                .count();

            info!(
                "Still loading... ({} checks, {:.1}s) - {}/{} ready, waiting for: {:?}",
                handles.check_count,
                elapsed.as_secs_f32(),
                ready_count,
                handles.handles.len(),
                path_info
            );
        }

        for p in &handles.handles {
            if !prototypes.is_ready_handle(p) {
                // Only update if it's a different proto than last time
                if handles.last_not_ready.as_ref() != Some(p) {
                    let path_info = asset_server
                        .get_handle_path(p.clone())
                        .map(|p| p.path().to_string_lossy().to_string())
                        .unwrap_or_else(|| "Unknown path".to_string());
                    info!("Waiting for prototype: {}", path_info);
                }
                handles.last_not_ready = Some(p.clone());
                return;
            }
        }

        let num = handles.handles.len();
        info!(
            "All {num} prototypes ready in {:.2}s after {} checks. Moving to main menu!",
            elapsed.as_secs_f32(),
            handles.check_count
        );
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
            filter_groups: Some(CollisionGroups::new(Group::GROUP_1, Group::GROUP_1)),
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
#[derive(Component, Clone, Schematic, Reflect, FromReflect)]
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
impl ColliderCapsulProto {
    pub fn scale(&mut self, scale: f32) -> Self {
        self.x1 *= scale;
        self.y1 *= scale;
        self.x2 *= scale;
        self.y2 *= scale;
        self.r *= scale;
        self.clone()
    }
}

#[derive(Schematic, Reflect, FromReflect)]
#[reflect(Schematic)]
#[schematic(into = Sapling)]
pub struct SaplingProto(f32);

impl From<SaplingProto> for Sapling {
    fn from(c: SaplingProto) -> Sapling {
        Sapling(Timer::from_seconds(c.0, TimerMode::Once))
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
