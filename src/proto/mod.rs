//! Legacy module name kept for call-site compatibility.
//! Runtime definitions live in [`crate::defs`]; this plugin only registers Reflect
//! types used by save/inspector and flushes the shared [`CustomFlush`] set.

use std::ops::{Range, RangeInclusive};

use bevy::{platform::collections::HashMap, prelude::*};

pub mod proto_param;

use crate::{
    ai::IdleState,
    animations::{
        enemy_sprites::{
            CharacterAnimationSpriteSheetData, EnemyAnimationState, LeftFacingSideProfile,
        },
        AnimationFrameTracker, AnimationPosTracker, AnimationTimer, DoneAnimation, FadeOpacity,
    },
    assets::{SpriteAnchor, SpriteSize},
    attributes::{
        Attack, AttributeQuality, AttributeValue, ItemAttributes, ItemRarity, MaxHealth,
        RawItemBaseAttributes, RawItemBonusAttributes,
    },
    enemy::{
        scorpion::{ScorpionClawAttack, ScorpionTailAttack, ScorpionTornadoAttack},
        BullChargeAttack, CircleAttack, CombatAlignment, FollowSpeed, LaserAttack, LeapAttack, Mob,
        MobLevel, MultiLeapAttack, ProjectileAttack,
    },
    inputs::FacingDirection,
    inventory::ItemStack,
    item::{
        item_actions::{ConsumableItem, ItemAction, ItemActions, ManaCost},
        item_upgrades::ClawUpgradeMultiThrow,
        melee::MeleeAttack,
        object_actions::{ObjectAction, ObjectActionCost, TouchTriggerObjectAction},
        projectile::{ArcProjectileData, Projectile, ProjectileState, RangedAttack},
        Block, BreaksWith, EquipmentType, FoliageSize, ItemDisplayMetaData, Loot, LootTable,
        PlacesInto, RequiredEquipmentType, Wall, WorldObject,
    },
    player::levels::ExperienceReward,
    sapling::{GrowsInto, Sapling},
    status_effects::{StatusEffectState, StatusEffectTracker},
    ui::{
        crafting_ui::CraftingContainerType,
        scrapper_ui::{Scrap, ScrapsInto},
    },
    world::{ForestGenerationParams, ShrineCount, WallTextureData},
    CustomFlush, Pet, PetState, YSort,
};

pub struct ProtoPlugin;

impl Plugin for ProtoPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.register_type::<FadeOpacity>()
            .register_type::<CombatAlignment>()
            .register_type::<AnimationFrameTracker>()
            .register_type::<EnemyAnimationState>()
            .register_type::<MaxHealth>()
            .register_type::<LootTable>()
            .register_type::<Loot>()
            .register_type::<Vec<Loot>>()
            .register_type::<WorldObject>()
            .register_type::<Option<WorldObject>>()
            .register_type::<PlacesInto>()
            .register_type::<BreaksWith>()
            .register_type::<Block>()
            .register_type::<DoneAnimation>()
            .register_type::<Wall>()
            .register_type::<Projectile>()
            .register_type::<ProjectileState>()
            .register_type::<RangedAttack>()
            .register_type::<Attack>()
            .register_type::<MeleeAttack>()
            .register_type::<ItemStack>()
            .register_type::<WallTextureData>()
            .register_type::<ItemAttributes>()
            .register_type::<RawItemBaseAttributes>()
            .register_type::<RawItemBonusAttributes>()
            .register_type::<ExperienceReward>()
            .register_type::<GrowsInto>()
            .register_type::<Sapling>()
            .register_type::<ItemDisplayMetaData>()
            .register_type::<YSort>()
            .register_type::<IdleState>()
            .register_type::<AnimationTimer>()
            .register_type::<MobLevel>()
            .register_type::<SpriteSize>()
            .register_type::<SpriteAnchor>()
            .register_type::<ItemAction>()
            .register_type::<ItemActions>()
            .register_type::<AttributeValue>()
            .register_type::<AttributeQuality>()
            .register_type::<ObjectAction>()
            .register_type::<ConsumableItem>()
            .register_type::<FoliageSize>()
            .register_type::<ArcProjectileData>()
            .register_type::<FollowSpeed>()
            .register_type::<EquipmentType>()
            .register_type::<ItemRarity>()
            .register_type::<LeftFacingSideProfile>()
            .register_type::<RequiredEquipmentType>()
            .register_type::<TouchTriggerObjectAction>()
            .register_type::<ClawUpgradeMultiThrow>()
            .register_type::<ObjectActionCost>()
            .register_type::<ManaCost>()
            .register_type::<Option<ItemStack>>()
            .register_type::<Pet>()
            .register_type::<PetState>()
            .register_type::<FacingDirection>()
            .register_type::<ForestGenerationParams>()
            .register_type::<ShrineCount>()
            .register_type::<HashMap<WorldObject, ShrineCount>>()
            .register_type::<CraftingContainerType>()
            .register_type::<LeapAttack>()
            .register_type::<ProjectileAttack>()
            .register_type::<Scrap>()
            .register_type::<ScrapsInto>()
            .register_type::<Vec<Scrap>>()
            .register_type::<StatusEffectTracker>()
            .register_type::<StatusEffectState>()
            .register_type::<CharacterAnimationSpriteSheetData>()
            .register_type::<AnimationPosTracker>()
            .register_type::<HashMap<WorldObject, Vec<WorldObject>>>()
            .register_type::<HashMap<WorldObject, f64>>()
            .register_type::<HashMap<WorldObject, f32>>()
            .register_type::<Vec<WorldObject>>()
            .register_type::<Vec<u8>>()
            .register_type::<Vec<StatusEffectState>>()
            .register_type::<Vec<f32>>()
            .register_type::<Vec<String>>()
            .register_type::<Vec<ItemAction>>()
            .register_type::<Option<Range<i32>>>()
            .register_type::<Option<RangeInclusive<i32>>>()
            .register_type::<Option<u8>>()
            .register_type::<Range<i32>>()
            .register_type::<CircleAttack>()
            .register_type::<MultiLeapAttack>()
            .register_type::<BullChargeAttack>()
            .register_type::<LaserAttack>()
            .register_type::<ScorpionClawAttack>()
            .register_type::<ScorpionTailAttack>()
            .register_type::<ScorpionTornadoAttack>()
            .register_type::<RangeInclusive<i32>>()
            .register_type::<Mob>()
            .register_type_data::<Range<i32>, ReflectDeserialize>()
            .register_type_data::<RangeInclusive<i32>, ReflectDeserialize>()
            .add_systems(Update, ApplyDeferred.in_set(CustomFlush));
    }
}
