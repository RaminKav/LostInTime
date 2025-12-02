use bevy::{prelude::*, render::view::RenderLayers};
use serde::{Deserialize, Serialize};

use crate::{
    audio::{BGMPicker, UpdateBGMTrackEvent},
    chaos::IncreaseChaosEvent,
    client::is_not_paused,
    colors::{overwrite_alpha, NIGHT},
    run_once_per_run, GameState, ScreenResolution, GAME_HEIGHT,
};

#[derive(Component)]
pub struct Night(Timer);

/// Resource to track infinite mode state (activated after defeating Era 3 boss)
#[derive(Default, Resource, Clone, Debug)]
pub struct InfiniteMode {
    pub active: bool,
    pub chaos_timer: Timer,
}

impl InfiniteMode {
    pub fn new() -> Self {
        Self {
            active: true,
            chaos_timer: Timer::from_seconds(30.0, TimerMode::Repeating),
        }
    }
}

/// Event sent when infinite mode starts
#[derive(Default)]
pub struct InfiniteModeStartedEvent;

#[derive(Default, Reflect, Resource, Clone, Debug, Serialize, Deserialize)]
#[reflect(Resource)]
pub struct NightTracker {
    pub days: u8,
    pub time: f32,
}
impl NightTracker {
    pub fn get_alpha(&self) -> f32 {
        if self.time < 6. {
            0.
        } else if self.time >= 6. && self.time <= 14. {
            return (self.time - 6.) * 0.09833333;
        } else {
            return 0.8 - (self.time - 14.) * 0.11666666;
        }
    }
    /// Returns the alpha for infinite mode (always dark)
    pub fn get_infinite_mode_alpha(&self) -> f32 {
        0.8 // Always full night darkness
    }
    pub fn is_night(&self) -> bool {
        // 12am to 6am ->
        self.time >= 15. && self.time <= 18.
    }
    pub fn is_start_of_new_day(&self) -> bool {
        self.time == 0.
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
            .add_event::<NewDayEvent>()
            .add_event::<InfiniteModeStartedEvent>()
            .init_resource::<InfiniteMode>()
            // .add_plugin(ResourceInspectorPlugin::<NightTracker>::default().run_if(dim_spawned))
            .add_system(
                spawn_night
                    .run_if(run_once_per_run())
                    .in_schedule(OnEnter(GameState::Main)),
            )
            .add_systems(
                (
                    tick_night_color.run_if(is_not_paused),
                    handle_infinite_mode_started,
                    tick_infinite_mode_chaos.run_if(is_not_paused),
                )
                    .in_set(OnUpdate(GameState::Main)),
            );
    }
}

pub fn spawn_night(
    mut commands: Commands,
    night_tracker: Res<NightTracker>,
    res: Res<ScreenResolution>,
) {
    info!("Spawning night overlay");
    commands
        .spawn(SpriteBundle {
            sprite: Sprite {
                color: overwrite_alpha(NIGHT, night_tracker.get_alpha()),
                custom_size: Some(Vec2::new(res.game_width, GAME_HEIGHT)),
                ..default()
            },
            transform: Transform {
                translation: Vec3::new(0., 0., 1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Night(Timer::from_seconds(10.5, TimerMode::Repeating)))
        .insert(Name::new("night"));
}

pub fn tick_night_color(
    time: Res<Time>,
    mut query: Query<(&mut Night, &mut Sprite)>,
    mut night_tracker: ResMut<NightTracker>,
    mut bgm_track_event: EventWriter<UpdateBGMTrackEvent>,
    bgm_tracker: Res<BGMPicker>,
    mut new_day_event: EventWriter<NewDayEvent>,
    infinite_mode: Res<InfiniteMode>,
) {
    // In infinite mode, keep it always night
    if infinite_mode.active {
        for (_night_state, mut sprite) in query.iter_mut() {
            sprite.color = overwrite_alpha(sprite.color, night_tracker.get_infinite_mode_alpha());
        }
        // Always play night music in infinite mode
        if bgm_tracker.current_track != *"sounds/bgm_night.ogg" {
            bgm_track_event.send(UpdateBGMTrackEvent {
                asset_path: "sounds/bgm_night.ogg".to_owned(),
            });
        }
        return;
    }

    let mut music_changed = false;
    for (mut night_state, mut sprite) in query.iter_mut() {
        night_state.0.tick(time.delta());
        if night_state.0.finished() {
            night_tracker.time += 1.;
            sprite.color = overwrite_alpha(sprite.color, night_tracker.get_alpha());
            if night_tracker.time == 24. {
                night_tracker.days += 1;
                night_tracker.time = 0.;
            }
            if night_tracker.is_start_of_new_day() && night_tracker.days > 0 {
                new_day_event.send_default();
            }
            music_changed = true;
        }
    }

    if music_changed || night_tracker.is_added() {
        // change music
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
}

/// Handle infinite mode started event - set up infinite mode state
pub fn handle_infinite_mode_started(
    mut events: EventReader<InfiniteModeStartedEvent>,
    mut infinite_mode: ResMut<InfiniteMode>,
) {
    for _ in events.iter() {
        if !infinite_mode.active {
            info!("INFINITE MODE ACTIVATED! Chaos will increase every 30 seconds.");
            *infinite_mode = InfiniteMode::new();
        }
    }
}

/// Tick the chaos timer in infinite mode and increase chaos every 30 seconds
pub fn tick_infinite_mode_chaos(
    time: Res<Time>,
    mut infinite_mode: ResMut<InfiniteMode>,
    mut chaos_event: EventWriter<IncreaseChaosEvent>,
) {
    if !infinite_mode.active {
        return;
    }

    infinite_mode.chaos_timer.tick(time.delta());
    if infinite_mode.chaos_timer.just_finished() {
        info!("Infinite mode: Increasing chaos by 1!");
        chaos_event.send(IncreaseChaosEvent { amount: 1.0 });
    }
}
