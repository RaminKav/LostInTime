use std::fs;

use bevy::prelude::*;
use bevy::reflect::TypeUuid;
use bevy::render::render_resource::{AsBindGroup, ShaderRef};
use bevy::sprite::{Material2d, Material2dPlugin};
use bevy::utils::HashMap;
use serde::Deserialize;

use crate::item::{WorldObject, WorldObjectData, WorldObjectResource};
use crate::{GameState, ImageAssets};
use ron::de::from_str;

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
    pub breaks_with: Option<WorldObject>,
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
            breaks_with: None,
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
        app.add_plugin(Material2dPlugin::<FoliageMaterial>::default())
            .insert_resource(Graphics {
                texture_atlas: None,
                spritesheet_map: None,
                image_handle_map: None,
            })
            .add_system_set(
                SystemSet::on_exit(GameState::Loading)
                    .with_system(Self::load_graphics.label("graphics")),
            );
    }
}

impl Material2d for FoliageMaterial {
    fn vertex_shader() -> ShaderRef {
        "shaders/test_wind.wgsl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "shaders/test_wind.wgsl".into()
    }
}

#[derive(AsBindGroup, TypeUuid, Debug, Clone)]
#[uuid = "9600d1e3-1911-4286-9810-e9bd9ff685e1"]
pub struct FoliageMaterial {
    #[uniform(0)]
    speed: f32,
    #[uniform(1)]
    minStrength: f32,
    #[uniform(2)]
    maxStrength: f32,
    #[uniform(3)]
    strengthScale: f32,
    #[uniform(4)]
    interval: f32,
    #[uniform(5)]
    detail: f32,
    #[uniform(6)]
    distortion: f32,
    #[uniform(7)]
    heightOffset: f32,
    #[uniform(8)]
    offset: f32,
    #[texture(9)]
    #[sampler(10)]
    pub source_texture: Option<Handle<Image>>,
}
#[derive(Resource)]

pub struct Graphics {
    pub texture_atlas: Option<Handle<TextureAtlas>>,
    pub spritesheet_map: Option<HashMap<WorldObject, (TextureAtlasSprite, usize)>>,
    pub image_handle_map: Option<HashMap<WorldObject, (Handle<FoliageMaterial>, usize)>>,
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
        mut materials: ResMut<Assets<FoliageMaterial>>,
        asset_server: Res<AssetServer>,
    ) {
        //let image_handle = assets.load("bevy_survival_sprites.png");
        let image_handle = sprite_sheet.sprite_sheet.clone();
        let sprite_desc = fs::read_to_string("assets/textures/sprites_desc.ron").unwrap();

        let sprite_desc: GraphicsDesc = from_str(&sprite_desc).unwrap_or_else(|e| {
            println!("Failed to load config for graphics: {}", e);
            std::process::exit(1);
        });

        let mut atlas = TextureAtlas::new_empty(image_handle.clone(), Vec2::new(256., 32.));

        let mut spritesheet_map = HashMap::default();
        let mut image_handle_map = HashMap::default();

        for (item, rect) in sprite_desc.map.iter() {
            println!("Found graphic {:?}", item);
            match item {
                WorldObject::Foliage(f) => {
                    let handle = asset_server.load(format!("{}.png", f.to_string().to_lowercase()));
                    let foliage_material = materials.add(FoliageMaterial {
                        source_texture: Some(handle),
                        speed: 0.5,
                        minStrength: 0.001,
                        maxStrength: 0.003,
                        strengthScale: 20.,
                        interval: 3.5,
                        detail: 1.,
                        distortion: 1.,
                        heightOffset: 0.4,
                        offset: 0.,
                        // alpha_mode: AlphaMode::Blend,
                    });
                    image_handle_map
                        .insert(*item, (foliage_material, get_index_from_pixel_cords(*rect)));
                }
                _ => {
                    let mut sprite =
                        TextureAtlasSprite::new(atlas.add_texture(rect.to_atlas_rect()));

                    //Set the size to be proportional to the source rectangle
                    sprite.custom_size = Some(Vec2::new(rect.size.0, rect.size.1));
                    spritesheet_map.insert(*item, (sprite, get_index_from_pixel_cords(*rect)));
                }
            }

            //Position the sprite anchor if one is defined
            // if let Some(anchor) = rect.anchor {
            //     sprite.anchor = Anchor::Custom(Vec2::new(
            //         anchor.0 / rect.size.0 - 0.5,
            //         0.5 - anchor.1 / rect.size.1,
            //     ));
            // };
            world_obj_data.properties.insert(
                *item,
                WorldObjectData {
                    size: Vec2::new(rect.size.0, rect.size.1),
                    anchor: rect.anchor,
                    collider: rect.collider,
                    breakable: rect.breakable,
                    breaks_into: rect.breaks_into,
                    equip_slot: rect.equip_slot,
                    breaks_with: rect.breaks_with,
                },
            );
        }

        let atlas_handle = texture_assets.add(atlas);

        *graphics = Graphics {
            texture_atlas: Some(atlas_handle),
            spritesheet_map: Some(spritesheet_map),
            image_handle_map: Some(image_handle_map),
        };
    }
}

pub fn get_index_from_pixel_cords(p: MyRect) -> usize {
    (p.pos.1 + (p.pos.0 / 16.)) as usize
}
