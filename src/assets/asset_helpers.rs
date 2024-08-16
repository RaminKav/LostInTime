use bevy::{prelude::*, render::view::RenderLayers};
pub fn spawn_sprite(
    commands: &mut Commands,
    translation: Vec3,
    icon: Handle<Image>,
    render_layer: u8,
) -> Entity {
    commands
        .spawn(SpriteBundle {
            texture: icon,
            sprite: Sprite {
                ..Default::default()
            },
            transform: Transform {
                translation,
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[render_layer]))
        .id()
}
