use crate::assets::Graphics;
use crate::GameState;
use bevy::prelude::*;
use bevy_inspector_egui::{Inspectable, RegisterInspectable};
use serde::Deserialize;

#[derive(Component, Inspectable)]
pub struct Pickupable {
    pub(crate) item: ItemType,
}

#[derive(Component, Inspectable)]
pub struct Harvestable {
    pub(crate) item: ItemType,
    pub(crate) tool_required: Option<Tool>,
    pub(crate) drops: Option<WorldObject>,
}

#[derive(Component, Inspectable)]
pub struct Breakable {
    object: WorldObject,
    pub turnsInto: Option<WorldObject>,
}

/// The core enum of the game, lists everything that can be held or placed in the game
#[derive(Debug, Inspectable, PartialEq, Eq, Clone, Copy, Hash, Deserialize, Component)]
pub enum WorldObject {
    Item(ItemType),
    Tree,
    Stump,
    Sapling,
    DeadSapling,
    Grass,
    PluckedGrass,
    GrowingTree,
    CampFire,
    GroundGreen,
    GroundBrown,
}

/// Everything that can be in the players inventory
#[derive(Inspectable, Debug, PartialEq, Eq, Clone, Copy, Hash, Deserialize)]
pub enum ItemType {
    None,
    Tool(Tool),
    Flint,
    Twig,
    Grass,
    Wood,
}

/// Everything the player can equip
#[derive(Inspectable, Debug, PartialEq, Eq, Clone, Copy, Hash, Deserialize)]
pub enum Tool {
    Axe,
    Shovel,
}

impl ItemType {
    #[allow(dead_code)]
    pub fn name(self) -> String {
        match self {
            ItemType::Tool(tool) => format!("{:?}", tool),
            _ => format!("{:?}", self),
        }
    }
}

impl WorldObject {
    pub fn spawn(self, commands: &mut Commands, graphics: &Graphics, position: Vec2) -> Entity {
        let sprite = graphics
            .item_map
            .get(&self)
            .expect(&format!("No graphic for object {:?}", self))
            .clone();

        let item = commands
            .spawn_bundle(SpriteSheetBundle {
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
            WorldObject::GroundGreen => Some(Breakable {
                object: WorldObject::GroundGreen,
                turnsInto: Some(WorldObject::GroundBrown),
            }),
            WorldObject::GroundBrown => Some(Breakable {
                object: WorldObject::GroundBrown,
                turnsInto: Some(WorldObject::GroundGreen),
            }),
            _ => None,
        }
    }

    // pub fn grow(
    //     self,
    //     commands: &mut Commands,
    //     graphics: &Graphics,
    //     ent: Entity,
    //     transform: &Transform,
    // ) -> Entity {
    //     if let Some(new_object) = self.grows_into() {
    //         commands.entity(ent).despawn_recursive();
    //         new_object.spawn(commands, graphics, transform.translation.truncate())
    //         //println!("{:?} grew into a beautiful {:?}", self, self.grows_into());
    //     } else {
    //         ent
    //     }
    // }

    // /// TODO it would be great to describe this outside of code, in a config or something
    // pub fn grows_into(&self) -> Option<WorldObject> {
    //     match self {
    //         WorldObject::DeadSapling => Some(WorldObject::Sapling),
    //         WorldObject::PluckedGrass => Some(WorldObject::Grass),
    //         WorldObject::GrowingTree => Some(WorldObject::Tree),
    //         _ => None,
    //     }
    // }

    // /// TODO it would be great to describe this outside of code, in a config or something
    // pub fn as_harvest(&self) -> Option<Harvestable> {
    //     match self {
    //         WorldObject::Sapling => Some(Harvestable {
    //             item: ItemType::Twig,
    //             tool_required: None,
    //             drops: Some(WorldObject::DeadSapling),
    //         }),
    //         WorldObject::Grass => Some(Harvestable {
    //             item: ItemType::Grass,
    //             tool_required: None,
    //             drops: Some(WorldObject::PluckedGrass),
    //         }),
    //         WorldObject::Tree => Some(Harvestable {
    //             item: ItemType::Wood,
    //             tool_required: Some(Tool::Axe),
    //             drops: Some(WorldObject::Stump),
    //         }),
    //         _ => None,
    //     }
    // }

    // pub fn as_pickup(&self) -> Option<Pickupable> {
    //     if self.as_harvest().is_some() {
    //         return None;
    //     }
    //     match self {
    //         WorldObject::Item(item) => Some(Pickupable { item: *item }),
    //         _ => None,
    //     }
    // }
}

impl Default for WorldObject {
    fn default() -> Self {
        WorldObject::Item(ItemType::None)
    }
}

impl Default for Tool {
    fn default() -> Self {
        Tool::Axe
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
        //FIXME I don't think this is working...
        // if cfg!(debug_assertions) {
        //     app.register_type::<GrowthTimer>()
        //         .register_inspectable::<WorldObject>()
        //         .register_inspectable::<ItemAndCount>()
        //         .register_inspectable::<Pickupable>();
        // }
    }
}

// #[derive(Component, Reflect, Default)]
// #[reflect(Component)]
// pub struct GrowthTimer {
//     timer: Timer,
// }

impl ItemsPlugin {
    /// Ticks the timers for everything in the world that can regrow and calls grow on them
    // fn world_object_growth(
    //     mut commands: Commands,
    //     time: Res<Time>,
    //     graphics: Res<Graphics>,
    //     mut growable_query: Query<(Entity, &Transform, &WorldObject, Option<&mut GrowthTimer>)>,
    // ) {
    //     for (ent, transform, world_object, regrowth_timer) in growable_query.iter_mut() {
    //         if let Some(mut timer) = regrowth_timer {
    //             timer.timer.tick(time.delta());
    //             if !timer.timer.finished() {
    //                 continue;
    //             }

    //             world_object.grow(&mut commands, &graphics, ent, transform);
    //         }
    //     }
    // }

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
    fn spawn_test_objects(mut commands: Commands, graphics: Res<Graphics>) {
        println!("YEAAAAAAAA");
        let mut children = Vec::new();
        for i in -20..20 {
            for j in -9..10 {
                children.push(WorldObject::GroundGreen.spawn(
                    &mut commands,
                    &graphics,
                    Vec2::new((i as f32) * 0.5, (j as f32) * 0.5),
                ));
            }
        }
        // children.push(WorldObject::Sapling.spawn(&mut commands, &graphics, Vec2::new(-1., 3.)));
        // children.push(WorldObject::Sapling.spawn(&mut commands, &graphics, Vec2::new(-1., 1.)));

        // children.push(WorldObject::Grass.spawn(&mut commands, &graphics, Vec2::new(3., -3.)));
        // children.push(WorldObject::Grass.spawn(&mut commands, &graphics, Vec2::new(3., -1.)));
        // children.push(WorldObject::Grass.spawn(&mut commands, &graphics, Vec2::new(1., -3.)));
        // children.push(WorldObject::Grass.spawn(&mut commands, &graphics, Vec2::new(1., -1.)));

        // children.push(WorldObject::Tree.spawn(&mut commands, &graphics, Vec2::new(-3., -3.)));
        // children.push(WorldObject::Tree.spawn(&mut commands, &graphics, Vec2::new(-3., -1.)));
        // children.push(WorldObject::Tree.spawn(&mut commands, &graphics, Vec2::new(-1., -3.)));
        // children.push(WorldObject::Tree.spawn(&mut commands, &graphics, Vec2::new(-1., -1.)));

        // children.push(WorldObject::Item(ItemType::Flint).spawn(
        //     &mut commands,
        //     &graphics,
        //     Vec2::new(3., 3.),
        // ));
        // children.push(WorldObject::Item(ItemType::Flint).spawn(
        //     &mut commands,
        //     &graphics,
        //     Vec2::new(3., 1.),
        // ));
        // children.push(WorldObject::Item(ItemType::Flint).spawn(
        //     &mut commands,
        //     &graphics,
        //     Vec2::new(1., 3.),
        // ));
        // children.push(WorldObject::Item(ItemType::Flint).spawn(
        //     &mut commands,
        //     &graphics,
        //     Vec2::new(1., 1.),
        // ));
        commands
            .spawn_bundle(TransformBundle::default())
            .insert(Name::new("Test Objects"))
            .push_children(&children);
    }
}

impl Default for ItemType {
    fn default() -> Self {
        ItemType::None
    }
}

#[derive(Clone, Copy, Default, Inspectable, Deserialize, Debug, PartialEq)]
pub struct ItemAndCount {
    pub item: ItemType,
    pub count: usize,
}

// impl std::fmt::Display for ItemAndCount {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         write!(f, "{}x {:?}", self.count, self.item)
//     }
// }
