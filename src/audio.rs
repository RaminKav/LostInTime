use bevy::prelude::*;

#[derive(Component)]
pub struct HitSound;

pub fn setup_audio(_commands: Commands, _asset_server: Res<AssetServer>) {
    // commands.spawn((
    //     AudioBundle {
    //         source: asset_server.load("sounds/Windless Slopes.ogg"),
    //         ..default()
    //     },
    //     HitSound,
    // ));
}
