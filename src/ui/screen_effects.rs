use bevy::{
    prelude::*,
    reflect::TypeUuid,
    render::{
        mesh::MeshVertexBufferLayout,
        render_resource::{
            AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState,
            RenderPipelineDescriptor, ShaderRef, SpecializedMeshPipelineError,
        },
        view::RenderLayers,
    },
    sprite::{Material2d, Material2dKey, Mesh2dHandle},
};

use crate::{
    attributes::{hunger::Hunger, CurrentHealth, MaxHealth},
    player::Player,
    ScreenResolution, GAME_HEIGHT,
};
const BLEND_ADD: BlendState = BlendState {
    color: BlendComponent {
        src_factor: BlendFactor::SrcAlpha,
        dst_factor: BlendFactor::One,
        operation: BlendOperation::Add,
    },

    alpha: BlendComponent {
        src_factor: BlendFactor::SrcAlpha,
        dst_factor: BlendFactor::OneMinusSrcAlpha,
        operation: BlendOperation::Add,
    },
};
#[derive(Component)]
pub struct HealthScreenEffect;

#[derive(Component)]
pub struct HungerScreenEffect;

impl Material2d for ScreenEffectMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/screen_effect.wgsl".into()
    }

    fn specialize(
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayout,
        _key: Material2dKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if let Some(fragment) = &mut descriptor.fragment {
            if let Some(target_state) = &mut fragment.targets[0] {
                target_state.blend = Some(BLEND_ADD);
            }
        }

        Ok(())
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
    res: Res<ScreenResolution>,
) {
    let Ok((current_hp, max_hp, hunger)) = hp.get_single() else {
        return;
    };
    let hp_percent = current_hp.0 as f32 / max_hp.0 as f32;
    let hunger_percent = hunger.current as f32 / hunger.max as f32;

    let hp_handle = asset_server.load("ui/HealthScreenEffect.png");
    let hp_effect_material = materials.add(ScreenEffectMaterial {
        source_texture: Some(hp_handle),
        opacity: 1. - hp_percent,
    });
    let hunger_handle = asset_server.load("ui/HungerScreenEffect.png");
    let hunger_effect_material = materials.add(ScreenEffectMaterial {
        source_texture: Some(hunger_handle),
        opacity: 1. - hunger_percent,
    });
    commands.spawn((
        Mesh2dHandle::from(meshes.add(Mesh::from(shape::Quad {
            size: Vec2::new(res.game_width, GAME_HEIGHT),
            ..Default::default()
        }))),
        hp_effect_material.clone(),
        HealthScreenEffect,
        RenderLayers::from_layers(&[3]),
        Name::new("hp screen effect"),
        SpatialBundle::from_transform(Transform::from_xyz(0., 0., 1.)),
    ));
    commands.spawn((
        Mesh2dHandle::from(meshes.add(Mesh::from(shape::Quad {
            size: Vec2::new(res.game_width, GAME_HEIGHT),
            ..Default::default()
        }))),
        hunger_effect_material.clone(),
        HungerScreenEffect,
        RenderLayers::from_layers(&[3]),
        Name::new("hunger screen effect"),
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
    hunger_effect: Query<&Handle<ScreenEffectMaterial>, With<HungerScreenEffect>>,
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
    if let Ok(hunger_mat_handle) = hunger_effect.get_single() {
        if let Some(hunger_effect_material) = materials.get_mut(hunger_mat_handle) {
            let hunger_percent = hunger.current as f32 / hunger.max as f32;
            hunger_effect_material.opacity = 1. - hunger_percent;
        }
    }

    // let hunger_percent = hunger.current as f32 / hunger.max as f32;
}
