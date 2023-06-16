use std::fs;

use bevy::prelude::*;
use bevy::reflect::TypeUuid;
use bevy::render::render_resource::{
    AsBindGroup, Extent3d, ShaderRef, TextureDimension, TextureFormat,
};
use bevy::sprite::{Material2d, Material2dPlugin};
use bevy::utils::HashMap;
use serde::Deserialize;
use strum::IntoEnumIterator;

use crate::item::{LootTableMap, RecipeList, Recipes, WorldObject, WorldObjectResource};
use crate::ui::UIElement;
use crate::{GameState, ImageAssets, Limb};
use ron::de::from_str;

pub struct GameAssetsPlugin;

/// Used to describe the location and styling of sprites on the sprite sheet
#[derive(Default, Clone, Copy, Debug, Deserialize)]
pub struct WorldObjectData {
    pub texture_pos: Vec2,
    pub size: Vec2,
    pub anchor: Option<Vec2>,
    pub collider: bool,
    pub breakable: bool,
    pub breaks_into: Option<WorldObject>,
    pub breaks_with: Option<WorldObject>,
    /// 0 = main hand, 1 = head, 2 = chest, 3 = legs
    pub equip_slot: Option<Limb>,
    pub places_into: Option<WorldObject>,
}

impl WorldObjectData {
    // pub fn new(texture_pos: Vec2, size: Vec2) -> Self {
    //     Self {
    //         texture_pos,
    //         size,
    //         anchor: None,
    //         collider: false,
    //         breakable: false,
    //         breaks_into: None,
    //         equip_slot: None,
    //         breaks_with: None,
    //         places_into: None,
    //         minimap_color: ,
    //     }
    // }

    pub fn to_atlas_rect(self) -> bevy::math::Rect {
        bevy::math::Rect {
            //A tiny amount is clipped off the sides of the rectangle
            //to stop contents of other sprites from bleeding through
            min: Vec2::new(self.texture_pos.x + 0.15, self.texture_pos.y + 0.15),
            max: Vec2::new(
                self.texture_pos.x + self.size.x - 0.15,
                self.texture_pos.y + self.size.y - 0.15,
            ),
        }
    }
}

/// Loaded from sprites_desc.ron and contains the description of every sprite in the game
#[derive(Deserialize)]
pub struct GraphicsDesc {
    items: HashMap<WorldObject, WorldObjectData>,
    icons: HashMap<WorldObject, WorldObjectData>,
}

impl Plugin for GameAssetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(Material2dPlugin::<FoliageMaterial>::default())
            .insert_resource(Graphics {
                texture_atlas: None,
                wall_texture_atlas: None,
                spritesheet_map: None,
                icons: None,
                foliage_material_map: None,
                ui_image_handles: None,
            })
            .add_system(Self::load_graphics.in_schedule(OnExit(GameState::Loading)));
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
    pub wall_texture_atlas: Option<Handle<TextureAtlas>>,
    pub spritesheet_map: Option<HashMap<WorldObject, TextureAtlasSprite>>,
    pub icons: Option<HashMap<WorldObject, TextureAtlasSprite>>,
    pub foliage_material_map: Option<HashMap<WorldObject, (Handle<FoliageMaterial>, usize)>>,
    pub ui_image_handles: Option<HashMap<UIElement, Handle<Image>>>,
}

/// Work around helper function to convert texture atlas sprites into stand alone image handles
/// Copies sprite data pixel by pixel, needed to render things in UI
fn convert_to_image(
    sprite_desc: WorldObjectData,
    original_image: Handle<Image>,
    assets: &mut ResMut<Assets<Image>>,
) -> Handle<Image> {
    //TODO convert if mismatch
    let original_image = assets.get(&original_image).unwrap();
    assert!(original_image.texture_descriptor.format == TextureFormat::Rgba8UnormSrgb);

    let mut data = Vec::default();
    //Every pixel is 4 entries in image.data
    let mut starting_index =
        (sprite_desc.texture_pos.x + original_image.size().x * sprite_desc.texture_pos.y) as usize;
    for _y in 0..sprite_desc.size.y as usize {
        for x in 0..sprite_desc.size.x as usize {
            let index = starting_index + x;
            //Copy 1 pixel at index
            data.push(original_image.data[index * 4]);
            data.push(original_image.data[index * 4 + 1]);
            data.push(original_image.data[index * 4 + 2]);
            data.push(original_image.data[index * 4 + 3]);
        }
        starting_index += original_image.size().y as usize;
    }

    let size = Extent3d {
        width: sprite_desc.size.x as u32,
        height: sprite_desc.size.y as u32,
        depth_or_array_layers: 1,
    };
    let image = Image::new(
        size,
        TextureDimension::D2,
        data,
        //FIXME
        TextureFormat::Rgba8UnormSrgb,
    );
    assets.add(image)
}

impl GameAssetsPlugin {
    /// Startup system that runs after images are loaded, indexes all loaded images
    /// and creates the graphics resource
    pub fn load_graphics(
        mut graphics: ResMut<Graphics>,
        mut recipes: ResMut<Recipes>,
        mut loot_tables: ResMut<LootTableMap>,
        sprite_sheet: Res<ImageAssets>,
        mut image_assets: ResMut<Assets<Image>>,
        mut texture_assets: ResMut<Assets<TextureAtlas>>,
        mut world_obj_data: ResMut<WorldObjectResource>,
        mut materials: ResMut<Assets<FoliageMaterial>>,
        asset_server: Res<AssetServer>,
    ) {
        //let image_handle = assets.load("bevy_survival_sprites.png");
        let image_handle = sprite_sheet.sprite_sheet.clone();
        let wall_image_handle = sprite_sheet.walls_sheet.clone();
        let sprite_desc = fs::read_to_string("assets/textures/sprites_desc.ron").unwrap();
        let recipe_desc = fs::read_to_string("assets/recipes/recipes.ron").unwrap();
        let loot_table_desc = fs::read_to_string("assets/loot/loot_tables.ron").unwrap();

        let sprite_desc: GraphicsDesc = from_str(&sprite_desc).unwrap_or_else(|e| {
            println!("Failed to load config for graphics: {e}");
            std::process::exit(1);
        });
        let recipes_desc: Recipes = from_str(&recipe_desc).unwrap_or_else(|e| {
            println!("Failed to load config for recipes: {e}");
            std::process::exit(1);
        });
        let loot_table_desc: LootTableMap = from_str(&loot_table_desc).unwrap_or_else(|e| {
            println!("Failed to load config for recipes: {e}");
            std::process::exit(1);
        });

        let mut atlas = TextureAtlas::new_empty(image_handle.clone(), Vec2::new(256., 32.));
        let wall_atlas = TextureAtlas::from_grid(
            wall_image_handle.clone(),
            Vec2::new(32., 48.),
            16,
            2,
            None,
            None,
        );

        let mut spritesheet_map = HashMap::default();
        let mut icon_map = HashMap::default();
        let mut ui_image_handles = HashMap::default();
        let mut foliage_material_map = HashMap::default();

        let mut recipes_list = RecipeList::default();
        let mut loot_table_list = LootTableMap::default();

        for (item, rect) in sprite_desc.items.iter() {
            println!("Found graphic {item:?}");
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
                    foliage_material_map
                        .insert(*item, (foliage_material, get_index_from_pixel_cords(*rect)));
                }
                _ => {
                    let mut sprite =
                        TextureAtlasSprite::new(atlas.add_texture(rect.to_atlas_rect()));

                    //Set the size to be proportional to the source rectangle
                    sprite.custom_size = Some(Vec2::new(rect.size.x, rect.size.y));
                    spritesheet_map.insert(*item, sprite);
                }
            }
            //TODO: maybe we can clean up our spawning code with this vvv
            //Position the sprite anchor if one is defined
            // if let Some(anchor) = rect.anchor {
            //     sprite.anchor = Anchor::Custom(Vec2::new(
            //         anchor.0 / rect.size.0 - 0.5,
            //         0.5 - anchor.1 / rect.size.1,
            //     ));
            // };
            world_obj_data.properties.insert(*item, *rect);
        }

        // load icons
        for (item, rect) in sprite_desc.icons.iter() {
            let mut sprite = TextureAtlasSprite::new(atlas.add_texture(rect.to_atlas_rect()));

            //Set the size to be proportional to the source rectangle
            sprite.custom_size = Some(Vec2::new(rect.size.x, rect.size.y));
            icon_map.insert(*item, sprite);
        }

        // load recipes
        for (result, recipe) in recipes_desc.recipes_list.iter() {
            recipes_list.insert(*result, *recipe);
            println!("Loaded recipe for {result:?}: {recipe:?}");
        }

        // load loot_tables
        for (enemy, loot_table) in loot_table_desc.table.iter() {
            loot_table_list
                .table
                .insert(enemy.clone(), loot_table.clone());
            println!("Loaded loot table for {enemy:?}: {loot_table:?}");
        }

        *recipes = Recipes { recipes_list };
        *loot_tables = loot_table_list;
        // load UI
        for u in UIElement::iter() {
            println!("LOADED UI ASSET {:?}", u.to_string());
            let handle = asset_server.load(format!("ui/{u}.png"));
            ui_image_handles.insert(u, handle);
        }

        let atlas_handle = texture_assets.add(atlas);
        let wall_atlas_handle = texture_assets.add(wall_atlas);

        *graphics = Graphics {
            texture_atlas: Some(atlas_handle),
            wall_texture_atlas: Some(wall_atlas_handle),
            spritesheet_map: Some(spritesheet_map),
            foliage_material_map: Some(foliage_material_map),
            ui_image_handles: Some(ui_image_handles),
            icons: Some(icon_map),
        };
    }
}

pub fn get_index_from_pixel_cords(p: WorldObjectData) -> usize {
    (p.texture_pos.y + (p.texture_pos.x / 16.)) as usize
}
