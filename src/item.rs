use std::ops::Mul;

use crate::assets::Graphics;
use crate::{Game, GameState};
use bevy::prelude::*;
use rand::Rng;
use serde::Deserialize;

#[derive(Component)]
pub struct Breakable {
    object: WorldObject,
    pub turnsInto: Option<WorldObject>,
}

/// The core enum of the game, lists everything that can be held or placed in the game
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, Deserialize, Component)]
pub enum WorldObject {
    None,
    Grass,
    Dirt,
    Stone,
    Coal,
}

impl WorldObject {
    pub fn spawn(self, commands: &mut Commands, graphics: &Graphics, position: Vec2) -> Entity {
        let sprite = graphics
            .item_map
            .get(&self)
            .expect(&format!("No graphic for object {:?}", self))
            .clone();

        let item = commands
            .spawn(SpriteSheetBundle {
                sprite,
                texture_atlas: graphics.texture_atlas.clone(),
                transform: Transform {
                    translation: position.extend(0.0),
                    ..Default::default()
                },
                ..Default::default()
            })
            .insert(Name::new("GroundItem"))
            .insert(self)
            .id();

        if let Some(breakable) = self.as_breakable() {
            commands.entity(item).insert(breakable);
        }

        // if let Some(pickup) = self.as_pickup() {
        //     commands.entity(item).insert(pickup);
        // }

        // if self.grows_into().is_some() {
        //     commands.entity(item).insert(GrowthTimer {
        //         timer: Timer::from_seconds(3.0, false),
        //     });
        // }

        item
    }
    pub fn as_breakable(&self) -> Option<Breakable> {
        match self {
            WorldObject::Grass => Some(Breakable {
                object: WorldObject::Grass,
                turnsInto: Some(WorldObject::Dirt),
            }),
            WorldObject::Stone => Some(Breakable {
                object: WorldObject::Stone,
                turnsInto: Some(WorldObject::Coal),
            }),
            _ => None,
        }
    }
}

impl Default for WorldObject {
    fn default() -> Self {
        WorldObject::None
    }
}

pub struct ItemsPlugin;

impl Plugin for ItemsPlugin {
    fn build(&self, app: &mut App) {
        app.add_system_set(
            SystemSet::on_enter(GameState::Main)
                .with_system(Self::spawn_test_objects.after("graphics")),
        )
        .add_system_set(
            SystemSet::on_update(GameState::Main).with_system(Self::update_graphics),
            // .with_system(Self::world_object_growth),
        );
    }
}

impl ItemsPlugin {
    /// Keeps the graphics up to date for things that are harvested or grown
    fn update_graphics(
        mut to_update_query: Query<(&mut TextureAtlasSprite, &WorldObject), Changed<WorldObject>>,
        graphics: Res<Graphics>,
    ) {
        for (mut sprite, world_object) in to_update_query.iter_mut() {
            sprite.clone_from(
                graphics
                    .item_map
                    .get(world_object)
                    .expect(&format!("No graphic for object {:?}", world_object)),
            );
        }
    }

    // Creates our testing map
    #[allow(clippy::vec_init_then_push)]
    fn spawn_test_objects(mut commands: Commands, graphics: Res<Graphics>, game: Res<Game>) {
        let mut children = Vec::new();
        let map_size = game.world_size / 2; //100
        let mut rng = rand::thread_rng();
        for i in -map_size..map_size {
            for j in -map_size..map_size {
                let block = if rng.gen::<f64>() >= 0.3 {
                    WorldObject::Grass
                } else {
                    WorldObject::Dirt
                };
                children.push(block.spawn(
                    &mut commands,
                    &graphics,
                    Vec2::new((i as f32) * 0.5, (j as f32) * 0.5),
                ));
            }
        }

        commands
            .spawn(SpatialBundle::default())
            // .insert(Name::new("Test Objects"))
            .push_children(&children);
    }
}
