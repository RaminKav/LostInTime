use bevy::{prelude::*, transform::TransformSystem};

use bevy_proto::prelude::ProtoCommands;
use bevy_rapier2d::{
    geometry::Sensor,
    prelude::{
        ActiveEvents, CharacterLength, Collider, KinematicCharacterController,
        KinematicCharacterControllerOutput, PhysicsSet, QueryFilterFlags, RigidBody,
    },
};
use combat_heirlooms::{
    break_crates_with_roll, handle_ant_farm_state, handle_boss_hit_mana_orb_drops,
    handle_crate_break_damage, handle_death_defiance_freeze, handle_dodge_crit_activation,
    handle_dodge_crit_next_hit_reset, handle_mana_charge_damage, handle_mana_charge_damage_reset,
    handle_mana_orb_attack, handle_mana_orb_drops, handle_mana_regen_lightning,
    handle_mana_regen_poison, handle_max_hp_hunt, handle_reaper_soul_spawns,
    handle_skill_mana_regen, handle_skill_power_hunt, handle_summon_ring_state,
    handle_trigger_summons_on_heal, tick_dodge_crit_buff, tick_stand_still_state,
    update_ant_farm_ants, update_reaper_souls, update_stone_tooth, update_summon_ring,
    TriggerSummonsEvent,
};
use melee_skills::{
    handle_delayed_heirloom_casts, handle_echo_after_heal, handle_parry, handle_parry_success,
    handle_second_split_attack, handle_spear, handle_spear_gravity, handle_spear_pull_delay,
    tick_heirloom_trigger_cooldowns, tick_parried_timer, ParrySuccessEvent,
};
use rand::seq::SliceRandom;
use rogue_skills::{
    handle_add_combo_counter, handle_dodge_crit, handle_enemy_death_sprint_reset, handle_lunge,
    handle_lunge_cooldown, handle_sprint_timer, handle_sprinting_cooldown, handle_toggle_sprinting,
    pause_combo_anim_when_done, tick_combo_counter,
};
use serde::Deserialize;
use strum_macros::{Display, EnumIter};
pub mod achievements;
pub mod class_rank;
pub mod combat_heirlooms;
pub mod currency;
pub mod ice_slide;
pub mod levels;
pub mod mage_skills;
pub mod melee_skills;
pub mod rogue_skills;
pub mod score;
pub mod skill_heirlooms;
pub mod skills;
pub mod time_crystals;
pub mod unlocks;
pub use achievements::*;
pub use class_rank::*;
pub use currency::*;
use mage_skills::{handle_teleport, tick_just_teleported};
pub use score::*;
pub use time_crystals::*;
pub use unlocks::*;
pub mod stats;
use crate::{
    ai::{follow, idle, leap_attack},
    animations::player_sprite::{PlayerAnimation, PlayerAnimationState},
    attributes::{
        health_regen::{HealthRegenTimer, ManaRegenTimer},
        hunger::{Hunger, HungerTracker},
        modifiers::handle_modify_health_event,
        ActiveConsumableBuffs, Attack, AttackCooldown, AttributeQuality, AttributeValue,
        BonusAttackSpeed, CritChance, CritDamage, CurrentMana, HealthRegen, InvincibilityCooldown,
        ItemAttributes, ManaRegen, MaxHealth, MaxMana, PlayerAttributeBundle, ShieldRegen,
    },
    blessings::{HeirloomStatsBonuses, OwnedBlessings},
    client::is_not_paused,
    combat::pickup_radius::BASE_PICKUP_RADIUS,
    container::Container,
    custom_commands::CommandsExt,
    handle_hits,
    inputs::{move_camera_with_player, player_move_inputs, FacingDirection, MovementVector},
    inventory::{Inventory, INVENTORY_SIZE},
    item::{item_upgrades::ClawUpgradeMultiThrow, ActiveMainHandState, WorldObject},
    juice::RunDustTimer,
    player::rogue_skills::tick_lunge_shadows,
    proto::proto_param::ProtoParam,
    ui::{damage_numbers::handle_add_damage_numbers_after_hit, FlashExpBarEvent},
    world::{world_helpers::tile_pos_to_world_pos, y_sort::YSort, TileMapPosition},
    AppExt, CustomFlush, Game, GameParam, GameState, RawPosition,
};
use crate::{
    player::{achievements::AchievementsPlugin, skills::PlayerClass},
    run_once_per_run,
};
use skills::*;

use self::{
    levels::{handle_level_up, hide_particles_when_inv_open, PlayerLevel},
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
    /// Era 3 ice patches: normalized slide direction while locked; cleared when off ice.
    pub ice_slide_direction: Option<Vec2>,
    /// Seconds of post-ice momentum; input cannot steer until this hits zero.
    pub ice_momentum_remaining: f32,
    pub ice_momentum_direction: Option<Vec2>,
    /// Multiplier on base move delta while sliding; starts at 1.0 and ramps up (see inputs ice constants).
    pub ice_slide_speed_factor: f32,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            direction: FacingDirection::Left,
            is_moving: true,
            is_dashing: false,
            main_hand_slot: None,
            position: Vec3::ZERO,
            reach_distance: 8.5,
            player_dash_cooldown: Timer::from_seconds(0.75, TimerMode::Once),
            player_dash_duration: Timer::from_seconds(0.28, TimerMode::Once),
            next_hit_crit: false,
            ice_slide_direction: None,
            ice_momentum_remaining: 0.0,
            ice_momentum_direction: None,
            ice_slide_speed_factor: 1.0,
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
        app.add_plugin(AchievementsPlugin)
            .init_resource::<CoinCurrency>()
            .init_resource::<TimeFragmentCurrency>()
            .init_resource::<skills::HeirloomTriggerCounts>()
            .init_resource::<score::RunTimer>()
            .init_resource::<time_crystals::TimeCrystals>()
            .with_default_schedule(CoreSchedule::FixedUpdate, |app| {
                app.add_event::<MovePlayerEvent>()
                    .add_event::<ModifyCurencyEvent>()
                    .add_event::<ActiveSkillUsedEvent>()
                    .add_event::<ParrySuccessEvent>()
                    .add_event::<TriggerSummonsEvent>();
            })
            .add_system(spawn_player.in_schedule(OnExit(GameState::MainMenu)))
            .add_systems(
                (
                    handle_sprint_timer
                        .after(player_move_inputs)
                        .run_if(is_not_paused),
                    handle_sprinting_cooldown.run_if(is_not_paused),
                    handle_enemy_death_sprint_reset.after(handle_lunge),
                    handle_lunge_cooldown.run_if(is_not_paused),
                    send_attribute_event_on_stats_update,
                    handle_level_up,
                    handle_toggle_sprinting,
                    handle_teleport
                        .run_if(is_not_paused)
                        .before(skill_heirlooms::handle_active_skill_event),
                    hide_particles_when_inv_open,
                    tick_just_teleported.run_if(is_not_paused),
                    handle_second_split_attack.after(handle_add_damage_numbers_after_hit),
                    handle_dodge_crit,
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_systems(
                (
                    handle_ant_farm_state.run_if(is_not_paused),
                    update_ant_farm_ants.run_if(is_not_paused),
                    handle_summon_ring_state.run_if(is_not_paused),
                    update_summon_ring.run_if(is_not_paused),
                    update_stone_tooth.run_if(is_not_paused),
                    handle_mana_orb_drops.run_if(is_not_paused),
                    handle_boss_hit_mana_orb_drops.run_if(is_not_paused),
                    handle_reaper_soul_spawns.run_if(is_not_paused),
                    update_reaper_souls.run_if(is_not_paused),
                    break_crates_with_roll.run_if(is_not_paused),
                    // New heirloom systems
                    handle_max_hp_hunt.run_if(is_not_paused),
                    tick_stand_still_state.run_if(is_not_paused),
                    handle_death_defiance_freeze.run_if(is_not_paused),
                    skill_heirlooms::handle_druid_tree_taunt
                        .run_if(is_not_paused)
                        .before(crate::ai::follow),
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_systems(
                (
                    handle_crate_break_damage.run_if(is_not_paused),
                    handle_dodge_crit_activation.run_if(is_not_paused),
                    tick_dodge_crit_buff.run_if(is_not_paused),
                    handle_dodge_crit_next_hit_reset
                        .run_if(is_not_paused)
                        .after(handle_hits),
                    skill_heirlooms::handle_crit_heal
                        .run_if(is_not_paused)
                        .after(handle_hits),
                    handle_mana_charge_damage.run_if(is_not_paused),
                    handle_mana_charge_damage_reset
                        .run_if(is_not_paused)
                        .after(handle_hits),
                    handle_mana_orb_attack.run_if(is_not_paused),
                    handle_mana_regen_lightning.run_if(is_not_paused),
                    handle_mana_regen_poison.run_if(is_not_paused),
                    handle_skill_mana_regen.run_if(is_not_paused),
                    handle_skill_power_hunt.run_if(is_not_paused),
                    skill_heirlooms::track_enemy_hit_projectiles.run_if(is_not_paused),
                    skill_heirlooms::track_dagger_throw_kills.run_if(is_not_paused),
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_systems(
                (
                    handle_lunge.after(player_move_inputs).run_if(is_not_paused),
                    tick_combo_counter.run_if(is_not_paused),
                    handle_add_combo_counter,
                    skill_heirlooms::break_stealth_on_player_attack
                        .run_if(is_not_paused)
                        .after(crate::inputs::mouse_click_system)
                        .after(handle_sprint_timer),
                    skill_heirlooms::handle_active_skill_event.run_if(is_not_paused),
                    skill_heirlooms::add_rapidfire_speed_to_bonus.run_if(is_not_paused),
                    skill_heirlooms::tick_stealth_and_buffs.run_if(is_not_paused),
                    skill_heirlooms::tick_class_skill_hit_clear_timers.run_if(is_not_paused),
                    skill_heirlooms::tick_class_skill_slots.run_if(is_not_paused),
                    skill_heirlooms::tick_fury_duration_and_throw.run_if(is_not_paused),
                    skill_heirlooms::finalize_rapidfire_fury_charges.run_if(is_not_paused),
                    skill_heirlooms::tick_druid_tree_dummy_timers.run_if(is_not_paused),
                    skill_heirlooms::handle_fire_pillar_hit_clear.run_if(is_not_paused),
                    skill_heirlooms::handle_laser_beam_hit_clear.run_if(is_not_paused),
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_systems(
                (
                    skill_heirlooms::update_stealth_color.run_if(is_not_paused),
                    tick_lunge_shadows.run_if(is_not_paused),
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_systems(
                (skill_heirlooms::reduce_skill_cooldown_on_crit
                    .after(handle_hits)
                    .run_if(is_not_paused),)
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_systems(
                (
                    skill_heirlooms::tick_arrow_volley.run_if(is_not_paused),
                    skill_heirlooms::tick_pending_dagger_slashes.run_if(is_not_paused),
                    skill_heirlooms::handle_fury_skill.run_if(is_not_paused),
                    skill_heirlooms::handle_attach_bomb_target.run_if(is_not_paused),
                    skill_heirlooms::handle_bomb_explosion.run_if(is_not_paused),
                    skill_heirlooms::handle_rapidfire_slow_enemies.run_if(is_not_paused),
                    skill_heirlooms::handle_rapidfire_slow_remove.run_if(is_not_paused),
                    skill_heirlooms::handle_attach_possessed_blade_return.run_if(is_not_paused),
                    skill_heirlooms::tick_possessed_blade_movement.run_if(is_not_paused),
                    skill_heirlooms::handle_possessed_blade_kill_lifesteal
                        .after(handle_hits)
                        .run_if(is_not_paused),
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_systems(
                (
                    pause_combo_anim_when_done,
                    handle_parry.run_if(is_not_paused),
                    handle_spear.after(player_move_inputs).run_if(is_not_paused),
                    tick_parried_timer.run_if(is_not_paused),
                    handle_parry_success,
                    score::track_mob_kills.after(handle_hits),
                    score::track_item_destruction.after(handle_hits),
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(handle_mob_death_out_of_run_currency.in_set(OnUpdate(GameState::Main)))
            .add_system(
                skill_heirlooms::initialize_class_skill_slots
                    .run_if(is_not_paused)
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                handle_spear_pull_delay
                    .after(handle_spear)
                    .run_if(is_not_paused)
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                handle_spear_gravity
                    .after(idle)
                    .after(follow)
                    .after(leap_attack)
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_systems((handle_modify_currency,))
            .add_systems(
                (
                    tick_heirloom_trigger_cooldowns,
                    handle_echo_after_heal
                        .after(tick_heirloom_trigger_cooldowns)
                        .after(handle_modify_health_event)
                        .before(handle_add_damage_numbers_after_hit),
                    handle_trigger_summons_on_heal.after(handle_echo_after_heal),
                    handle_delayed_heirloom_casts,
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                reset_time_fragment_counters
                    .run_if(run_once_per_run())
                    .in_schedule(OnEnter(GameState::Main)),
            )
            .add_system(
                reset_coin_counters
                    .run_if(run_once_per_run())
                    .in_schedule(OnEnter(GameState::Main)),
            )
            .add_system(
                score::reset_run_score
                    .run_if(run_once_per_run())
                    .in_schedule(OnEnter(GameState::Main)),
            )
            .add_system(
                score::reset_run_timer
                    .run_if(run_once_per_run())
                    .in_schedule(OnEnter(GameState::Main)),
            )
            .add_system(
                score::tick_run_timer
                    .run_if(is_not_paused)
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                give_player_starting_items
                    .run_if(run_once_per_run())
                    .in_schedule(OnEnter(GameState::Main)),
            )
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
    mut game: ResMut<Game>,
    mut exp_sync_event: EventWriter<FlashExpBarEvent>,
    proto: ProtoParam,
    player_class: Option<Res<PlayerClass>>,
) {
    let cape_stack = proto.get_item_data(WorldObject::GreyCape).unwrap();
    let class = if let Some(class) = player_class {
        class.clone()
    } else {
        PlayerClass::default()
    };
    let p = commands
        .spawn((
            TransformBundle::from_transform(Transform::from_translation(Vec3::new(0., 0., 1.))),
            PlayerAnimation::Idle,
            PlayerAnimationState::new(),
            Player,
            Inventory {
                items: Container::with_size(INVENTORY_SIZE),
                equipment_items: Container::with_size(4)
                    .with_item_in_slot(3, cape_stack.clone())
                    .clone(),
                accessory_items: Container::with_size(4),
                weapon_items: Container::with_size(1),
                pet_items: Container::with_size(1),
                crafting_items: Container::with_size(0),
                furnace_items: Container::with_size(2),
                trash_items: Container::with_size(1),
                crafting_inputs_items: Container::with_size(3),
            },
            //TODO: remove itematt and construct from components?
            get_class_starting_stats(class.class.clone()),
            Hunger::new(100),
            HungerTracker::new(7., 8),
            InvincibilityCooldown(0.5),
            HealthRegenTimer(Timer::from_seconds(10., TimerMode::Once)),
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
        .insert(SkillClass::None)
        .insert(crate::combat::pickup_radius::PickupRadius(
            BASE_PICKUP_RADIUS,
        ))
        .insert(ShieldRegen {
            delay_timer: Timer::from_seconds(5.0, TimerMode::Once),
            regen_timer: Timer::from_seconds(0.2, TimerMode::Once),
        })
        .insert(RawPosition::default())
        .insert(PlayerAttributeBundle {
            health: MaxHealth(get_max_health_for_class(class.class.clone())),
            mana: MaxMana(get_max_mana_for_class(class.class.clone())),
            attack: Attack(0),
            health_regen: HealthRegen(2),
            mana_regen: ManaRegen(10),
            crit_chance: CritChance(5),
            crit_damage: CritDamage(150),
            attack_cooldown: AttackCooldown(0.4),
            ..default()
        })
        .insert(CurrentMana(get_max_mana_for_class(class.class.clone())))
        .insert(VisibilityBundle::default())
        .insert(FacingDirection::Down)
        .insert(ActiveEvents::COLLISION_EVENTS)
        .insert(ManaRegenTimer(Timer::from_seconds(6., TimerMode::Once)))
        .insert(RunDustTimer(Timer::from_seconds(0.25, TimerMode::Once)))
        .insert(RigidBody::KinematicPositionBased)
        .insert(PlayerLevel::new(1))
        .insert(PlayerStats::new())
        .insert(Sensor)
        .insert(OwnedBlessings::default())
        .insert(HeirloomStatsBonuses::default())
        .insert(PlayerSkills::default())
        .insert(skills::ClassSkillSlots::default())
        .insert(SkillPoints { count: 0 })
        .insert(BonusAttackSpeed::new())
        .insert(ActiveConsumableBuffs::default())
        .insert(ClawUpgradeMultiThrow(
            Timer::from_seconds(0.12, TimerMode::Once),
            0,
        ))
        .id();

    // let mut hunger = Hunger::new(100);

    // Try to load inv from save
    // if let Ok(save_file) = File::open(datafiles::save_file()) {
    //     let reader = BufReader::new(save_file);

    //     // Read the JSON contents of the file as an instance of `User`.
    //     match serde_json::from_reader::<_, CurrentRunSaveData>(reader) {
    //         Ok(data) => {
    //             hunger.current = data.player_hunger;
    //             commands.entity(p).insert((
    //                 data.inventory,
    //                 data.player_level,
    //                 data.player_stats,
    //                 data.skill_points,
    //                 data.current_health,
    //                 data.player_skills.clone(),
    //                 PreviousHealth(data.current_health.0),
    //                 TimeFragmentCurrency::new(
    //                     data.currency.0,
    //                     data.currency.1,
    //                     total_currency_all_time,
    //                 ),
    //                 hunger,
    //                 Transform::from_translation(data.player_transform.extend(0.)),
    //                 RawPosition(data.player_transform),
    //             ));
    //             for skill in data.player_skills.heirlooms.clone() {
    //                 skill.heirloom.add_skill_components(
    //                     p,
    //                     &mut commands,
    //                     data.player_skills.clone(),
    //                 );
    //             }
    //             info!("LOADED PLAYER DATA FROM SAVE FILE");
    //         }
    //         Err(err) => error!("Failed to load data from file {err:?}"),
    //     }
    // }
    game.player = p;
    exp_sync_event.send_default();
}

fn give_player_starting_items(
    mut proto_commands: ProtoCommands,
    mut commands: Commands,
    proto: ProtoParam,
    mut game: GameParam,
    player_class: Option<Res<PlayerClass>>,
    class_ranks: Option<Res<ClassRankSystem>>,
    run_state: ResMut<RunUnlockState>,
    unlock_upgrades: Option<Res<UnlockUpgrades>>,
) {
    // if let Ok(save_file) = File::open(datafiles::save_file()) {
    //     let reader = BufReader::new(save_file);

    //     if serde_json::from_reader::<_, CurrentRunSaveData>(reader).is_ok() {
    //         return;
    //     }
    // }

    let selected_class = player_class
        .as_ref()
        .map(|pc| pc.class.clone())
        .unwrap_or(SkillClass::None);

    // Give class-specific starting weapon with rarity based on class rank
    let starting_weapon = selected_class.get_starting_wep();
    let weapon_rarity = if let Some(ranks) = &class_ranks {
        ranks.get_starting_weapon_rarity(&selected_class)
    } else {
        crate::attributes::ItemRarity::Common
    };

    for pet in player_class
        .as_ref()
        .map(|pc| pc.pets.clone())
        .unwrap_or_default()
    {
        commands.spawn((
            pet,
            YSort(0.001),
            // Collider::capsule(Vec2::new(0., -6.), Vec2::new(0., -6.), 5.0),
            Transform::from_xyz(40.0, -40.0, 1.0),
            Name::new("Pet"),
        ));
    }

    // Give starting tools based on unlocks (always, regardless of pending rewards)
    let player_pos = game.player().position.truncate();
    if let Some(upgrades) = unlock_upgrades.as_ref() {
        if upgrades.has_wood_axe() {
            proto_commands.spawn_item_from_proto(WorldObject::WoodAxe, &proto, player_pos, 1, None);
        }

        if upgrades.has_pickaxe() {
            proto_commands.spawn_item_from_proto(
                WorldObject::WoodPickaxe,
                &proto,
                player_pos,
                1,
                None,
            );
        }

        force_player_autopick(&mut game);
    }

    // proto_commands.spawn_item_from_proto(WorldObject::UpgradeTome, &proto, Vec2::ZERO, 64, None);
    // proto_commands.spawn_item_from_proto(WorldObject::PlasmaStaff, &proto, Vec2::ZERO, 1, Some(1));
    // proto_commands.spawn_item_from_proto(WorldObject::Chestplate, &proto, Vec2::ZERO, 1, Some(1));
    // proto_commands.spawn_item_from_proto(WorldObject::Chestplate, &proto, Vec2::ZERO, 1, Some(1));
    // proto_commands.spawn_item_from_proto(WorldObject::Pendant, &proto, Vec2::ZERO, 1, Some(1));
    // proto_commands.spawn_item_from_proto(WorldObject::Ring, &proto, Vec2::ZERO, 1, Some(1));
    // proto_commands.spawn_item_from_proto(WorldObject::Pendant, &proto, Vec2::ZERO, 1, Some(1));
    // proto_commands.spawn_item_from_proto(WorldObject::Ring, &proto, Vec2::ZERO, 1, Some(1));
    // proto_commands.spawn_item_from_proto(WorldObject::Pendant, &proto, Vec2::ZERO, 1, Some(1));
    // proto_commands.spawn_item_from_proto(WorldObject::Ring, &proto, Vec2::ZERO, 1, Some(1));
    // proto_commands.spawn_item_from_proto(
    //     WorldObject::OrbOfTransformation,
    //     &proto,
    //     Vec2::ZERO,
    //     64,
    //     None,
    // );

    // Handle pending rewards (food, tomes, orbs)
    if run_state.pending_rewards {
        let mut rng = rand::thread_rng();
        let food_options = [
            WorldObject::RedMushroomBlock,
            WorldObject::BrownMushroomBlock,
            WorldObject::Apple,
        ];

        let player_pos = game.player().position.truncate();

        let mut upgrade_rewards = vec![];

        for _ in 0..run_state.pending_food {
            upgrade_rewards.push(
                *food_options
                    .choose(&mut rng)
                    .unwrap_or(&WorldObject::RedMushroomBlock),
            );
        }
        // for _ in 0..run_state.pending_tomes {
        //     upgrade_rewards.push(WorldObject::UpgradeTome);
        // }

        // for _ in 0..run_state.pending_orbs {
        //     upgrade_rewards.push(WorldObject::OrbOfTransformation);
        // }
        upgrade_rewards.iter().for_each(|obj| {
            spawn_reward_drop(&mut proto_commands, &proto, player_pos, *obj, 1);
        });

        force_player_autopick(&mut game);
    }

    // Spawn the starting weapon and mark it for rarity override
    let player_pos = game.player().position.truncate();
    if let Some(weapon_entity) = proto_commands.spawn_item_from_proto(
        starting_weapon,
        &proto,
        player_pos,
        1,
        Some(1), // Use default level, we'll override rarity instead
    ) {
        // Mark this weapon as a starting weapon with specific rarity
        commands.entity(weapon_entity).insert(StartingWeapon {
            rarity: weapon_rarity,
        });
        force_player_autopick(&mut game);
    }
    // run_state.pending_food = 0;
    // run_state.pending_tomes = 0;
    // run_state.pending_orbs = 0;
    // run_state.pending_rewards = false;
    // proto_commands.spawn_item_from_proto(WorldObject::Spear, &proto, Vec2::ZERO, 1, Some(1));
    // proto_commands.spawn_item_from_proto(WorldObject::Hammer, &proto, Vec2::ZERO, 1, Some(1));
    // proto_commands.spawn_item_from_proto(WorldObject::Dagger, &proto, Vec2::ZERO, 1, Some(1));
    // proto_commands.spawn_item_from_proto(WorldObject::FireStaff, &proto, Vec2::ZERO, 1, Some(1));
    // proto_commands.spawn_item_from_proto(WorldObject::IceStaff, &proto, Vec2::ZERO, 1, Some(1));
    // proto_commands.spawn_item_from_proto(WorldObject::MagicWhip, &proto, Vec2::ZERO, 1, None);
    // proto_commands.spawn_item_from_proto(WorldObject::BasicStaff, &proto, Vec2::ZERO, 1, None);
    // proto_commands.spawn_item_from_proto(WorldObject::Blowdart, &proto, Vec2::ZERO, 1, Some(1));
    // proto_commands.spawn_item_from_proto(WorldObject::Gun, &proto, Vec2::ZERO, 1, Some(1));
    // proto_commands.spawn_item_from_proto(WorldObject::WoodBow, &proto, Vec2::ZERO, 1, Some(1));
    // proto_commands.spawn_item_from_proto(WorldObject::Claw, &proto, Vec2::ZERO, 1, None);
    // proto_commands.spawn_item_from_proto(WorldObject::Essence, &proto, Vec2::ZERO, 10, None);
    // proto_commands.spawn_item_from_proto(WorldObject::BedBlock, &proto, Vec2::ZERO, 1, None);
    // proto_commands.spawn_item_from_proto(WorldObject::MagicTusk, &proto, Vec2::ZERO, 64, None);
    // proto_commands.spawn_item_from_proto(WorldObject::WoodWallBlock, &proto, Vec2::ZERO, 64, None);
    // proto_commands.spawn_item_from_proto(WorldObject::WoodAxe, &proto, Vec2::ZERO, 1, None);
    // proto_commands.spawn_item_from_proto(WorldObject::WoodPlank, &proto, Vec2::ZERO, 1,None);
    // proto_commands.spawn_item_from_proto(WorldObject::WoodDoorBlock, &proto, Vec2::ZERO, 40, None);
    // proto_commands.spawn_item_from_proto(WorldObject::ThrowingStar, &proto, Vec2::ZERO, 10,None);
    // proto_commands.spawn_item_from_proto(WorldObject::BridgeBlock, &proto, Vec2::ZERO, 64,None);
    // proto_commands.spawn_item_from_proto(WorldObject::FurnaceBlock, &proto, Vec2::ZERO, 64, None);
    // proto_commands.spawn_item_from_proto(WorldObject::Pendant, &proto, Vec2::ZERO, 1, Some(3));
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
    // proto_commands.spawn_item_from_proto(WorldObject::ScrapperBlock, &proto, Vec2::ZERO, 64, None);
}

pub fn spawn_reward_drop(
    proto_commands: &mut ProtoCommands,
    proto: &ProtoParam,
    pos: Vec2,
    obj: WorldObject,
    count: usize,
) -> bool {
    proto_commands
        .spawn_item_from_proto(obj, proto, pos, count, None)
        .is_some()
}

pub fn force_player_autopick(game: &mut GameParam) {
    if game.player().is_moving {
        return;
    }

    game.player_mut().is_moving = true;
}
pub fn get_max_health_for_class(class: SkillClass) -> i32 {
    match class {
        SkillClass::Warrior => 150,
        SkillClass::Wizard => 70,
        SkillClass::Rogue => 125,
        SkillClass::Thief => 100,
        SkillClass::Hunter => 120,
        SkillClass::None => 100,
    }
}
pub fn get_max_mana_for_class(class: SkillClass) -> i32 {
    match class {
        SkillClass::Warrior => 60,
        SkillClass::Wizard => 150,
        SkillClass::Rogue => 100,
        SkillClass::Thief => 100,
        SkillClass::Hunter => 100,
        SkillClass::None => 100,
    }
}
pub fn get_class_starting_stats(class: SkillClass) -> ItemAttributes {
    match class {
        SkillClass::Warrior => ItemAttributes {
            health: AttributeValue::new(
                get_max_health_for_class(class.clone()),
                AttributeQuality::Low,
                0.,
            ),
            mana: AttributeValue::new(
                get_max_mana_for_class(class.clone()),
                AttributeQuality::Low,
                0.,
            ),
            attack: AttributeValue::new(0, AttributeQuality::Low, 0.),
            health_regen: AttributeValue::new(5, AttributeQuality::Low, 0.),
            mana_regen: AttributeValue::new(10, AttributeQuality::Low, 0.),
            crit_chance: AttributeValue::new(5, AttributeQuality::Low, 0.),
            crit_damage: AttributeValue::new(150, AttributeQuality::Low, 0.),
            ..default()
        },
        SkillClass::Wizard => ItemAttributes {
            health: AttributeValue::new(
                get_max_health_for_class(class.clone()),
                AttributeQuality::Low,
                0.,
            ),
            mana: AttributeValue::new(
                get_max_mana_for_class(class.clone()),
                AttributeQuality::Low,
                0.,
            ),
            attack: AttributeValue::new(0, AttributeQuality::Low, 0.),
            health_regen: AttributeValue::new(2, AttributeQuality::Low, 0.),
            mana_regen: AttributeValue::new(15, AttributeQuality::Low, 0.),
            crit_chance: AttributeValue::new(5, AttributeQuality::Low, 0.),
            crit_damage: AttributeValue::new(150, AttributeQuality::Low, 0.),
            ..default()
        },
        SkillClass::Rogue => ItemAttributes {
            health: AttributeValue::new(
                get_max_health_for_class(class.clone()),
                AttributeQuality::Low,
                0.,
            ),
            mana: AttributeValue::new(
                get_max_mana_for_class(class.clone()),
                AttributeQuality::Low,
                0.,
            ),
            attack: AttributeValue::new(0, AttributeQuality::Low, 0.),
            health_regen: AttributeValue::new(2, AttributeQuality::Low, 0.),
            mana_regen: AttributeValue::new(10, AttributeQuality::Low, 0.),
            crit_chance: AttributeValue::new(15, AttributeQuality::Low, 0.),
            crit_damage: AttributeValue::new(150, AttributeQuality::Low, 0.),
            speed: AttributeValue::new(20, AttributeQuality::Low, 0.),
            ..default()
        },
        SkillClass::Thief => ItemAttributes {
            health: AttributeValue::new(
                get_max_health_for_class(class.clone()),
                AttributeQuality::Low,
                0.,
            ),
            mana: AttributeValue::new(
                get_max_mana_for_class(class.clone()),
                AttributeQuality::Low,
                0.,
            ),
            attack: AttributeValue::new(0, AttributeQuality::Low, 0.),
            health_regen: AttributeValue::new(2, AttributeQuality::Low, 0.),
            mana_regen: AttributeValue::new(10, AttributeQuality::Low, 0.),
            crit_chance: AttributeValue::new(10, AttributeQuality::Low, 0.),
            crit_damage: AttributeValue::new(150, AttributeQuality::Low, 0.),
            speed: AttributeValue::new(7, AttributeQuality::Low, 0.),
            ..default()
        },
        SkillClass::Hunter => ItemAttributes {
            health: AttributeValue::new(
                get_max_health_for_class(class.clone()),
                AttributeQuality::Low,
                0.,
            ),
            mana: AttributeValue::new(
                get_max_mana_for_class(class.clone()),
                AttributeQuality::Low,
                0.,
            ),
            attack: AttributeValue::new(0, AttributeQuality::Low, 0.),
            health_regen: AttributeValue::new(2, AttributeQuality::Low, 0.),
            mana_regen: AttributeValue::new(10, AttributeQuality::Low, 0.),
            crit_chance: AttributeValue::new(20, AttributeQuality::Low, 0.),
            crit_damage: AttributeValue::new(150, AttributeQuality::Low, 0.),
            ..default()
        },
        SkillClass::None => ItemAttributes {
            health: AttributeValue::new(
                get_max_health_for_class(class.clone()),
                AttributeQuality::Low,
                0.,
            ),
            mana: AttributeValue::new(
                get_max_mana_for_class(class.clone()),
                AttributeQuality::Low,
                0.,
            ),
            attack: AttributeValue::new(0, AttributeQuality::Low, 0.),
            health_regen: AttributeValue::new(2, AttributeQuality::Low, 0.),
            mana_regen: AttributeValue::new(10, AttributeQuality::Low, 0.),
            crit_chance: AttributeValue::new(5, AttributeQuality::Low, 0.),
            crit_damage: AttributeValue::new(150, AttributeQuality::Low, 0.),
            ..default()
        },
    }
}
