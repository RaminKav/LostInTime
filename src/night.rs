use bevy::{prelude::*, render::view::RenderLayers};
use bevy_inspector_egui::quick::ResourceInspectorPlugin;
use serde::{Deserialize, Serialize};

use crate::{
    audio::{BGMPicker, UpdateBGMTrackEvent},
    colors::{overwrite_alpha, NIGHT},
    world::dimension::dim_spawned,
    GameState, GAME_HEIGHT, GAME_WIDTH,
};

#[derive(Component)]
pub struct Night(Timer);

#[derive(Default, Reflect, Resource, Clone, Debug, Serialize, Deserialize)]
#[reflect(Resource)]
pub struct NightTracker {
    pub days: u8,
    pub time: f32,
}
impl NightTracker {
    pub fn get_alpha(&self) -> f32 {
        if self.time < 6. {
            return 0.;
        } else if self.time >= 6. && self.time < 18. {
            return (self.time - 6.) * 0.05833333;
        } else {
            return 0.7 - (self.time - 18.) * 0.11666666;
        }
    }
    pub fn is_night(&self) -> bool {
        self.time - 12. >= 0.
    }
    pub fn is_dawn(&self) -> bool {
        self.time == 0.
    }
}

pub struct NightPlugin;

#[derive(Default)]
pub struct NewDayEvent;

impl Plugin for NightPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(NightTracker::default())
            .register_type::<NightTracker>()
            .add_event::<NewDayEvent>()
            .add_plugin(ResourceInspectorPlugin::<NightTracker>::default().run_if(dim_spawned))
            .add_system(spawn_night.in_schedule(OnEnter(GameState::Main)))
            .add_system(tick_night_color.in_set(OnUpdate(GameState::Main)));
    }
}

pub fn spawn_night(mut commands: Commands, night_tracker: Res<NightTracker>) {
    commands
        .spawn(SpriteBundle {
            sprite: Sprite {
                color: overwrite_alpha(NIGHT, night_tracker.get_alpha()),
                custom_size: Some(Vec2::new(GAME_WIDTH, GAME_HEIGHT)),
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
        .insert(Night(Timer::from_seconds(20., TimerMode::Repeating)))
        .insert(Name::new("night"));
}

pub fn tick_night_color(
    time: Res<Time>,
    mut query: Query<(&mut Night, &mut Sprite)>,
    mut night_tracker: ResMut<NightTracker>,
    mut bgm_track_event: EventWriter<UpdateBGMTrackEvent>,
    bgm_tracker: Res<BGMPicker>,
    mut new_day_event: EventWriter<NewDayEvent>,
) {
    for (mut night_state, mut sprite) in query.iter_mut() {
        night_state.0.tick(time.delta());
        if night_state.0.finished() {
            night_tracker.time += 1.;
            sprite.color = overwrite_alpha(sprite.color, night_tracker.get_alpha());
            if night_tracker.time == 24. {
                night_tracker.days += 1;
                night_tracker.time = 0.;
            }
        }
        // change music
        if night_tracker.is_night()
            && bgm_tracker.current_track != "sounds/bgm_night.ogg".to_owned()
        {
            bgm_track_event.send(UpdateBGMTrackEvent {
                asset_path: "sounds/bgm_night.ogg".to_owned(),
            });
        } else if !night_tracker.is_night()
            && bgm_tracker.current_track != "sounds/bgm_day.ogg".to_owned()
        {
            bgm_track_event.send(UpdateBGMTrackEvent {
                asset_path: "sounds/bgm_day.ogg".to_owned(),
            });
        }

        if night_tracker.is_dawn() && night_tracker.days > 0 {
            new_day_event.send_default();
        }
    }
}
