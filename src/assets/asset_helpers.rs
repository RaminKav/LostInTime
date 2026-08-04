use bevy::{camera::visibility::RenderLayers, prelude::*};
pub fn spawn_sprite(
    commands: &mut Commands,
    translation: Vec3,
    icon: Handle<Image>,
    render_layer: u8,
) -> Entity {
    commands
        .spawn((
            Sprite {
                image: icon,

                ..default()
            },
            Transform {
                translation,
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
        ))
        .insert(RenderLayers::from_layers(&[render_layer as usize]))
        .id()
}
