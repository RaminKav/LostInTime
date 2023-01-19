use std::{cmp::max, time::Duration};

use bevy::prelude::*;
use bevy::time::FixedTimestep;
use bevy_inspector_egui::{Inspectable, RegisterInspectable};

use crate::{
    item::{Equipment, ItemStack},
    Game, GameState, Player, TIME_STEP,
};

pub struct AnimationsPlugin;

#[derive(Component, Inspectable)]
pub struct AnimationPosTracker(pub f32, pub f32, pub f32);

#[derive(Component, Deref, DerefMut)]
pub struct AnimationTimer(pub Timer);

impl Plugin for AnimationsPlugin {
    fn build(&self, app: &mut App) {
        app.register_inspectable::<AnimationPosTracker>()
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
        texture_atlases: Res<Assets<TextureAtlas>>,
        game: Res<Game>,
        mut query: Query<(&mut AnimationTimer, &Children), With<Player>>,
        mut limb_query: Query<(&mut TextureAtlasSprite, &Handle<TextureAtlas>), Without<Equipment>>,
    ) {
        for (mut timer, limb_children) in &mut query {
            let d = time.delta();
            timer.tick(if game.player.is_dashing {
                Duration::new(
                    (d.as_secs() as f32 * 4.) as u64,
                    (d.subsec_nanos() as f32 * 4.) as u32,
                )
            } else {
                d
            });
            if timer.just_finished() && game.player.is_moving {
                for l in limb_children {
                    if let Ok((mut limb_sprite, limb_handle)) = limb_query.get_mut(*l) {
                        let texture_atlas = texture_atlases.get(limb_handle).unwrap();
                        if texture_atlas.textures.len() > 1 {
                            limb_sprite.index =
                                max((limb_sprite.index + 1) % texture_atlas.textures.len(), 1);
                        }
                    }
                }
            } else if !game.player.is_moving {
                for l in limb_children {
                    if let Ok((mut limb_sprite, _)) = limb_query.get_mut(*l) {
                        limb_sprite.index = 0
                    }
                }
            }
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
