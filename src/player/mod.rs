use bevy::prelude::*;

use bevy_proto::prelude::ProtoCommands;
use bevy_rapier2d::prelude::{
    ActiveEvents, CharacterLength, Collider, KinematicCharacterController,
    KinematicCharacterControllerOutput, QueryFilterFlags, RigidBody,
};
use serde::Deserialize;
use strum_macros::{Display, EnumIter};
pub mod levels;
pub mod stats;
use crate::{
    animations::{
        enemy_sprites::{CharacterAnimationSpriteSheetData, EnemyAnimationState},
        AnimationTimer,
    },
    attributes::{
        health_regen::{HealthRegenTimer, ManaRegenTimer},
        hunger::Hunger,
        Attack, AttackCooldown, CritChance, CritDamage, HealthRegen, InvincibilityCooldown,
        ItemAttributes, Mana, ManaRegen, MaxHealth, PlayerAttributeBundle,
    },
    custom_commands::CommandsExt,
    inputs::{move_player, FacingDirection, MovementVector},
    inventory::{Container, Inventory, INVENTORY_SIZE},
    item::{get_crafting_inventory_item_stacks, EquipmentData, Recipes, WorldObject},
    juice::RunDustTimer,
    proto::proto_param::ProtoParam,
    ui::crafting_ui::CraftingContainerType,
    world::{world_helpers::tile_pos_to_world_pos, y_sort::YSort, TileMapPosition},
    AppExt, Game, GameParam, GameState, RawPosition,
};

use self::{
    levels::{handle_level_up, spawn_particles_when_leveling, PlayerLevel},
    stats::{send_attribute_event_on_stats_update, PlayerStats, SkillPoints},
};
pub struct PlayerPlugin;

pub struct MovePlayerEvent {
    pub pos: TileMapPosition,
}
#[derive(Component, Debug)]
pub struct Player;
#[derive(Debug, Clone)]
pub struct PlayerState {
    pub direction: FacingDirection,
    pub is_moving: bool,
    pub is_dashing: bool,
    pub is_attacking: bool,
    pub main_hand_slot: Option<EquipmentData>,
    pub position: Vec3,
    pub reach_distance: f32,
    pub player_dash_cooldown: Timer,
    pub player_dash_duration: Timer,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            direction: FacingDirection::Left,
            is_moving: false,
            is_dashing: false,
            is_attacking: false,
            main_hand_slot: None,
            position: Vec3::ZERO,
            reach_distance: 1.5,
            player_dash_cooldown: Timer::from_seconds(1.0, TimerMode::Once),
            player_dash_duration: Timer::from_seconds(0.225, TimerMode::Once),
        }
    }
}
#[derive(Component, EnumIter, Display, Debug, Hash, Copy, Clone, PartialEq, Eq, Deserialize)]
pub enum Limb {
    Torso,
    Hands,
    Legs,
    Head,
}
impl Limb {
    pub fn from_slot(slot: usize) -> Vec<Self> {
        match slot {
            3 => vec![Self::Head],
            2 => vec![Self::Torso, Self::Hands],
            1 => vec![Self::Legs],
            0 => vec![],
            _ => panic!("Invalid slot"),
        }
    }
}
impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.with_default_schedule(CoreSchedule::FixedUpdate, |app| {
            app.add_event::<MovePlayerEvent>();
        })
        .add_startup_system(spawn_player)
        .add_systems((
            send_attribute_event_on_stats_update,
            load_recipes_into_inventory_container_on_startup.run_if(resource_changed::<Recipes>()),
            handle_level_up,
            spawn_particles_when_leveling,
            give_player_starting_items.in_schedule(OnEnter(GameState::Main)),
        ))
        .add_system(handle_move_player.in_set(OnUpdate(GameState::Main)))
        .add_system(
            handle_player_raw_position
                .before(move_player)
                .in_set(OnUpdate(GameState::Main)),
        );
    }
}
pub fn handle_move_player(
    mut player: Query<(&mut RawPosition, &mut Transform), With<Player>>,
    mut move_events: EventReader<MovePlayerEvent>,
) {
    for m in move_events.iter() {
        //TODO: Add world helper to get chunk -> world pos, lots of copy code in item.rs

        let world_pos = tile_pos_to_world_pos(m.pos, false);

        let (mut raw_pos, mut pos) = player.single_mut();
        raw_pos.0 = world_pos;
        pos.translation = world_pos.extend(0.);
    }
}
/// Updates the player's [RawPosition] based on the [KinematicCharacterControllerOutput]
/// we store the un-rounded raw position, and then round the [Transform] position.
pub fn handle_player_raw_position(
    mut player_pos: Query<(&mut RawPosition, &mut Transform), With<Player>>,
    kcc: Query<
        &KinematicCharacterControllerOutput,
        (With<Player>, Changed<KinematicCharacterControllerOutput>),
    >,
    mut game: GameParam,
) {
    let (mut raw_pos, mut pos) = player_pos.single_mut();
    if let Ok(kcc) = kcc.get_single() {
        raw_pos.0 += kcc.effective_translation;
    };

    let delta = raw_pos.0 - pos.translation.truncate();
    pos.translation.x += delta.x;
    pos.translation.y += delta.y;
    pos.translation.x = pos.translation.x.round();
    pos.translation.y = pos.translation.y.round();
    game.player_mut().position = pos.translation;
}
fn load_recipes_into_inventory_container_on_startup(
    mut added_inv: Query<&mut Inventory>,
    recipes: Res<Recipes>,
    proto: ProtoParam,
) {
    if recipes.crafting_list.len() == 0 {
        return;
    }
    for mut inv in added_inv.iter_mut() {
        //TODO: fix this to read from recipes not a hardcoded list
        let objs = recipes
            .crafting_list
            .iter()
            .filter(|r| r.1 .1 == CraftingContainerType::Inventory)
            .map(|r| *r.0)
            .collect();
        inv.crafting_items = Container {
            items: get_crafting_inventory_item_stacks(objs, &recipes, &proto),
            ..default()
        };
    }
}
fn spawn_player(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlases: ResMut<Assets<TextureAtlas>>,
    mut game: ResMut<Game>,
) {
    //spawn player entity with limb spritesheets as children
    let player_texture_handle = asset_server.load("textures/player/player_down.png");
    let player_texture_atlas =
        TextureAtlas::from_grid(player_texture_handle, Vec2::new(64., 64.), 7, 5, None, None);
    let player_texture_atlas_handle = texture_atlases.add(player_texture_atlas);

    let p = commands
        .spawn((
            SpriteSheetBundle {
                texture_atlas: player_texture_atlas_handle,
                transform: Transform::from_translation(Vec3::new(0., 0., 1.)),
                ..default()
            },
            AnimationTimer(Timer::from_seconds(0.1, TimerMode::Repeating)),
            Player,
            Inventory {
                items: Container::with_size(INVENTORY_SIZE),
                equipment_items: Container::with_size(4),
                accessory_items: Container::with_size(4),
                ..default()
            },
            //TODO: remove itematt and construct from components?
            ItemAttributes {
                health: 100,
                attack: 0,
                health_regen: 2,
                crit_chance: 5,
                crit_damage: 150,
                ..default()
            },
            Hunger::new(100, 5., 8),
            PlayerAttributeBundle {
                health: MaxHealth(100),
                mana: Mana::new(100),
                attack: Attack(0),
                health_regen: HealthRegen(2),
                mana_regen: ManaRegen(1),
                crit_chance: CritChance(5),
                crit_damage: CritDamage(150),
                attack_cooldown: AttackCooldown(0.4),
                ..default()
            },
            InvincibilityCooldown(1.),
            HealthRegenTimer(Timer::from_seconds(20., TimerMode::Once)),
            MovementVector::default(),
            YSort(0.001),
            Name::new("Player"),
            Collider::capsule(Vec2::new(0., -4.0), Vec2::new(0., -4.5), 4.5),
            KinematicCharacterController {
                // The character offset is set to 0.01.
                offset: CharacterLength::Absolute(0.01),
                filter_flags: QueryFilterFlags::EXCLUDE_SENSORS,
                ..default()
            },
            RawPosition::default(),
        ))
        .insert(CharacterAnimationSpriteSheetData {
            animation_frames: vec![6, 6, 4, 6, 7],
            anim_offset: 0,
        })
        .insert(FacingDirection::Down)
        .insert(ActiveEvents::COLLISION_EVENTS)
        .insert(EnemyAnimationState::Idle)
        .insert(ManaRegenTimer(Timer::from_seconds(0.5, TimerMode::Once)))
        .insert(PlayerLevel::new(1))
        .insert(PlayerStats::new())
        .insert(PlayerStats::new())
        .insert(RunDustTimer(Timer::from_seconds(0.25, TimerMode::Once)))
        .insert(SkillPoints { count: 0 })
        .insert(RigidBody::KinematicPositionBased)
        .id();
    game.player = p;
}

fn give_player_starting_items(mut proto_commands: ProtoCommands, proto: ProtoParam) {
    proto_commands.spawn_item_from_proto(WorldObject::WoodSword, &proto, Vec2::ZERO, 1);
    // proto_commands.spawn_item_from_proto(WorldObject::FireStaff, &proto, Vec2::ZERO, 1);
    // proto_commands.spawn_item_from_proto(WorldObject::Claw, &proto, Vec2::ZERO, 1);
    // proto_commands.spawn_item_from_proto(WorldObject::ThrowingStar, &proto, Vec2::ZERO, 10);
    // proto_commands.spawn_item_from_proto(WorldObject::BasicStaff, &proto, Vec2::ZERO, 1);
    // proto_commands.spawn_item_from_proto(WorldObject::MagicWhip, &proto, Vec2::ZERO, 1);
    // proto_commands.spawn_item_from_proto(WorldObject::BridgeBlock, &proto, Vec2::ZERO, 64);
    // proto_commands.spawn_item_from_proto(WorldObject::FurnaceBlock, &proto, Vec2::ZERO, 64);
    // proto_commands.spawn_item_from_proto(WorldObject::UpgradeStationBlock, &proto, Vec2::ZERO, 64);
    // proto_commands.spawn_item_from_proto(WorldObject::UpgradeTome, &proto, Vec2::ZERO, 64);
    // proto_commands.spawn_item_from_proto(WorldObject::OrbOfTransformation, &proto, Vec2::ZERO, 64);
    // proto_commands.spawn_item_from_proto(WorldObject::Chestplate, &proto, Vec2::ZERO, 64);
    // proto_commands.spawn_item_from_proto(WorldObject::RawMeat, &proto, Vec2::ZERO, 64);
    // proto_commands.spawn_item_from_proto(WorldObject::WoodPickaxe, &proto, Vec2::ZERO, 1);
    // proto_commands.spawn_item_from_proto(WorldObject::Log, &proto, Vec2::ZERO, 64);
    // proto_commands.spawn_item_from_proto(WorldObject::StoneChunk, &proto, Vec2::ZERO, 64);
    // proto_commands.spawn_item_from_proto(WorldObject::Coal, &proto, Vec2::ZERO, 64);
    // proto_commands.spawn_item_from_proto(WorldObject::MetalShard, &proto, Vec2::ZERO, 64);
    // proto_commands.spawn_item_from_proto(WorldObject::MetalBar, &proto, Vec2::ZERO, 64);
    // proto_commands.spawn_item_from_proto(WorldObject::PlantFibre, &proto, Vec2::ZERO, 64);
    // proto_commands.spawn_item_from_proto(WorldObject::Stick, &proto, Vec2::ZERO, 64);
    // proto_commands.spawn_item_from_proto(WorldObject::LargePotion, &proto, Vec2::ZERO, 64);
    // proto_commands.spawn_item_from_proto(WorldObject::BushlingScale, &proto, Vec2::ZERO, 64);
    // proto_commands.spawn_item_from_proto(WorldObject::Tusk, &proto, Vec2::ZERO, 64);
    // proto_commands.spawn_item_from_proto(WorldObject::Leather, &proto, Vec2::ZERO, 64);
    // proto_commands.spawn_item_from_proto(WorldObject::Feather, &proto, Vec2::ZERO, 64);
    // proto_commands.spawn_item_from_proto(WorldObject::CraftingTableBlock, &proto, Vec2::ZERO, 64);
    // proto_commands.spawn_item_from_proto(WorldObject::AlchemyTableBlock, &proto, Vec2::ZERO, 64);
    // proto_commands.spawn_item_from_proto(WorldObject::StoneWallBlock, &proto, Vec2::ZERO, 64);
    // proto_commands.spawn_item_from_proto(WorldObject::ChestBlock, &proto, Vec2::ZERO, 64);
}
