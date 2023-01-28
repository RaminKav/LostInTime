use std::{cmp::max, time::Duration};

use bevy::reflect::TypeUuid;
use bevy::render::render_resource::ShaderRef;
use bevy::sprite::{Material2d, Material2dPlugin};
use bevy::time::FixedTimestep;
use bevy::{prelude::*, render::render_resource::AsBindGroup};
use bevy_inspector_egui::{Inspectable, RegisterInspectable};

use crate::Limb;
use crate::{
    item::{Equipment, ItemStack},
    Game, GameState, Player, TIME_STEP,
};

pub struct AnimationsPlugin;

#[derive(Component, Inspectable)]
pub struct AnimationPosTracker(pub f32, pub f32, pub f32);

// #[derive(Component, Inspectable)]
// pub struct CameraOffsetTracker(Vec2, Vec2);

#[derive(Component, Inspectable)]
pub struct AnimationFrameTracker(pub i32, pub i32);

#[derive(Component, Deref, DerefMut)]
pub struct AnimationTimer(pub Timer);

#[derive(AsBindGroup, TypeUuid, Debug, Clone)]
#[uuid = "f690fdae-d598-45ab-8225-97e2a3f056f0"]
pub struct AnimatedTextureMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub source_texture: Option<Handle<Image>>,
    #[texture(2)]
    #[sampler(3)]
    pub lookup_texture: Option<Handle<Image>>,
    #[uniform(4)]
    pub flip: f32,
}

impl Material2d for AnimatedTextureMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/texture_map.wgsl".into()
    }

    // fn alpha_mode(&self) -> AlphaMode {
    //     self.alpha_mode
    // }
}

impl Plugin for AnimationsPlugin {
    fn build(&self, app: &mut App) {
        app.register_inspectable::<AnimationPosTracker>()
            .add_plugin(Material2dPlugin::<AnimatedTextureMaterial>::default())
            .add_system_set(
                SystemSet::on_update(GameState::Main)
                    .with_run_criteria(FixedTimestep::step(TIME_STEP as f64))
                    .with_system(Self::animate_sprite)
                    .with_system(Self::animate_dropped_items),
            );
    }
}

impl AnimationsPlugin {
    fn animate_sprite(
        time: Res<Time>,
        game: Res<Game>,
        asset_server: Res<AssetServer>,
        mut materials: ResMut<Assets<AnimatedTextureMaterial>>,
        mut player_query: Query<(&mut AnimationTimer, &Children), With<Player>>,
        mut limb_query: Query<(
            &mut AnimationFrameTracker,
            &Handle<AnimatedTextureMaterial>,
            &Limb,
        )>,
        // mut eq_query: Query<&mut Transform, With<Equipment>>,
    ) {
        for (mut timer, limb_children) in &mut player_query {
            let d = time.delta();
            timer.tick(if game.player.is_dashing {
                Duration::new(
                    (d.as_secs() as f32 * 4.) as u64,
                    (d.subsec_nanos() as f32 * 4.) as u32,
                )
            } else {
                d
            });
            // if timer.just_finished() && game.player.is_moving {
            for l in limb_children {
                if let Ok((mut tracker, limb_handle, limb)) = limb_query.get_mut(*l) {
                    let limb_material = materials.get_mut(limb_handle);
                    if let Some(mat) = limb_material {
                        if timer.just_finished() && game.player.is_moving {
                            tracker.0 = max((tracker.0 + 1) % (tracker.1 - 1), 0);
                        } else if !game.player.is_moving {
                            tracker.0 = 0;
                        }
                        mat.source_texture = Some(asset_server.load(format!(
                            "textures/player/player-run-down/player-{}-run-down-source-{}.png",
                            limb.to_string().to_lowercase(),
                            tracker.0
                        )));
                    }
                }
                // else if let Ok(mut t) = eq_query.get_mut(*l) {
                //     // t.translation.y = (t.translation.y + 1.) % 2.;
                //     // t.translation.x = (t.translation.y + 1.) % 2.;
                // }
            }

            // } else if !game.player.is_moving {
            //     for l in limb_children {
            //         if let Ok((mut limb_sprite, _)) = limb_query.get_mut(*l) {
            //             limb_sprite.index = 0
            //         } else if let Ok(mut t) = eq_query.get_mut(*l) {
            //             // t.translation.y = -1.;
            //             // t.translation.x = 0.;
            //         }
            //     }
            // }
        }
    }
    fn animate_dropped_items(
        time: Res<Time>,
        mut drop_query: Query<
            (
                &mut Transform,
                &mut AnimationTimer,
                &mut AnimationPosTracker,
            ),
            &ItemStack,
        >,
    ) {
        for (mut transform, mut timer, mut tracker) in &mut drop_query {
            let d = time.delta();
            let s = tracker.2;
            timer.tick(d);
            if timer.just_finished() {
                transform.translation.y += s;
                tracker.1 += s;

                if tracker.1 <= -2. || tracker.1 >= 2. {
                    tracker.2 *= -1.;
                }
            }
        }
    }
}
