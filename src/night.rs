use bevy::{prelude::*, render::view::RenderLayers};
use bevy_inspector_egui::quick::ResourceInspectorPlugin;

use crate::{
    colors::{overwrite_alpha, NIGHT},
    GameState, GAME_HEIGHT, GAME_WIDTH,
};

#[derive(Component)]
pub struct Night(Timer, f32);

#[derive(Default, Reflect, Resource, Debug)]
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
}

pub struct NightPlugin;

impl Plugin for NightPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(NightTracker::default())
            .register_type::<NightTracker>()
            .add_plugin(ResourceInspectorPlugin::<NightTracker>::default())
            .add_startup_system(spawn_night)
            .add_system(tick_night_color.in_set(OnUpdate(GameState::Main)));
    }
}

pub fn spawn_night(mut commands: Commands) {
    commands
        .spawn(SpriteBundle {
            sprite: Sprite {
                color: overwrite_alpha(NIGHT, 0.),
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
        .insert(Night(Timer::from_seconds(30., TimerMode::Repeating), 1.))
        .insert(Name::new("night"));
}

pub fn tick_night_color(
    time: Res<Time>,
    mut query: Query<(&mut Night, &mut Sprite)>,
    mut night_tracker: ResMut<NightTracker>,
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
    }
}
