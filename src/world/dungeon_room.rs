use bevy::prelude::*;
use bevy::render::view::RenderLayers;
use bevy::sprite::Anchor;
use bevy_proto::prelude::{ProtoCommands, Prototypes};
use bevy_rapier2d::prelude::Collider;
use rand::Rng;

use crate::{
    assets::Graphics,
    colors::RED,
    custom_commands::CommandsExt,
    enemy::red_mushling::{MushlingWakeupState, SproutingState, WaitingToSproutState},
    enemy::{CombatAlignment, Mob},
    inputs::FacingDirection,
    inventory::ItemStack,
    item::{
        dungeon_shrine::{DungeonShrineMob, DungeonShrineMobDeathEvent},
        Loot, LootTable, WorldObject,
    },
    player::Player,
    proto::proto_param::ProtoParam,
    ui::{
        damage_numbers::spawn_text, game_fonts::FLOATING_TEXT,
        global_text_message::GlobalTextMessageEvent, key_input_guide::InteractionGuideTrigger,
        spawn_item_stack_icon,
    },
    world::{dimension::ActiveDimension, dungeon::Dungeon, world_helpers::world_pos_to_tile_pos},
    GameParam, GameState,
};

/// The full room asset is 464x368, drawn centered on the world origin.
const ROOM_ASSET_SIZE: Vec2 = Vec2::new(464., 368.);
/// Where the player is teleported when entering the dungeon (bottom-center of the room).
const PLAYER_SPAWN: Vec2 = Vec2::new(0., -120.);
/// Central shrine position.
const SHRINE_POS: Vec2 = Vec2::new(0., -16.);
/// Exit door position (top-center of the room, drawn into the asset). Kept low
/// enough that the player can walk up into the doorway gap to trigger the prompt.
const DOOR_POS: Vec2 = Vec2::new(0., 116.);

/// Manual colliders for the room walls/objects, defined as (center, half_extents)
/// in world space relative to the room origin. The room is an octagon-ish shape:
/// flat top wall (with a door gap), stair-stepped diagonal corners that narrow
/// toward the top, straight side walls, stepped bottom corners and a flat bottom
/// wall. Many small boxes approximate the stair-stepped art so the walls feel
/// tight. Tune against the asset art if needed.
// Boxes are authored so their INNER face lands exactly on the floor/wall edge
// (measured from the art); they freely extend outward into the wall/black region.
const DUNGEON_COLLIDERS: &[(Vec2, Vec2)] = &[
    // --- Top flat wall (door filled in, treated as solid wall) ---
    (Vec2::new(0., 150.), Vec2::new(140., 48.)),
    // --- Top-left diagonal staircase (top wall end -> left side wall) ---
    (Vec2::new(-152., 144.), Vec2::new(24., 55.)),
    (Vec2::new(-174., 130.), Vec2::new(24., 70.)),
    (Vec2::new(-192., 122.), Vec2::new(24., 80.)),
    (Vec2::new(-204., 102.), Vec2::new(12., 99.)),
    (Vec2::new(-220., 96.), Vec2::new(12., 99.)),
    // --- Top-right diagonal staircase ---
    (Vec2::new(152., 144.), Vec2::new(24., 55.)),
    (Vec2::new(174., 130.), Vec2::new(24., 70.)),
    (Vec2::new(192., 122.), Vec2::new(24., 80.)),
    (Vec2::new(204., 102.), Vec2::new(8., 99.)),
    (Vec2::new(220., 96.), Vec2::new(12., 99.)),
    // --- Straight side walls ---
    (Vec2::new(-234., -75.), Vec2::new(10., 75.)),
    (Vec2::new(234., -75.), Vec2::new(10., 75.)),
    // --- Bottom corners (clipped diagonal cuts) ---
    (Vec2::new(-226., -170.), Vec2::new(21., 10.)),
    (Vec2::new(-236., -164.), Vec2::new(21., 10.)),
    (Vec2::new(226., 170.), Vec2::new(21., 10.)),
    (Vec2::new(236., 164.), Vec2::new(21., 10.)),
    // --- Bottom flat wall ---a
    (Vec2::new(0., -182.), Vec2::new(300., 12.)),
];

/// Marks every entity that belongs to the current dungeon room instance so the
/// dimension swap cleanup can despawn them.
#[derive(Component)]
pub struct DungeonRoomEntity;

/// Marks the central wave shrine entity.
#[derive(Component)]
pub struct DungeonWaveShrine;

/// Root marker for the floating reward preview shown above the exit door.
#[derive(Component)]
pub struct ExitRewardPreview;

/// How close the player must be to the exit for the reward preview to appear.
const REWARD_PREVIEW_DISTANCE: f32 = 48.;

/// Sent when the player interacts with the wave shrine to begin the next wave.
pub struct StartNextDungeonWaveEvent(pub Entity);

#[derive(Default, PartialEq, Eq, Clone, Copy, Debug)]
pub enum WavePhase {
    /// No wave running; the next interaction starts a wave.
    #[default]
    Idle,
    /// A wave's mini-waves are spawning / being fought.
    InProgress,
    /// Wave finished; the next interaction starts the following wave.
    Cleared,
    /// All 3 waves finished; the shrine does nothing further.
    Done,
}

#[derive(Resource, Default)]
pub struct DungeonWaveState {
    pub shrine: Option<Entity>,
    pub phase: WavePhase,
    /// Last started wave (1..=3). 0 before the first wave begins.
    pub wave: u8,
    /// Index of the last spawned mini-wave within the current wave (0..=2).
    pub mini_wave: u8,
    /// Seconds elapsed since the current wave started.
    pub clock: f32,
    /// Number of mobs from the current wave still alive.
    pub mobs_alive: i32,
}

impl DungeonWaveState {
    fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Accumulated rewards earned across the dungeon's waves. Spawned for the player
/// when they return to the overworld, then cleared. Coins are tracked as a range
/// (rolled at drop time) so the exit preview can show "min-max".
#[derive(Resource, Default)]
pub struct DungeonRewards {
    pub items: Vec<(WorldObject, usize)>,
    pub coin_min: usize,
    pub coin_max: usize,
}

impl DungeonRewards {
    fn clear(&mut self) {
        self.items.clear();
        self.coin_min = 0;
        self.coin_max = 0;
    }

    fn is_empty(&self) -> bool {
        self.items.is_empty() && self.coin_max == 0
    }
}

/// Schedules the reward drop once the player has returned to the overworld.
#[derive(Resource, Default)]
pub struct DungeonRewardDrop {
    pub timer: Option<Timer>,
}

const COIN_MIN: usize = 10;
const COIN_MAX: usize = 25;

/// The three random-material pool for waves 2 and 3.
const MATERIAL_POOL: [WorldObject; 3] = [
    WorldObject::MagicGem,
    WorldObject::UpgradeTome,
    WorldObject::OrbOfTransformation,
];

/// Mob composition for each wave. `WAVES[wave_index][mini_wave_index]` is a list
/// of `(mob, count)` to spawn.
const WAVES: [[&[(Mob, u32)]; 3]; 3] = [
    // Wave 1
    [
        &[
            (Mob::FurDevil, 12),
            (Mob::Bushling, 4),
            (Mob::RedMushling, 6),
        ],
        &[(Mob::FurDevil, 12), (Mob::Bushling, 3), (Mob::StingFly, 5)],
        &[
            (Mob::Bushling, 16),
            (Mob::SpikeSlime, 6),
            (Mob::RedMushling, 3),
        ],
    ],
    // Wave 2
    [
        &[
            (Mob::FurDevil, 16),
            (Mob::Bushling, 6),
            (Mob::RedMushling, 12),
            (Mob::StoneGolem, 1),
        ],
        &[
            (Mob::FurDevil, 16),
            (Mob::StingFly, 6),
            (Mob::RedMushling, 10),
            (Mob::Bushling, 3),
        ],
        &[
            (Mob::FurDevil, 20),
            (Mob::RedMushling, 3),
            (Mob::SpikeSlime, 4),
            (Mob::Bushling, 4),
            (Mob::StingFly, 2),
        ],
    ],
    // Wave 3
    [
        &[
            (Mob::RedMushling, 24),
            (Mob::SpikeSlime, 12),
            (Mob::StoneGolem, 2),
            (Mob::FurDevil, 8),
        ],
        &[
            (Mob::RedMushling, 12),
            (Mob::FurDevil, 16),
            (Mob::StingFly, 8),
            (Mob::SpikeSlime, 3),
        ],
        &[
            (Mob::SpikeSlime, 12),
            (Mob::RedMushling, 16),
            (Mob::Bushling, 8),
        ],
    ],
];

pub struct DungeonRoomPlugin;
impl Plugin for DungeonRoomPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<StartNextDungeonWaveEvent>()
            .init_resource::<DungeonWaveState>()
            .init_resource::<DungeonRewards>()
            .init_resource::<DungeonRewardDrop>()
            .add_systems(
                (
                    spawn_dungeon_room,
                    handle_start_dungeon_wave,
                    tick_dungeon_waves,
                    handle_dungeon_wave_mob_deaths,
                    wake_dungeon_wave_mushlings,
                    update_exit_reward_preview,
                    drop_dungeon_rewards_on_return,
                )
                    .in_set(OnUpdate(GameState::Main)),
            );
    }
}

/// Spawns the room sprite, manual colliders, central wave shrine and exit door
/// when a new dungeon dimension becomes active.
fn spawn_dungeon_room(
    new_dungeon: Query<Entity, (Added<ActiveDimension>, With<Dungeon>)>,
    mut commands: Commands,
    mut proto_commands: ProtoCommands,
    prototypes: Prototypes,
    asset_server: Res<AssetServer>,
    mut move_player_event: EventWriter<crate::player::MovePlayerEvent>,
    mut wave_state: ResMut<DungeonWaveState>,
    mut rewards: ResMut<DungeonRewards>,
) {
    let Ok(_dim_e) = new_dungeon.get_single() else {
        return;
    };

    // Fresh instance: clear any stale wave state / rewards.
    wave_state.reset();
    rewards.clear();

    // Move the player to the room's spawn point.
    move_player_event.send(crate::player::MovePlayerEvent {
        pos: world_pos_to_tile_pos(PLAYER_SPAWN),
    });

    // Room art.
    commands.spawn((
        SpriteBundle {
            texture: asset_server.load("textures/dungeon.png"),
            sprite: Sprite {
                custom_size: Some(ROOM_ASSET_SIZE),
                ..default()
            },
            // Floor sits at z=0 like the normal chunk tilemap; world objects
            // y-sort to much higher z so they render on top. A negative z would
            // fall outside the 2D camera's near clip and render nothing.
            transform: Transform::from_translation(Vec3::new(0., 0., 0.)),
            ..default()
        },
        DungeonRoomEntity,
        Name::new("Dungeon Room"),
    ));

    // Manual colliders.
    for (center, half_extents) in DUNGEON_COLLIDERS.iter() {
        commands.spawn((
            TransformBundle::from_transform(Transform::from_translation(center.extend(0.))),
            Collider::cuboid(half_extents.x, half_extents.y),
            DungeonRoomEntity,
            Name::new("Dungeon Collider"),
        ));
    }

    // Central wave shrine (reuse the weapon shrine art/animation).
    if let Some(shrine_e) =
        proto_commands.spawn_from_proto(WorldObject::WeaponShrine, &prototypes, SHRINE_POS)
    {
        commands
            .entity(shrine_e)
            .insert(Transform::from_translation(SHRINE_POS.extend(0.)))
            .insert(DungeonRoomEntity)
            .insert(DungeonWaveShrine);
        wave_state.shrine = Some(shrine_e);
    }

    // Exit door. The art is part of the room asset, so hide the proto sprite and
    // keep only the interaction trigger + collider.
    if let Some(door_e) =
        proto_commands.spawn_from_proto(WorldObject::DungeonExit, &prototypes, DOOR_POS)
    {
        commands
            .entity(door_e)
            .insert(Transform::from_translation(DOOR_POS.extend(0.)))
            .insert(Visibility::Hidden)
            .insert(DungeonRoomEntity);
    }
}

/// Starts the next wave when the player interacts with the shrine.
fn handle_start_dungeon_wave(
    mut events: EventReader<StartNextDungeonWaveEvent>,
    mut wave_state: ResMut<DungeonWaveState>,
    mut proto_commands: ProtoCommands,
    proto: ProtoParam,
    mut commands: Commands,
    game: GameParam,
    mut global_text: EventWriter<GlobalTextMessageEvent>,
    mut guides: Query<&mut InteractionGuideTrigger>,
) {
    for event in events.iter() {
        // Only start a wave from Idle (before wave 1) or Cleared (between waves).
        if wave_state.phase == WavePhase::InProgress || wave_state.phase == WavePhase::Done {
            continue;
        }
        wave_state.shrine = Some(event.0);
        wave_state.wave += 1;
        wave_state.phase = WavePhase::InProgress;
        wave_state.mini_wave = 0;
        wave_state.clock = 0.;
        wave_state.mobs_alive = 0;

        let count = spawn_mini_wave(
            wave_state.wave,
            0,
            event.0,
            &mut proto_commands,
            &proto,
            &mut commands,
            &game,
        );
        wave_state.mobs_alive += count as i32;

        global_text.send(GlobalTextMessageEvent::new("Danger!!!", RED));

        if let Ok(mut guide) = guides.get_mut(event.0) {
            guide.text = Some("...".to_string());
        }
    }
}

/// Advances mini-waves on timers / low-enemy-count, and grants rewards on clear.
fn tick_dungeon_waves(
    time: Res<Time>,
    mut wave_state: ResMut<DungeonWaveState>,
    mut proto_commands: ProtoCommands,
    proto: ProtoParam,
    mut commands: Commands,
    game: GameParam,
    mut rewards: ResMut<DungeonRewards>,
    mut guides: Query<&mut InteractionGuideTrigger>,
) {
    if wave_state.phase != WavePhase::InProgress {
        return;
    }
    wave_state.clock += time.delta_seconds();

    let Some(shrine_e) = wave_state.shrine else {
        return;
    };

    if wave_state.mini_wave < 2 {
        // Mini-wave 2 spawns at 20s, mini-wave 3 at 40s (from wave start), or
        // early once the previous mini-wave is nearly cleared.
        let threshold = if wave_state.mini_wave == 0 { 20. } else { 40. };
        if wave_state.clock >= threshold || wave_state.mobs_alive <= 1 {
            wave_state.mini_wave += 1;
            let count = spawn_mini_wave(
                wave_state.wave,
                wave_state.mini_wave,
                shrine_e,
                &mut proto_commands,
                &proto,
                &mut commands,
                &game,
            );
            wave_state.mobs_alive += count as i32;
        }
    } else if wave_state.mobs_alive <= 0 {
        // All mini-waves spawned and cleared: grant rewards and advance state.
        grant_wave_rewards(wave_state.wave, &mut rewards);

        if wave_state.wave >= 3 {
            wave_state.phase = WavePhase::Done;
            if let Ok(mut guide) = guides.get_mut(shrine_e) {
                guide.text = Some("You have proven yourself...".to_string());
            }
        } else {
            wave_state.phase = WavePhase::Cleared;
            if let Ok(mut guide) = guides.get_mut(shrine_e) {
                guide.text = Some("Tempt the dungeon further?".to_string());
            }
        }
    }
}

/// Decrements the alive count when a wave mob dies.
fn handle_dungeon_wave_mob_deaths(
    mut deaths: EventReader<DungeonShrineMobDeathEvent>,
    mut wave_state: ResMut<DungeonWaveState>,
) {
    for death in deaths.iter() {
        if Some(death.0) == wave_state.shrine {
            wave_state.mobs_alive -= 1;
        }
    }
}

/// Wave mushlings spawn dormant (`WaitingToSproutState`) and only sprout when the
/// player walks within line-of-sight. Force them awake immediately so they sprout
/// and enter their chase (`FollowState`) right away. Mirrors the manual wake
/// transition in `handle_mushling_wakeup_timers`.
fn wake_dungeon_wave_mushlings(
    mut mushlings: Query<
        (Entity, &Mob, &mut MushlingWakeupState),
        (With<DungeonShrineMob>, With<WaitingToSproutState>),
    >,
    mut commands: Commands,
) {
    for (e, mob, mut wakeup) in mushlings.iter_mut() {
        if mob != &Mob::RedMushling {
            continue;
        }
        wakeup.is_awake = true;
        commands
            .entity(e)
            .remove::<WaitingToSproutState>()
            .insert(SproutingState);
    }
}

fn spawn_mini_wave(
    wave: u8,
    mini_wave: u8,
    shrine_e: Entity,
    proto_commands: &mut ProtoCommands,
    proto: &ProtoParam,
    commands: &mut Commands,
    _game: &GameParam,
) -> u32 {
    let wave_idx = (wave.saturating_sub(1)).min(2) as usize;
    let mini_idx = (mini_wave).min(2) as usize;
    let composition = WAVES[wave_idx][mini_idx];

    let mut rng = rand::thread_rng();
    let mut total = 0;
    for (mob, count) in composition.iter() {
        for _ in 0..*count {
            let offset = Vec2::new(rng.gen_range(-170. ..=170.), rng.gen_range(-140. ..=110.));
            let spawn_pos = offset;
            if let Some(mob_e) =
                proto_commands.spawn_from_proto(mob.clone(), &proto.prototypes, spawn_pos)
            {
                commands
                    .entity(mob_e)
                    .insert(CombatAlignment::Hostile)
                    .insert(LootTable {
                        drops: vec![Loot {
                            item: WorldObject::Coin,
                            min: 1,
                            max: 1,
                            rate: 0.2,
                        }],
                    })
                    .insert(DungeonShrineMob {
                        parent_shrine: shrine_e,
                    });
                total += 1;
            }
        }
    }
    total
}

fn grant_wave_rewards(wave: u8, rewards: &mut DungeonRewards) {
    let mut rng = rand::thread_rng();
    // Every wave adds the same coin range; the total range accumulates.
    rewards.coin_min += COIN_MIN;
    rewards.coin_max += COIN_MAX;
    match wave {
        1 => {
            rewards.items.push((WorldObject::UpgradeTome, 1));
            rewards.items.push((WorldObject::OrbOfTransformation, 1));
        }
        2 => {
            for _ in 0..3 {
                let mat = MATERIAL_POOL[rng.gen_range(0..MATERIAL_POOL.len())];
                rewards.items.push((mat, 1));
            }
        }
        3 => {
            for _ in 0..2 {
                let mat = MATERIAL_POOL[rng.gen_range(0..MATERIAL_POOL.len())];
                rewards.items.push((mat, 1));
            }
            let chest = if rng.gen_bool(0.5) {
                WorldObject::ChestBlock
            } else {
                WorldObject::HeirloomChest
            };
            rewards.items.push((chest, 1));
        }
        _ => {}
    }
}

/// Once the player has returned to the overworld, drops the accumulated rewards
/// in front of them and clears the reward resource.
fn drop_dungeon_rewards_on_return(
    time: Res<Time>,
    mut drop: ResMut<DungeonRewardDrop>,
    mut rewards: ResMut<DungeonRewards>,
    player_query: Query<(&GlobalTransform, &FacingDirection), With<Player>>,
    mut proto_commands: ProtoCommands,
    proto: ProtoParam,
    game: GameParam,
) {
    let Some(timer) = drop.timer.as_mut() else {
        return;
    };
    timer.tick(time.delta());
    if !timer.finished() {
        return;
    }
    drop.timer = None;

    // Safety: never drop while still in a dungeon.
    if game.era.current_era.is_dungeon() {
        rewards.clear();
        return;
    }

    let Ok((player_t, facing)) = player_query.get_single() else {
        return;
    };
    let base = player_t.translation().truncate() + facing.get_dir_vec() * 28.;
    let level = game.get_player_level();
    let mut rng = rand::thread_rng();

    // Roll the accumulated coin range into a single coin drop.
    if rewards.coin_max > 0 {
        let coins = rng.gen_range(rewards.coin_min..=rewards.coin_max);
        let offset = Vec2::new(rng.gen_range(-20. ..=20.), rng.gen_range(-20. ..=20.));
        proto_commands.spawn_item_from_proto(
            WorldObject::Coin,
            &proto,
            base + offset,
            coins,
            Some(level),
        );
    }

    for (obj, count) in rewards.items.drain(..) {
        let offset = Vec2::new(rng.gen_range(-20. ..=20.), rng.gen_range(-20. ..=20.));
        proto_commands.spawn_item_from_proto(obj, &proto, base + offset, count, Some(level));
    }
    rewards.clear();
}

/// Collapses the accumulated rewards into display entries: a coin range plus one
/// entry per distinct non-coin item with its total count.
fn reward_preview_entries(rewards: &DungeonRewards) -> Vec<(WorldObject, String)> {
    let mut entries: Vec<(WorldObject, String)> = vec![];
    if rewards.coin_max > 0 {
        let label = if rewards.coin_min == rewards.coin_max {
            format!("{}", rewards.coin_max)
        } else {
            format!("{}-{}", rewards.coin_min, rewards.coin_max)
        };
        entries.push((WorldObject::Coin, label));
    }
    // Aggregate non-coin items, preserving first-seen order.
    let mut aggregated: Vec<(WorldObject, usize)> = vec![];
    for (obj, count) in rewards.items.iter() {
        if let Some(existing) = aggregated.iter_mut().find(|(o, _)| o == obj) {
            existing.1 += *count;
        } else {
            aggregated.push((*obj, *count));
        }
    }
    for (obj, count) in aggregated {
        entries.push((obj, count.to_string()));
    }
    entries
}

/// Shows a floating reward preview (dark panel + icons) above the exit door while
/// the player is near, sitting above the exit's keybind prompt.
fn update_exit_reward_preview(
    mut commands: Commands,
    rewards: Res<DungeonRewards>,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    player_query: Query<&GlobalTransform, With<Player>>,
    exit_query: Query<(&GlobalTransform, &WorldObject), With<DungeonRoomEntity>>,
    existing: Query<Entity, With<ExitRewardPreview>>,
) {
    let Ok(player_t) = player_query.get_single() else {
        return;
    };
    let Some(exit_pos) = exit_query
        .iter()
        .find(|(_, obj)| **obj == WorldObject::DungeonExit)
        .map(|(t, _)| t.translation().truncate())
    else {
        for e in existing.iter() {
            commands.entity(e).despawn_recursive();
        }
        return;
    };

    let near = player_t.translation().truncate().distance(exit_pos) < REWARD_PREVIEW_DISTANCE;
    let should_show = near && !rewards.is_empty();

    if !should_show {
        for e in existing.iter() {
            commands.entity(e).despawn_recursive();
        }
        return;
    }

    // Already shown and unchanged: leave it in place.
    if !existing.is_empty() && !rewards.is_changed() {
        return;
    }
    for e in existing.iter() {
        commands.entity(e).despawn_recursive();
    }

    let entries = reward_preview_entries(&rewards);
    if entries.is_empty() {
        return;
    }

    const CELL_W: f32 = 26.;
    const PANEL_PAD: f32 = 8.;
    const PANEL_H: f32 = 30.;
    let panel_w = entries.len() as f32 * CELL_W + PANEL_PAD * 2.;

    // Above the door, and above where the keybind prompt floats over the player.
    let root_pos = Vec3::new(exit_pos.x, exit_pos.y + 30., 750.);
    let root = commands
        .spawn((
            SpatialBundle::from_transform(Transform::from_translation(root_pos)),
            RenderLayers::from_layers(&[0]),
            ExitRewardPreview,
            Name::new("Exit Reward Preview"),
        ))
        .id();

    // Dark semi-transparent background container.
    commands
        .spawn((
            SpriteBundle {
                sprite: Sprite {
                    color: Color::rgba(0., 0., 0., 0.6),
                    custom_size: Some(Vec2::new(panel_w, PANEL_H)),
                    ..default()
                },
                transform: Transform::from_translation(Vec3::new(0., 0., 0.)),
                ..default()
            },
            RenderLayers::from_layers(&[0]),
            Name::new("Exit Reward Preview Panel"),
        ))
        .set_parent(root);

    let start_x = -panel_w / 2. + PANEL_PAD + CELL_W / 2.;
    for (i, (obj, label)) in entries.iter().enumerate() {
        let x = start_x + i as f32 * CELL_W;
        let icon = spawn_item_stack_icon(
            &mut commands,
            &graphics,
            &ItemStack::crate_icon_stack(*obj),
            &asset_server,
            Vec2::new(x, 3.),
            Vec2::ZERO,
            0,
        );
        commands.entity(icon).insert(Transform {
            translation: Vec3::new(x, 3., 1.),
            ..default()
        });
        commands.entity(icon).set_parent(root);

        let text = spawn_text(
            &mut commands,
            &asset_server,
            Vec3::new(x, -9., 2.),
            Color::WHITE,
            label.clone(),
            Anchor::Center,
            FLOATING_TEXT,
            0,
        );
        commands.entity(text).set_parent(root);
    }
}
