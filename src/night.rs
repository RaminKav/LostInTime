use bevy::{
    prelude::*,
    reflect::TypeUuid,
    render::{
        mesh::MeshVertexBufferLayout,
        render_resource::{
            AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState,
            RenderPipelineDescriptor, ShaderRef, SpecializedMeshPipelineError,
        },
    },
    sprite::{Material2d, Material2dKey, Material2dPlugin, MaterialMesh2dBundle, Mesh2dHandle},
};
use serde::{Deserialize, Serialize};

use crate::{
    audio::{BGMPicker, UpdateBGMTrackEvent},
    chaos::hp_multiplier_for_total_chaos,
    chaos::ChaosTracker,
    client::is_not_paused,
    colors::WHITE,
    enemy::spawner::MobSpawningPaused,
    player::Player,
    run_once_per_run,
    ui::global_text_message::GlobalTextMessageEvent,
    world::dimension::{ActiveDimension, EraManager},
    world::dungeon::Dungeon,
    GameState, ScreenResolution, TextureCamera,
};

/// Daytime hour forced while inside a dungeon so no night effects (dark overlay,
/// 2x mob speed, night BGM) apply. Must be outside the night window.
const DUNGEON_FORCED_DAY_HOUR: f32 = 6.0;

/// Stashes the overworld day/night time while the player is in a dungeon so the
/// cycle can resume exactly where it left off on exit.
#[derive(Resource, Default)]
pub struct DungeonNightStash {
    saved_time: Option<f32>,
}

#[derive(Component)]
pub struct Night(Timer);

impl Night {
    /// Progress through the current in-game hour (0–1). Used for smooth overlay visuals only.
    #[inline]
    pub fn hour_progress(&self) -> f32 {
        self.0.percent().clamp(0.0, 1.0)
    }
}

/// Marker for the full-screen night overlay mesh entity.
#[derive(Component)]
pub struct NightOverlay;

/// World z for the overlay quad — just under the camera far plane (1000), above YSort sprites.
const NIGHT_OVERLAY_WORLD_Z: f32 = 999.0;
/// Extra size beyond the camera view so fast movement / camera lag never exposes edges.
const NIGHT_OVERLAY_OVERSCAN_FRAC: f32 = 0.25;
const NIGHT_OVERLAY_OVERSCAN_MIN: f32 = 80.0;
/// Radius (in uv units) of the clear bubble kept around the player at night.
const NIGHT_BUBBLE_RADIUS: f32 = 0.6;
/// How soft the edge of the player bubble is (0 = hard ring, 1 = fully gradual).
const NIGHT_BUBBLE_SOFTNESS: f32 = 0.85;
/// How much extra darkening is layered toward the screen edges (vignette).
const NIGHT_EDGE_BOOST: f32 = 0.4;
/// Shader tint saturation (< 1 desaturates, 1 = as-authored).
const TINT_SATURATION: f32 = 0.96;

/// Hour when night BGM / mob-spawn night begins — keep in sync with [`NightTracker::is_night`].
pub const NIGHT_PERIOD_START_HOUR: f32 = 15.0;
/// Hour when night BGM ends.
pub const NIGHT_PERIOD_END_HOUR: f32 = 18.0;
/// How many in-game hours before night music the overlay begins fading in.
const NIGHT_OVERLAY_LEADUP: f32 = 4.0;
/// Overlay fade-in starts here (11) and finishes at [`NIGHT_PERIOD_START_HOUR`] (15).
const NIGHT_FADE_IN_START: f32 = NIGHT_PERIOD_START_HOUR - NIGHT_OVERLAY_LEADUP;
const NIGHT_FADE_IN_DURATION: f32 = NIGHT_OVERLAY_LEADUP;
/// Overlay begins fading out when night music ends.
const NIGHT_FADE_OUT_START: f32 = NIGHT_PERIOD_END_HOUR;
/// Length of the dusk fade-out after night (intensity + tint fade together).
const NIGHT_FADE_OUT_DURATION: f32 = 4.0;

/// Muted sunset warmth.
pub const DUSK_AMBER: Color = Color::rgb(0.38, 0.20, 0.12);
/// Muted twilight purple.
pub const TWILIGHT_PURPLE: Color = Color::rgb(0.26, 0.13, 0.24);
/// Warm moonlit plum (less cool-blue than a straight indigo).
pub const NIGHT_PLUM: Color = Color::rgb(0.18, 0.09, 0.22);
/// Neutral warm shadow — fade-out target as the overlay disappears.
const DAY_NEUTRAL: Color = Color::rgb(0.16, 0.13, 0.11);

#[inline]
fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color::rgb(
        a.r() + (b.r() - a.r()) * t,
        a.g() + (b.g() - a.g()) * t,
        a.b() + (b.b() - a.b()) * t,
    )
}

#[inline]
fn smoothstep01(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
fn smooth_lerp_color(a: Color, b: Color, t: f32) -> Color {
    lerp_color(a, b, smoothstep01(t))
}

#[inline]
fn night_overlay_world_size(res: &ScreenResolution) -> Vec2 {
    let pad_w =
        (res.world_view_width * NIGHT_OVERLAY_OVERSCAN_FRAC).max(NIGHT_OVERLAY_OVERSCAN_MIN);
    let pad_h =
        (res.world_view_height * NIGHT_OVERLAY_OVERSCAN_FRAC).max(NIGHT_OVERLAY_OVERSCAN_MIN);
    Vec2::new(res.world_view_width + pad_w, res.world_view_height + pad_h)
}

/// Linear RGB + saturation boost packed for the night overlay shader.
#[inline]
fn night_overlay_color_uniform(color: Color) -> Vec4 {
    let [r, g, b, _] = color.as_linear_rgba_f32();
    Vec4::new(r, g, b, TINT_SATURATION)
}

const NIGHT_OVERLAY_BLEND: BlendState = BlendState {
    color: BlendComponent {
        src_factor: BlendFactor::SrcAlpha,
        dst_factor: BlendFactor::OneMinusSrcAlpha,
        operation: BlendOperation::Add,
    },
    alpha: BlendComponent {
        src_factor: BlendFactor::One,
        dst_factor: BlendFactor::OneMinusSrcAlpha,
        operation: BlendOperation::Add,
    },
};

/// Full-screen night overlay: time-of-day tint with a clear bubble around the player.
/// `params`: x = intensity, y = bubble radius (uv), z = bubble softness, w = edge boost.
/// `player_uv`: xy = player position in quad-uv space, z = aspect ratio (w/h).
#[derive(AsBindGroup, TypeUuid, Debug, Clone)]
#[uuid = "0c0f6b2a-6d3e-4a7b-9c5e-2f8a1d4b6e30"]
pub struct NightOverlayMaterial {
    #[uniform(0)]
    pub params: Vec4,
    #[uniform(1)]
    pub tint: Vec4,
    #[uniform(2)]
    pub player_uv: Vec4,
}

impl Material2d for NightOverlayMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/night_overlay.wgsl".into()
    }

    fn specialize(
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayout,
        _key: Material2dKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if let Some(fragment) = &mut descriptor.fragment {
            if let Some(target_state) = &mut fragment.targets[0] {
                target_state.blend = Some(NIGHT_OVERLAY_BLEND);
            }
        }
        Ok(())
    }
}

/// Resource to track infinite mode state (activated after defeating Era 3 boss or era timer expiration)
#[derive(Default, Resource, Clone, Debug)]
pub struct InfiniteMode {
    pub active: bool,
    /// Difficulty level (0-5), increases every 2 minutes
    pub difficulty_level: u8,
    /// Timer for difficulty scaling (1.5 minutes per level, max 5 levels)
    pub difficulty_timer: Timer,
    /// Elapsed time in endless mode (seconds); chaos bonus is computed from this
    pub elapsed_seconds: f32,
}

/// Maximum difficulty level in infinite mode (Tier 1: 1-5, Tier 2: 6-10)
pub const MAX_DIFFICULTY_LEVEL: u8 = 15;
/// Seconds between difficulty increases
pub const DIFFICULTY_INCREASE_INTERVAL: f32 = 90.0; // 1.5 minutes
/// Endless chaos bonus: +1 chaos per this many seconds of elapsed endless time.
pub const ENDLESS_CHAOS_SECONDS_PER_POINT: f32 = 15.0;
/// HUD total chaos when endless typically begins (Era 3 base); excess above this scales mobs harder.
const ENDLESS_CHAOS_REFERENCE: f32 = 22.0;
/// Extra HP scaling on chaos above [`ENDLESS_CHAOS_REFERENCE`] (steeper than normal mobs/objects).
const ENDLESS_HP_EXCESS_EXPONENT: f32 = 1.35;
const ENDLESS_HP_EXCESS_COEFFICIENT: f32 = 2.2;
const ENDLESS_ATK_EXCESS_EXPONENT: f32 = 1.1;
const ENDLESS_ATK_EXCESS_COEFFICIENT: f32 = 0.10;
/// Slight attack boost on top of the normal chaos attack curve during endless.
pub const ENDLESS_MOB_ATTACK_BONUS: f32 = 1.10;
/// Scales only the speed gain above 1x (0.8 = 20% slower ramp than before).
const ENDLESS_SPEED_GAIN_SCALE: f32 = 0.8;

/// Era timer - 10 minutes per era. Timer pauses in dungeons.
pub const ERA_TIMER_SECONDS: f32 = 11.0 * 60.0; // 10 minutes

impl InfiniteMode {
    pub fn new() -> Self {
        Self {
            active: true,
            difficulty_level: 1,
            difficulty_timer: Timer::from_seconds(
                DIFFICULTY_INCREASE_INTERVAL,
                TimerMode::Repeating,
            ),
            elapsed_seconds: 0.0,
        }
    }

    /// Raw endless chaos from elapsed time (+1 per [`ENDLESS_CHAOS_SECONDS_PER_POINT`]).
    /// Used for the HUD / chaos tracker display only.
    pub fn chaos_from_elapsed(elapsed_seconds: f32) -> f32 {
        if elapsed_seconds <= 0.0 {
            return 0.0;
        }
        elapsed_seconds / ENDLESS_CHAOS_SECONDS_PER_POINT
    }

    /// HP multiplier for void endless mobs from HUD total chaos (`global + endless bonus`).
    /// Uses the normal curve plus a power-law bonus on chaos above [`ENDLESS_CHAOS_REFERENCE`].
    pub fn endless_mob_hp_multiplier(total_chaos: f32) -> f32 {
        let normal = hp_multiplier_for_total_chaos(total_chaos);
        let excess = (total_chaos - ENDLESS_CHAOS_REFERENCE).max(0.0);
        if excess <= 0.0 {
            return normal;
        }
        normal + 1.1 * excess.powf(ENDLESS_HP_EXCESS_EXPONENT) * ENDLESS_HP_EXCESS_COEFFICIENT
    }

    /// Attack multiplier for void endless mobs from HUD total chaos (`global + endless bonus`).
    pub fn endless_mob_attack_multiplier(total_chaos: f32) -> f32 {
        let normal = (1.0 + total_chaos).powf(0.56);
        let excess = (total_chaos - ENDLESS_CHAOS_REFERENCE).max(0.0);
        if excess <= 0.0 {
            return normal * ENDLESS_MOB_ATTACK_BONUS;
        }
        normal
            * (1.0 + (excess * ENDLESS_ATK_EXCESS_COEFFICIENT).powf(ENDLESS_ATK_EXCESS_EXPONENT))
            * ENDLESS_MOB_ATTACK_BONUS
    }

    /// Adds elapsed endless time and keeps difficulty level in sync (for dev tools).
    pub fn add_elapsed_seconds(&mut self, seconds: f32) {
        if !self.active {
            return;
        }
        self.elapsed_seconds += seconds;
        self.sync_difficulty_from_elapsed();
    }

    fn sync_difficulty_from_elapsed(&mut self) {
        let level = (1.0 + self.elapsed_seconds / DIFFICULTY_INCREASE_INTERVAL).floor() as u8;
        self.difficulty_level = level.clamp(1, MAX_DIFFICULTY_LEVEL);
    }

    /// Get the elapsed time formatted as MM:SS
    pub fn get_elapsed_display_string(&self) -> String {
        let minutes = (self.elapsed_seconds / 60.0) as u32;
        let seconds = (self.elapsed_seconds % 60.0) as u32;
        format!("{:02}:{:02}", minutes, seconds)
    }

    /// Bonus chaos from elapsed endless time (linear; added on top of global chaos).
    pub fn get_chaos_bonus(&self) -> f32 {
        if self.active {
            Self::chaos_from_elapsed(self.elapsed_seconds)
        } else {
            0.0
        }
    }

    /// Get which tier the current difficulty level is in
    /// Tier 1: levels 1-5, Tier 2: levels 6-10
    pub fn get_tier(&self) -> u8 {
        if self.difficulty_level <= 5 {
            1
        } else {
            2
        }
    }

    /// Get the speed multiplier for the current difficulty level
    /// Tier 1 (1-5): 50% speed per level (1.5x to 3.5x before gain scaling)
    /// Tier 2 (6+): +150% per level above tier 1 cap
    pub fn get_speed_multiplier(&self) -> f32 {
        let raw = if self.difficulty_level <= 5 {
            1. + (self.difficulty_level as f32 * 0.5)
        } else {
            let tier1_max = 1. + (5.0 * 0.5);
            let tier2_levels = self.difficulty_level - 5;
            tier1_max + (tier2_levels as f32 * 1.5)
        };
        1.0 + (raw - 1.0) * ENDLESS_SPEED_GAIN_SCALE
    }

    /// Get the tint alpha for the current difficulty level
    /// Tier 1 (1-5): Purple tint from 0.2 to 1.0 alpha
    /// Tier 2 (6-10): Red tint always at 1.0 alpha
    pub fn get_tint_alpha(&self) -> f32 {
        if self.difficulty_level <= 5 {
            // Tier 1: Purple tint from 0.2 (level 1) to 1.0 (level 5)
            // Range is 0.8 over 4 intervals (level 1->2, 2->3, 3->4, 4->5)
            let base_alpha = 0.2;
            let alpha_range = 1.0 - base_alpha; // 0.8
            let intervals = 4.0; // 4 intervals between levels 1-5
            if self.difficulty_level == 0 {
                base_alpha
            } else {
                base_alpha + (alpha_range / intervals * (self.difficulty_level - 1) as f32)
            }
        } else {
            // Tier 2: Always 1.0 alpha for red tint
            1.0
        }
    }
}

/// Resource to track era timer
#[derive(Resource, Clone, Debug)]
pub struct EraTimer {
    pub remaining_seconds: f32,
    pub paused: bool,
}

impl Default for EraTimer {
    fn default() -> Self {
        Self {
            remaining_seconds: ERA_TIMER_SECONDS,
            paused: false,
        }
    }
}

impl EraTimer {
    pub fn reset(&mut self) {
        self.remaining_seconds = ERA_TIMER_SECONDS;
        self.paused = false;
    }

    pub fn get_display_string(&self) -> String {
        let minutes = (self.remaining_seconds / 60.0).floor() as u32;
        let seconds = (self.remaining_seconds % 60.0).floor() as u32;
        format!("{:02}:{:02}", minutes, seconds)
    }

    pub fn is_expired(&self) -> bool {
        self.remaining_seconds <= 0.0
    }
}

/// Marker component for mobs spawned during infinite mode
/// These mobs get red tint and 50% speed boost
#[derive(Component, Debug, Clone)]
pub struct InfiniteModeMob;

/// Event sent when infinite mode starts
#[derive(Default)]
pub struct InfiniteModeStartedEvent;

#[derive(Reflect, Resource, Clone, Debug, Serialize, Deserialize)]
#[reflect(Resource)]
pub struct NightTracker {
    pub days: u8,
    pub time: f32,
}

impl Default for NightTracker {
    fn default() -> Self {
        Self { days: 0, time: 0. }
    }
}

impl NightTracker {
    /// Player-facing day number (`days` is 0-based internally for scaling / chaos).
    pub fn display_day(&self) -> u8 {
        self.days.saturating_add(1)
    }
    /// Peak overlay strength during full night.
    const PEAK_OVERLAY_INTENSITY: f32 = 0.8;

    #[inline]
    fn overlay_fade_in(t: f32) -> f32 {
        if t < NIGHT_FADE_IN_START {
            0.0
        } else if t < NIGHT_FADE_IN_START + NIGHT_FADE_IN_DURATION {
            smoothstep01((t - NIGHT_FADE_IN_START) / NIGHT_FADE_IN_DURATION)
        } else {
            1.0
        }
    }

    #[inline]
    fn overlay_fade_out(t: f32) -> f32 {
        if t < NIGHT_FADE_OUT_START {
            1.0
        } else if t >= NIGHT_FADE_OUT_START + NIGHT_FADE_OUT_DURATION {
            0.0
        } else {
            1.0 - smoothstep01((t - NIGHT_FADE_OUT_START) / NIGHT_FADE_OUT_DURATION)
        }
    }

    pub fn get_alpha(&self) -> f32 {
        self.get_overlay_intensity()
    }

    /// Overlay darkness at fractional time `t` (0–24). Use [`Self::visual_time`] for smooth visuals.
    pub fn get_overlay_intensity_at(&self, t: f32) -> f32 {
        Self::PEAK_OVERLAY_INTENSITY * Self::overlay_fade_in(t) * Self::overlay_fade_out(t)
    }

    /// Overlay darkness: fade-in (11–15), peak during night music (15–18), fade-out (18–22).
    pub fn get_overlay_intensity(&self) -> f32 {
        self.get_overlay_intensity_at(self.time)
    }

    /// Returns the alpha for infinite mode (always dark)
    pub fn get_infinite_mode_alpha(&self) -> f32 {
        Self::PEAK_OVERLAY_INTENSITY
    }

    /// Fractional time-of-day for overlay visuals only. Gameplay still uses whole-hour [`Self::time`].
    pub fn visual_time(&self, hour_progress: f32) -> f32 {
        (self.time + hour_progress.clamp(0.0, 1.0)).min(24.0)
    }

    /// Overlay tint at fractional time `t` (0–24).
    pub fn get_tint_at(&self, t: f32) -> Color {
        if t < NIGHT_FADE_IN_START {
            return DAY_NEUTRAL;
        }

        let leadup_half = NIGHT_FADE_IN_DURATION * 0.5;

        if t < NIGHT_FADE_IN_START + leadup_half {
            smooth_lerp_color(
                DUSK_AMBER,
                TWILIGHT_PURPLE,
                (t - NIGHT_FADE_IN_START) / leadup_half,
            )
        } else if t < NIGHT_PERIOD_START_HOUR {
            smooth_lerp_color(
                TWILIGHT_PURPLE,
                NIGHT_PLUM,
                (t - (NIGHT_FADE_IN_START + leadup_half)) / leadup_half,
            )
        } else if t < NIGHT_FADE_OUT_START {
            NIGHT_PLUM
        } else if t < NIGHT_FADE_OUT_START + NIGHT_FADE_OUT_DURATION {
            let u = smoothstep01((t - NIGHT_FADE_OUT_START) / NIGHT_FADE_OUT_DURATION);
            if u < 0.5 {
                smooth_lerp_color(NIGHT_PLUM, TWILIGHT_PURPLE, u / 0.5)
            } else {
                smooth_lerp_color(TWILIGHT_PURPLE, DAY_NEUTRAL, (u - 0.5) / 0.5)
            }
        } else {
            DAY_NEUTRAL
        }
    }

    /// Time-of-day overlay tint; fade-out (18–22) shifts hue and dims in sync with intensity.
    pub fn get_tint(&self) -> Color {
        self.get_tint_at(self.time)
    }
    pub fn is_night(&self) -> bool {
        self.time >= NIGHT_PERIOD_START_HOUR && self.time <= NIGHT_PERIOD_END_HOUR
    }
    pub fn get_hour(&self) -> u8 {
        self.time as u8
    }
}

pub struct NightPlugin;

#[derive(Default)]
pub struct NewDayEvent;

impl Plugin for NightPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<NightTracker>()
            .add_plugin(Material2dPlugin::<NightOverlayMaterial>::default())
            .add_event::<NewDayEvent>()
            .add_event::<InfiniteModeStartedEvent>()
            .add_event::<EraTimerExpiredEvent>()
            .init_resource::<InfiniteMode>()
            .init_resource::<EraTimer>()
            .init_resource::<DungeonNightStash>()
            // .add_plugin(ResourceInspectorPlugin::<NightTracker>::default().run_if(dim_spawned))
            .add_system(
                spawn_night
                    .run_if(run_once_per_run())
                    .in_schedule(OnEnter(GameState::Main)),
            )
            // Reset era timer and infinite mode when starting a new run
            .add_system(reset_era_timer_on_new_run.in_schedule(OnEnter(GameState::MainMenu)))
            .add_systems(
                (
                    manage_dungeon_night_freeze,
                    sync_night_overlay_on_tracker_change,
                    tick_night_color.run_if(is_not_paused),
                    handle_infinite_mode_started,
                    tick_infinite_mode_chaos.run_if(is_not_paused),
                    tick_infinite_mode_difficulty.run_if(is_not_paused),
                    tick_era_timer.run_if(is_not_paused),
                    handle_era_timer_expired,
                    transition_to_daytime_on_peaceful_mode,
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            // PostUpdate: must run after `move_camera_with_player`; ordering from `OnUpdate`
            // creates an Update ↔ PostUpdate cycle.
            .add_system(
                update_night_overlay
                    .after(crate::inputs::move_camera_with_player)
                    .run_if(in_state(GameState::Main))
                    .in_base_set(CoreSet::PostUpdate),
            );
    }
}

/// Event sent when era timer expires
#[derive(Default)]
pub struct EraTimerExpiredEvent;

pub fn spawn_night(
    mut commands: Commands,
    night_tracker: Res<NightTracker>,
    infinite_mode: Res<InfiniteMode>,
    res: Res<ScreenResolution>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<NightOverlayMaterial>>,
) {
    info!("Spawning night overlay");

    let overlay_size = night_overlay_world_size(&res);

    let (intensity, tint) = if infinite_mode.active {
        (night_tracker.get_infinite_mode_alpha(), NIGHT_PLUM)
    } else {
        (
            night_tracker.get_overlay_intensity(),
            night_tracker.get_tint(),
        )
    };
    let aspect = res.world_view_width / res.world_view_height.max(1.0);

    let material = materials.add(NightOverlayMaterial {
        params: Vec4::new(
            intensity,
            NIGHT_BUBBLE_RADIUS,
            NIGHT_BUBBLE_SOFTNESS,
            NIGHT_EDGE_BOOST,
        ),
        tint: night_overlay_color_uniform(tint),
        player_uv: Vec4::new(0.5, 0.5, aspect, 0.0),
    });

    // Unit quad scaled to the camera view each frame, so UVs stay 0..1 regardless of zoom.
    let mesh = Mesh2dHandle::from(meshes.add(Mesh::from(shape::Quad {
        size: Vec2::ONE,
        ..default()
    })));

    commands.spawn((
        MaterialMesh2dBundle {
            mesh,
            material,
            transform: Transform {
                translation: Vec3::new(0.0, 0.0, NIGHT_OVERLAY_WORLD_Z),
                scale: Vec3::new(overlay_size.x, overlay_size.y, 1.0),
                ..default()
            },
            ..default()
        },
        NightOverlay,
        Night(Timer::from_seconds(9.5, TimerMode::Repeating)),
        Name::new("night"),
    ));
}

/// Drives the night overlay material each frame: time-of-day tint, darkness intensity, the
/// player-centred clear bubble, and quad sizing/position to match the (zoom-dependent) camera view.
fn update_night_overlay(
    night_tracker: Res<NightTracker>,
    infinite_mode: Res<InfiniteMode>,
    res: Res<ScreenResolution>,
    mut materials: ResMut<Assets<NightOverlayMaterial>>,
    mut night_query: Query<
        (&Handle<NightOverlayMaterial>, &mut Transform, &Night),
        (With<NightOverlay>, With<Night>),
    >,
    camera_query: Query<&GlobalTransform, (With<TextureCamera>, Without<NightOverlay>)>,
    player_query: Query<&GlobalTransform, (With<Player>, Without<NightOverlay>)>,
) {
    let hour_progress = night_query
        .iter()
        .next()
        .map(|(_, _, night)| night.hour_progress())
        .unwrap_or(0.0);
    let visual_time = night_tracker.visual_time(hour_progress);

    let (intensity, tint) = if infinite_mode.active {
        (night_tracker.get_infinite_mode_alpha(), NIGHT_PLUM)
    } else {
        (
            night_tracker.get_overlay_intensity_at(visual_time),
            night_tracker.get_tint_at(visual_time),
        )
    };
    let aspect = res.world_view_width / res.world_view_height.max(1.0);

    let player_uv = match (camera_query.get_single(), player_query.get_single()) {
        (Ok(cam), Ok(player)) => {
            let cam = cam.translation();
            let player = player.translation();
            let dx = (player.x - cam.x) / res.world_view_width;
            let dy = (player.y - cam.y) / res.world_view_height;
            Vec2::new(0.5 + dx, 0.5 - dy)
        }
        _ => Vec2::splat(0.5),
    };

    let overlay_size = night_overlay_world_size(&res);
    let camera_pos = camera_query
        .get_single()
        .map(|cam| cam.translation())
        .unwrap_or(Vec3::ZERO);

    for (handle, mut transform, _) in night_query.iter_mut() {
        if let Some(material) = materials.get_mut(handle) {
            material.params.x = intensity;
            material.tint = night_overlay_color_uniform(tint);
            material.player_uv = Vec4::new(player_uv.x, player_uv.y, aspect, 0.0);
        }

        // Track the game camera each frame (world root — not parented; camera children don't render).
        transform.translation = Vec3::new(camera_pos.x, camera_pos.y, NIGHT_OVERLAY_WORLD_Z);
        transform.scale.x = overlay_size.x;
        transform.scale.y = overlay_size.y;
    }
}

/// Switches BGM when `NightTracker` is changed externally (e.g. era transition). Visuals are
/// handled every frame by [`update_night_overlay`].
fn sync_night_overlay_on_tracker_change(
    night_tracker: Res<NightTracker>,
    infinite_mode: Res<InfiniteMode>,
    mut bgm_track_event: EventWriter<UpdateBGMTrackEvent>,
    bgm_tracker: Res<BGMPicker>,
) {
    if !night_tracker.is_changed() {
        return;
    }

    if infinite_mode.active {
        return;
    }
    if night_tracker.is_night() && bgm_tracker.current_track != *"sounds/bgm_night.ogg" {
        bgm_track_event.send(UpdateBGMTrackEvent {
            asset_path: "sounds/bgm_night.ogg".to_owned(),
        });
    } else if !night_tracker.is_night() && bgm_tracker.current_track != *"sounds/bgm_day.ogg" {
        bgm_track_event.send(UpdateBGMTrackEvent {
            asset_path: "sounds/bgm_day.ogg".to_owned(),
        });
    }
}

/// While in a dungeon, freeze the day/night cycle at a daytime hour (stashing the
/// real overworld time) so no night effects apply; restore it on exit.
pub fn manage_dungeon_night_freeze(
    mut night_tracker: ResMut<NightTracker>,
    mut stash: ResMut<DungeonNightStash>,
    dungeon_check: Query<&Dungeon, With<ActiveDimension>>,
) {
    let in_dungeon = dungeon_check.get_single().is_ok();
    if in_dungeon {
        if stash.saved_time.is_none() {
            stash.saved_time = Some(night_tracker.time);
            night_tracker.time = DUNGEON_FORCED_DAY_HOUR;
        }
    } else if let Some(saved) = stash.saved_time.take() {
        night_tracker.time = saved;
    }
}

pub fn tick_night_color(
    time: Res<Time>,
    mut query: Query<&mut Night>,
    mut night_tracker: ResMut<NightTracker>,
    mut bgm_track_event: EventWriter<UpdateBGMTrackEvent>,
    bgm_tracker: Res<BGMPicker>,
    mut new_day_event: EventWriter<NewDayEvent>,
    mut global_text_events: EventWriter<GlobalTextMessageEvent>,
    infinite_mode: Res<InfiniteMode>,
    mut chaos_tracker: ResMut<ChaosTracker>,
    dungeon_check: Query<&Dungeon, With<ActiveDimension>>,
) {
    // Day/night cycle is frozen while in a dungeon (see manage_dungeon_night_freeze).
    if dungeon_check.get_single().is_ok() {
        return;
    }

    // In infinite mode, keep it always night
    if infinite_mode.active {
        // Always play night music in infinite mode
        if bgm_tracker.current_track != *"sounds/bgm_night.ogg" {
            bgm_track_event.send(UpdateBGMTrackEvent {
                asset_path: "sounds/bgm_night.ogg".to_owned(),
            });
        }
        return;
    }

    let mut music_changed = false;
    for mut night_state in query.iter_mut() {
        night_state.0.tick(time.delta());
        if night_state.0.finished() {
            let prev_time = night_tracker.time;
            night_tracker.time += 1.;
            if night_tracker.time >= 24. {
                night_tracker.time = 0.;
            }
            let was_night =
                prev_time >= NIGHT_PERIOD_START_HOUR && prev_time <= NIGHT_PERIOD_END_HOUR;
            // New day begins when a swarm night ends (BGM switches back to day), not at midnight.
            if was_night && !night_tracker.is_night() {
                night_tracker.days += 1;
                chaos_tracker.add_chaos(1.);
                new_day_event.send_default();
                global_text_events.send(GlobalTextMessageEvent::day_announcement(
                    night_tracker.display_day(),
                    WHITE,
                ));
            }
            music_changed = true;
        }
    }

    if music_changed || night_tracker.is_added() || night_tracker.is_changed() {
        // change music
        if night_tracker.is_night() && bgm_tracker.current_track != *"sounds/bgm_night.ogg" {
            bgm_track_event.send(UpdateBGMTrackEvent {
                asset_path: "sounds/bgm_night.ogg".to_owned(),
            });

            // if !seen_tips.has_seen(&Tip::Night) {
            //     tip_event.send(TipEvent {
            //         tip: Tip::Night,
            //         pos: Vec3::new(-184., -116., 70.),
            //     });
            // }
        } else if !night_tracker.is_night() && bgm_tracker.current_track != *"sounds/bgm_day.ogg" {
            bgm_track_event.send(UpdateBGMTrackEvent {
                asset_path: "sounds/bgm_day.ogg".to_owned(),
            });
        }
    }
}

/// Handle infinite mode started event - set up infinite mode state
pub fn handle_infinite_mode_started(
    mut events: EventReader<InfiniteModeStartedEvent>,
    mut infinite_mode: ResMut<InfiniteMode>,
    world_obj_cache: Res<crate::world::generation::WorldObjectCache>,
    chunk_query: Query<&crate::world::chunk::Chunk>,
) {
    for _ in events.iter() {
        if !infinite_mode.active {
            *infinite_mode = InfiniteMode::new();
            let tier = infinite_mode.get_tier();
            let tier_name = if tier == 1 { "Purple" } else { "Red" };
            info!(
                "INFINITE MODE ACTIVATED! Starting at difficulty {} (Tier {} - {}), speed {:.1}x, tint {:.1}. Chaos will increase every 30 seconds.",
                infinite_mode.difficulty_level,
                tier,
                tier_name,
                infinite_mode.get_speed_multiplier(),
                infinite_mode.get_tint_alpha()
            );

            info!(
                "DEBUG: WorldObjectCache sizes - objects: {}, unique_objs: {}, dungeon_objects: {}, generated_chunks: {}, tile_data_cache: {}",
                world_obj_cache.objects.len(),
                world_obj_cache.unique_objs.len(),
                world_obj_cache.dungeon_objects.len(),
                world_obj_cache.generated_chunks.len(),
                world_obj_cache.tile_data_cache.len()
            );
            info!("DEBUG: Active chunks count: {}", chunk_query.iter().count());
        }
    }
}

/// Tick endless mode elapsed time; chaos is computed smoothly from elapsed_seconds (power-law).
pub fn tick_infinite_mode_chaos(time: Res<Time>, mut infinite_mode: ResMut<InfiniteMode>) {
    if !infinite_mode.active {
        return;
    }
    infinite_mode.elapsed_seconds += time.delta_seconds();
}

/// Tick the difficulty timer in infinite mode - increases difficulty every 1.5 minutes (up to level 10)
pub fn tick_infinite_mode_difficulty(time: Res<Time>, mut infinite_mode: ResMut<InfiniteMode>) {
    if !infinite_mode.active {
        return;
    }

    // Already at max difficulty
    if infinite_mode.difficulty_level >= MAX_DIFFICULTY_LEVEL {
        return;
    }

    infinite_mode.difficulty_timer.tick(time.delta());
    if infinite_mode.difficulty_timer.just_finished() {
        infinite_mode.difficulty_level += 1;
        let tier = infinite_mode.get_tier();
        let tier_name = if tier == 1 { "Purple" } else { "Red" };
        info!(
            "Infinite mode: Difficulty increased to level {} (Tier {} - {})! Speed: {:.1}x, Tint: {:.1}",
            infinite_mode.difficulty_level,
            tier,
            tier_name,
            infinite_mode.get_speed_multiplier(),
            infinite_mode.get_tint_alpha()
        );
    }
}

/// Tick the era timer - pauses in dungeons, triggers infinite mode when expired
pub fn tick_era_timer(
    time: Res<Time>,
    mut era_timer: ResMut<EraTimer>,
    era_manager: Res<EraManager>,
    infinite_mode: Res<InfiniteMode>,
    mut expired_event: EventWriter<EraTimerExpiredEvent>,
    // mut tip_event: EventWriter<TipEvent>,
    // seen_tips: Res<SeenTips>,
) {
    // Don't tick if infinite mode is already active
    if infinite_mode.active {
        return;
    }

    // Pause timer in dungeons
    if era_manager.current_era.is_dungeon() {
        return;
    }

    let was_above_3min = era_timer.remaining_seconds > 180.0;

    // Tick the timer
    era_timer.remaining_seconds -= time.delta_seconds();

    if was_above_3min && era_timer.remaining_seconds <= 180.0 {
        // if !seen_tips.has_seen(&Tip::EndlessMode) {
        //     tip_event.send(TipEvent {
        //         tip: Tip::EndlessMode,
        //         pos: Vec3::new(-184., -116., 65.),
        //     });
        // }
    }

    // Check if expired
    if era_timer.is_expired() {
        era_timer.remaining_seconds = 0.0; // Clamp to 0
        expired_event.send_default();
    }
}

/// Handle era timer expiration - trigger infinite mode
pub fn handle_era_timer_expired(
    mut events: EventReader<EraTimerExpiredEvent>,
    mut infinite_mode_event: EventWriter<InfiniteModeStartedEvent>,
    infinite_mode: Res<InfiniteMode>,
    mut mob_spawning_paused: ResMut<crate::enemy::spawner::MobSpawningPaused>,
) {
    for _ in events.iter() {
        if !infinite_mode.active {
            info!("ERA TIMER EXPIRED! Entering infinite mode!");
            infinite_mode_event.send_default();
        }
        // Resume mob spawning when timer expires
        if mob_spawning_paused.paused {
            info!("Era timer expired - resuming mob spawning");
            mob_spawning_paused.paused = false;
        }
    }
}

/// Reset era timer and infinite mode when entering a new era (called from dimension.rs)
pub fn reset_era_timer_and_infinite_mode(
    era_timer: &mut EraTimer,
    infinite_mode: &mut InfiniteMode,
    mob_spawning_paused: &mut MobSpawningPaused,
) {
    era_timer.reset();

    infinite_mode.active = false;
    infinite_mode.difficulty_level = 0;
    infinite_mode.difficulty_timer =
        Timer::from_seconds(DIFFICULTY_INCREASE_INTERVAL, TimerMode::Repeating);
    infinite_mode.elapsed_seconds = 0.0;

    if mob_spawning_paused.paused {
        info!("Era changed - resuming mob spawning");
        mob_spawning_paused.paused = false;
    }

    info!(
        "Era timer reset to {:02}:{:02}, infinite mode deactivated",
        (ERA_TIMER_SECONDS / 60.0) as u32,
        (ERA_TIMER_SECONDS % 60.0) as u32
    );
}

/// Reset era timer and infinite mode when starting a new run
fn reset_era_timer_on_new_run(
    mut era_timer: ResMut<EraTimer>,
    mut infinite_mode: ResMut<InfiniteMode>,
    mut mob_spawning_paused: ResMut<MobSpawningPaused>,
) {
    // Reset era timer to full 12 minutes
    era_timer.reset();

    // Reset infinite mode completely
    infinite_mode.active = false;
    infinite_mode.difficulty_level = 0;
    infinite_mode.difficulty_timer =
        Timer::from_seconds(DIFFICULTY_INCREASE_INTERVAL, TimerMode::Repeating);
    infinite_mode.elapsed_seconds = 0.0;

    // Reset mob spawning paused state
    mob_spawning_paused.paused = false;

    info!("Era timer and infinite mode reset for new run");
}

/// Transition to daytime when peaceful mode is activated (boss defeated)
/// Only transitions if currently in night time
fn transition_to_daytime_on_peaceful_mode(
    mob_spawning_paused: Res<MobSpawningPaused>,
    mut night_tracker: ResMut<NightTracker>,
    mut bgm_track_event: EventWriter<UpdateBGMTrackEvent>,
    infinite_mode: Res<InfiniteMode>,
) {
    if infinite_mode.active {
        return;
    }

    if mob_spawning_paused.paused && night_tracker.is_night() {
        night_tracker.time = 12.0;
        info!(
            "Peaceful mode activated - transitioning to daytime (time: {})",
            night_tracker.time
        );

        bgm_track_event.send(UpdateBGMTrackEvent {
            asset_path: "sounds/bgm_day.ogg".to_owned(),
        });
    }
}
