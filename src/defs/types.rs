use bevy::prelude::Vec2;

use crate::{
    animations::{
        enemy_sprites::{CharacterAnimationSpriteSheetData, EnemyAnimationState},
        AnimationFrameTracker, AnimationPosTracker, FadeOpacity,
    },
    assets::{SpriteAnchor, SpriteSize},
    attributes::{Attack, MaxHealth, RawItemBaseAttributes, RawItemBonusAttributes},
    enemy::{
        scorpion::{ScorpionClawAttack, ScorpionTailAttack, ScorpionTornadoAttack},
        BullChargeAttack, CircleAttack, CombatAlignment, FollowSpeed, LaserAttack, LeapAttack, Mob,
        MultiLeapAttack, ProjectileAttack,
    },
    inventory::ItemStack,
    item::{
        item_actions::{ConsumableItem, ItemActions, ManaCost},
        melee::MeleeAttack,
        object_actions::{ObjectAction, ObjectActionCost, TouchTriggerObjectAction},
        projectile::{ArcProjectileData, Projectile, ProjectileState, RangedAttack},
        Block, BreaksWith, EquipmentType, Foliage, FoliageSize, LootTable, PlacesInto,
        RequiredEquipmentType, Wall, WorldObject,
    },
    pets::state::Pet,
    player::levels::ExperienceReward,
    sapling::GrowsInto,
    ui::scrapper_ui::ScrapsInto,
    world::{y_sort::YSort, WallTextureData, WorldGeneration},
};

#[derive(Clone, Debug, Default)]
pub struct ColliderDef {
    pub kind: ColliderKind,
}

#[derive(Clone, Debug)]
pub enum ColliderKind {
    Cuboid {
        x: f32,
        y: f32,
    },
    Capsule {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        r: f32,
    },
}

impl Default for ColliderKind {
    fn default() -> Self {
        Self::Cuboid { x: 0.0, y: 0.0 }
    }
}

#[derive(Clone, Debug, Default)]
pub struct SpriteSheetDef {
    pub asset: String,
    pub size: Vec2,
    pub cols: usize,
    pub rows: usize,
}

#[derive(Clone, Debug, Default)]
pub struct AnimationTimerDef {
    pub secs: f32,
}

/// Unified entity definition — one registry entry per prototype name.
/// Fields mirror what prototypes carried; all optional except name.
#[derive(Clone, Default)]
pub struct EntityDef {
    pub name: String,
    /// Template names merged before this def (for documentation / regen)
    pub templates: Vec<String>,
    pub world_object: Option<WorldObject>,
    pub mob: Option<Mob>,
    pub projectile: Option<Projectile>,
    pub item_stack: Option<ItemStack>,
    pub sprite_size: Option<SpriteSize>,
    pub sprite_anchor: Option<SpriteAnchor>,
    pub y_sort: Option<YSort>,
    pub collider: Option<ColliderDef>,
    pub sensor: bool,
    pub kcc: bool,
    pub sprite_sheet: Option<SpriteSheetDef>,
    /// Standalone PNG path from old `SpriteBundle` (trees, large cactuses, etc.).
    pub sprite_texture: Option<String>,

    pub animation_timer: Option<AnimationTimerDef>,
    pub max_health: Option<MaxHealth>,
    pub attack: Option<Attack>,
    pub experience_reward: Option<ExperienceReward>,
    pub loot_table: Option<LootTable>,
    pub equipment_type: Option<EquipmentType>,
    pub required_equipment_type: Option<RequiredEquipmentType>,
    pub ranged: Option<RangedAttack>,
    pub melee: Option<MeleeAttack>,
    pub projectile_state: Option<ProjectileState>,
    pub combat_alignment: Option<CombatAlignment>,
    pub follow_speed: Option<FollowSpeed>,
    pub item_actions: Option<ItemActions>,
    pub consumable: Option<ConsumableItem>,
    pub mana_cost: Option<ManaCost>,
    pub object_action: Option<ObjectAction>,
    pub object_action_cost: Option<ObjectActionCost>,
    pub raw_item_base: Option<RawItemBaseAttributes>,
    pub raw_item_bonus: Option<RawItemBonusAttributes>,
    pub scraps_into: Option<ScrapsInto>,
    pub grows_into: Option<GrowsInto>,
    /// Sapling growth timer seconds (`SaplingProto`).
    pub sapling_secs: Option<f32>,
    pub wall: Option<Wall>,
    pub foliage: Option<Foliage>,
    pub foliage_size: Option<FoliageSize>,
    pub places_into: Option<PlacesInto>,
    pub breaks_with: Option<BreaksWith>,
    pub block: Option<Block>,
    pub done_animation: bool,
    pub fade_opacity: Option<FadeOpacity>,
    pub animation_pos_tracker: Option<AnimationPosTracker>,
    pub enemy_anim_state: Option<EnemyAnimationState>,
    pub anim_sprite_sheet_data: Option<CharacterAnimationSpriteSheetData>,
    pub leap_attack: Option<LeapAttack>,
    pub status_effect_tracker: bool,
    pub idle_state: Option<(f32, f32)>,
    pub mob_level: Option<u8>,
    pub wall_texture_data: Option<WallTextureData>,
    pub touch_trigger: Option<TouchTriggerObjectAction>,
    pub projectile_attack: Option<ProjectileAttack>,
    pub pet: Option<Pet>,
    pub left_facing_side_profile: bool,
    pub animation_frame_tracker: Option<AnimationFrameTracker>,
    pub multi_leap_attack: Option<MultiLeapAttack>,
    pub bull_charge_attack: Option<BullChargeAttack>,
    pub arc_projectile_data: Option<ArcProjectileData>,
    pub scorpion_claw_attack: Option<ScorpionClawAttack>,
    pub scorpion_tail_attack: Option<ScorpionTailAttack>,
    pub scorpion_tornado_attack: Option<ScorpionTornadoAttack>,
    pub circle_attack: Option<CircleAttack>,
    pub laser_attack: Option<LaserAttack>,
}

#[derive(Clone, Debug)]
pub struct EraDef {
    pub name: String,
    pub world_generation: WorldGeneration,
}
