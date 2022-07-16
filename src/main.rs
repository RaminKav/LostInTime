use assets::GameAssetsPlugin;
use bevy::{prelude::*, window::PresentMode};
use bevy_asset_loader::*;
use bevy_inspector_egui::{WorldInspectorParams, WorldInspectorPlugin};
use change_tile::ChangeTilePlugin;
use game_camera::GameCameraPlugin;
use item::ItemsPlugin;
use mouse::MousePlugin;
use player::PlayerPlugin;
mod assets;
mod change_tile;
mod game_camera;
mod item;
mod mouse;
mod player;

pub const HEIGHT: f32 = 900.;
pub const RESOLUTION: f32 = 16.0 / 9.0;

#[derive(Clone, Eq, PartialEq, Debug, Hash)]
pub enum GameState {
    Splash,
    Main,
}

#[derive(AssetCollection)]
struct ImageAssets {
    #[asset(path = "bevy_survival_sprites.png")]
    pub sprite_sheet: Handle<Image>,
}
fn main() {
    let mut app = App::new();
    AssetLoader::new(GameState::Splash)
        .continue_to_state(GameState::Main)
        .with_collection::<ImageAssets>()
        .build(&mut app);
    app.add_state(GameState::Splash)
        .insert_resource(ClearColor(Color::hex("000000").unwrap()))
        .insert_resource(WindowDescriptor {
            width: HEIGHT * RESOLUTION,
            height: HEIGHT,
            title: "DST clone".to_string(),
            present_mode: PresentMode::Fifo,
            resizable: false,
            ..Default::default()
        })
        .add_plugins(DefaultPlugins)
        .add_plugin(GameAssetsPlugin)
        .add_plugin(ItemsPlugin)
        .add_plugin(GameCameraPlugin)
        .add_plugin(MousePlugin)
        // .add_plugin(ChangeTilePlugin)
        .add_plugin(PlayerPlugin)
        .run();
}
