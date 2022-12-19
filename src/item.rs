use crate::assets::Graphics;
use crate::{Game, GameState, WORLD_SIZE};
use bevy::prelude::*;
use noise::{NoiseFn, Seedable, Simplex};
use rand::rngs::ThreadRng;
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
    StoneHalf,
    StoneFull,
    StoneTop,
    Water,
    Sand,
    Tree,
}

impl WorldObject {
    pub fn spawn(self, commands: &mut Commands, graphics: &Graphics, position: Vec3) -> Entity {
        // println!("I SPAWNED A TREE AT {:?}", position);
        let item_map = &graphics.item_map;
        if let None = item_map {
            panic!("graphics not loaded");
        }
        let sprite = graphics
            .item_map
            .as_ref()
            .unwrap()
            .get(&self)
            .expect(&format!("No graphic for object {:?}", self))
            .0
            .clone();

        let item = commands
            .spawn(SpriteSheetBundle {
                sprite,
                texture_atlas: graphics.texture_atlas.as_ref().unwrap().clone(),
                transform: Transform {
                    translation: position,
                    ..Default::default()
                },
                ..Default::default()
            })
            .insert(Name::new("GroundItem"))
            .insert(self)
            .id();
        return item;

        // if let Some(breakable) = self.as_breakable() {
        //     commands.entity(item).insert(breakable);
        // }

        // if let Some(pickup) = self.as_pickup() {
        //     commands.entity(item).insert(pickup);
        // }

        // if self.grows_into().is_some() {
        //     commands.entity(item).insert(GrowthTimer {
        //         timer: Timer::from_seconds(3.0, false),
        //     });
        // }
    }
    // pub fn as_breakable(&self) -> Option<Breakable> {
    //     match self {
    //         WorldObject::Grass => Some(Breakable {
    //             object: WorldObject::Grass,
    //             turnsInto: Some(WorldObject::Dirt),
    //         }),
    //         WorldObject::Stone => Some(Breakable {
    //             object: WorldObject::Stone,
    //             turnsInto: Some(WorldObject::Coal),
    //         }),
    //         _ => None,
    //     }
    // }
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
        let item_map = &&graphics.item_map;
        if let Some(item_map) = item_map {
            for (mut sprite, world_object) in to_update_query.iter_mut() {
                sprite.clone_from(
                    &item_map
                        .get(world_object)
                        .expect(&format!("No graphic for object {:?}", world_object))
                        .0,
                );
            }
        }
    }
}
