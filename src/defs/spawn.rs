//! Spawn entities from [`EntityDef`] / apply era resources from [`GameDefs`].

use bevy::{ecs::system::EntityCommands, prelude::*};
use bevy_rapier2d::prelude::{
    ActiveEvents, Collider, CollisionGroups, Group, KinematicCharacterController, QueryFilterFlags,
    Sensor,
};

use crate::{
    ai::IdleState,
    animations::{AnimationTimer, DoneAnimation},
    combat::status_effects::StatusEffectTracker,
    enemy::MobLevel,
    inputs::FacingDirection,
    item::WorldObject,
    sapling::Sapling,
    world::WorldGeneration,
};

use super::registry::GameDefs;
use super::types::{ColliderDef, ColliderKind, EntityDef, SpriteSheetDef};

/// Queued on spawn; resolved into a sprite with an atlas layout before gameplay systems run.
#[derive(Component, Clone)]
pub struct PendingSpriteSheet(pub SpriteSheetDef);

/// Standalone PNG from old `SpriteBundle` — applied when `spawn_object_from_proto` is not used.
#[derive(Component, Clone)]
pub struct PendingSpriteTexture(pub String);

/// Insert all definition components onto an entity (no Transform / ActiveEvents).
pub fn insert_entity_def(entity: &mut EntityCommands, def: &EntityDef) {
    entity.insert(Name::new(def.name.clone()));

    if let Some(v) = def.world_object {
        entity.insert(v);
    }
    if let Some(v) = def.mob.clone() {
        entity.insert(v);
    }
    if let Some(v) = def.projectile {
        entity.insert(v);
    }
    if let Some(v) = def.item_stack.clone() {
        entity.insert(v);
    }
    if let Some(v) = def.sprite_size.clone() {
        entity.insert(v);
    }
    if let Some(v) = def.sprite_anchor.clone() {
        entity.insert(v);
    }
    if let Some(v) = def.y_sort.clone() {
        entity.insert(v);
    }
    if let Some(col) = &def.collider {
        entity.insert(collider_from_def(col));
    }
    if def.sensor {
        entity.insert(Sensor);
    }
    if def.kcc {
        entity.insert(kcc_default());
    }
    if let Some(sheet) = &def.sprite_sheet {
        entity.insert(PendingSpriteSheet(sheet.clone()));
    }
    if let Some(path) = &def.sprite_texture {
        entity.insert(PendingSpriteTexture(path.clone()));
    }
    if let Some(t) = &def.animation_timer {
        entity.insert(AnimationTimer(Timer::from_seconds(
            t.secs,
            TimerMode::Repeating,
        )));
    }
    if let Some(v) = def.max_health {
        entity.insert(v);
    }
    if let Some(v) = def.attack {
        entity.insert(v);
    }
    if let Some(v) = def.experience_reward.clone() {
        entity.insert(v);
    }
    if let Some(v) = def.loot_table.clone() {
        entity.insert(v);
    }
    if let Some(v) = def.equipment_type.clone() {
        entity.insert(v);
    }
    if let Some(v) = def.required_equipment_type.clone() {
        entity.insert(v);
    }
    if let Some(v) = def.ranged.clone() {
        entity.insert(v);
    }
    if let Some(v) = def.melee.clone() {
        entity.insert(v);
    }
    if let Some(v) = def.projectile_state.clone() {
        entity.insert(v);
    }
    if let Some(v) = def.combat_alignment.clone() {
        entity.insert(v);
    }
    if let Some(v) = def.follow_speed.clone() {
        entity.insert(v);
    }
    if let Some(v) = def.item_actions.clone() {
        entity.insert(v);
    }
    if let Some(v) = def.consumable.clone() {
        entity.insert(v);
    }
    if let Some(v) = def.mana_cost.clone() {
        entity.insert(v);
    }
    if let Some(v) = def.object_action.clone() {
        entity.insert(v);
    }
    if let Some(v) = def.object_action_cost.clone() {
        entity.insert(v);
    }
    if let Some(v) = def.raw_item_base.clone() {
        entity.insert(v);
    }
    if let Some(v) = def.raw_item_bonus.clone() {
        entity.insert(v);
    }
    if let Some(v) = def.scraps_into.clone() {
        entity.insert(v);
    }
    if let Some(v) = def.grows_into.clone() {
        entity.insert(v);
    }
    if let Some(secs) = def.sapling_secs {
        entity.insert(Sapling(Timer::from_seconds(secs, TimerMode::Once)));
    }
    if let Some(v) = def.wall {
        entity.insert(v);
    }
    if let Some(v) = def.foliage {
        entity.insert(v);
    }
    if let Some(v) = def.foliage_size.clone() {
        entity.insert(v);
    }
    if let Some(v) = def.places_into.clone() {
        entity.insert(v);
    }
    if let Some(v) = def.breaks_with.clone() {
        entity.insert(v);
    }
    if let Some(v) = def.block.clone() {
        entity.insert(v);
    }
    if def.done_animation {
        entity.insert(DoneAnimation);
    }
    if let Some(v) = def.fade_opacity.clone() {
        entity.insert(v);
    }
    if let Some(v) = def.animation_pos_tracker.clone() {
        entity.insert(v);
    }
    if let Some(v) = def.enemy_anim_state.clone() {
        entity.insert(v);
    }
    if let Some(v) = def.anim_sprite_sheet_data.clone() {
        entity.insert(v);
    }
    if let Some(v) = def.leap_attack.clone() {
        entity.insert(v);
    }
    if def.status_effect_tracker {
        entity.insert(StatusEffectTracker {
            effects: Vec::new(),
        });
    }
    if let Some((walk_dir_change_time, speed)) = def.idle_state {
        entity.insert(IdleState {
            walk_timer: Timer::from_seconds(walk_dir_change_time, TimerMode::Repeating),
            direction: FacingDirection::new_rand_dir(rand::thread_rng()),
            speed,
            is_stopped: false,
        });
    }
    if let Some(v) = def.mob_level {
        entity.insert(MobLevel(v));
    }
    if let Some(v) = def.wall_texture_data.clone() {
        entity.insert(v);
    }
    if let Some(v) = def.touch_trigger.clone() {
        entity.insert(v);
    }
    if let Some(v) = def.projectile_attack.clone() {
        entity.insert(v);
    }
    if let Some(v) = def.pet {
        entity.insert(v);
    }
    if def.left_facing_side_profile {
        entity.insert(crate::animations::enemy_sprites::LeftFacingSideProfile);
    }
    if let Some(v) = def.animation_frame_tracker {
        entity.insert(v);
    }
    if let Some(v) = def.multi_leap_attack.clone() {
        entity.insert(v);
    }
    if let Some(v) = def.bull_charge_attack.clone() {
        entity.insert(v);
    }
    if let Some(v) = def.arc_projectile_data.clone() {
        entity.insert(v);
    }
    if let Some(v) = def.scorpion_claw_attack.clone() {
        entity.insert(v);
    }
    if let Some(v) = def.scorpion_tail_attack.clone() {
        entity.insert(v);
    }
    if let Some(v) = def.scorpion_tornado_attack.clone() {
        entity.insert(v);
    }
    if let Some(v) = def.circle_attack.clone() {
        entity.insert(v);
    }
    if let Some(v) = def.laser_attack.clone() {
        entity.insert(v);
    }

    entity.insert(Visibility::default());
}

pub fn spawn_from_def(commands: &mut Commands, def: &EntityDef, pos: Vec2) -> Entity {
    let mut entity = commands.spawn_empty();
    insert_entity_def(&mut entity, def);
    let id = entity.id();
    // TransformBundle (not bare Transform) — atlas/sprite rendering needs GlobalTransform.
    // Old proto SpriteSheetBundle templates supplied this; GameDefs spawn must too.
    commands.entity(id).insert((
        Transform::from_translation(pos.extend(0.)),
        ActiveEvents::COLLISION_EVENTS,
    ));
    id
}

pub fn apply_era_generation(commands: &mut Commands, defs: &GameDefs, era_name: &str) {
    match apply_era_generation_resource(defs, era_name) {
        Some(gen) => {
            commands.insert_resource(gen);
        }
        None => {
            error!("GameDefs missing era generation params: {era_name}");
        }
    }
}

pub fn apply_era_generation_resource(defs: &GameDefs, era_name: &str) -> Option<WorldGeneration> {
    defs.get_era(era_name).map(|e| e.world_generation.clone())
}

/// Resolves [`PendingSpriteSheet`] into a [`Sprite`].
///
/// Important: only insert/replace the `Sprite`. Re-inserting `Transform` here used to
/// clobber elite scale (and any other in-place Transform edits) when this system's
/// deferred `insert` flushed after those systems ran.
pub fn apply_pending_sprite_sheets(
    mut commands: Commands,
    pending: Query<(Entity, &PendingSpriteSheet)>,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    for (entity, pending) in pending.iter() {
        let texture_handle = asset_server.load(&pending.0.asset);
        let texture_atlas_layout = TextureAtlasLayout::from_grid(
            UVec2::new(pending.0.size.x as u32, pending.0.size.y as u32),
            pending.0.cols as u32,
            pending.0.rows as u32,
            None,
            None,
        );
        let layout = texture_atlas_layouts.add(texture_atlas_layout);
        let Ok(mut e_cmds) = commands.get_entity(entity) else {
            continue;
        };
        e_cmds
            .try_insert(Sprite {
                image: texture_handle,
                texture_atlas: Some(TextureAtlas { layout, index: 0 }),
                ..default()
            })
            .remove::<PendingSpriteSheet>();
    }
}

pub fn apply_pending_sprite_textures(
    mut commands: Commands,
    pending: Query<(Entity, &PendingSpriteTexture, Option<&WorldObject>)>,
    asset_server: Res<AssetServer>,
) {
    for (entity, pending, world_object) in pending.iter() {
        let custom_size = world_object
            .filter(|o| **o == WorldObject::BossShrine)
            .map(|_| Vec2::new(128., 128.));
        let Ok(mut e_cmds) = commands.get_entity(entity) else {
            continue;
        };
        e_cmds
            .try_insert(Sprite {
                image: asset_server.load(&pending.0),
                custom_size,
                ..default()
            })
            .remove::<PendingSpriteTexture>()
            .remove::<PendingSpriteSheet>();
    }
}

pub struct DefsSpawnPlugin;

impl Plugin for DefsSpawnPlugin {
    fn build(&self, app: &mut App) {
        // Resolve custom sprite sheets the same frame they were queued (before most Update systems).
        app.add_systems(
            Update,
            (apply_pending_sprite_sheets, apply_pending_sprite_textures),
        );
    }
}

fn collider_from_def(col: &ColliderDef) -> Collider {
    match col.kind {
        ColliderKind::Cuboid { x, y } => Collider::cuboid(x, y),
        ColliderKind::Capsule { x1, y1, x2, y2, r } => {
            Collider::capsule(Vec2::new(x1, y1), Vec2::new(x2, y2), r)
        }
    }
}

fn kcc_default() -> KinematicCharacterController {
    KinematicCharacterController {
        filter_flags: QueryFilterFlags::EXCLUDE_SENSORS | QueryFilterFlags::EXCLUDE_KINEMATIC,
        filter_groups: Some(CollisionGroups::new(Group::GROUP_1, Group::GROUP_1)),
        ..default()
    }
}
