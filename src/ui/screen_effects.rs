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
    attributes::hunger::Hunger,
    player::Player,
    ScreenResolution,
};

const SCREEN_BLEND: BlendState = BlendState {
    color: BlendComponent {
        src_factor: BlendFactor::SrcAlpha,
        dst_factor: BlendFactor::OneMinusSrcAlpha,
        operation: BlendOperation::Add,
    },
    alpha: BlendComponent {
        src_factor: BlendFactor::One,
        dst_factor: BlendFactor::OneMinusSrcAlpha,
        operation: BlendOperation::Add,
    },
};
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
                target_state.blend = Some(SCREEN_BLEND);
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
    player_stats: Query<&Hunger, With<Player>>,
    existing_hunger_effect: Query<Entity, With<HungerScreenEffect>>,
    res: Res<ScreenResolution>,
) {
    if existing_hunger_effect.iter().next().is_some() {
        return;
    }

    let Ok(hunger) = player_stats.get_single() else {
        return;
    };
    let hunger_percent = (hunger.current as f32 / hunger.max as f32).clamp(0.0, 1.0);

    let hunger_handle = asset_server.load("ui/HungerScreenEffect.png");
    let hunger_effect_material = materials.add(ScreenEffectMaterial {
        source_texture: Some(hunger_handle),
        opacity: (1.0 - hunger_percent).clamp(0.0, 1.0),
    });
    commands.spawn((
        Mesh2dHandle::from(meshes.add(Mesh::from(shape::Quad {
            size: Vec2::new(res.game_width, res.game_height),
            ..Default::default()
        }))),
        hunger_effect_material.clone(),
        HungerScreenEffect,
        RenderLayers::from_layers(&[3]),
        Name::new("hunger screen effect"),
        SpatialBundle::from_transform(Transform::from_xyz(0., 0., 1.)),
    ));
}

pub fn handle_screen_effects(
    player_stats: Query<&Hunger, With<Player>>,
    mut materials: ResMut<Assets<ScreenEffectMaterial>>,
    hunger_effect: Query<&Handle<ScreenEffectMaterial>, With<HungerScreenEffect>>,
) {
    let Ok(hunger) = player_stats.get_single() else {
        return;
    };
    if let Ok(hunger_mat_handle) = hunger_effect.get_single() {
        if let Some(hunger_effect_material) = materials.get_mut(hunger_mat_handle) {
            let hunger_percent = (hunger.current as f32 / hunger.max as f32).clamp(0.0, 1.0);
            hunger_effect_material.opacity = (1.0 - hunger_percent).clamp(0.0, 1.0);
        }
    }
}
