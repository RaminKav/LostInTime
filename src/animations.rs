use std::time::Duration;

use bevy::prelude::*;
use bevy::time::FixedTimestep;

use crate::{AnimationTimer, Game, GameState, TIME_STEP};

pub struct AnimationsPlugin;

impl Plugin for AnimationsPlugin {
    fn build(&self, app: &mut App) {
        app.add_system_set(
            SystemSet::on_update(GameState::Main)
                .with_run_criteria(FixedTimestep::step(TIME_STEP as f64))
                .with_system(Self::animate_sprite),
        );
    }
}

impl AnimationsPlugin {
    fn animate_sprite(
        time: Res<Time>,
        texture_atlases: Res<Assets<TextureAtlas>>,
        game: Res<Game>,
        mut query: Query<(
            &mut AnimationTimer,
            &mut TextureAtlasSprite,
            &Handle<TextureAtlas>,
        )>,
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
}
