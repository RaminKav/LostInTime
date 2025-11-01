pub mod asset_helpers;
use std::fs::File;
use std::io::BufReader;

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
use crate::client::GameData;
use crate::enemy::Mob;
use crate::inventory::ItemStack;
use crate::item::active_skill_shrine::ActiveSkillSprite;
use crate::item::combat_shrine::CombatShrineAnim;
use crate::item::dungeon_shrine::AccessoryShrineAnim;
use crate::item::dungeon_shrine::ArmorShrineAnim;
use crate::item::dungeon_shrine::WeaponShrineAnim;
use crate::item::gamble_shrine::GambleShrineAnim;
use crate::item::heirloom_shrine::HeirloomMerchantSprite;
use crate::item::{
    Equipment, FurnaceRecipeList, RecipeList, RecipeListProto, Recipes, Wall, WorldObject,
    WorldObjectResource,
};
use crate::pets::state::Pet;
use crate::player::skills::SkillClass;
use crate::player::skills::{ActiveSkill, Heirloom};
use crate::status_effects::StatusEffect;
use crate::ui::{BlacksmithMerchant, UIElement};
use crate::world::portal::Portal;
use crate::{datafiles, GameState, ImageAssets};

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

/// Data structure for class information loaded from RON
#[derive(Clone, Debug, Deserialize)]
pub struct ClassData {
    pub name: String,
    pub weapon_description: Vec<String>,
    pub stat_description: Vec<String>,
    pub class_icon: UIElement,
    pub skill_icon: UIElement,
}

/// Data structure for pet information loaded from RON
#[derive(Clone, Debug, Deserialize)]
pub struct PetData {
    pub name: String,
    pub description: Vec<String>,
    pub pet_icon: UIElement,
    pub skill_icon: UIElement,
}

/// Container for all class and pet data loaded from RON
#[derive(Deserialize, TypeUuid, Clone)]
#[uuid = "a1b2c3d4-e5f6-7890-abcd-ef1234567890"]
pub struct ClassPetData {
    pub classes: HashMap<SkillClass, ClassData>,
    pub pets: HashMap<Pet, PetData>,
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
#[derive(Deserialize, TypeUuid)]
#[uuid = "413be529-bfeb-41b3-9db0-4b8b380a2c36"]
pub struct GraphicsDesc {
    items: HashMap<WorldObject, WorldObjectData>,
    icons: HashMap<WorldObject, SpriteData>,
    heirlooms: HashMap<crate::player::skills::Heirloom, WorldObjectData>,
}

impl Plugin for GameAssetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(Material2dPlugin::<FoliageMaterial>::default())
            .insert_resource(Graphics {
                texture_atlas: None,
                wall_texture_atlas: None,
                spritesheet_map: None,
                icons: None,
                ui_image_handles: None,
                mob_spritesheets: None,
                status_effect_icons: None,
                skill_icons: None,
                heirloom_skill_icons: None,
                heirloom_sprites: None,
                item_glows: None,
                combat_shrine_anim: None,
                gamble_shrine_anim: None,
                weapon_shrine_anim: None,
                armor_shrine_anim: None,
                accessory_shrine_anim: None,
                blacksmith_merchant: None,
                active_skill_shrine: None,
                heirloom_shrine_anim: None,
                portal_ase: None,
                class_pet_data: None,
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
    pub ui_image_handles: Option<HashMap<UIElement, Handle<Image>>>,
    pub mob_spritesheets: Option<HashMap<Mob, Vec<Handle<Image>>>>,
    pub status_effect_icons: Option<HashMap<StatusEffect, Handle<Image>>>,
    pub skill_icons: Option<HashMap<ActiveSkill, Handle<Image>>>,
    pub heirloom_skill_icons: Option<HashMap<Heirloom, Handle<Image>>>,
    pub heirloom_sprites: Option<HashMap<Heirloom, TextureAtlasSprite>>,
    pub item_glows: Option<HashMap<ItemGlow, Handle<Image>>>,
    pub combat_shrine_anim: Option<Handle<Aseprite>>,
    pub weapon_shrine_anim: Option<Handle<Aseprite>>,
    pub armor_shrine_anim: Option<Handle<Aseprite>>,
    pub accessory_shrine_anim: Option<Handle<Aseprite>>,
    pub gamble_shrine_anim: Option<Handle<Aseprite>>,
    pub blacksmith_merchant: Option<Handle<Aseprite>>,
    pub active_skill_shrine: Option<Handle<Aseprite>>,
    pub heirloom_shrine_anim: Option<Handle<Aseprite>>,
    pub portal_ase: Option<Handle<Aseprite>>,
    pub class_pet_data: Option<ClassPetData>,
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
    pub fn get_heirloom_icon(&self, heirloom: Heirloom) -> TextureAtlasSprite {
        self.heirloom_sprites
            .as_ref()
            .unwrap()
            .get(&heirloom)
            .unwrap_or_else(|| panic!("No graphic for object {:?}", heirloom))
            .clone()
    }
    pub fn get_active_skill_icon(&self, active_skill: ActiveSkill) -> Handle<Image> {
        self.skill_icons
            .as_ref()
            .unwrap()
            .get(&active_skill)
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
    pub fn get_class_data(&self, class: SkillClass) -> &ClassData {
        self.class_pet_data
            .as_ref()
            .unwrap()
            .classes
            .get(&class)
            .unwrap_or_else(|| panic!("No data for class {:?}", class))
    }
    pub fn get_pet_data(&self, pet: Pet) -> &PetData {
        self.class_pet_data
            .as_ref()
            .unwrap()
            .pets
            .get(&pet)
            .unwrap_or_else(|| panic!("No data for pet {:?}", pet))
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
        graphics_desc: Res<Assets<GraphicsDesc>>,
        recipes_desc: Res<Assets<RecipeListProto>>,
        class_pet_desc: Res<Assets<ClassPetData>>,
        mut commands: Commands,
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
        // Load class ranks early during loading so Class Selection UI can use it
        let game_data_file_path = datafiles::game_data();
        if let Ok(file_file) = File::open(game_data_file_path) {
            let reader = BufReader::new(file_file);
            match serde_json::from_reader::<_, GameData>(reader) {
                Ok(game_data) => {
                    commands.insert_resource(game_data.class_ranks);
                    info!("Loaded class ranks from game data (loading state)");
                }
                Err(err) => {
                    error!("Failed to load class ranks from game_data.json: {err:?}");
                    commands.insert_resource(crate::player::class_rank::ClassRankSystem::new());
                }
            }
        } else {
            commands.insert_resource(crate::player::class_rank::ClassRankSystem::new());
        }
        let sprite_desc_handle: Handle<GraphicsDesc> = sprite_sheet.sprite_desc.clone();
        let recipes_desc_handle: Handle<RecipeListProto> = sprite_sheet.recipes.clone();
        let class_pet_desc_handle: Handle<ClassPetData> = sprite_sheet.class_desc.clone();
        let sprite_desc = graphics_desc.get(&sprite_desc_handle).unwrap();
        let recipes_desc: &RecipeListProto = recipes_desc.get(&recipes_desc_handle).unwrap();
        let class_pet_data = class_pet_desc.get(&class_pet_desc_handle).unwrap();
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
        let mut status_effect_handles = HashMap::default();
        let skill_handles = HashMap::default();
        let mut item_glow_handles = HashMap::default();

        let mob_spritesheets = Mob::iter()
            .map(|mob| {
                (
                    mob.clone(),
                    vec![
                        asset_server
                            .load(format!("textures/{}/{}_side.png", mob, mob).to_lowercase()),
                        asset_server
                            .load(format!("textures/{}/{}_up.png", mob, mob).to_lowercase()),
                        asset_server
                            .load(format!("textures/{}/{}_down.png", mob, mob).to_lowercase()),
                    ],
                )
            })
            .collect::<HashMap<_, _>>();
        let mut recipes_list = RecipeList::default();
        let mut furnace_list = FurnaceRecipeList::default();
        let mut upgradeable_items = Vec::new();

        for (item, rect) in sprite_desc.items.iter() {
            {
                let mut sprite = TextureAtlasSprite::new(atlas.add_texture(rect.to_atlas_rect()));

                //Set the size to be proportional to the source rectangle
                sprite.custom_size = Some(Vec2::new(rect.size.x, rect.size.y));
                spritesheet_map.insert(*item, sprite);
            }
            world_obj_data.properties.insert(*item, *rect);
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

        // load heirloom sprites
        let mut heirloom_sprites = HashMap::default();
        for (heirloom, rect) in sprite_desc.heirlooms.iter() {
            let mut sprite = TextureAtlasSprite::new(atlas.add_texture(rect.to_atlas_rect()));
            sprite.custom_size = Some(Vec2::new(rect.size.x, rect.size.y));
            heirloom_sprites.insert(heirloom.clone(), sprite);
        }

        // load recipes
        for (result, recipe) in recipes_desc.0.iter() {
            recipes_list.insert(*result, (recipe.0.clone(), recipe.1.clone(), recipe.2));
        }
        // load furnace recipes
        for (result, recipe) in recipes_desc.1.iter() {
            furnace_list.insert(*result, *recipe);
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
            debug!("LOADED UI ASSET {:?}", u.to_string());
            let handle = asset_server.load(format!("ui/{u}.png"));
            ui_image_handles.insert(u, handle);
        }
        // load Status Effect Icons
        for u in StatusEffect::iter() {
            let handle = asset_server.load(format!("effects/{u}Icon.png"));
            status_effect_handles.insert(u, handle);
        }
        // load Active Skill Icons
        let mut active_skill_handles = HashMap::default();
        for u in crate::player::skills::ActiveSkill::iter() {
            let handle = asset_server.load(format!("effects/{u}Icon.png"));
            active_skill_handles.insert(u, handle);
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

            ui_image_handles: Some(ui_image_handles),
            icons: Some(icon_map),
            mob_spritesheets: Some(mob_spritesheets),
            status_effect_icons: Some(status_effect_handles),
            skill_icons: Some(active_skill_handles),
            heirloom_skill_icons: Some(skill_handles),
            heirloom_sprites: Some(heirloom_sprites),
            item_glows: Some(item_glow_handles),
            combat_shrine_anim: Some(asset_server.load(CombatShrineAnim::PATH)),
            gamble_shrine_anim: Some(asset_server.load(GambleShrineAnim::PATH)),
            weapon_shrine_anim: Some(asset_server.load(WeaponShrineAnim::PATH)),
            armor_shrine_anim: Some(asset_server.load(ArmorShrineAnim::PATH)),
            accessory_shrine_anim: Some(asset_server.load(AccessoryShrineAnim::PATH)),
            blacksmith_merchant: Some(asset_server.load(BlacksmithMerchant::PATH)),
            active_skill_shrine: Some(asset_server.load(ActiveSkillSprite::PATH)),
            heirloom_shrine_anim: Some(asset_server.load(HeirloomMerchantSprite::PATH)),
            portal_ase: Some(asset_server.load(Portal::PATH)),
            class_pet_data: Some(class_pet_data.clone()),
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
                if let Some(texture_atlas) = texture_atlases.get(spritesheet) {
                    if texture_atlas.textures.len() < 100 {
                        continue;
                    }
                }
                let has_icon = graphics.icons.as_ref().unwrap().get(world_object);
                let new_sprite = if let Some(icon) = has_icon {
                    icon
                } else {
                    item_map
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
