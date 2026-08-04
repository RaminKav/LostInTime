pub mod asset_helpers;
pub mod skill_icons;
use std::fs::File;
use std::io::BufReader;

use crate::aseprite_assets::{
    AccessoryShrineAnim, ArmorShrineAnim, CombatShrineAnim, IceExplosion, PinkFlowerAseprite,
    Portal, ShrineEye, ShrineRepairRingAnim, SmallExplosion, UIPortal, WeaponShrineAnim,
};
use bevy::asset::RenderAssetUsages;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, Extent3d, TextureDimension, TextureFormat};
use bevy::sprite_render::{Material2d, Material2dPlugin};
use bevy_aseprite_ultra::prelude::Aseprite;
use serde::Deserialize;
use strum::IntoEnumIterator;

use crate::attributes::ItemGlow;
use crate::client::GameData;
use crate::enemy::Mob;
use crate::item::{
    FurnaceRecipeList, RecipeList, RecipeListProto, Recipes, WorldObject, WorldObjectResource,
};
use crate::pets::state::Pet;
use crate::player::skills::SkillClass;
use crate::player::skills::{ActiveSkill, Heirloom};
use crate::player::{
    get_default_unlocked_classes, Achievements, ClassUnlockConfig, ClassUnlockData, CoinCurrency,
    TimeCrystals, TimeFragmentCurrency, UnlockUpgrades, UnlockedClasses, UnlockedSkills,
};
use crate::status_effects::StatusEffect;
use crate::ui::tips::SeenTips;
use crate::ui::tutorial_ui::{seen_tutorial_chunks_from_game_data, SeenTutorialChunks};
use crate::ui::UIElement;

use self::skill_icons::{load_skill_icons, SkillIcon};
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
    pub fn to_atlas_rect(self) -> URect {
        let rect = bevy::math::Rect {
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
        };
        URect {
            min: UVec2::new(rect.min.x.round() as u32, rect.min.y.round() as u32),
            max: UVec2::new(rect.max.x.round() as u32, rect.max.y.round() as u32),
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
    /// The 4 active skills for this class (displayed in skill slots 0-3)
    pub active_skills: [crate::player::skills::ActiveSkill; 4],
    /// All starting weapons for this class
    pub starting_weapons: Vec<crate::item::WorldObject>,
}

/// Data structure for pet information loaded from RON
#[derive(Clone, Debug, Deserialize)]
pub struct PetData {
    pub name: String,
    pub skill_description: Vec<String>,
    pub skill_name: String,
    // pub passive_name: String,
    pub passive_description: Vec<String>,
    pub pet_icon: UIElement,
}

/// Container for all class and pet data loaded from RON
#[derive(Asset, TypePath, Deserialize, Clone)]
pub struct ClassPetData {
    pub classes: HashMap<SkillClass, ClassData>,
    pub pets: HashMap<Pet, PetData>,
}

#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component)]
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

#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component)]
pub struct SpriteAnchor(pub Vec2);

/// Loaded from sprites_desc.ron and contains the description of every sprite in the game
#[derive(Asset, TypePath, Deserialize)]
pub struct GraphicsDesc {
    items: HashMap<WorldObject, WorldObjectData>,
    icons: HashMap<WorldObject, SpriteData>,
    heirlooms: HashMap<crate::player::skills::Heirloom, WorldObjectData>,
}

impl Plugin for GameAssetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(Material2dPlugin::<FoliageMaterial>::default())
            .insert_resource(Graphics {
                texture_atlas_layout: None,
                texture_atlas_image: None,
                wall_texture_atlas_layout: None,
                wall_texture_atlas_image: None,
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
                shrine_eye: None,
                shrine_repair_ring: None,
                weapon_shrine_anim: None,
                armor_shrine_anim: None,
                accessory_shrine_anim: None,
                portal_ase: None,
                ui_portal_ase: None,
                class_pet_data: None,
                ice_explosion_ase: None,
                small_explosion_ase: None,
                cherry_bomb_ase: None,
                cherry_bomb_explosion_ase: None,
                bomb_ase: None,
                stone_pillar_ase: None,
                pink_flower_ase: None,
                void_laser_ase: None,
                inv_stat_highlight_common_ase: None,
                inv_stat_highlight_uncommon_ase: None,
                inv_stat_highlight_rare_ase: None,
                inv_stat_highlight_legendary_ase: None,
                cursor_color_sprites: None,
            })
            .add_systems(OnExit(GameState::Loading), Self::load_graphics);
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

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
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
    pub texture_atlas_layout: Option<Handle<TextureAtlasLayout>>,
    pub texture_atlas_image: Option<Handle<Image>>,
    pub wall_texture_atlas_layout: Option<Handle<TextureAtlasLayout>>,
    pub wall_texture_atlas_image: Option<Handle<Image>>,
    pub spritesheet_map: Option<HashMap<WorldObject, Sprite>>,
    pub icons: Option<HashMap<WorldObject, Sprite>>,
    pub ui_image_handles: Option<HashMap<UIElement, Handle<Image>>>,
    pub mob_spritesheets: Option<HashMap<Mob, Vec<Handle<Image>>>>,
    pub status_effect_icons: Option<HashMap<StatusEffect, Handle<Image>>>,
    pub skill_icons: Option<HashMap<SkillIcon, Handle<Image>>>,
    pub heirloom_skill_icons: Option<HashMap<Heirloom, Handle<Image>>>,
    pub heirloom_sprites: Option<HashMap<Heirloom, Sprite>>,
    pub item_glows: Option<HashMap<ItemGlow, Handle<Image>>>,
    pub combat_shrine_anim: Option<Handle<Aseprite>>,
    pub weapon_shrine_anim: Option<Handle<Aseprite>>,
    pub armor_shrine_anim: Option<Handle<Aseprite>>,
    pub accessory_shrine_anim: Option<Handle<Aseprite>>,
    pub shrine_eye: Option<Handle<Aseprite>>,
    pub shrine_repair_ring: Option<Handle<Aseprite>>,
    pub portal_ase: Option<Handle<Aseprite>>,
    pub ui_portal_ase: Option<Handle<Aseprite>>,
    pub class_pet_data: Option<ClassPetData>,
    pub ice_explosion_ase: Option<Handle<Aseprite>>,
    pub small_explosion_ase: Option<Handle<Aseprite>>,
    pub cherry_bomb_ase: Option<Handle<Aseprite>>,
    pub cherry_bomb_explosion_ase: Option<Handle<Aseprite>>,
    pub bomb_ase: Option<Handle<Aseprite>>,
    pub stone_pillar_ase: Option<Handle<Aseprite>>,
    pub pink_flower_ase: Option<Handle<Aseprite>>,
    /// Void Worm laser beam aseprite. Retained here so the asset + its built atlas
    /// stay resident; loading it on demand lets it unload between spawns, which
    /// causes the beam to randomly not render or freeze on a single frame.
    pub void_laser_ase: Option<Handle<Aseprite>>,
    /// Inventory tooltip stat-highlight aseprites (per rarity). Retained for the
    /// same reason as `void_laser_ase` (on-demand loads randomly fail to appear).
    pub inv_stat_highlight_common_ase: Option<Handle<Aseprite>>,
    pub inv_stat_highlight_uncommon_ase: Option<Handle<Aseprite>>,
    pub inv_stat_highlight_rare_ase: Option<Handle<Aseprite>>,
    pub inv_stat_highlight_legendary_ase: Option<Handle<Aseprite>>,
    /// Selectable custom-cursor color sprites, in selection order (sheet positions (4,1)..(11,1)).
    pub cursor_color_sprites: Option<Vec<Sprite>>,
}
fn atlas_frame_sprite(
    image: Handle<Image>,
    layout: Handle<TextureAtlasLayout>,
    index: usize,
    custom_size: Vec2,
) -> Sprite {
    let mut sprite = Sprite::from_atlas_image(
        image,
        TextureAtlas {
            layout,
            index,
        },
    );
    sprite.custom_size = Some(custom_size);
    sprite
}

impl Graphics {
    /// Wall sheet frame by atlas index (legacy wall objects).
    pub fn wall_sprite(&self, index: usize) -> Sprite {
        Sprite::from_atlas_image(
            self.wall_texture_atlas_image
                .as_ref()
                .expect("wall sprite sheet image is not loaded")
                .clone(),
            TextureAtlas {
                layout: self.wall_texture_atlas_layout
                    .as_ref()
                    .expect("wall sprite sheet layout is not loaded")
                    .clone(),
                index,
            },
        )
    }

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
    pub fn get_heirloom_icon(&self, heirloom: Heirloom) -> Sprite {
        self.heirloom_sprites
            .as_ref()
            .unwrap()
            .get(&heirloom)
            .unwrap_or_else(|| panic!("No graphic for object {:?}", heirloom))
            .clone()
    }
    pub fn get_skill_icon(&self, icon: SkillIcon) -> Handle<Image> {
        self.skill_icons
            .as_ref()
            .unwrap()
            .get(&icon)
            .unwrap_or_else(|| panic!("No skill icon for {:?}", icon))
            .clone()
    }

    pub fn get_active_skill_icon(&self, active_skill: ActiveSkill) -> Handle<Image> {
        self.get_skill_icon(SkillIcon::Active(active_skill))
    }

    pub fn get_class_passive_skill_icon(&self, class: SkillClass) -> Handle<Image> {
        self.get_skill_icon(SkillIcon::ClassPassive(class))
    }

    pub fn get_pet_active_skill_icon(&self, pet: Pet) -> Handle<Image> {
        self.get_skill_icon(SkillIcon::PetActive(pet))
    }

    pub fn get_pet_passive_skill_icon(&self, pet: Pet) -> Handle<Image> {
        self.get_skill_icon(SkillIcon::PetPassive(pet))
    }
    /// Cursor color sprite for the given selection index, wrapping if out of range.
    pub fn get_cursor_color_sprite(&self, index: u8) -> Option<Sprite> {
        self.cursor_color_sprites.as_ref().and_then(|sprites| {
            if sprites.is_empty() {
                None
            } else {
                sprites.get(index as usize % sprites.len()).cloned()
            }
        })
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
    let size = Extent3d {
        width: sprite_desc.size.x as u32,
        height: sprite_desc.size.y as u32,
        depth_or_array_layers: 1,
    };
    //Every pixel is 4 entries in image.data
    let width = original_image.size().x as f32;
    let mut starting_index =
        (sprite_desc.texture_pos.x + width * sprite_desc.texture_pos.y) as usize;
    let Some(image_data) = original_image.data.as_ref() else {
        return assets.add(Image::new(
            size,
            TextureDimension::D2,
            data,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        ));
    };
    for _y in 0..sprite_desc.size.y as usize {
        for x in 0..sprite_desc.size.x as usize {
            let index = starting_index + x;
            //Copy 1 pixel at index
            data.push(image_data[index * 4]);
            data.push(image_data[index * 4 + 1]);
            data.push(image_data[index * 4 + 2]);
            data.push(image_data[index * 4 + 3]);
        }
        starting_index += original_image.size().y as usize;
    }

    let image = Image::new(
        size,
        TextureDimension::D2,
        data,
        //FIXME
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
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
        mut texture_assets: ResMut<Assets<TextureAtlasLayout>>,
        mut world_obj_data: ResMut<WorldObjectResource>,
        asset_server: Res<AssetServer>,
        graphics_desc: Res<Assets<GraphicsDesc>>,
        recipes_desc: Res<Assets<RecipeListProto>>,
        class_pet_desc: Res<Assets<ClassPetData>>,
        class_unlocks_assets: Res<Assets<ClassUnlockConfig>>,
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
            match GameData::try_from_json_reader(reader) {
                Ok(game_data) => {
                    let class_ranks = game_data.class_ranks.clone();
                    let high_scores = game_data.high_scores.clone();
                    let achievements = game_data.achievements.clone();
                    let unlock_upgrades = game_data.unlock_upgrades.clone();
                    let unlocked_classes_vec = game_data.unlocked_classes.clone();
                    let unlocked_skills = game_data.unlocked_skills.clone();
                    let time_fragments = game_data.time_fragments;
                    let seen_tips_set = game_data.seen_tips.clone();
                    let time_crystals = game_data.time_crystals.clone();
                    let beastiary = game_data.beastiary.clone();

                    commands.insert_resource(class_ranks);
                    commands.insert_resource(high_scores);
                    commands.insert_resource(achievements);
                    let seen_tutorial_chunks = seen_tutorial_chunks_from_game_data(&game_data);
                    // Store GameData as a resource so achievements UI can access cumulative analytics
                    commands.insert_resource(game_data);
                    let mut unlocked = UnlockedClasses::new(unlocked_classes_vec);
                    unlocked.ensure_defaults(&get_default_unlocked_classes());
                    commands.insert_resource(unlocked);
                    commands.insert_resource(unlocked_skills);
                    commands.insert_resource(unlock_upgrades);
                    commands.insert_resource(TimeFragmentCurrency::new(
                        (time_fragments.min(i32::MAX as u128)) as i32,
                        0,
                        time_fragments,
                    ));
                    commands.insert_resource(SeenTips {
                        seen: seen_tips_set,
                    });
                    commands.insert_resource(seen_tutorial_chunks);
                    commands.insert_resource(time_crystals);
                    commands.insert_resource(beastiary);
                    commands.insert_resource(crate::player::beastiary::RunBeastiary::default());
                    commands.insert_resource(
                        crate::player::beastiary::LastPlayerAttackerMob::default(),
                    );
                }
                Err(err) => {
                    error!("Failed to load class ranks from game_data.json: {err:?}");
                    commands.insert_resource(crate::player::class_rank::ClassRankSystem::new());
                    commands.insert_resource(UnlockUpgrades::default());
                    let mut unlocked = UnlockedClasses::default();
                    unlocked.ensure_defaults(&get_default_unlocked_classes());
                    commands.insert_resource(unlocked);
                    commands.insert_resource(UnlockedSkills::default());
                    commands.insert_resource(Achievements::default());
                    commands.insert_resource(TimeFragmentCurrency::default());
                    commands.insert_resource(SeenTips::default());
                    commands.insert_resource(SeenTutorialChunks::default());
                    commands.insert_resource(TimeCrystals::default());
                    commands.insert_resource(crate::player::beastiary::Beastiary::default());
                    commands.insert_resource(crate::player::beastiary::RunBeastiary::default());
                    commands.insert_resource(
                        crate::player::beastiary::LastPlayerAttackerMob::default(),
                    );
                }
            }
        } else {
            commands.insert_resource(crate::player::class_rank::ClassRankSystem::new());
            commands.insert_resource(UnlockUpgrades::default());
            let mut unlocked = UnlockedClasses::default();
            unlocked.ensure_defaults(&get_default_unlocked_classes());
            commands.insert_resource(unlocked);
            commands.insert_resource(UnlockedSkills::default());
            commands.insert_resource(Achievements::default());
            commands.insert_resource(TimeFragmentCurrency::default());
            commands.insert_resource(SeenTips::default());
            commands.insert_resource(SeenTutorialChunks::default());
            commands.insert_resource(TimeCrystals::default());
            commands.insert_resource(crate::player::beastiary::Beastiary::default());
            commands.insert_resource(crate::player::beastiary::RunBeastiary::default());
            commands.insert_resource(crate::player::beastiary::LastPlayerAttackerMob::default());
        }
        commands.insert_resource(CoinCurrency::default());
        let sprite_desc_handle: Handle<GraphicsDesc> = sprite_sheet.sprite_desc.clone();
        let recipes_desc_handle: Handle<RecipeListProto> = sprite_sheet.recipes.clone();
        let class_pet_desc_handle: Handle<ClassPetData> = sprite_sheet.class_desc.clone();
        let class_unlock_desc_handle: Handle<ClassUnlockConfig> =
            sprite_sheet.class_unlocks.clone();
        let sprite_desc = graphics_desc.get(&sprite_desc_handle).unwrap();
        let recipes_desc: &RecipeListProto = recipes_desc.get(&recipes_desc_handle).unwrap();
        let class_pet_data = class_pet_desc.get(&class_pet_desc_handle).unwrap();
        if let Some(config) = class_unlocks_assets.get(&class_unlock_desc_handle) {
            commands.insert_resource(ClassUnlockData::from_config(config));
        } else {
            commands.insert_resource(ClassUnlockData::default());
        }
        let mut atlas = TextureAtlasLayout::new_empty(UVec2::new(256, 736));
        let wall_atlas = TextureAtlasLayout::from_grid(UVec2::new(16, 32), 32, 4, None, None);

        let mut spritesheet_entries: Vec<(WorldObject, usize, Vec2)> = Vec::new();
        let mut icon_entries: Vec<(WorldObject, usize, Vec2)> = Vec::new();
        let mut heirloom_entries: Vec<(Heirloom, usize, Vec2)> = Vec::new();
        let mut cursor_entries: Vec<(usize, Vec2)> = Vec::new();
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
            let index = atlas.add_texture(rect.to_atlas_rect());
            spritesheet_entries.push((*item, index, rect.size));
            world_obj_data.properties.insert(*item, *rect);
        }

        // load icons
        for (item, rect) in sprite_desc.icons.iter() {
            let index = atlas.add_texture(URect::from_corners(
                UVec2::new(
                    (rect.texture_pos.x * 16.) as u32,
                    (rect.texture_pos.y * 16.) as u32,
                ),
                UVec2::new(
                    (rect.texture_pos.x * 16. + rect.size.x) as u32,
                    (rect.texture_pos.y * 16. + rect.size.y) as u32,
                ),
            ));
            icon_entries.push((*item, index, rect.size));
        }

        // load heirloom sprites
        for (heirloom, rect) in sprite_desc.heirlooms.iter() {
            let index = atlas.add_texture(rect.to_atlas_rect());
            heirloom_entries.push((heirloom.clone(), index, rect.size));
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
        // Active skills, class passives, and pet active/passive icons.
        let skill_icon_handles = load_skill_icons(&asset_server);
        // load Item Glows
        for u in ItemGlow::iter() {
            let handle = asset_server.load(format!("effects/{u}ItemGlow.png"));
            item_glow_handles.insert(u, handle);
        }

        // Selectable cursor color sprites laid out horizontally on row 1 of the sheet,
        // starting at column 4 (positions (4,1)..(11,1)).
        for i in 0..crate::cursor::NUM_CURSOR_COLORS {
            let rect = WorldObjectData {
                texture_pos: Vec2::new(4. + i as f32, 1.),
                size: Vec2::new(16., 16.),
                anchor: None,
            };
            let index = atlas.add_texture(rect.to_atlas_rect());
            cursor_entries.push((index, rect.size));
        }

        let atlas_handle = texture_assets.add(atlas);
        let wall_atlas_handle = texture_assets.add(wall_atlas);

        let spritesheet_map = spritesheet_entries
            .into_iter()
            .map(|(item, index, size)| {
                (
                    item,
                    atlas_frame_sprite(
                        image_handle.clone(),
                        atlas_handle.clone(),
                        index,
                        size,
                    ),
                )
            })
            .collect::<HashMap<_, _>>();
        let icon_map = icon_entries
            .into_iter()
            .map(|(item, index, size)| {
                (
                    item,
                    atlas_frame_sprite(
                        image_handle.clone(),
                        atlas_handle.clone(),
                        index,
                        size,
                    ),
                )
            })
            .collect::<HashMap<_, _>>();
        let heirloom_sprites = heirloom_entries
            .into_iter()
            .map(|(heirloom, index, size)| {
                (
                    heirloom,
                    atlas_frame_sprite(
                        image_handle.clone(),
                        atlas_handle.clone(),
                        index,
                        size,
                    ),
                )
            })
            .collect::<HashMap<_, _>>();
        let cursor_color_sprites = cursor_entries
            .into_iter()
            .map(|(index, size)| {
                atlas_frame_sprite(
                    image_handle.clone(),
                    atlas_handle.clone(),
                    index,
                    size,
                )
            })
            .collect::<Vec<_>>();

        *graphics = Graphics {
            texture_atlas_layout: Some(atlas_handle),
            texture_atlas_image: Some(image_handle),
            wall_texture_atlas_layout: Some(wall_atlas_handle),
            wall_texture_atlas_image: Some(wall_image_handle),
            spritesheet_map: Some(spritesheet_map),
            ui_image_handles: Some(ui_image_handles),
            icons: Some(icon_map),
            mob_spritesheets: Some(mob_spritesheets),
            status_effect_icons: Some(status_effect_handles),
            skill_icons: Some(skill_icon_handles),
            heirloom_skill_icons: Some(skill_handles),
            heirloom_sprites: Some(heirloom_sprites),
            item_glows: Some(item_glow_handles),
            combat_shrine_anim: Some(asset_server.load(CombatShrineAnim::PATH)),
            shrine_eye: Some(asset_server.load(ShrineEye::PATH)),
            shrine_repair_ring: Some(asset_server.load(ShrineRepairRingAnim::PATH)),
            weapon_shrine_anim: Some(asset_server.load(WeaponShrineAnim::PATH)),
            armor_shrine_anim: Some(asset_server.load(ArmorShrineAnim::PATH)),
            accessory_shrine_anim: Some(asset_server.load(AccessoryShrineAnim::PATH)),
            ice_explosion_ase: Some(asset_server.load(IceExplosion::PATH)),
            small_explosion_ase: Some(asset_server.load(SmallExplosion::PATH)),
            cherry_bomb_ase: Some(asset_server.load("textures/effects/CherryBomb.aseprite")),
            cherry_bomb_explosion_ase: Some(
                asset_server.load("textures/effects/CherryBombExplosion.ase"),
            ),
            bomb_ase: Some(asset_server.load("textures/effects/Bomb.ase")),
            portal_ase: Some(asset_server.load(Portal::PATH)),
            ui_portal_ase: Some(asset_server.load(UIPortal::PATH)),
            stone_pillar_ase: Some(asset_server.load("textures/stonegolem/StonePillar.ase")),
            pink_flower_ase: Some(asset_server.load(PinkFlowerAseprite::PATH)),
            void_laser_ase: Some(asset_server.load("textures/VoidWorm/VoidLaser.ase")),
            inv_stat_highlight_common_ase: Some(
                asset_server.load("textures/effects/InventoryStatHighlightCommon.ase"),
            ),
            inv_stat_highlight_uncommon_ase: Some(
                asset_server.load("textures/effects/InventoryStatHighlightUncommon.ase"),
            ),
            inv_stat_highlight_rare_ase: Some(
                asset_server.load("textures/effects/InventoryStatHighlightRare.ase"),
            ),
            inv_stat_highlight_legendary_ase: Some(
                asset_server.load("textures/effects/InventoryStatHighlightLegendary.ase"),
            ),
            class_pet_data: Some(class_pet_data.clone()),
            cursor_color_sprites: Some(cursor_color_sprites),
        };
    }
}

pub fn _get_index_from_pixel_cords(p: WorldObjectData) -> usize {
    (p.texture_pos.y + (p.texture_pos.x / 16.)) as usize
}
