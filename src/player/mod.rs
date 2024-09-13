use std::{fs::File, io::BufReader};

use bevy::{prelude::*, transform::TransformSystem};

use bevy_aseprite::{anim::AsepriteAnimation, AsepriteBundle};
use bevy_proto::prelude::ProtoCommands;
use bevy_rapier2d::{
    geometry::Sensor,
    prelude::{
        ActiveEvents, CharacterLength, Collider, KinematicCharacterController,
        KinematicCharacterControllerOutput, PhysicsSet, QueryFilterFlags, RigidBody,
    },
};
use melee_skills::{handle_echo_after_heal, handle_on_hit_skills, handle_second_split_attack};
use serde::Deserialize;
use sprint::{
    handle_dodge_crit, handle_enemy_death_sprint_reset, handle_sprint_timer,
    handle_sprinting_cooldown, handle_toggle_sprinting,
};
use strum_macros::{Display, EnumIter};
pub mod currency;
pub mod levels;
pub mod melee_skills;
pub mod skills;
pub mod sprint;
pub mod teleport;
pub use currency::*;
use teleport::{handle_teleport, tick_just_teleported, tick_teleport_timer};
pub mod stats;
use crate::{
    animations::player_sprite::{PlayerAnimation, PlayerAnimationState, PlayerAseprite},
    attributes::{
        health_regen::{HealthRegenTimer, ManaRegenTimer},
        hunger::{Hunger, HungerTracker},
        Attack, AttackCooldown, AttributeQuality, AttributeValue, CritChance, CritDamage,
        HealthRegen, InvincibilityCooldown, ItemAttributes, Mana, ManaRegen, MaxHealth,
        PlayerAttributeBundle,
    },
    client::CurrentRunSaveData,
    container::Container,
    custom_commands::CommandsExt,
    datafiles, handle_hits,
    inputs::{move_camera_with_player, move_player, FacingDirection, MovementVector},
    inventory::{Inventory, INVENTORY_SIZE},
    item::{ActiveMainHandState, WorldObject},
    juice::RunDustTimer,
    proto::proto_param::ProtoParam,
    ui::{
        damage_numbers::{handle_add_damage_numbers_after_hit, PreviousHealth},
        FlashExpBarEvent,
    },
    world::{world_helpers::tile_pos_to_world_pos, y_sort::YSort, TileMapPosition},
    AppExt, CustomFlush, Game, GameParam, GameState, RawPosition,
};
use skills::*;

use self::{
    levels::{
        handle_level_up, hide_particles_when_inv_open, spawn_particles_when_leveling, PlayerLevel,
    },
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
    pub main_hand_slot: Option<ActiveMainHandState>,
    pub position: Vec3,
    pub reach_distance: f32,
    pub player_dash_cooldown: Timer,
    pub player_dash_duration: Timer,
    pub next_hit_crit: bool,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            direction: FacingDirection::Left,
            is_moving: true,
            is_dashing: false,
            main_hand_slot: None,
            position: Vec3::ZERO,
            reach_distance: 1.5,
            player_dash_cooldown: Timer::from_seconds(1.5, TimerMode::Once),
            player_dash_duration: Timer::from_seconds(0.39, TimerMode::Once),
            next_hit_crit: false,
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
            app.add_event::<MovePlayerEvent>()
                .add_event::<ModifyTimeFragmentsEvent>();
        })
        .add_system(spawn_player.in_schedule(OnExit(GameState::MainMenu)))
        .add_systems(
            (
                handle_sprint_timer.after(move_player),
                handle_sprinting_cooldown,
                send_attribute_event_on_stats_update,
                handle_level_up,
                handle_enemy_death_sprint_reset,
                handle_toggle_sprinting,
                spawn_particles_when_leveling,
                hide_particles_when_inv_open,
                handle_modify_time_fragments,
                tick_just_teleported,
                handle_teleport,
                tick_teleport_timer,
                handle_second_split_attack.after(handle_add_damage_numbers_after_hit),
                handle_on_hit_skills.after(handle_hits),
                handle_dodge_crit,
            )
                .in_set(OnUpdate(GameState::Main)),
        )
        .add_systems(
            (handle_echo_after_heal.before(handle_add_damage_numbers_after_hit),)
                .in_set(OnUpdate(GameState::Main)),
        )
        .add_system(give_player_starting_items.in_schedule(OnEnter(GameState::Main)))
        .add_system(handle_move_player.before(CustomFlush))
        .add_system(
            handle_player_raw_position
                .run_if(in_state(GameState::Main))
                .after(PhysicsSet::SyncBackendFlush)
                .before(TransformSystem::TransformPropagate)
                .before(move_camera_with_player)
                .in_base_set(CoreSet::PostUpdate),
        );
    }
}
pub fn handle_move_player(
    mut player: Query<(&mut RawPosition, &mut Transform), With<Player>>,
    mut move_events: EventReader<MovePlayerEvent>,
) {
    for m in move_events.iter() {
        debug!("MOVING PLAYER TO {:?}", m.pos);
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
    if let Ok((mut raw_pos, mut pos)) = player_pos.get_single_mut() {
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
}
fn spawn_player(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut game: ResMut<Game>,
    mut exp_sync_event: EventWriter<FlashExpBarEvent>,
) {
    //spawn player entity with limb spritesheets as children
    let p = commands
        .spawn((
            AsepriteBundle {
                aseprite: asset_server.load(PlayerAseprite::PATH),
                animation: AsepriteAnimation::from(PlayerAseprite::tags::IDLE_FRONT),
                transform: Transform::from_translation(Vec3::new(0., 0., 1.)),
                ..default()
            },
            PlayerAnimation::Idle,
            PlayerAnimationState::new(),
            Player,
            Inventory {
                items: Container::with_size(INVENTORY_SIZE),
                equipment_items: Container::with_size(4),
                accessory_items: Container::with_size(4),
                crafting_items: Container::with_size(0),
                ..default()
            },
            //TODO: remove itematt and construct from components?
            ItemAttributes {
                health: AttributeValue::new(100, AttributeQuality::Low, 0.),
                attack: AttributeValue::new(0, AttributeQuality::Low, 0.),
                health_regen: AttributeValue::new(2, AttributeQuality::Low, 0.),
                crit_chance: AttributeValue::new(5, AttributeQuality::Low, 0.),
                crit_damage: AttributeValue::new(150, AttributeQuality::Low, 0.),
                ..default()
            },
            Hunger::new(100),
            HungerTracker::new(7., 8),
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
        ))
        .insert(RawPosition::default())
        .insert(PlayerAttributeBundle {
            health: MaxHealth(100),
            mana: Mana::new(100),
            attack: Attack(0),
            health_regen: HealthRegen(2),
            mana_regen: ManaRegen(5),
            crit_chance: CritChance(5),
            crit_damage: CritDamage(150),
            attack_cooldown: AttackCooldown(0.4),
            ..default()
        })
        .insert(VisibilityBundle::default())
        .insert(FacingDirection::Down)
        .insert(ActiveEvents::COLLISION_EVENTS)
        .insert(ManaRegenTimer(Timer::from_seconds(2.5, TimerMode::Once)))
        .insert(RunDustTimer(Timer::from_seconds(0.25, TimerMode::Once)))
        .insert(RigidBody::KinematicPositionBased)
        .insert(PlayerLevel::new(1))
        .insert(PlayerStats::new())
        .insert(TimeFragmentCurrency::default())
        .insert(Sensor)
        .insert(PlayerSkills::default())
        .insert(SkillPoints { count: 0 })
        .id();

    let mut hunger = Hunger::new(100);
    // Try to load inv from save
    if let Ok(save_file) = File::open(datafiles::save_file()) {
        let reader = BufReader::new(save_file);

        // Read the JSON contents of the file as an instance of `User`.
        match serde_json::from_reader::<_, CurrentRunSaveData>(reader) {
            Ok(data) => {
                hunger.current = data.player_hunger;
                commands.entity(p).insert((
                    data.inventory,
                    data.player_level,
                    data.player_stats,
                    data.skill_points,
                    data.current_health,
                    data.player_skills.clone(),
                    PreviousHealth(data.current_health.0),
                    TimeFragmentCurrency::new(data.currency),
                    hunger,
                    Transform::from_translation(data.player_transform.extend(0.)),
                    RawPosition(data.player_transform),
                ));
                for skill in data.player_skills.skills.clone() {
                    skill.add_skill_components(p, &mut commands, data.player_skills.clone());
                }
                info!("LOADED PLAYER DATA FROM SAVE FILE");
            }
            Err(err) => error!("Failed to load data from file {err:?}"),
        }
    }
    game.player = p;
    exp_sync_event.send_default();
}

fn give_player_starting_items(mut proto_commands: ProtoCommands, proto: ProtoParam) {
    if let Ok(_) = File::open(datafiles::save_file()) {
        return;
    }
    proto_commands.spawn_item_from_proto(WorldObject::WoodSword, &proto, Vec2::ZERO, 1, Some(1));
    // proto_commands.spawn_item_from_proto(WorldObject::Essence, &proto, Vec2::ZERO, 10, None);
    // proto_commands.spawn_item_from_proto(WorldObject::BedBlock, &proto, Vec2::ZERO, 1, None);
    // proto_commands.spawn_item_from_proto(WorldObject::Dagger, &proto, Vec2::ZERO, 1, Some(1));
    // proto_commands.spawn_item_from_proto(WorldObject::MagicTusk, &proto, Vec2::ZERO, 64, None);
    // proto_commands.spawn_item_from_proto(WorldObject::WoodWallBlock, &proto, Vec2::ZERO, 64, None);
    // proto_commands.spawn_item_from_proto(WorldObject::WoodAxe, &proto, Vec2::ZERO, 1, None);
    // proto_commands.spawn_item_from_proto(WorldObject::WoodPlank, &proto, Vec2::ZERO, 1,None);
    // proto_commands.spawn_item_from_proto(WorldObject::WoodDoorBlock, &proto, Vec2::ZERO, 40, None);
    // proto_commands.spawn_item_from_proto(WorldObject::FireStaff, &proto, Vec2::ZERO, 1, Some(1));
    // proto_commands.spawn_item_from_proto(WorldObject::WoodBow, &proto, Vec2::ZERO, 1, Some(1));
    // proto_commands.spawn_item_from_proto(WorldObject::Claw, &proto, Vec2::ZERO, 1, None);
    // proto_commands.spawn_item_from_proto(WorldObject::ThrowingStar, &proto, Vec2::ZERO, 10,None);
    // proto_commands.spawn_item_from_proto(WorldObject::BasicStaff, &proto, Vec2::ZERO, 1, None);
    // proto_commands.spawn_item_from_proto(WorldObject::MagicWhip, &proto, Vec2::ZERO, 1,None);
    // proto_commands.spawn_item_from_proto(WorldObject::BridgeBlock, &proto, Vec2::ZERO, 64,None);
    // proto_commands.spawn_item_from_proto(WorldObject::FurnaceBlock, &proto, Vec2::ZERO, 64, None);
    // proto_commands.spawn_item_from_proto(
    //     WorldObject::UpgradeStationBlock,
    //     &proto,
    //     Vec2::ZERO,
    //     64,
    //     None,
    // );
    // proto_commands.spawn_item_from_proto(WorldObject::UpgradeTome, &proto, Vec2::ZERO, 64, None);
    // proto_commands.spawn_item_from_proto(
    //     WorldObject::OrbOfTransformation,
    //     &proto,
    //     Vec2::ZERO,
    //     64,
    //     None,
    // );
    // proto_commands.spawn_item_from_proto(WorldObject::Ring, &proto, Vec2::ZERO, 1, Some(3));
    // proto_commands.spawn_item_from_proto(WorldObject::RawMeat, &proto, Vec2::ZERO, 64, None);
    // proto_commands.spawn_item_from_proto(WorldObject::WoodPickaxe, &proto, Vec2::ZERO, 1,None);
    // proto_commands.spawn_item_from_proto(WorldObject::Log, &proto, Vec2::ZERO, 64,None);
    // proto_commands.spawn_item_from_proto(WorldObject::StoneChunk, &proto, Vec2::ZERO, 64,None);
    // proto_commands.spawn_item_from_proto(WorldObject::Coal, &proto, Vec2::ZERO, 64,None);
    // proto_commands.spawn_item_from_proto(WorldObject::MetalShard, &proto, Vec2::ZERO, 64,None);
    // proto_commands.spawn_item_from_proto(WorldObject::MetalBar, &proto, Vec2::ZERO, 64,None);
    // proto_commands.spawn_item_from_proto(WorldObject::PlantFibre, &proto, Vec2::ZERO, 64,None);
    // proto_commands.spawn_item_from_proto(WorldObject::Stick, &proto, Vec2::ZERO, 64,None);
    // proto_commands.spawn_item_from_proto(WorldObject::SmallPotion, &proto, Vec2::ZERO, 64, None);
    // proto_commands.spawn_item_from_proto(WorldObject::Apple, &proto, Vec2::ZERO, 64, None);
    // proto_commands.spawn_item_from_proto(WorldObject::WoodPickaxe, &proto, Vec2::ZERO, 1, Some(1));
    // proto_commands.spawn_item_from_proto(WorldObject::WoodAxe, &proto, Vec2::ZERO, 1, Some(1));
    // proto_commands.spawn_item_from_proto(WorldObject::BushlingScale, &proto, Vec2::ZERO, 64,None);
    // proto_commands.spawn_item_from_proto(WorldObject::Tusk, &proto, Vec2::ZERO, 64,None);
    // proto_commands.spawn_item_from_proto(WorldObject::WoodDoor, &proto, Vec2::ZERO, 1, None);
    // proto_commands.spawn_item_from_proto(WorldObject::WoodWallBlock, &proto, Vec2::ZERO, 64, None);
    // proto_commands.spawn_item_from_proto(
    //     WorldObject::CraftingTableBlock,
    //     &proto,
    //     Vec2::ZERO,
    //     64,
    //     None,
    // );
    // proto_commands.spawn_item_from_proto(
    //     WorldObject::AlchemyTableBlock,
    //     &proto,
    //     Vec2::ZERO,
    //     64,
    //     None,
    // );
    // proto_commands.spawn_item_from_proto(WorldObject::StoneWallBlock, &proto, Vec2::ZERO, 64, None);
    // proto_commands.spawn_item_from_proto(WorldObject::ChestBlock, &proto, Vec2::ZERO, 64, None);
    proto_commands.spawn_item_from_proto(WorldObject::ScrapperBlock, &proto, Vec2::ZERO, 64, None);
}
