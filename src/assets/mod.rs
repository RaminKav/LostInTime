use std::fs;
pub mod asset_helpers;
use bevy::prelude::*;
use bevy::reflect::TypeUuid;
use bevy::render::render_resource::{AsBindGroup, Extent3d, TextureDimension, TextureFormat};
use bevy::sprite::{Material2d, Material2dPlugin};
use bevy::utils::HashMap;
use bevy_aseprite::Aseprite;
use bevy_proto::prelude::{ReflectSchematic, Schematic};
use serde::Deserialize;
use strum::IntoEnumIterator;

use crate::attributes::{add_item_glows, ItemGlow};
use crate::enemy::Mob;
use crate::inventory::ItemStack;
use crate::item::combat_shrine::CombatShrineAnim;
use crate::item::gamble_shrine::GambleShrineAnim;
use crate::item::{
    Equipment, Foliage, FurnaceRecipeList, RecipeList, RecipeListProto, Recipes, Wall, WorldObject,
    WorldObjectResource,
};
use crate::player::skills::Skill;
use crate::status_effects::StatusEffect;
use crate::ui::UIElement;
use crate::{GameState, ImageAssets};
use ron::de::from_str;

pub struct GameAssetsPlugin;

/// Used to describe the location and styling of sprites on the sprite sheet
#[derive(Default, Clone, Copy, Debug, Deserialize)]
pub struct WorldObjectData {
    pub texture_pos: Vec2,
    pub size: Vec2,
    pub anchor: Option<Vec2>,
}

impl WorldObjectData {
    pub fn to_atlas_rect(self) -> bevy::math::Rect {
        bevy::math::Rect {
            //A tiny amount is clipped off the sides of the rectangle
            //to stop contents of other sprites from bleeding through
            min: Vec2::new(
                self.texture_pos.x * 16. + 0.15,
                self.texture_pos.y * 16. + 0.15,
            ),
            max: Vec2::new(
                self.texture_pos.x * 16. + self.size.x - 0.15,
                self.texture_pos.y * 16. + self.size.y - 0.15,
            ),
        }
    }
}
#[derive(Default, Clone, Copy, Debug, Deserialize)]
pub struct SpriteData {
    pub texture_pos: Vec2,
    pub size: Vec2,
}

#[derive(Component, Reflect, FromReflect, Schematic, Default)]
#[reflect(Component, Schematic)]
pub enum SpriteSize {
    #[default]
    Small,
    Medium,
}

impl SpriteSize {
    pub fn is_medium(&self) -> bool {
        matches!(self, SpriteSize::Medium)
    }
}

#[derive(Component, Reflect, FromReflect, Schematic, Default)]
#[reflect(Component, Schematic)]
pub struct SpriteAnchor(pub Vec2);

/// Loaded from sprites_desc.ron and contains the description of every sprite in the game
#[derive(Deserialize)]
pub struct GraphicsDesc {
    items: HashMap<WorldObject, WorldObjectData>,
    icons: HashMap<WorldObject, SpriteData>,
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
                player_spritesheets: None,
                mob_spritesheets: None,
                status_effect_icons: None,
                skill_icons: None,
                item_glows: None,
                combat_shrine_anim: None,
                gamble_shrine_anim: None,
            })
            .add_system(Self::update_graphics.in_set(OnUpdate(GameState::Main)))
            .add_system(Self::load_graphics.in_schedule(OnExit(GameState::Loading)));
    }
}

impl Material2d for FoliageMaterial {
    // fn vertex_shader() -> ShaderRef {
    //     "shaders/test_wind.wgsl".into()
    // }
    // fn fragment_shader() -> ShaderRef {
    //     "shaders/test_wind.wgsl".into()
    // }
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
    #[uniform(9)]
    pub opacity: f32,
    #[texture(10)]
    #[sampler(11)]
    pub source_texture: Option<Handle<Image>>,
}
#[derive(Resource)]

pub struct Graphics {
    pub texture_atlas: Option<Handle<TextureAtlas>>,
    pub wall_texture_atlas: Option<Handle<TextureAtlas>>,
    pub spritesheet_map: Option<HashMap<WorldObject, TextureAtlasSprite>>,
    pub icons: Option<HashMap<WorldObject, TextureAtlasSprite>>,
    pub foliage_material_map: Option<HashMap<Foliage, FoliageMaterial>>,
    pub ui_image_handles: Option<HashMap<UIElement, Handle<Image>>>,
    pub player_spritesheets: Option<Vec<Handle<Image>>>,
    pub mob_spritesheets: Option<HashMap<Mob, Vec<Handle<Image>>>>,
    pub status_effect_icons: Option<HashMap<StatusEffect, Handle<Image>>>,
    pub skill_icons: Option<HashMap<Skill, Handle<Image>>>,
    pub item_glows: Option<HashMap<ItemGlow, Handle<Image>>>,
    pub combat_shrine_anim: Option<Handle<Aseprite>>,
    pub gamble_shrine_anim: Option<Handle<Aseprite>>,
}
impl Graphics {
    pub fn get_ui_element_texture(&self, element: UIElement) -> Handle<Image> {
        self.ui_image_handles
            .as_ref()
            .unwrap()
            .get(&element)
            .unwrap()
            .clone()
    }
    pub fn get_status_effect_icon(&self, status: StatusEffect) -> Handle<Image> {
        self.status_effect_icons
            .as_ref()
            .unwrap()
            .get(&status)
            .unwrap()
            .clone()
    }
    pub fn get_skill_icon(&self, skill: Skill) -> Handle<Image> {
        self.skill_icons
            .as_ref()
            .unwrap()
            .get(&skill)
            .unwrap()
            .clone()
    }
    pub fn get_item_glow(&self, glow: ItemGlow) -> Handle<Image> {
        self.item_glows
            .as_ref()
            .unwrap()
            .get(&glow)
            .unwrap()
            .clone()
    }
}

/// Work around helper function to convert texture atlas sprites into stand alone image handles
/// Copies sprite data pixel by pixel, needed to render things in UI
fn _convert_to_image(
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
        sprite_sheet: Res<ImageAssets>,
        mut texture_assets: ResMut<Assets<TextureAtlas>>,
        mut world_obj_data: ResMut<WorldObjectResource>,
        asset_server: Res<AssetServer>,
    ) {
        //let image_handle = assets.load("bevy_survival_sprites.png");
        let image_handle = sprite_sheet.sprite_sheet.clone();
        let wall_image_handle = sprite_sheet.walls_sheet.clone();

        #[cfg(feature = "release-bundle")]
        {
            std::env::set_current_dir(
                std::env::current_exe()
                    .map(|path| {
                        path.parent()
                            .map(|exe_parent_path| exe_parent_path.to_owned())
                            .unwrap()
                    })
                    .unwrap(),
            )
            .unwrap();
        }

        let sprite_desc = fs::read_to_string("./assets/textures/sprites_desc.ron").unwrap();
        let recipe_desc = fs::read_to_string("./assets/recipes/recipes.ron").unwrap();

        let sprite_desc: GraphicsDesc = from_str(&sprite_desc).unwrap_or_else(|e| {
            println!("Failed to load config for graphics: {e}");
            std::process::exit(1);
        });
        let recipes_desc: RecipeListProto = from_str(&recipe_desc).unwrap_or_else(|e| {
            println!("Failed to load config for recipes: {e}");
            std::process::exit(1);
        });

        let mut atlas = TextureAtlas::new_empty(image_handle.clone(), Vec2::new(256., 384.));
        let wall_atlas = TextureAtlas::from_grid(
            wall_image_handle.clone(),
            Vec2::new(16., 32.),
            32,
            4,
            None,
            None,
        );

        let mut spritesheet_map = HashMap::default();
        let mut icon_map = HashMap::default();
        let mut ui_image_handles = HashMap::default();
        let mut foliage_material_map = HashMap::default();
        let mut status_effect_handles = HashMap::default();
        let mut skill_handles = HashMap::default();
        let mut item_glow_handles = HashMap::default();
        let player_spritesheets = vec![
            asset_server.load("textures/player/player_side.png"),
            asset_server.load("textures/player/player_up.png"),
            asset_server.load("textures/player/player_down.png"),
        ];
        let mob_spritesheets = Mob::iter()
            .map(|mob| {
                (
                    mob.clone(),
                    vec![
                        asset_server.load(format!("textures/{}/{}_side.png", mob, mob)),
                        asset_server.load(format!("textures/{}/{}_up.png", mob, mob)),
                        asset_server.load(format!("textures/{}/{}_down.png", mob, mob)),
                    ],
                )
            })
            .collect::<HashMap<_, _>>();
        let mut recipes_list = RecipeList::default();
        let mut furnace_list = FurnaceRecipeList::default();
        let mut upgradeable_items = Vec::new();

        for (item, rect) in sprite_desc.items.iter() {
            match item {
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
        // load foliage mat
        for f in Foliage::iter() {
            // let handle = asset_server.load(format!("{}.png", f.to_string().to_lowercase()));
            // let foliage_material = FoliageMaterial {
            //     source_texture: Some(handle),
            //     speed: 0.5,
            //     minStrength: 0.001,
            //     maxStrength: 0.003,
            //     strengthScale: 20.,
            //     interval: 3.5,
            //     detail: 1.,
            //     distortion: 1.,
            //     heightOffset: 0.4,
            //     offset: 0.,
            //     opacity: 1.,
            //     // alpha_mode: AlphaMode::Blend,
            // };
            // foliage_material_map.insert(f, foliage_material);
        }

        // load icons
        for (item, rect) in sprite_desc.icons.iter() {
            let mut sprite =
                TextureAtlasSprite::new(atlas.add_texture(bevy::math::Rect::from_corners(
                    rect.texture_pos * 16.,
                    rect.texture_pos * 16. + rect.size,
                )));

            //Set the size to be proportional to the source rectangle
            sprite.custom_size = Some(Vec2::new(rect.size.x, rect.size.y));
            icon_map.insert(*item, sprite);
        }

        // load recipes
        for (result, recipe) in recipes_desc.0.iter() {
            recipes_list.insert(*result, (recipe.0.clone(), recipe.1.clone(), recipe.2));
        }
        // load furnace recipes
        for (result, recipe) in recipes_desc.1.iter() {
            furnace_list.insert(*result, recipe.clone());
        }
        // load upgradeable items
        for item in recipes_desc.2.iter() {
            upgradeable_items.push(*item);
        }

        *recipes = Recipes {
            crafting_list: recipes_list,
            furnace_list,
            upgradeable_items,
        };
        // load UI
        for u in UIElement::iter() {
            println!("LOADED UI ASSET {:?}", u.to_string());
            let handle = asset_server.load(format!("ui/{u}.png"));
            ui_image_handles.insert(u, handle);
        }
        // load Status Effect Icons
        for u in StatusEffect::iter() {
            let handle = asset_server.load(format!("effects/{u}Icon.png"));
            status_effect_handles.insert(u, handle);
        }
        // load Skill Icons
        for u in Skill::iter() {
            let handle = asset_server.load(format!("effects/{u}Icon.png"));
            skill_handles.insert(u, handle);
        }
        // load Item Glows
        for u in ItemGlow::iter() {
            let handle = asset_server.load(format!("effects/{u}ItemGlow.png"));
            item_glow_handles.insert(u, handle);
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
            player_spritesheets: Some(player_spritesheets),
            mob_spritesheets: Some(mob_spritesheets),
            status_effect_icons: Some(status_effect_handles),
            skill_icons: Some(skill_handles),
            item_glows: Some(item_glow_handles),
            combat_shrine_anim: Some(asset_server.load(CombatShrineAnim::PATH)),
            gamble_shrine_anim: Some(asset_server.load(GambleShrineAnim::PATH)),
        };
    }
    /// Keeps the graphics up to date for things that are spawned from proto, or change Obj type
    pub fn update_graphics(
        mut to_update_query: Query<
            (
                Entity,
                &mut TextureAtlasSprite,
                &Handle<TextureAtlas>,
                &WorldObject,
                Option<&ItemStack>,
            ),
            (Changed<WorldObject>, Without<Wall>, Without<Equipment>),
        >,
        mut commands: Commands,
        graphics: Res<Graphics>,
        texture_atlases: Res<Assets<TextureAtlas>>,
    ) {
        let item_map = &&graphics.spritesheet_map;
        if let Some(item_map) = item_map {
            for (e, mut sprite, spritesheet, world_object, maybe_stack) in
                to_update_query.iter_mut()
            {
                if let Some(texture_atlas) = texture_atlases.get(&spritesheet) {
                    if texture_atlas.textures.len() < 100 {
                        continue;
                    }
                }
                let has_icon = graphics.icons.as_ref().unwrap().get(&world_object);
                let new_sprite = if let Some(icon) = has_icon {
                    icon
                } else {
                    &item_map
                        .get(world_object)
                        .unwrap_or_else(|| panic!("No graphic for object {world_object:?}"))
                };
                commands
                    .entity(e)
                    .insert(graphics.texture_atlas.as_ref().unwrap().clone());
                sprite.clone_from(new_sprite);
                if let Some(stack) = maybe_stack {
                    add_item_glows(&mut commands, &graphics, e, stack.rarity.clone());
                }
            }
        }
    }
}

pub fn _get_index_from_pixel_cords(p: WorldObjectData) -> usize {
    (p.texture_pos.y + (p.texture_pos.x / 16.)) as usize
}
