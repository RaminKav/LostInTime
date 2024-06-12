use bevy::{
    prelude::*,
    reflect::TypeUuid,
    render::{
        render_resource::{AsBindGroup, ShaderRef},
        view::RenderLayers,
    },
    sprite::{Material2d, Mesh2dHandle},
};

use crate::{
    attributes::{hunger::Hunger, CurrentHealth, MaxHealth},
    player::Player,
    GAME_HEIGHT, GAME_WIDTH,
};

#[derive(Component)]
pub struct HealthScreenEffect;

#[derive(Component)]
pub struct HungerScreenEffect;

impl Material2d for ScreenEffectMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/screen_effect.wgsl".into()
    }
}

#[derive(AsBindGroup, TypeUuid, Debug, Clone)]
#[uuid = "3e77336e-4012-4d79-b559-7267288b4d16"]
pub struct ScreenEffectMaterial {
    #[uniform(0)]
    pub opacity: f32,
    #[texture(1)]
    #[sampler(2)]
    pub source_texture: Option<Handle<Image>>,
}

pub fn setup_screen_effects(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ScreenEffectMaterial>>,
    hp: Query<(&CurrentHealth, &MaxHealth, &Hunger), (Added<CurrentHealth>, With<Player>)>,
) {
    let Ok((current_hp, max_hp, hunger)) = hp.get_single() else {
        return;
    };
    let hp_percent = current_hp.0 as f32 / max_hp.0 as f32;
    let hunger_percent = hunger.current as f32 / hunger.max as f32;

    let handle = asset_server.load("ui/HealthScreenEffect.png");
    let effect_material = materials.add(ScreenEffectMaterial {
        source_texture: Some(handle),
        opacity: 1. - hp_percent,
    });
    commands.spawn((
        Mesh2dHandle::from(meshes.add(Mesh::from(shape::Quad {
            size: Vec2::new(GAME_WIDTH, GAME_HEIGHT),
            ..Default::default()
        }))),
        effect_material.clone(),
        HealthScreenEffect,
        RenderLayers::from_layers(&[3]),
        Name::new("hp screen effect"),
        SpatialBundle::from_transform(Transform::from_xyz(0., 0., 1.)),
    ));
}

pub fn handle_add_screen_effects(
    hp: Query<
        (&CurrentHealth, &MaxHealth, &Hunger),
        (With<Player>, Or<(Changed<CurrentHealth>, Changed<Hunger>)>),
    >,
    mut materials: ResMut<Assets<ScreenEffectMaterial>>,
    hp_effect: Query<&Handle<ScreenEffectMaterial>, With<HealthScreenEffect>>,
    // hunger_effect: Query<&mut ScreenEffectMaterial, With<HungerScreenEffect>>,
) {
    let Ok((current_hp, max_hp, hunger)) = hp.get_single() else {
        return;
    };
    if let Ok(hp_mat_handle) = hp_effect.get_single() {
        if let Some(hp_effect_material) = materials.get_mut(hp_mat_handle) {
            let hp_percent = current_hp.0 as f32 / max_hp.0 as f32;
            hp_effect_material.opacity = 1. - hp_percent;
        }
    }

    // let hunger_percent = hunger.current as f32 / hunger.max as f32;
}
