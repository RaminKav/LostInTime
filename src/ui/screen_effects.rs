use bevy::{
    camera::visibility::RenderLayers,
    math::primitives::Rectangle,
    mesh::Mesh2d,
    prelude::*,
    render::{
        mesh::MeshVertexBufferLayoutRef,
        render_resource::{
            AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState,
            RenderPipelineDescriptor, SpecializedMeshPipelineError,
        },
    },
    shader::ShaderRef,
    sprite_render::{AlphaMode2d, Material2d, Material2dKey, MeshMaterial2d},
};

use crate::{attributes::hunger::Hunger, player::Player, ScreenResolution};

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

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }

    fn specialize(
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
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

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
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

    let Ok(hunger) = player_stats.single() else {
        return;
    };
    let hunger_percent = (hunger.current as f32 / hunger.max as f32).clamp(0.0, 1.0);

    let hunger_handle = asset_server.load("ui/HungerScreenEffect.png");
    let hunger_effect_material = materials.add(ScreenEffectMaterial {
        source_texture: Some(hunger_handle),
        opacity: (1.0 - hunger_percent).clamp(0.0, 1.0),
    });
    commands.spawn((
        Mesh2d(meshes.add(Mesh::from(Rectangle::new(res.game_width, res.game_height)))),
        MeshMaterial2d(hunger_effect_material.clone()),
        HungerScreenEffect,
        RenderLayers::from_layers(&[3]),
        Name::new("hunger screen effect"),
        (Transform::from_xyz(0., 0., 1.), Visibility::default()),
    ));
}

pub fn handle_screen_effects(
    player_stats: Query<&Hunger, With<Player>>,
    mut materials: ResMut<Assets<ScreenEffectMaterial>>,
    hunger_effect: Query<&MeshMaterial2d<ScreenEffectMaterial>, With<HungerScreenEffect>>,
) {
    let Ok(hunger) = player_stats.single() else {
        return;
    };
    if let Ok(hunger_mat_handle) = hunger_effect.single() {
        if let Some(mut hunger_effect_material) = materials.get_mut(&hunger_mat_handle.0) {
            let hunger_percent = (hunger.current as f32 / hunger.max as f32).clamp(0.0, 1.0);
            hunger_effect_material.opacity = (1.0 - hunger_percent).clamp(0.0, 1.0);
        }
    }
}
