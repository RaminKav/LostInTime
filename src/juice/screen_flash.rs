use bevy::{camera::visibility::RenderLayers, prelude::*};

use crate::{ScreenResolution, DEBUG};

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
    resolution: Res<ScreenResolution>,
) {
    if let Ok((e, mut flash)) = existing_flash.single_mut() {
        if flash_state.timer.is_finished() {
            commands.entity(e).despawn();
            commands.remove_resource::<FlashEffect>();
            return;
        }
        flash.color = flash
            .color
            .with_alpha(flash_state.timer.fraction_remaining());
    } else {
        commands
            .spawn((
                Sprite {
                    color: flash_state.color,
                    custom_size: Some(crate::ui::ui_helpers::full_screen_overlay_size(&resolution)),
                    ..default()
                },
                Transform {
                    translation: Vec3::new(0., 0., 10.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
            ))
            .insert(ScreenFlash)
            .insert(RenderLayers::from_layers(&[3]))
            .insert(Name::new("flash overlay"));
    }
    flash_state.timer.tick(time.delta());
}

pub fn test_flash(keys: Res<ButtonInput<KeyCode>>, mut commands: Commands) {
    if keys.just_pressed(KeyCode::KeyG) && *DEBUG {
        commands.insert_resource(FlashEffect {
            timer: Timer::from_seconds(0.5, TimerMode::Once),
            color: Color::srgba(1., 1., 1., 1.),
        });
    }
}
