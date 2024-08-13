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
use strum::IntoEnumIterator;

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
    ui::{crafting_ui::CraftingContainerType, EssenceOption, EssenceShopChoices},
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
            .add_system(Self::load_prototypes.in_set(OnUpdate(GameState::Loading)))
            .add_system(
                Self::check_proto_ready
                    .after(Self::load_prototypes)
                    .in_set(OnUpdate(GameState::Loading)),
            );
    }
}

impl ProtoPlugin {
    fn load_prototypes(mut prototypes: PrototypesMut) {
        println!("Loading prototypes...");
        //TODO: automate this
        prototypes.load("proto/item_drop.prototype.ron");
        prototypes.load("proto/world_object.prototype.ron");
        prototypes.load("proto/smallgreentree.prototype.ron");
        prototypes.load("proto/inventorybag.prototype.ron");
        prototypes.load("proto/redtree.prototype.ron");
        prototypes.load("proto/smallyellowtree.prototype.ron");
        prototypes.load("proto/smallyellowtree.prototype.ron");
        prototypes.load("proto/mediumyellowtree.prototype.ron");
        prototypes.load("proto/mediumgreentree.prototype.ron");
        prototypes.load("proto/Era1WorldGenerationParams.prototype.ron");
        prototypes.load("proto/DungeonWorldGenerationParams.prototype.ron");
        prototypes.load("proto/stonewall.prototype.ron");
        prototypes.load("proto/stonewallblock.prototype.ron");
        prototypes.load("proto/projectile.prototype.ron");
        prototypes.load("proto/rock.prototype.ron");
        prototypes.load("proto/fireball.prototype.ron");
        prototypes.load("proto/electricity.prototype.ron");
        prototypes.load("proto/arc.prototype.ron");
        prototypes.load("proto/fireattack.prototype.ron");
        prototypes.load("proto/redmushking.prototype.ron");
        prototypes.load("proto/sword.prototype.ron");
        prototypes.load("proto/chestplate.prototype.ron");
        prototypes.load("proto/metalpants.prototype.ron");
        prototypes.load("proto/metalshoes.prototype.ron");
        prototypes.load("proto/leatherpants.prototype.ron");
        prototypes.load("proto/forestpants.prototype.ron");
        prototypes.load("proto/forestshirt.prototype.ron");
        prototypes.load("proto/forestshoes.prototype.ron");
        prototypes.load("proto/leathertunic.prototype.ron");
        prototypes.load("proto/leathershoes.prototype.ron");
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
        prototypes.load("proto/grass2.prototype.ron");
        prototypes.load("proto/grass3.prototype.ron");
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
        prototypes.load("proto/metalbar.prototype.ron");
        prototypes.load("proto/coalboulder.prototype.ron");
        prototypes.load("proto/metalboulder.prototype.ron");
        prototypes.load("proto/slimegooprojectile.prototype.ron");
        prototypes.load("proto/stonechunk.prototype.ron");
        prototypes.load("proto/woodsword.prototype.ron");
        prototypes.load("proto/pebbleblock.prototype.ron");
        prototypes.load("proto/redmushroom.prototype.ron");
        prototypes.load("proto/redmushroomblock.prototype.ron");
        prototypes.load("proto/brownmushroom.prototype.ron");
        prototypes.load("proto/brownmushroomblock.prototype.ron");
        prototypes.load("proto/berrybush.prototype.ron");
        prototypes.load("proto/berries.prototype.ron");
        prototypes.load("proto/feather.prototype.ron");
        prototypes.load("proto/woodpickaxe.prototype.ron");
        prototypes.load("proto/hog.prototype.ron");
        prototypes.load("proto/mob_passive.prototype.ron");
        prototypes.load("proto/tusk.prototype.ron");
        prototypes.load("proto/bed.prototype.ron");
        prototypes.load("proto/bedblock.prototype.ron");
        prototypes.load("proto/magictusk.prototype.ron");
        prototypes.load("proto/magicgem.prototype.ron");
        prototypes.load("proto/leather.prototype.ron");
        prototypes.load("proto/rawmeat.prototype.ron");
        prototypes.load("proto/cookedmeat.prototype.ron");
        prototypes.load("proto/dirtpath.prototype.ron");
        prototypes.load("proto/stingfly.prototype.ron");
        prototypes.load("proto/bushling.prototype.ron");
        prototypes.load("proto/redmushling.prototype.ron");
        prototypes.load("proto/crate2.prototype.ron");
        prototypes.load("proto/bushlingscale.prototype.ron");
        prototypes.load("proto/bush.prototype.ron");
        prototypes.load("proto/bush2.prototype.ron");
        prototypes.load("proto/boulder2.prototype.ron");
        prototypes.load("proto/largestump.prototype.ron");
        prototypes.load("proto/largemushroomstump.prototype.ron");
        prototypes.load("proto/yellowflower.prototype.ron");
        prototypes.load("proto/yellowflowerblock.prototype.ron");
        prototypes.load("proto/redflower.prototype.ron");
        prototypes.load("proto/redflowerblock.prototype.ron");
        prototypes.load("proto/pinkflower.prototype.ron");
        prototypes.load("proto/pinkflowerblock.prototype.ron");
        prototypes.load("proto/stump.prototype.ron");
        prototypes.load("proto/stump2.prototype.ron");
        prototypes.load("proto/cattail.prototype.ron");
        prototypes.load("proto/lillypad.prototype.ron");
        prototypes.load("proto/waterboulder.prototype.ron");
        prototypes.load("proto/waterboulder2.prototype.ron");
        prototypes.load("proto/craftingtable.prototype.ron");
        prototypes.load("proto/alchemytable.prototype.ron");
        prototypes.load("proto/anvil.prototype.ron");
        prototypes.load("proto/cauldron.prototype.ron");
        prototypes.load("proto/furnace.prototype.ron");
        prototypes.load("proto/craftingtableblock.prototype.ron");
        prototypes.load("proto/alchemytableblock.prototype.ron");
        prototypes.load("proto/anvilblock.prototype.ron");
        prototypes.load("proto/cauldronblock.prototype.ron");
        prototypes.load("proto/furnaceblock.prototype.ron");
        prototypes.load("proto/redstew.prototype.ron");
        prototypes.load("proto/upgradestation.prototype.ron");
        prototypes.load("proto/upgradestationblock.prototype.ron");
        prototypes.load("proto/upgradetome.prototype.ron");
        prototypes.load("proto/orboftransformation.prototype.ron");
        prototypes.load("proto/bridge.prototype.ron");
        prototypes.load("proto/bridgeblock.prototype.ron");
        prototypes.load("proto/largemanapotion.prototype.ron");
        prototypes.load("proto/smallmanapotion.prototype.ron");
        prototypes.load("proto/dungeonexit.prototype.ron");
        prototypes.load("proto/woodwall.prototype.ron");
        prototypes.load("proto/woodwallblock.prototype.ron");
        prototypes.load("proto/wooddoor.prototype.ron");
        prototypes.load("proto/wooddooropen.prototype.ron");
        prototypes.load("proto/wooddoorblock.prototype.ron");
        prototypes.load("proto/essence.prototype.ron");
        prototypes.load("proto/key.prototype.ron");
        prototypes.load("proto/fairy.prototype.ron");
        prototypes.load("proto/miracleseed.prototype.ron");
        prototypes.load("proto/combatshrine.prototype.ron");
        prototypes.load("proto/combatshrinedone.prototype.ron");
        prototypes.load("proto/gambleshrine.prototype.ron");
        prototypes.load("proto/gambleshrinedone.prototype.ron");

        // Sapplings
        prototypes.load("proto/redsapplingblock.prototype.ron");
        prototypes.load("proto/yellowsapplingblock.prototype.ron");
        prototypes.load("proto/greensapplingblock.prototype.ron");
        prototypes.load("proto/redsapplingstage1.prototype.ron");
        prototypes.load("proto/greensapplingstage1.prototype.ron");
        prototypes.load("proto/yellowsapplingstage1.prototype.ron");
        prototypes.load("proto/redsapplingstage2.prototype.ron");
        prototypes.load("proto/greensapplingstage2.prototype.ron");
        prototypes.load("proto/yellowsapplingstage2.prototype.ron");
        prototypes.load("proto/redsapplingstage3.prototype.ron");
        prototypes.load("proto/greensapplingstage3.prototype.ron");
        prototypes.load("proto/yellowsapplingstage3.prototype.ron");

        // Era 2
        prototypes.load("proto/era2smalltree.prototype.ron");
        prototypes.load("proto/era2mediumtree.prototype.ron");
        prototypes.load("proto/era2largetree.prototype.ron");
        prototypes.load("proto/era2grass.prototype.ron");
        prototypes.load("proto/era2grass2.prototype.ron");
        prototypes.load("proto/era2grass3.prototype.ron");
        prototypes.load("proto/era2stump.prototype.ron");
        prototypes.load("proto/era2stump2.prototype.ron");
        prototypes.load("proto/era2deadbranch.prototype.ron");
        prototypes.load("proto/era2berrybush.prototype.ron");
        prototypes.load("proto/era2boulder.prototype.ron");
        prototypes.load("proto/era2boulder2.prototype.ron");
        prototypes.load("proto/era2brownmushroom.prototype.ron");
        prototypes.load("proto/era2brownmushroomblock.prototype.ron");
        prototypes.load("proto/era2redmushroom.prototype.ron");
        prototypes.load("proto/era2redmushroomblock.prototype.ron");
        prototypes.load("proto/era2coalboulder.prototype.ron");
        prototypes.load("proto/era2magicboulder.prototype.ron");
        prototypes.load("proto/era2pebble.prototype.ron");
        prototypes.load("proto/era2redflower.prototype.ron");
        prototypes.load("proto/era2redflowerblock.prototype.ron");
        prototypes.load("proto/era2whiteflower.prototype.ron");
        prototypes.load("proto/era2whiteflowerblock.prototype.ron");
        prototypes.load("proto/Era2WorldGenerationParams.prototype.ron");

        prototypes.load("proto/bossshrine.prototype.ron");
        prototypes.load("proto/timegate.prototype.ron");
        prototypes.load("proto/timefragment.prototype.ron");
    }
    fn check_proto_ready(prototypes: Prototypes) {
        for obj in WorldObject::iter() {
            if obj == WorldObject::None
                || obj == WorldObject::WaterTile
                || obj == WorldObject::GrassTile
                || obj == WorldObject::StoneTile
            {
                continue;
            }
            let p = <WorldObject as Into<&str>>::into(obj.clone()).to_owned();

            if !prototypes.is_ready(&p) {
                println!("proto {p:?} not ready");
                return;
            }
        }
        for mob in Mob::iter() {
            if mob == Mob::None {
                continue;
            }
            let p = <Mob as Into<&str>>::into(mob.clone()).to_owned();

            if !prototypes.is_ready(&p) {
                println!("proto {p:?} not ready");
                return;
            }
        }
        println!("READY, ENTERING GAME STATE");
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
