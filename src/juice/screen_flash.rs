use bevy::{prelude::*, render::view::RenderLayers};

use crate::{DEBUG_MODE, GAME_HEIGHT, GAME_WIDTH};

#[derive(Resource)]
pub struct FlashEffect {
    pub timer: Timer,
    pub color: Color,
}

#[derive(Component)]
pub struct ScreenFlash;

pub fn screen_flash_effect(
    mut commands: Commands,
    mut flash_state: ResMut<FlashEffect>,
    mut existing_flash: Query<(Entity, &mut Sprite), With<ScreenFlash>>,
    time: Res<Time>,
) {
    if let Ok((e, mut flash)) = existing_flash.get_single_mut() {
        if flash_state.timer.finished() {
            commands.entity(e).despawn_recursive();
            commands.remove_resource::<FlashEffect>();
            return;
        }
        flash.color.set_a(flash_state.timer.percent_left());
    } else {
        commands
            .spawn(SpriteBundle {
                sprite: Sprite {
                    color: flash_state.color,
                    custom_size: Some(Vec2::new(GAME_WIDTH + 10., GAME_HEIGHT + 10.)),
                    ..default()
                },
                transform: Transform {
                    translation: Vec3::new(0., 0., 10.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..default()
            })
            .insert(ScreenFlash)
            .insert(RenderLayers::from_layers(&[3]))
            .insert(Name::new("flash overlay"));
    }
    flash_state.timer.tick(time.delta());
}

pub fn test_flash(keys: Res<Input<KeyCode>>, mut commands: Commands) {
    if keys.just_pressed(KeyCode::G) && *DEBUG_MODE {
        commands.insert_resource(FlashEffect {
            timer: Timer::from_seconds(0.5, TimerMode::Once),
            color: Color::rgba(1., 1., 1., 1.),
        });
    }
}
