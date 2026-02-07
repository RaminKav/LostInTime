use crate::blessings::OwnedBlessings;
use crate::chaos::ChaosTracker;
use crate::cursor::CursorPos;
use crate::item::potion_buffs::MovementSpeedBuff;
use crate::ui::tips::SeenTips;
use std::f32::consts::PI;
use std::time::Duration;

use crate::ai::pathfinding::world_pos_to_AIPos;

use crate::animations::player_sprite::PlayerAnimation;
use crate::animations::AttackEvent;
use crate::assets::SpriteAnchor;
use crate::attributes::hunger::Hunger;
use crate::audio::{AudioSoundEffect, SoundSpawner};
use crate::client::is_not_paused;
use crate::enemy::spawn_helpers::can_spawn_mob_here;
use crate::enemy::spawner::GlobalSpawners;
use crate::juice::{DustParticles, RunDustTimer};
use crate::player::skills::{
    ActiveSkill, ActiveSkillUsedEvent, BombState, BuckshotSkillState, DaggerThrowState,
    DruidTreeSkillState, FuryState, HealSkillState, Heirloom, IceWallSkillState, LaserBeamState,
    LightningState, PlayerSkills, SlashState, SpinAttackState, TripleThrowState,
};
use crate::ui::key_input_guide::InteractionGuideTrigger;
use crate::world::dimension::{DimensionSpawnEvent, Era};
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::transform::TransformSystem;
use bevy::window::PrimaryWindow;

use bevy_hanabi::EffectSpawner;
use bevy_proto::prelude::{ProtoCommands, ReflectSchematic, Schematic};

use bevy_rapier2d::prelude::{KinematicCharacterController, PhysicsSet};
use interpolation::Lerp;
use rand::rngs::ThreadRng;
use rand::seq::IteratorRandom;
use rand::Rng;

use crate::attributes::{CurrentMana, Speed};
use crate::combat::{AttackTimer, HitEvent};

use crate::enemy::Mob;
use crate::inventory::Inventory;
use crate::item::ammo::Ammo;
use crate::item::item_actions::{ItemActionParam, ItemActions, ManaCost};
use crate::item::object_actions::ObjectAction;
use crate::item::projectile::{RangedAttack, RangedAttackEvent};
use crate::item::{Equipment, WorldObject};
use crate::proto::proto_param::ProtoParam;
use crate::ui::{
    change_hotbar_slot,
    tips::{Tip, TipEvent},
    EssenceShopChoices, FlashExpBarEvent, InventoryState, UIState,
};
use crate::world::chunk::Chunk;

use crate::world::world_helpers::world_pos_to_tile_pos;

use crate::player::mage_skills::TeleportState;
use crate::player::melee_skills::SpearState;
use crate::player::rogue_skills::{LungeState, SprintState};
use crate::player::skills::{
    FirePillarState, PiercingStarSkillState, RapidfireState, ShoutSkillState, StealthState,
};
use crate::{
    bounce_player, update_bounce_effect, update_shadow, BounceEffect, BounceEvent, Game,
    GameUpscale, InputMappings, Player, UpdatePetWeaponEvent, DEBUG, PLAYER_DASH_SPEED, TIME_STEP,
};
use crate::{
    custom_commands::CommandsExt, AppExt, CustomFlush, GameParam, GameState, MainCamera,
    RawPosition, TextureCamera, UICamera, PLAYER_MOVE_SPEED,
};

const HOTBAR_KEYCODES: [KeyCode; 6] = [
    KeyCode::Key1,
    KeyCode::Key2,
    KeyCode::Key3,
    KeyCode::Key4,
    KeyCode::Key5,
    KeyCode::Key6,
];
pub struct InputsPlugin;

impl Plugin for InputsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(CursorPos::default())
            .register_type::<CursorPos>()
            .add_event::<BounceEvent>()
            // .add_plugin(ResourceInspectorPlugin::<CursorPos>::default())
            .with_default_schedule(CoreSchedule::FixedUpdate, |app| {
                app.add_event::<AttackEvent>();
            })
            .add_systems(
                (
                    bounce_player.run_if(is_not_paused),
                    update_shadow.run_if(is_not_paused),
                    update_bounce_effect.before(handle_hotbar_key_input),
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_systems(
                (
                    player_move_inputs.run_if(is_not_paused),
                    turn_player.run_if(is_not_paused),
                    mouse_click_system.run_if(is_not_paused).after(CustomFlush),
                    dispatch_active_skill_events.run_if(is_not_paused),
                    handle_hotbar_key_input,
                    tick_dash_timer.run_if(is_not_paused),
                    handle_open_essence_ui,
                    diagnostics,
                    handle_quick_hotbar_consume.before(handle_hotbar_key_input),
                    handle_interact_objects.run_if(is_not_paused),
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_systems((
                toggle_inventory.run_if(in_state(GameState::Main)),
                close_container.run_if(in_state(GameState::Main)),
            ))
            .add_system(
                move_camera_with_player
                    .after(PhysicsSet::SyncBackendFlush)
                    .before(TransformSystem::TransformPropagate)
                    .in_base_set(CoreSet::PostUpdate)
                    .run_if(in_state(GameState::Main)),
            );
    }
}

#[derive(Component, Debug, Default)]
pub struct MovementVector(pub Vec2);

#[derive(Debug, Clone, PartialEq, Component, Eq, Default, Schematic, FromReflect, Reflect)]
#[reflect(Component, Schematic, Default)]

pub enum FacingDirection {
    Left,
    #[default]
    Right,
    Up,
    Down,
}
impl FacingDirection {
    pub fn get_anim_dir_str(&self) -> &str {
        match self {
            Self::Left => "Side",
            Self::Right => "Side",
            Self::Up => "Back",
            Self::Down => "Front",
        }
    }
    pub fn get_dir_vec(&self) -> Vec2 {
        match self {
            Self::Left => Vec2::new(-1., 0.),
            Self::Right => Vec2::new(1., 0.),
            Self::Up => Vec2::new(0., 1.),
            Self::Down => Vec2::new(0., -1.),
        }
    }
    pub fn from_translation(translation: Vec2) -> Self {
        if translation.x.abs() > translation.y.abs() {
            if translation.x > 0. {
                Self::Right
            } else {
                Self::Left
            }
        } else if translation.y > 0. {
            Self::Up
        } else {
            Self::Down
        }
    }
    pub fn get_next_rand_dir(&self, mut rng: ThreadRng) -> &Self {
        let mut new_dir = self;
        while new_dir == self {
            let rng = rng.gen_range(0..=4);
            if rng <= 1 {
                new_dir = &Self::Left;
            } else if rng <= 2 {
                new_dir = &Self::Right;
            } else if rng <= 3 {
                new_dir = &Self::Up;
            } else if rng <= 4 {
                new_dir = &Self::Down;
            }
        }
        new_dir
    }
    pub fn new_rand_dir(mut rng: ThreadRng) -> Self {
        let mut new_dir = Self::Left;

        let rng = rng.gen_range(0..=4);
        if rng <= 1 {
            new_dir = Self::Left;
        } else if rng <= 2 {
            new_dir = Self::Right;
        } else if rng <= 3 {
            new_dir = Self::Up;
        } else if rng <= 4 {
            new_dir = Self::Down;
        }
        new_dir
    }
}

fn turn_player(
    mut game: ResMut<Game>,
    player_query: Query<&FacingDirection, With<Player>>,
    cursor_pos: Res<CursorPos>,
    mut commands: Commands,
) {
    let angle = f32::atan(cursor_pos.ui_coords.x / cursor_pos.ui_coords.y).abs();
    let dir = if cursor_pos.ui_coords.y > 0. {
        if angle < PI / 4. {
            FacingDirection::Up
        } else if cursor_pos.ui_coords.x < 0. {
            FacingDirection::Left
        } else {
            FacingDirection::Right
        }
    } else if angle < PI / 4. {
        FacingDirection::Down
    } else if cursor_pos.ui_coords.x < 0. {
        FacingDirection::Left
    } else {
        FacingDirection::Right
    };
    //TODO: make center point based on player pos on screen?
    //TODO: add some way for attack to know dir
    let curr_dir = player_query.single();
    if &dir != curr_dir {
        commands.entity(game.player).insert(dir.clone());
        game.player_state.direction = dir.clone();
    }
}
pub fn player_move_inputs(
    mut game: GameParam,
    mut player_query: Query<
        (
            Entity,
            &mut KinematicCharacterController,
            &mut MovementVector,
            &PlayerAnimation,
            &Speed,
            &Hunger,
            &mut RunDustTimer,
            &PlayerSkills,
            Option<&BounceEffect>,
            Option<&MovementSpeedBuff>,
        ),
        (
            With<Player>,
            Without<MainCamera>,
            Without<Chunk>,
            Without<Equipment>,
        ),
    >,
    time: Res<Time>,
    key_input: Res<Input<KeyCode>>,
    mouse_input: Res<Input<MouseButton>>,
    mut commands: Commands,
    mut particle: Query<&mut EffectSpawner, With<DustParticles>>,
    asset_server: Res<AssetServer>,
    audio: Res<Audio>,
    mut audio_timer: Local<Timer>,
    mut active_skill_event: EventWriter<ActiveSkillUsedEvent>,
    mut ammo_query: Query<&mut Ammo>,
    keybinds: Res<crate::keybinds::InputMappings>,
) {
    if audio_timer.duration() == Duration::ZERO {
        *audio_timer = Timer::from_seconds(0.2, TimerMode::Once);
    }
    let (
        player_e,
        mut player_kcc,
        mut mv,
        curr_anim,
        speed,
        hunger,
        mut run_dust_timer,
        skills,
        bounce_option,
        movement_speed_buff,
    ) = player_query.single_mut();
    if bounce_option.is_some() {
        return;
    }
    let player = game.player_mut();
    let mut d = Vec2::ZERO;
    let movement_speed_multiplier = movement_speed_buff
        .map(|buff| buff.speed_multiplier)
        .unwrap_or(1.0);

    let s = PLAYER_MOVE_SPEED
        * time.delta_seconds()
        * (1. + speed.0 as f32 / 100.)
        * (if hunger.is_starving() { 0.7 } else { 1. })
        * movement_speed_multiplier
        * curr_anim.action_movement_restriction(player.main_hand_slot.clone().map(|s| s.get_obj()));

    if key_input.pressed(KeyCode::A) || key_input.pressed(KeyCode::Left) {
        d.x -= 1.;
        player.is_moving = true;
    }
    if key_input.pressed(KeyCode::D) || key_input.pressed(KeyCode::Right) {
        d.x += 1.;
        player.is_moving = true;
    }
    if key_input.pressed(KeyCode::W) || key_input.pressed(KeyCode::Up) {
        d.y += 1.;
        player.is_moving = true;
    }
    if key_input.pressed(KeyCode::S) || key_input.pressed(KeyCode::Down) {
        d.y -= 1.;
        player.is_moving = true;
    }
    //TODO: move this tick to animations.rs
    if let Some(roll_slot) = skills.has_active_skill(ActiveSkill::Roll) {
        if player.player_dash_cooldown.finished()
            && keybinds.check_skill_input(roll_slot, &key_input, &mouse_input)
        {
            player.is_dashing = true;
            active_skill_event.send(ActiveSkillUsedEvent {
                slot: roll_slot,
                cooldown: player.player_dash_cooldown.duration().as_secs_f32(),
            });
            player.player_dash_cooldown.reset();
            commands.spawn(SoundSpawner::new(AudioSoundEffect::Roll, 0.25));
        }
    }
    if (key_input.any_just_released([KeyCode::A, KeyCode::D, KeyCode::S, KeyCode::W])
        && !key_input.any_pressed([KeyCode::A, KeyCode::D, KeyCode::S, KeyCode::W]))
        || (d.x == 0. && d.y == 0.)
    {
        player.is_moving = false;
    }

    // Manual reload on R key for current ranged weapon
    if key_input.just_pressed(KeyCode::R) {
        if let Some(main_hand) = player.main_hand_slot.clone() {
            if let Ok(mut ammo) = ammo_query.get_mut(main_hand.entity) {
                if !ammo.reloading && ammo.current < ammo.max {
                    ammo.start_reload();
                }
            }
        }
    }
    if d.x != 0. || d.y != 0. {
        d = d.normalize() * s;
    }
    let is_speeding_up = player.player_dash_duration.percent() < 0.5;
    if player.is_dashing {
        if curr_anim != &PlayerAnimation::Roll && player.player_dash_duration.percent() == 0. {
            commands.entity(player_e).insert(PlayerAnimation::Roll);
        }
        d.x = if is_speeding_up {
            d.x.lerp(
                &(d.x * PLAYER_DASH_SPEED * TIME_STEP),
                &(player.player_dash_duration.percent() * 2.),
            )
        } else {
            d.x.lerp(
                &(d.x * PLAYER_DASH_SPEED * TIME_STEP),
                &(1. - (player.player_dash_duration.percent())),
            )
        };
        d.y = if is_speeding_up {
            d.y.lerp(
                &(d.y * PLAYER_DASH_SPEED * TIME_STEP),
                &(player.player_dash_duration.percent() * 2.),
            )
        } else {
            d.y.lerp(
                &(d.y * PLAYER_DASH_SPEED * TIME_STEP),
                &(1. - (player.player_dash_duration.percent())),
            )
        };
    }
    mv.0 = d;
    if d.x != 0. || d.y != 0. {
        player_kcc.translation = Some(Vec2::new(d.x, d.y));

        if curr_anim == &PlayerAnimation::Idle {
            commands.entity(player_e).insert(PlayerAnimation::Walk);
        }

        if run_dust_timer.0.percent() == 0. {
            particle.single_mut().reset();
            run_dust_timer.0.tick(time.delta());
        } else {
            run_dust_timer.0.tick(time.delta());
            if run_dust_timer.0.finished() {
                run_dust_timer.0.reset()
            }
        }
        //audio
        audio_timer.tick(time.delta());
        if audio_timer.finished() {
            audio_timer.reset();
            let walk1 = asset_server.load("sounds/walk_grass1.ogg");
            let walk2 = asset_server.load("sounds/walk_grass2.ogg");
            let walk3 = asset_server.load("sounds/walk_grass3.ogg");
            let walk4 = asset_server.load("sounds/walk_grass4.ogg");
            let walk5 = asset_server.load("sounds/walk_grass5.ogg");
            let walks = vec![walk1, walk2, walk3, walk4, walk5];
            walks.iter().choose(&mut rand::thread_rng()).map(|sound| {
                audio.play_with_settings(sound.clone(), PlaybackSettings::ONCE.with_volume(0.35))
            });
        }
    } else if curr_anim.is_walking() {
        commands.entity(player_e).insert(PlayerAnimation::Idle);
    }
}

pub fn dispatch_active_skill_events(
    mut ev: EventWriter<ActiveSkillUsedEvent>,
    key_input: Res<Input<KeyCode>>,
    mouse_input: Res<Input<MouseButton>>,
    player_q: Query<
        (
            &PlayerSkills,
            Option<&SprintState>,
            Option<&LungeState>,
            Option<&TeleportState>,
            Option<&SpearState>,
            Option<&StealthState>,
            Option<&RapidfireState>,
            Option<&FirePillarState>,
            Option<&HealSkillState>,
            Option<&BuckshotSkillState>,
            Option<&IceWallSkillState>,
            Option<&DruidTreeSkillState>,
            Option<&ShoutSkillState>,
            Option<&PiercingStarSkillState>,
            Option<&LaserBeamState>,
        ),
        With<Player>,
    >,
    player_q2: Query<
        (
            Option<&LightningState>,
            Option<&DaggerThrowState>,
            Option<&SlashState>,
            Option<&TripleThrowState>,
            Option<&FuryState>,
            Option<&BombState>,
            Option<&SpinAttackState>,
        ),
        With<Player>,
    >,
    slot1_trackers: Query<&crate::player::skills::Slot1ChargeTracker, With<Player>>,
    slot2_trackers: Query<&crate::player::skills::Slot2ChargeTracker, With<Player>>,
    keybinds: Res<crate::keybinds::InputMappings>,
) {
    let Ok((
        skills,
        sprint_state,
        lunge_state,
        teleport_state,
        spear_state,
        stealth_state,
        rapid_state,
        pillar_state,
        heal_state,
        buckshot_state,
        icewall_state,
        druidtree_state,
        shout_state,
        piercing_star_state,
        laser_beam_state,
    )) = player_q.get_single()
    else {
        return;
    };
    let Ok((
        lightning_state,
        dagger_throw_state,
        slash_state,
        triple_throw_state,
        fury_state,
        bomb_state,
        spin_attack_state,
    )) = player_q2.get_single()
    else {
        return;
    };
    // Only handle ONE key per frame to prevent multiple slots from triggering
    // Check all keys first, then handle only the highest priority one

    let slot_4_pressed = keybinds.check_skill_input(4, &key_input, &mouse_input);
    let slot_3_pressed = keybinds.check_skill_input(3, &key_input, &mouse_input);
    let slot_2_pressed = keybinds.check_skill_input(2, &key_input, &mouse_input);
    let slot_1_pressed = keybinds.check_skill_input(1, &key_input, &mouse_input);
    let slot_0_pressed = keybinds.check_skill_input(0, &key_input, &mouse_input);

    let pressed_slot = if slot_4_pressed {
        Some(4)
    } else if slot_3_pressed {
        Some(3)
    } else if slot_2_pressed {
        Some(2)
    } else if slot_1_pressed {
        Some(1)
    } else if slot_0_pressed {
        Some(0)
    } else {
        None
    };

    if let Some(slot) = pressed_slot {
        if let Some(skill) = skills.get_active_skill_in_slot(slot) {
            // Determine base cooldown from skill definition
            // And gate dispatch by cooldown state if present
            let base_cooldown = skill.get_base_cooldown();

            // For slot 1 and 2 (class skills), check charges first
            if slot == 1 {
                if let Ok(tracker) = slot1_trackers.get_single() {
                    // If we have charges available, allow activation regardless of cooldown
                    if tracker.0.current_charges > 0 {
                        ev.send(ActiveSkillUsedEvent {
                            slot,
                            cooldown: base_cooldown,
                        });
                        return; // Exit early after handling this key press
                    }
                }
            } else if slot == 2 {
                if let Ok(tracker) = slot2_trackers.get_single() {
                    // If we have charges available, allow activation regardless of cooldown
                    if tracker.0.current_charges > 0 {
                        ev.send(ActiveSkillUsedEvent {
                            slot,
                            cooldown: base_cooldown,
                        });
                        return; // Exit early after handling this key press
                    }
                }
            }

            // Otherwise, check cooldown as normal
            let on_cooldown = match skill {
                ActiveSkill::Roll => true, // handled in player_move_inputs
                ActiveSkill::Sprint => sprint_state
                    .map(|s| !s.sprint_cooldown_timer.finished())
                    .unwrap_or(false),
                ActiveSkill::SprintLunge => lunge_state
                    .map(|s| !s.lunge_cooldown_timer.finished())
                    .unwrap_or(false),
                ActiveSkill::Teleport => teleport_state
                    .map(|t| !t.cooldown_timer.finished())
                    .unwrap_or(false),
                ActiveSkill::Parry => false, //--- IGNORE ---
                ActiveSkill::ParrySpear => spear_state
                    .map(|s| !s.cooldown_timer.finished())
                    .unwrap_or(false),
                ActiveSkill::Stealth => stealth_state
                    .map(|s| !s.cooldown_timer.finished())
                    .unwrap_or(false),
                ActiveSkill::Rapidfire => rapid_state
                    .map(|s| !s.cooldown_timer.finished())
                    .unwrap_or(false),
                ActiveSkill::FirePillar => pillar_state
                    .map(|s| !s.cooldown_timer.finished())
                    .unwrap_or(false),
                ActiveSkill::Heal => heal_state
                    .map(|s| !s.cooldown_timer.finished())
                    .unwrap_or(false),
                ActiveSkill::Buckshot => buckshot_state
                    .map(|s| !s.cooldown_timer.finished())
                    .unwrap_or(false),
                ActiveSkill::IceWall => icewall_state
                    .map(|s| !s.cooldown_timer.finished())
                    .unwrap_or(false),
                ActiveSkill::DruidTree => druidtree_state
                    .map(|s| !s.cooldown_timer.finished())
                    .unwrap_or(false),
                ActiveSkill::Shout => shout_state
                    .map(|s| !s.cooldown_timer.finished())
                    .unwrap_or(false),
                ActiveSkill::PiercingStar => piercing_star_state
                    .map(|s| !s.cooldown_timer.finished())
                    .unwrap_or(false),
                ActiveSkill::LaserBeam => laser_beam_state
                    .map(|s| !s.cooldown_timer.finished())
                    .unwrap_or(false),
                ActiveSkill::Lightning => lightning_state
                    .map(|s| !s.cooldown_timer.finished())
                    .unwrap_or(false),
                ActiveSkill::DaggerThrow => dagger_throw_state
                    .map(|s| !s.cooldown_timer.finished())
                    .unwrap_or(false),
                ActiveSkill::DaggerSlash => slash_state
                    .map(|s| !s.cooldown_timer.finished())
                    .unwrap_or(false),
                ActiveSkill::TripleThrow => triple_throw_state
                    .map(|s| !s.cooldown_timer.finished())
                    .unwrap_or(false),
                ActiveSkill::Fury => fury_state
                    .map(|s| !s.cooldown_timer.finished())
                    .unwrap_or(false),
                ActiveSkill::Bomb => bomb_state
                    .map(|s| !s.cooldown_timer.finished())
                    .unwrap_or(false),
                ActiveSkill::SpinAttack => spin_attack_state
                    .map(|s| !s.cooldown_timer.finished())
                    .unwrap_or(false),
            };
            if !on_cooldown && base_cooldown > 0.0 {
                ev.send(ActiveSkillUsedEvent {
                    slot,
                    cooldown: base_cooldown,
                });
            }
        }
    }
}
pub fn tick_dash_timer(mut game: GameParam, time: Res<Time>) {
    let player = game.player_mut();

    if player.is_dashing {
        player.player_dash_duration.tick(time.delta());
        if player.player_dash_duration.just_finished() {
            player.player_dash_duration.reset();
            player.is_dashing = false;
        }
    } else {
        player.player_dash_cooldown.tick(time.delta());
    }
}
pub fn close_container(
    key_input: ResMut<Input<KeyCode>>,
    mut next_inv_state: ResMut<NextState<UIState>>,
    curr_state: Res<State<UIState>>,
    game_state: Res<State<GameState>>,
) {
    if key_input.just_pressed(KeyCode::Escape) {
        // During gameplay (GameState::Main), toggle between closed and options
        if game_state.0 == GameState::Main {
            match curr_state.0 {
                UIState::Closed => {
                    // Open options menu
                    next_inv_state.set(UIState::Options);
                }
                UIState::Options => {
                    // Close options menu, resume game
                    next_inv_state.set(UIState::Closed);
                }
                // Don't close important selection UIs with ESC
                UIState::ItemChest | UIState::Skills => {
                    // Player must make a choice, can't accidentally close
                }
                UIState::ActiveSkills | UIState::ActiveSkillShrine => {
                    // Allow closing active skill selection with ESC
                    next_inv_state.set(UIState::Closed);
                }
                _ => {
                    // Close any other UI
                    next_inv_state.set(UIState::Closed);
                }
            }
        } else {
            // In main menu or other game states, just close UI
            // But still don't close important selection UIs
            match curr_state.0 {
                UIState::ItemChest | UIState::Skills => {
                    // Don't close
                }
                UIState::ActiveSkills | UIState::ActiveSkillShrine => {
                    // Allow closing active skill selection with ESC
                    next_inv_state.set(UIState::Closed);
                }
                _ => {
                    next_inv_state.set(UIState::Closed);
                }
            }
        }
    }
}
pub fn toggle_inventory(
    mut game: GameParam,
    key_input: Res<Input<KeyCode>>,
    mouse_input: Res<Input<MouseButton>>,
    mut proto_commands: ProtoCommands,
    mut dim_event: EventWriter<DimensionSpawnEvent>,
    proto: ProtoParam,
    inv: Query<&Inventory>,
    mut next_ui_state: ResMut<NextState<UIState>>,
    curr_ui_state: Res<State<UIState>>,
    cursor: Res<CursorPos>,
    mut flash_event: EventWriter<FlashExpBarEvent>,
    keybinds: Res<InputMappings>,
    mut chaos_tracker: ResMut<ChaosTracker>,
    mut tip_event: EventWriter<TipEvent>,
    seen_tips: Res<SeenTips>,
) {
    if keybinds.check_inv_input(&key_input, &mouse_input) {
        // Don't allow opening inventory while item chest is open
        if curr_ui_state.0 != UIState::ItemChest {
            next_ui_state.set(UIState::Inventory);

            if let Ok(inventory) = inv.get_single() {
                let occupied_slots = inventory
                    .items
                    .items
                    .iter()
                    .filter(|slot| slot.is_some())
                    .count();
                if occupied_slots >= 6 && !seen_tips.has_seen(&Tip::Recipes) {
                    tip_event.send(TipEvent {
                        tip: Tip::Recipes,
                        pos: Vec3::new(0., 0., 100.),
                    });
                }

                // UpgradingGear tip: has UpgradeTome or OrbOfTransformation
                let has_upgrade_item = inventory.items.items.iter().any(|slot| {
                    if let Some(item) = slot {
                        let obj = item.get_obj();
                        *obj == WorldObject::UpgradeTome || *obj == WorldObject::OrbOfTransformation
                    } else {
                        false
                    }
                });
                if has_upgrade_item && !seen_tips.has_seen(&Tip::UpgradingGear) {
                    tip_event.send(TipEvent {
                        tip: Tip::UpgradingGear,
                        pos: Vec3::new(0., 0., 100.),
                    });
                }

                // InventoryStats tip: has gear in inventory slots (6+) that's not in hotbar
                let has_inventory_gear =
                    inventory.items.items.iter().enumerate().any(|(idx, slot)| {
                        if idx >= 6 && slot.is_some() {
                            if let Some(item) = slot {
                                let obj = item.get_obj();
                                // Check if it's equipment (weapon, armor, accessory)
                                if let Some(equip_type) = obj.get_equip_type(&proto) {
                                    equip_type.is_weapon()
                                        || equip_type.is_armor()
                                        || equip_type.is_accessory()
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    });
                if has_inventory_gear && !seen_tips.has_seen(&Tip::InventoryStats) {
                    tip_event.send(TipEvent {
                        tip: Tip::InventoryStats,
                        pos: Vec3::new(0., 0., 100.),
                    });
                }
            }
        }
    }
    if *DEBUG {
        if key_input.just_pressed(KeyCode::P) {
            dim_event.send(DimensionSpawnEvent {
                swap_to_dim_now: true,
                new_era: Some(Era::DungeonMain),
            });
        }
        if key_input.just_pressed(KeyCode::O) {
            dim_event.send(DimensionSpawnEvent {
                swap_to_dim_now: true,
                new_era: Some(Era::Third),
            });
        }
        if key_input.just_pressed(KeyCode::C) {
            let skills = game.get_player_skills().clone();
            let did_level = game
                .get_player_level_mut()
                .add_xp(200, &skills, &mut chaos_tracker);

            flash_event.send(FlashExpBarEvent {
                amount: 200,
                did_level,
            });
        }
        if key_input.just_pressed(KeyCode::K) {
            dim_event.send(DimensionSpawnEvent {
                swap_to_dim_now: true,
                new_era: Some(Era::Main),
            });
        }

        if key_input.just_pressed(KeyCode::L) {
            let pos = cursor.world_coords.truncate();
            if !can_spawn_mob_here(pos, &game, &proto, false) {
                return;
            }
            // proto_commands.spawn_item_from_proto(WorldObject::Crate, &proto, pos, 1, None);
            // proto_commands.spawn_item_from_proto(WorldObject::TimeFragment, &proto, pos, 1, None);
            // proto_commands.spawn_item_from_proto(WorldObject::Dagger, &proto, pos, 1, Some(5));
            // proto_commands.spawn_item_from_proto(WorldObject::WoodBow, &proto, pos, 1, Some(5));
            // proto_commands.spawn_item_from_proto(WorldObject::Claw, &proto, pos, 1, Some(5));
            // proto_commands.spawn_item_from_proto(WorldObject::IceStaff, &proto, pos, 1, Some(5));
            // proto_commands.spawn_item_from_proto(WorldObject::BasicStaff, &proto, pos, 1, Some(5));
            // proto_commands.spawn_from_proto(Mob::SpikeSlime, &proto.prototypes, pos);
            // proto_commands.spawn_from_proto(Mob::StingFly, &proto.prototypes, pos);
            // proto_commands.spawn_from_proto(Mob::FurDevil, &proto.prototypes, pos);
            // proto_commands.spawn_from_proto(Mob::Bushling, &proto.prototypes, pos);
            proto_commands.spawn_from_proto(Mob::FurDevil, &proto.prototypes, pos);
            // proto_commands.spawn_from_proto(Mob::Fairy, &proto.prototypes, pos);
            // proto_commands.spawn_from_proto(Mob::FurDevil, &proto.prototypes, pos);
            // commands.entity(t.unwrap()).insert(MobLevel(10));
            // proto_commands.spawn_from_proto(Mob::RedMushking, &proto.prototypes, pos);
            // let f = proto_commands.spawn_from_proto(Mob::SpikeSlime, &proto.prototypes, pos);
            // commands.entity(f.unwrap()).insert(MobLevel(10));
            // proto_commands.spawn_from_proto(Mob::Slime, &proto.prototypes, pos);
        }
    }
}
fn handle_hotbar_key_input(
    mut game: GameParam,
    key_input: ResMut<Input<KeyCode>>,
    mut mouse_wheel_event: EventReader<MouseWheel>,
    mut inv_state: ResMut<InventoryState>,
    mut events: EventWriter<UpdatePetWeaponEvent>,
) {
    for e in mouse_wheel_event.iter() {
        if e.y > 0. {
            change_hotbar_slot(
                (inv_state.active_hotbar_slot + 5) % 6,
                &mut inv_state,
                &mut game.inv_slot_query,
            );
            events.send(UpdatePetWeaponEvent);
        } else if e.y < 0. {
            change_hotbar_slot(
                (inv_state.active_hotbar_slot + 1) % 6,
                &mut inv_state,
                &mut game.inv_slot_query,
            );
            events.send(UpdatePetWeaponEvent);
        }
    }
    for (slot, key) in HOTBAR_KEYCODES.iter().enumerate() {
        if key_input.just_pressed(*key) {
            change_hotbar_slot(slot, &mut inv_state, &mut game.inv_slot_query);
            events.send(UpdatePetWeaponEvent);
        }
    }
}

pub fn handle_quick_hotbar_consume(
    mut key_input: ResMut<Input<KeyCode>>,
    mut game: GameParam,
    proto_param: ProtoParam,
    mut commands: Commands,
    inv: Query<&mut Inventory>,
    mut item_action_param: ItemActionParam,
) {
    let shift_key_pressed =
        key_input.pressed(KeyCode::LShift) || key_input.pressed(KeyCode::RShift);
    let hot_bar_key_pressed = HOTBAR_KEYCODES.iter().any(|k| key_input.just_pressed(*k));
    if shift_key_pressed && hot_bar_key_pressed {
        let hotbar_slot = HOTBAR_KEYCODES
            .iter()
            .position(|k| key_input.just_pressed(*k))
            .unwrap();
        key_input.clear_just_pressed(HOTBAR_KEYCODES[hotbar_slot]);
        let held_item_option = inv.single().items.items[hotbar_slot].clone();
        if let Some(held_item) = held_item_option {
            let held_obj = *held_item.get_obj();
            if let Some(item_actions) = proto_param.get_component::<ItemActions, _>(held_obj) {
                item_actions.run_action(
                    held_obj,
                    held_item.slot,
                    &mut item_action_param,
                    &mut game,
                    &proto_param,
                    &mut commands,
                );
            }
        }
    }
}
// Converts the cursor position into a world position, taking into account any transforms applied
// the camera.
pub fn cursor_pos_in_world(
    windows: &Query<&Window, With<PrimaryWindow>>,
    cursor_pos: Vec2,
    cam_t: &Transform,
    cam: &Camera,
) -> Vec3 {
    let window = windows.single();

    let window_size = Vec2::new(window.width(), window.height());

    // Convert screen position [0..resolution] to ndc [-1..1]
    // (ndc = normalized device coordinates)
    let ndc_to_world = cam_t.compute_matrix() * cam.projection_matrix().inverse();
    let ndc = (cursor_pos / window_size) * 2.0 - Vec2::ONE;
    ndc_to_world.project_point3(ndc.extend(0.0))
}
pub fn cursor_pos_in_ui(
    windows: &Query<&Window, With<PrimaryWindow>>,
    cursor_pos: Vec2,
    cam: &Camera,
) -> Vec3 {
    let window = windows.single();

    let window_size = Vec2::new(window.width(), window.height());

    // Convert screen position [0..resolution] to ndc [-1..1]
    // (ndc = normalized device coordinates)
    let t = Transform::from_translation(Vec3::new(0., 0., 0.));
    let ndc_to_world = t.compute_matrix() * cam.projection_matrix().inverse();
    let ndc = (cursor_pos / window_size) * 2.0 - Vec2::ONE;
    ndc_to_world.project_point3(ndc.extend(0.0))
}
pub fn diagnostics(
    mouse_button_input: Res<Input<MouseButton>>,
    entities: Query<Entity>,
    mobs: Query<&Mob>,
    spawners: Res<GlobalSpawners>,
) {
    if mouse_button_input.just_pressed(MouseButton::Right) {
        debug!("Entity Count: {:?}", entities.iter().count());
        debug!("Mob Count: {:?}", mobs.iter().count());
        debug!("Spawner Count: {:?}", spawners.spawners.iter().count());
    }
}
pub fn mouse_click_system(
    mut commands: Commands,
    mouse_button_input: Res<Input<MouseButton>>,
    cursor_pos: Res<CursorPos>,
    mut game: GameParam,
    mut proto_param: ProtoParam,
    mut attack_event: EventWriter<AttackEvent>,
    mut hit_event: EventWriter<HitEvent>,

    mut player_query: Query<
        (
            Entity,
            Option<&AttackTimer>,
            &PlayerAnimation,
            &OwnedBlessings,
            &mut CurrentMana,
        ),
        With<Player>,
    >,
    mut inv: Query<&mut Inventory>,
    inv_state: Res<InventoryState>,
    ui_state: Res<State<UIState>>,
    ranged_query: Query<(&WorldObject, &RangedAttack), With<Equipment>>,
    mut ranged_attack_event: EventWriter<RangedAttackEvent>,
    mut item_action_param: ItemActionParam,
    obj_actions: Query<&ObjectAction>,
    ammo_query_any: Query<&Ammo>,
) {
    if ui_state.0 != UIState::Closed {
        return;
    }

    let cursor_tile_pos = world_pos_to_tile_pos(cursor_pos.world_coords.truncate());
    let player_pos = game.player().position;
    let (player_e, attack_timer_option, player_anim, blessings, mut current_mana) =
        player_query.single_mut();

    if mouse_button_input.just_pressed(MouseButton::Left) {
        let hotbar_slot = inv_state.active_hotbar_slot;
        let held_item_option = inv.single().items.items[hotbar_slot].clone();
        if let Some(held_item) = held_item_option {
            let held_obj = *held_item.get_obj();
            if let Some(item_actions) = proto_param.get_component::<ItemActions, _>(held_obj) {
                item_actions.run_action(
                    held_obj,
                    held_item.slot,
                    &mut item_action_param,
                    &mut game,
                    &proto_param,
                    &mut commands,
                );
                return;
            }
        }
        if let Some((obj_e, obj)) = game.get_obj_entity_at_tile(cursor_tile_pos, &proto_param) {
            if let Ok(obj_action) = obj_actions.get(obj_e) {
                obj_action.run_action(
                    obj_e,
                    cursor_tile_pos,
                    obj,
                    &mut game,
                    &mut item_action_param,
                    &mut commands,
                    &mut proto_param,
                    &mut inv.single_mut(),
                );
                return;
            }
        }
    }

    // Hit Item, send attack event
    if mouse_button_input.pressed(MouseButton::Left) {
        if *DEBUG && mouse_button_input.just_pressed(MouseButton::Left) {
            let obj = game.get_object_from_chunk_cache(cursor_tile_pos);
            let ai_pos = world_pos_to_AIPos(cursor_pos.world_coords.truncate());
            let is_valid = game
                .get_pos_validity_for_pathfinding(ai_pos)
                .unwrap_or(true);
            info!(
                "C: {cursor_tile_pos:?} -> {obj:?} {is_valid:?} {:?} || {ai_pos:?}",
                cursor_pos.ui_coords,
            );
        }
        if attack_timer_option.is_some() || player_anim.is_one_time_anim() {
            return;
        }

        // AttackManaCost blessing: check mana cost and deduct before attacking
        let attack_mana_cost = blessings.get_attack_mana_cost();
        if attack_mana_cost > 0 {
            if current_mana.0 < attack_mana_cost {
                return;
            }
            current_mana.0 -= attack_mana_cost;
        }

        let mut main_hand_option = None;
        // if it has AttackTimer, the action is on cooldown, so we abort.
        if let Some(tool) = &game.player().main_hand_slot {
            main_hand_option = Some(tool.get_obj());
        }
        let direction =
            (cursor_pos.world_coords.truncate() - player_pos.truncate()).normalize_or_zero();
        if let Ok((obj, ranged_tool)) = ranged_query.get_single() {
            // Gate ranged attacks on ammo availability for non-magic ranged weapons
            if obj.is_ranged_weapon() && !obj.is_magic_weapon() {
                if let Some(main_hand) = game.player().main_hand_slot.clone() {
                    if let Ok(ammo) = ammo_query_any.get(main_hand.entity) {
                        if !ammo.can_fire() {
                            // Out of ammo: do not send any attack or animation for ranged
                            return;
                        }
                    } else {
                        // No ammo component yet: treat as empty and block attack until projectile handler initializes
                        return;
                    }
                }
            }
            let mana_cost_option =
                proto_param.get_component::<ManaCost, _>(main_hand_option.unwrap());
            let mut rng = rand::thread_rng();
            let trigger_count = if game.has_skill(Heirloom::ChanceToProcExtraAttack)
                && rng.gen_bool(
                    (game.skill_count(Heirloom::ChanceToProcExtraAttack) as f64 * 0.25)
                        .clamp(0., 1.),
                ) {
                let mana_cost = Heirloom::ChanceToProcExtraAttack.get_mana_cost();
                if current_mana.0 >= mana_cost {
                    current_mana.0 -= mana_cost;
                    2
                } else {
                    1
                }
            } else {
                1
            };
            for i in 0..trigger_count {
                ranged_attack_event.send(RangedAttackEvent {
                    projectile: ranged_tool.0.clone(),
                    direction,
                    from_enemy: false,
                    is_followup_proj: false,
                    from_entity: None,
                    mana_cost: mana_cost_option.map(|m| -m.0),
                    dmg_override: None,
                    pos_override: if ranged_tool.0.is_anchored_to_player_pos() {
                        Some(Vec2::ZERO)
                    } else {
                        None
                    },
                    spawn_delay: if obj == &WorldObject::WoodBow {
                        if i == 0 {
                            0.36
                        } else {
                            0.2
                        }
                    } else {
                        if i == 0 {
                            0.01
                        } else {
                            0.2
                        }
                    },
                })
            }
        }
        let mut did_attack = false;
        if let Some(main_hand) = main_hand_option {
            if main_hand == WorldObject::WoodBow {
                commands
                    .entity(player_e)
                    .insert(PlayerAnimation::Bow)
                    .insert(crate::animations::player_sprite::AttackAnimationTimer(
                        Timer::from_seconds(1.0, TimerMode::Once),
                    ));
                attack_event.send(AttackEvent {
                    direction,
                    ignore_cooldown: false,
                });
            } else if main_hand.is_melee_weapon()
                || main_hand.is_ranged_weapon()
                || main_hand.is_tool()
            {
                if !player_anim.is_sprinting() {
                    did_attack = true;
                }
            }
        }
        if did_attack {
            commands
                .entity(player_e)
                .insert(PlayerAnimation::Attack)
                .insert(crate::animations::player_sprite::AttackAnimationTimer(
                    Timer::from_seconds(1.0, TimerMode::Once),
                ));
        }
        attack_event.send(AttackEvent {
            direction,
            ignore_cooldown: false,
        });
        if player_pos
            .truncate()
            .distance(cursor_pos.world_coords.truncate())
            > game.player().reach_distance * 32.
        {
            return;
        }
        if let Some((hit_obj, _)) = game.get_obj_entity_at_tile(cursor_tile_pos, &proto_param) {
            if *DEBUG {
                debug!("OBJ: {hit_obj:?}");
            }
            let (damage, was_crit, was_overcrit) =
                game.calculate_player_damage(&mut commands, hit_obj, 0, None, 0, None, 0);
            hit_event.send(HitEvent {
                hit_by_pet: None,
                hit_entity: hit_obj,
                damage: damage as i32,
                dir: Vec2::new(0., 0.),
                hit_with_melee: main_hand_option,
                hit_with_projectile: None,
                ignore_tool: false,
                hit_by_mob: None,
                from_heirloom_effect: false,
                was_crit,
                was_overcrit,
            });
        }
    }
}

pub fn handle_interact_objects(
    objs: Query<
        (
            Entity,
            &GlobalTransform,
            &ObjectAction,
            &WorldObject,
            &SpriteAnchor,
        ),
        With<InteractionGuideTrigger>,
    >,
    mut player_query: Query<(&GlobalTransform, &mut Inventory), With<Player>>,
    mut game: GameParam,
    mut proto_param: ProtoParam,
    mut item_action_param: ItemActionParam,
    mut commands: Commands,
    key_input: ResMut<Input<KeyCode>>,
) {
    if key_input.just_pressed(KeyCode::F) {
        for (obj_e, t, obj_action, obj, anchor) in objs.iter() {
            let obj_t = t.translation().truncate() - anchor.0;
            let (player_t, mut inv) = player_query.single_mut();
            if obj_t.distance(player_t.translation().truncate()) <= 32. {
                obj_action.run_action(
                    obj_e,
                    world_pos_to_tile_pos(obj_t),
                    *obj,
                    &mut game,
                    &mut item_action_param,
                    &mut commands,
                    &mut proto_param,
                    &mut inv,
                );
            }
        }
    }
}

pub fn handle_open_essence_ui(
    mut commands: Commands,
    key_input: ResMut<Input<KeyCode>>,
    player_query: Query<&GlobalTransform, With<Player>>,
    nearby_merchant_query: Query<(&GlobalTransform, &EssenceShopChoices)>,
    mut next_inv_state: ResMut<NextState<UIState>>,
) {
    if key_input.just_pressed(KeyCode::F) {
        let player_t = player_query.single().translation().truncate();
        for (transform, choices) in nearby_merchant_query.iter() {
            if player_t.distance(transform.translation().truncate()) < 32. {
                commands.insert_resource(choices.clone());
                next_inv_state.set(UIState::Essence);
            }
        }
    }
}

pub fn move_camera_with_player(
    player_query: Query<
        (&Transform, &RawPosition, &MovementVector),
        (
            With<Player>,
            // Changed<Transform>,
            Without<MainCamera>,
            Without<TextureCamera>,
            Without<UICamera>,
        ),
    >,
    mut game_camera: Query<
        (&mut Transform, &mut RawPosition),
        (Without<MainCamera>, Without<UICamera>, With<TextureCamera>),
    >,
    mut screen_camera: Query<
        (&mut Transform, &GameUpscale),
        (With<MainCamera>, Without<UICamera>, Without<TextureCamera>),
    >,
    time: Res<Time>,
) {
    let (mut game_camera_transform, mut raw_camera_pos) = game_camera.single_mut();
    let Ok((_player_pos, raw_player_pos, _player_movement_vec)) = player_query.get_single() else {
        return;
    };

    let camera_lookahead_scale = 4.0;
    let delta = raw_player_pos.0 - raw_camera_pos.0;
    raw_camera_pos.0 += delta * camera_lookahead_scale * time.delta_seconds();

    let decimals = 10i32.pow(3) as f32;

    let camera_final_pos = Vec2::new(raw_camera_pos.x, raw_camera_pos.y);
    let camera_final_pos = Vec2::new(
        (camera_final_pos.x * decimals).round() / decimals,
        (camera_final_pos.y * decimals).round() / decimals,
    );

    game_camera_transform.translation.x = camera_final_pos.x.trunc();
    game_camera_transform.translation.y = camera_final_pos.y.trunc();
    let (mut screen_camera_transform, game_upscale) = screen_camera.single_mut();
    screen_camera_transform.translation.x = camera_final_pos.x.fract() * game_upscale.0;
    screen_camera_transform.translation.y = camera_final_pos.y.fract() * game_upscale.0;
}
