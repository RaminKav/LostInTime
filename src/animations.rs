use std::time::Duration;

use bevy::prelude::*;
use bevy::time::FixedTimestep;
use bevy_inspector_egui::{Inspectable, RegisterInspectable};

use crate::{item::ItemStack, AnimationTimer, Game, GameState, Player, TIME_STEP};

pub struct AnimationsPlugin;

#[derive(Component, Inspectable)]
pub struct AnimationPosTracker(pub f32, pub f32, pub f32);

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
        mut query: Query<
            (
                &mut AnimationTimer,
                &mut TextureAtlasSprite,
                &Handle<TextureAtlas>,
            ),
            With<Player>,
        >,
    ) {
        for (mut timer, mut sprite, handle) in &mut query {
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
                let texture_atlas = texture_atlases.get(handle).unwrap();
                sprite.index = (sprite.index + 1) % texture_atlas.textures.len();
            } else if !game.player.is_moving {
                sprite.index = 0
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
