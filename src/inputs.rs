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
use crate::enemy::spawner::ChunkSpawners;
use crate::juice::{DustParticles, RunDustTimer};
use crate::player::skills::{ActiveSkillUsedEvent, PlayerSkills, Skill};
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

use crate::attributes::Speed;
use crate::combat::{AttackTimer, HitEvent};

use crate::enemy::Mob;
use crate::inventory::Inventory;
use crate::item::item_actions::{ItemActionParam, ItemActions, ManaCost};
use crate::item::object_actions::ObjectAction;
use crate::item::projectile::{Projectile, RangedAttack, RangedAttackEvent};
use crate::item::{Equipment, WorldObject};
use crate::proto::proto_param::ProtoParam;
use crate::ui::minimap::UpdateMiniMapEvent;
use crate::ui::{
    change_hotbar_slot, EssenceShopChoices, FlashExpBarEvent, InventoryState, UIState,
};
use crate::world::chunk::Chunk;

use crate::world::world_helpers::world_pos_to_tile_pos;

use crate::{
    custom_commands::CommandsExt, AppExt, CustomFlush, GameParam, GameState, MainCamera,
    RawPosition, TextureCamera, UICamera, PLAYER_MOVE_SPEED,
};
use crate::{
    get_active_skill_keybind, Game, GameUpscale, Player, DEBUG, PLAYER_DASH_SPEED, TIME_STEP,
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
            // .add_plugin(ResourceInspectorPlugin::<CursorPos>::default())
            .with_default_schedule(CoreSchedule::FixedUpdate, |app| {
                app.add_event::<AttackEvent>();
            })
            .add_systems(
                (
                    player_move_inputs.run_if(is_not_paused),
                    turn_player.run_if(is_not_paused),
                    mouse_click_system.run_if(is_not_paused).after(CustomFlush),
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
            .add_system(update_cursor_pos.after(player_move_inputs))
            .add_system(
                move_camera_with_player
                    .after(PhysicsSet::SyncBackendFlush)
                    .before(TransformSystem::TransformPropagate)
                    .in_base_set(CoreSet::PostUpdate)
                    .run_if(in_state(GameState::Main)),
            );
    }
}
#[derive(Default, Reflect, Resource, Debug)]
#[reflect(Resource)]
pub struct CursorPos {
    pub world_coords: Vec3,
    pub screen_coords: Vec3,
    pub ui_coords: Vec3,
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
        ),
        (
            With<Player>,
            Without<MainCamera>,
            Without<Chunk>,
            Without<Equipment>,
        ),
    >,
    time: Res<Time>,
    key_input: ResMut<Input<KeyCode>>,
    mut minimap_event: EventWriter<UpdateMiniMapEvent>,
    mut commands: Commands,
    mut particle: Query<&mut EffectSpawner, With<DustParticles>>,
    asset_server: Res<AssetServer>,
    audio: Res<Audio>,
    mut audio_timer: Local<Timer>,
    mut active_skill_event: EventWriter<ActiveSkillUsedEvent>,
) {
    if audio_timer.duration() == Duration::ZERO {
        *audio_timer = Timer::from_seconds(0.2, TimerMode::Once);
    }
    let (player_e, mut player_kcc, mut mv, curr_anim, speed, hunger, mut run_dust_timer, skills) =
        player_query.single_mut();
    let player = game.player_mut();
    if curr_anim.is_movement_restricting() {
        mv.0 = Vec2::ZERO;
        return;
    }
    let mut d = Vec2::ZERO;
    let s = PLAYER_MOVE_SPEED
        * time.delta_seconds()
        * (1. + speed.0 as f32 / 100.)
        * (if hunger.is_starving() { 0.7 } else { 1. });

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
    if let Some(roll_slot) = skills.has_active_skill(Skill::Roll) {
        if player.player_dash_cooldown.finished()
            && key_input.pressed(get_active_skill_keybind(roll_slot))
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

        minimap_event.send(UpdateMiniMapEvent {
            pos: None,
            new_tile: None,
        });
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
) {
    if key_input.just_pressed(KeyCode::Escape) {
        next_inv_state.set(UIState::Closed);
    }
}
pub fn toggle_inventory(
    mut game: GameParam,
    key_input: ResMut<Input<KeyCode>>,
    mut proto_commands: ProtoCommands,
    mut dim_event: EventWriter<DimensionSpawnEvent>,
    proto: ProtoParam,
    _inv: Query<&mut Inventory>,
    mut next_ui_state: ResMut<NextState<UIState>>,
    cursor: Res<CursorPos>,
    mut flash_event: EventWriter<FlashExpBarEvent>,
) {
    if key_input.just_pressed(KeyCode::I)
        || key_input.just_pressed(KeyCode::Tab)
        || key_input.just_pressed(KeyCode::E)
    {
        next_ui_state.set(UIState::Inventory);
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
                new_era: Some(Era::Second),
            });
        }
        if key_input.just_pressed(KeyCode::C) {
            let did_level = game.get_player_level_mut().add_xp(100);

            flash_event.send(FlashExpBarEvent {
                amount: 100,
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
            // proto_commands.spawn_item_from_proto(WorldObject::Sword, &proto, pos, 1, Some(5));
            // proto_commands.spawn_item_from_proto(WorldObject::Dagger, &proto, pos, 1, Some(5));
            // proto_commands.spawn_item_from_proto(WorldObject::WoodBow, &proto, pos, 1, Some(5));
            // proto_commands.spawn_item_from_proto(WorldObject::Claw, &proto, pos, 1, Some(5));
            // proto_commands.spawn_item_from_proto(WorldObject::IceStaff, &proto, pos, 1, Some(5));
            // proto_commands.spawn_item_from_proto(WorldObject::BasicStaff, &proto, pos, 1, Some(5));
            // proto_commands.spawn_from_proto(Mob::SpikeSlime, &proto.prototypes, pos);
            // proto_commands.spawn_from_proto(Mob::StingFly, &proto.prototypes, pos);
            // proto_commands.spawn_from_proto(Mob::Bushling, &proto.prototypes, pos);
            // proto_commands.spawn_from_proto(Mob::Fairy, &proto.prototypes, pos);
            proto_commands.spawn_from_proto(Mob::StingFly, &proto.prototypes, pos);
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
) {
    for e in mouse_wheel_event.iter() {
        if e.y > 0. {
            change_hotbar_slot(
                (inv_state.active_hotbar_slot + 5) % 6,
                &mut inv_state,
                &mut game.inv_slot_query,
            );
        } else if e.y < 0. {
            change_hotbar_slot(
                (inv_state.active_hotbar_slot + 1) % 6,
                &mut inv_state,
                &mut game.inv_slot_query,
            );
        }
    }
    for (slot, key) in HOTBAR_KEYCODES.iter().enumerate() {
        if key_input.just_pressed(*key) {
            change_hotbar_slot(slot, &mut inv_state, &mut game.inv_slot_query);
        }
    }
}
pub fn update_cursor_pos(
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Transform, &Camera), With<TextureCamera>>,
    mut cursor_moved_events: EventReader<CursorMoved>,
    mut cursor_pos: ResMut<CursorPos>,
) {
    for cursor_moved in cursor_moved_events.iter() {
        // To get the mouse's world position, we have to transform its window position by
        // any transforms on the camera. This is done by projecting the cursor position into
        // camera space (world space).
        for (cam_t, cam) in camera_q.iter() {
            *cursor_pos = CursorPos {
                world_coords: cursor_pos_in_world(&windows, cursor_moved.position, cam_t, cam),
                ui_coords: cursor_pos_in_ui(&windows, cursor_moved.position, cam),
                screen_coords: cursor_moved.position.extend(0.),
            };
        }
    }
}
pub fn handle_quick_hotbar_consume(
    mut key_input: ResMut<Input<KeyCode>>,
    mut game: GameParam,
    proto_param: ProtoParam,

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
    spawners: Query<&ChunkSpawners>,
) {
    if mouse_button_input.just_pressed(MouseButton::Right) {
        debug!("Entity Count: {:?}", entities.iter().count());
        debug!("Mob Count: {:?}", mobs.iter().count());
        debug!("Spawner Count: {:?}", spawners.iter().count());
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

    player_query: Query<(Entity, Option<&AttackTimer>, &PlayerAnimation), With<Player>>,
    mut inv: Query<&mut Inventory>,
    inv_state: Res<InventoryState>,
    ui_state: Res<State<UIState>>,
    ranged_query: Query<(&WorldObject, &RangedAttack), With<Equipment>>,
    mut ranged_attack_event: EventWriter<RangedAttackEvent>,
    mut item_action_param: ItemActionParam,
    obj_actions: Query<&ObjectAction>,
    // mut meshes: ResMut<Assets<Mesh>>,
    // mut materials: ResMut<Assets<ColorMaterial>>,
) {
    if ui_state.0 != UIState::Closed {
        return;
    }

    let cursor_tile_pos = world_pos_to_tile_pos(cursor_pos.world_coords.truncate());
    let player_pos = game.player().position;
    let (player_e, attack_timer_option, player_anim) = player_query.single();
    // Hit Item, send attack event
    if mouse_button_input.pressed(MouseButton::Left) {
        if *DEBUG && mouse_button_input.just_pressed(MouseButton::Left) {
            let obj = game.get_object_from_chunk_cache(cursor_tile_pos);
            let ai_pos = world_pos_to_AIPos(cursor_pos.world_coords.truncate());
            let is_valid = game
                .get_pos_validity_for_pathfinding(ai_pos)
                .unwrap_or(true);
            debug!(
                "C: {cursor_tile_pos:?} -> {obj:?} {is_valid:?} {:?} || {ai_pos:?}",
                cursor_pos.ui_coords,
            );
        }
        if attack_timer_option.is_some() || player_anim.is_one_time_anim() {
            return;
        }
        let mut main_hand_option = None;
        // if it has AttackTimer, the action is on cooldown, so we abort.
        if let Some(tool) = &game.player().main_hand_slot {
            main_hand_option = Some(tool.get_obj());
        }
        let direction =
            (cursor_pos.world_coords.truncate() - player_pos.truncate()).normalize_or_zero();
        if let Ok((obj, ranged_tool)) = ranged_query.get_single() {
            let mana_cost_option =
                proto_param.get_component::<ManaCost, _>(main_hand_option.unwrap());
            ranged_attack_event.send(RangedAttackEvent {
                projectile: ranged_tool.0.clone(),
                direction,
                from_enemy: None,
                is_followup_proj: false,
                mana_cost: mana_cost_option.map(|m| -m.0),
                dmg_override: None,
                pos_override: if ranged_tool.0 == Projectile::Electricity {
                    Some(Vec2::ZERO)
                } else {
                    None
                },
                spawn_delay: if obj == &WorldObject::WoodBow {
                    0.36
                } else {
                    0.01
                },
            })
        }
        let mut did_attack = false;
        if let Some(main_hand) = main_hand_option {
            if main_hand == WorldObject::WoodBow {
                commands.entity(player_e).insert(PlayerAnimation::Bow);
                attack_event.send(AttackEvent {
                    direction,
                    ignore_cooldown: false,
                });
            } else if main_hand.is_melee_weapon() || main_hand.is_tool() {
                if !player_anim.is_sprinting() {
                    did_attack = true;
                }
            }
        }
        if did_attack {
            commands.entity(player_e).insert(PlayerAnimation::Attack);
        }
        attack_event.send(AttackEvent {
            direction,
            ignore_cooldown: false,
        });
        if player_pos
            .truncate()
            .distance(cursor_pos.world_coords.truncate())
            > game.player().reach_distance * 32.
            || ranged_query.get_single().is_ok()
        {
            return;
        }
        if let Some((hit_obj, _)) = game.get_obj_entity_at_tile(cursor_tile_pos, &proto_param) {
            if *DEBUG {
                debug!("OBJ: {hit_obj:?}");
            }
            hit_event.send(HitEvent {
                hit_entity: hit_obj,
                damage: game
                    .calculate_player_damage(&mut commands, hit_obj, 0, None, 0, None)
                    .0 as i32,
                dir: Vec2::new(0., 0.),
                hit_with_melee: main_hand_option,
                hit_with_projectile: None,
                ignore_tool: false,
                hit_by_mob: None,
                was_crit: false,
            });
        }
    }
    // Attempt to place block in hand
    if mouse_button_input.just_pressed(MouseButton::Right) {
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
                );
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
            }
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
