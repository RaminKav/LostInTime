use std::fs;

use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::utils::HashMap;
use serde::Deserialize;

use crate::item::{WorldObject, WorldObjectData, WorldObjectResource};
use crate::{GameState, ImageAssets};
use ron::de::from_str;

// use ron::de::from_str;

// pub const PIXEL_SCALE: f32 = 3.;
pub const SOURCE_TILE_SIZE: f32 = 32.;
pub const WORLD_SCALE: f32 = 3.5; //SOURCE_TILE_SIZE * PIXEL_SCALE;

pub struct GameAssetsPlugin;

/// Used to describe the location and styling of sprites on the sprite sheet
#[derive(Default, Clone, Copy, Debug, Deserialize)]
pub struct MyRect {
    pub pos: (f32, f32),
    pub size: (f32, f32),
    pub anchor: Option<Vec2>,
    pub collider: bool,
    pub breakable: bool,
    pub breaks_into: Option<WorldObject>,
    pub equip_slot: Option<usize>,
}

impl MyRect {
    pub fn new(pos: (f32, f32), size: (f32, f32)) -> Self {
        Self {
            pos,
            size,
            anchor: None,
            collider: false,
            breakable: false,
            breaks_into: None,
            equip_slot: None,
        }
    }

    pub fn to_atlas_rect(self) -> bevy::math::Rect {
        bevy::math::Rect {
            //A tiny amount is clipped off the sides of the rectangle
            //to stop contents of other sprites from bleeding through
            min: Vec2::new(self.pos.0 + 0.15, self.pos.1 + 0.15),
            max: Vec2::new(
                self.pos.0 + self.size.0 - 0.15,
                self.pos.1 + self.size.1 - 0.15,
            ),
        }
    }
}

/// Loaded from sprites_desc.ron and contains the description of every sprite in the game
#[derive(Deserialize)]
pub struct GraphicsDesc {
    map: HashMap<WorldObject, MyRect>,
}

impl Plugin for GameAssetsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Graphics {
            texture_atlas: None,
            item_map: None,
        })
        .add_system_set(
            SystemSet::on_exit(GameState::Loading)
                .with_system(Self::load_graphics.label("graphics")),
        );
    }
}

/// The great Graphics Resource, used by everything that needs to create sprites
/// Contains an image map which is the work around for UI not supporting texture atlas sprites
#[derive(Resource)]

pub struct Graphics {
    pub texture_atlas: Option<Handle<TextureAtlas>>,
    pub item_map: Option<HashMap<WorldObject, (TextureAtlasSprite, usize)>>,
}

/// Work around helper function to convert texture atlas sprites into stand alone image handles
/// Copies sprite data pixel by pixel, needed to render things in UI
// fn convert_to_image(
//     sprite_desc: MyRect,
//     original_image: Handle<Image>,
//     assets: &mut ResMut<Assets<Image>>,
// ) -> Handle<Image> {
//     //TODO convert if mismatch
//     let original_image = assets.get(original_image).unwrap();
//     assert!(original_image.texture_descriptor.format == TextureFormat::Rgba8UnormSrgb);

//     let mut data = Vec::default();
//     //Every pixel is 4 entries in image.data
//     let mut starting_index =
//         (sprite_desc.pos.0 + original_image.size().x * sprite_desc.pos.1) as usize;
//     for _y in 0..sprite_desc.size.1 as usize {
//         for x in 0..sprite_desc.size.0 as usize {
//             let index = starting_index + x;
//             //Copy 1 pixel at index
//             data.push(original_image.data[index * 4]);
//             data.push(original_image.data[index * 4 + 1]);
//             data.push(original_image.data[index * 4 + 2]);
//             data.push(original_image.data[index * 4 + 3]);
//         }
//         starting_index += original_image.size().y as usize;
//     }

//     let size = Extent3d {
//         width: sprite_desc.size.0 as u32,
//         height: sprite_desc.size.1 as u32,
//         depth_or_array_layers: 1,
//     };
//     let image = Image::new(
//         size,
//         TextureDimension::D2,
//         data,
//         //FIXME
//         TextureFormat::Rgba8UnormSrgb,
//     );
//     assets.add(image)
// }

impl GameAssetsPlugin {
    /// Startup system that runs after images are loaded, indexes all loaded images
    /// and creates the graphics resource
    pub fn load_graphics(
        mut graphics: ResMut<Graphics>,
        sprite_sheet: Res<ImageAssets>,
        mut texture_assets: ResMut<Assets<TextureAtlas>>,
        mut world_obj_data: ResMut<WorldObjectResource>,
    ) {
        //let image_handle = assets.load("bevy_survival_sprites.png");
        let image_handle = sprite_sheet.sprite_sheet.clone();
        let sprite_desc = fs::read_to_string("assets/textures/sprites_desc.ron").unwrap();

        let sprite_desc: GraphicsDesc = from_str(&sprite_desc).unwrap_or_else(|e| {
            println!("Failed to load config for graphics: {}", e);
            std::process::exit(1);
        });

        let mut atlas = TextureAtlas::new_empty(image_handle.clone(), Vec2::new(256., 32.));

        let mut item_map = HashMap::default();

        for (item, rect) in sprite_desc.map.iter() {
            println!("Found graphic {:?}", item);
            let mut sprite = TextureAtlasSprite::new(atlas.add_texture(rect.to_atlas_rect()));

            //Set the size to be proportional to the source rectangle
            sprite.custom_size = Some(Vec2::new(rect.size.0, rect.size.1));

            //Position the sprite anchor if one is defined
            // if let Some(anchor) = rect.anchor {
            //     sprite.anchor = Anchor::Custom(Vec2::new(
            //         anchor.0 / rect.size.0 - 0.5,
            //         0.5 - anchor.1 / rect.size.1,
            //     ));
            // };
            world_obj_data.data.insert(
                *item,
                WorldObjectData {
                    size: Vec2::new(rect.size.0, rect.size.1),
                    anchor: rect.anchor,
                    collider: rect.collider,
                    breakable: rect.breakable,
                    breaks_into: rect.breaks_into,
                    equip_slot: rect.equip_slot,
                },
            );

            item_map.insert(*item, (sprite, get_index_from_pixel_cords(*rect)));
        }

        let atlas_handle = texture_assets.add(atlas);

        *graphics = Graphics {
            texture_atlas: Some(atlas_handle),
            item_map: Some(item_map),
        };
    }
}

pub fn get_index_from_pixel_cords(p: MyRect) -> usize {
    (p.pos.1 + (p.pos.0 / 16.)) as usize
}
