pub mod enemy_sprites;
pub mod game_over;

use std::cmp::max;
use std::f32::consts::PI;

pub mod ui_animaitons;
use bevy::shader::ShaderRef;
use bevy::sprite_render::MeshMaterial2d;
use bevy::sprite_render::{AlphaMode2d, Material2d, Material2dPlugin};
use bevy::{prelude::*, render::render_resource::AsBindGroup};
use bevy_rapier2d::prelude::KinematicCharacterController;
use game_over::{
    handle_game_over_fadeout, handle_game_over_final_stats_tooltip,
    handle_spawn_collected_time_fragments, maintain_player_red_tint, tick_game_over_overlay,
    update_game_over_rank_text,
};
use player_sprite::{
    change_player_class_visuals, cleanup_one_time_animations,
    handle_anim_change_when_player_dir_changes, handle_player_animation_change,
    handle_restart_player_attack_anim, preload_player_sprites, PlayerAnimation,
};
use serde::{Deserialize, Serialize};
use ui_animaitons::{handle_move_animations, handle_ui_time_fragments};
pub mod player_sprite;

use crate::ai::LeapAttackState;
use crate::enemy::{EnemyMaterial, Mob};
use crate::inputs::{mouse_click_system, FacingDirection, MovementVector};
use crate::item::projectile::{ArcProjectileData, Projectile, ProjectileState, RangedAttackEvent};
use crate::item::{Equipment, MainHand, WorldObject, PLAYER_EQUIPMENT_POSITIONS};
use crate::player::Limb;
use crate::sapling::Sapling;
use crate::ui::CleanUpRunStateEvent;
use crate::world::chunk::Chunk;
use crate::{inventory::ItemStack, Game, Player};
use crate::{GameParam, GameState};

use self::enemy_sprites::{
    animate_character_spritesheet_animations,
    change_anim_offset_when_character_action_state_changes, change_character_anim_direction,
    CharacterAnimationSpriteSheetData, EnemyAnimationState,
};

pub struct AnimationsPlugin;

#[derive(Component, Reflect, Default, Clone, Debug)]
pub struct AnimationPosTracker(pub f32, pub f32, pub f32);

#[derive(Component, Reflect, Default, Clone, Copy, Debug, Deserialize)]
pub struct AnimationFrameTracker(pub i32, pub i32);

#[derive(Component, Clone, Deref, DerefMut, Reflect)]
pub struct AnimationTimer(pub Timer);
/// Per-entity hit-reaction state.
///
/// Used to be a transient component inserted on hit and removed when the
/// animation finished. That insert/remove churn was a major contributor to
/// archetype fragmentation on mobs (every mob cycled through ~2 archetype
/// variants per hit, combining with every other transient component). It's
/// now always-present on mobs (added by `ensure_mob_components`) and driven
/// by the `is_active` flag: set to `true` when a hit lands, flipped back to
/// `false` when the timer finishes. Non-mob entities (player, world objects)
/// may still have it inserted on demand; once present it's never removed.
#[derive(Component, Debug, Clone)]
pub struct HitAnimationTracker {
    pub is_active: bool,
    pub timer: Timer,
    pub knockback: f32,
    pub dir: Vec2,
}

impl Default for HitAnimationTracker {
    fn default() -> Self {
        Self {
            is_active: false,
            timer: Timer::from_seconds(0.0, TimerMode::Once),
            knockback: 0.0,
            dir: Vec2::ZERO,
        }
    }
}

/// Marker inserted on one-shot animation entities so the cleanup system can
/// tear them down after the animation finishes. Highly transient — stored
/// `SparseSet` so attaching/detaching doesn't move the animation entity
/// through extra archetypes every time an effect plays.
#[derive(Component, Reflect, Debug)]
#[component(storage = "SparseSet")]
pub struct DoneAnimation;

#[derive(Debug, Clone, Default, Message)]
pub struct AttackEvent {
    pub direction: Vec2,
    pub ignore_cooldown: bool,
}
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct AnimatedTextureMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub source_texture: Option<Handle<Image>>,
    #[texture(2)]
    #[sampler(3)]
    pub lookup_texture: Option<Handle<Image>>,
    #[uniform(4)]
    pub flip: f32,
    #[uniform(5)]
    pub opacity: f32,
}

impl Material2d for AnimatedTextureMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/texture_map.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

impl Plugin for AnimationsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(Material2dPlugin::<AnimatedTextureMaterial>::default())
            .add_message::<CleanUpRunStateEvent>()
            .add_systems(OnExit(GameState::Loading), preload_player_sprites)
            .add_systems(
                Update,
                (
                    change_anim_offset_when_character_action_state_changes,
                    change_character_anim_direction,
                    animate_character_spritesheet_animations,
                    animate_enemies,
                    animate_dropped_items,
                    move_player_attack_collider,
                    animate_hit,
                    animate_foliage_opacity,
                )
                    .run_if(in_state(GameState::Main)),
            )
            .add_systems(
                Update,
                animate_spritesheet_animations
                    .after(mouse_click_system)
                    .run_if(in_state(GameState::Main)),
            )
            .add_systems(
                Update,
                handle_game_over_fadeout.run_if(in_state(GameState::Main)),
            )
            .add_systems(
                Update,
                (
                    handle_anim_change_when_player_dir_changes,
                    handle_player_animation_change,
                    handle_restart_player_attack_anim,
                    cleanup_one_time_animations,
                )
                    .chain()
                    .run_if(in_state(GameState::Main)),
            )
            .add_systems(
                Update,
                (change_player_class_visuals,).run_if(in_state(GameState::Main)),
            )
            .add_systems(
                Update,
                (
                    tick_game_over_overlay,
                    handle_spawn_collected_time_fragments,
                    update_game_over_rank_text,
                    handle_game_over_final_stats_tooltip,
                    handle_move_animations,
                    handle_ui_time_fragments,
                ),
            )
            .add_systems(
                Update,
                maintain_player_red_tint.run_if(in_state(GameState::GameOver)),
            );
    }
}

fn animate_enemies(
    time: Res<Time>,
    _game: Res<Game>,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<EnemyMaterial>>,
    mut enemy_query: Query<(
        &mut AnimationFrameTracker,
        &mut AnimationTimer,
        &MeshMaterial2d<EnemyMaterial>,
        &Mob,
        Option<&LeapAttackState>,
    )>,
) {
    for (mut tracker, mut timer, enemy_mat, _enemy, att_option) in enemy_query.iter_mut() {
        timer.tick(time.delta());

        let frame_changed = timer.just_finished();
        if frame_changed {
            tracker.0 = max((tracker.0 + 1) % (tracker.1 - 1), 0);
        }

        let new_attacking = att_option.map_or(0., |attack| {
            if attack.attack_startup_timer.is_finished()
                && !attack.attack_duration_timer.is_finished()
            {
                1.
            } else {
                0.
            }
        });

        let needs_material_update = frame_changed
            || materials
                .get(&enemy_mat.0)
                .map_or(false, |m| m.is_attacking != new_attacking);

        if !needs_material_update {
            continue;
        }

        if let Some(mut mat) = materials.get_mut(&enemy_mat.0) {
            mat.source_texture = Some(
                asset_server.load(format!("textures/slime/{}-move-{}.png", "slime", tracker.0)),
            );
            mat.is_attacking = new_attacking;
        }
    }
}
fn animate_dropped_items(
    time: Res<Time>,
    mut drop_query: Query<
        (
            &mut Transform,
            &mut AnimationTimer,
            &mut AnimationPosTracker,
        ),
        With<ItemStack>,
    >,
) {
    for (mut transform, mut timer, mut tracker) in &mut drop_query {
        let d = time.delta();
        let s = tracker.2;
        timer.tick(d);
        if timer.just_finished() {
            transform.translation.y += s;
            tracker.1 += s;

            if tracker.1 <= -2. || tracker.1 >= 2. {
                tracker.2 *= -1.;
            }
        }
    }
}
fn animate_hit(
    mut commands: Commands,
    mut transforms: Query<&mut Transform>,
    mut hit_tracker: Query<(
        Entity,
        &mut HitAnimationTracker,
        Option<&EnemyAnimationState>,
    )>,
    mut player: Query<
        (
            Entity,
            &mut KinematicCharacterController,
            &mut MovementVector,
        ),
        With<Player>,
    >,
    anim_state: Query<(&CharacterAnimationSpriteSheetData, &Sprite)>,
    time: Res<Time>,
) {
    let Ok((p_e, mut kcc, _mv)) = player.single_mut() else {
        return;
    };
    for (e, mut hit, mob_option) in hit_tracker.iter_mut() {
        if !hit.is_active {
            continue;
        }
        if let Some(state) = mob_option {
            if state != &EnemyAnimationState::Hit && state != &EnemyAnimationState::Attack {
                commands.entity(e).insert(EnemyAnimationState::Hit);
            }
        }
        hit.timer.tick(time.delta());

        if hit.timer.fraction() <= 0.25 {
            if e == p_e {
                let d = hit.dir * hit.knockback * time.delta_secs();
                kcc.translation = Some(d);
            } else if let Ok(mut hit_t) = transforms.get_mut(e) {
                hit_t.translation += hit.dir.extend(0.) * hit.knockback * time.delta_secs();
            }
        }

        if hit.timer.is_finished() {
            if let Some(state) = mob_option {
                // For mobs we keep `is_active = true` (and keep the system ticking
                // this entity each frame) until the sprite has cycled back to the
                // starting frame of the Hit animation. Only then do we transition
                // back to Walk and deactivate.
                let (anim_data, sprite) = anim_state.get(e).unwrap();
                let sprite_index = sprite.texture_atlas.as_ref().map(|a| a.index).unwrap_or(0);
                if sprite_index == anim_data.get_starting_frame_for_animation(state)
                    && state == &EnemyAnimationState::Hit
                {
                    commands.entity(e).insert(EnemyAnimationState::Walk);
                    hit.is_active = false;
                }
            } else {
                hit.is_active = false;
            }
        }
    }
    //TODO: move to hit_handler fn
}
fn move_player_attack_collider(
    game: GameParam,
    mut tool_query: Query<(&WorldObject, &mut Transform), (With<Equipment>, Without<Chunk>)>,
    mut attack_event: MessageReader<AttackEvent>,
    mut dir_state: Local<Vec2>,
    player_anim: Query<&PlayerAnimation>,
) {
    if let Ok((obj, mut t)) = tool_query.single_mut() {
        let attack_option = attack_event.read().next();
        if let Some(attack) = attack_option {
            *dir_state = attack.direction;
        }

        if attack_option.is_some() || player_anim.single().ok().is_some_and(|a| a.is_an_attack()) {
            let mut x_offset = 0.;
            let mut y_offset = 0.;
            let angle = dir_state.y.atan2(dir_state.x);

            if *dir_state != Vec2::ZERO {
                x_offset = (angle.cos() * (8.) + angle.cos() * (8.)) / 2.;
                y_offset = (angle.sin() * (8.) + angle.sin() * (8.)) / 2.;
            }
            t.rotation = Quat::from_rotation_z(angle - PI / 2.);
            t.translation.y = y_offset;
            t.translation.x = x_offset;
        } else {
            t.rotation = Quat::from_rotation_z(0.);
            let obj_data = game.world_obj_data.properties.get(obj).unwrap();
            let anchor = obj_data.anchor.unwrap_or(Vec2::ZERO);
            t.translation.x =
                PLAYER_EQUIPMENT_POSITIONS[&Limb::Hands].x + anchor.x * obj_data.size.x;
            t.translation.y =
                PLAYER_EQUIPMENT_POSITIONS[&Limb::Hands].y + anchor.y * obj_data.size.y;

            *dir_state = Vec2::ZERO;
        }
    }
}
fn animate_spritesheet_animations(
    mut commands: Commands,
    time: Res<Time>,
    texture_atlases: Res<Assets<TextureAtlasLayout>>,
    mut query: Query<
        (
            Entity,
            &mut AnimationTimer,
            &mut Sprite,
            &GlobalTransform,
            Option<&Children>,
            Option<&ArcProjectileData>,
            Option<&DoneAnimation>,
            Option<&Projectile>,
            Option<&ProjectileState>,
        ),
        (
            Without<ItemStack>,
            Without<CharacterAnimationSpriteSheetData>,
        ),
    >,
    mut children_txfm_query: Query<&mut Transform>,
    mut ranged_att_event: MessageWriter<RangedAttackEvent>,
) {
    for (
        e,
        mut timer,
        mut sprite,
        transform,
        children_option,
        proj_arc_option,
        remove_me_option,
        proj_option,
        proj_state_option,
    ) in query.iter_mut()
    {
        timer.tick(time.delta());
        if timer.just_finished() {
            let Some(atlas) = sprite.texture_atlas.as_ref() else {
                continue;
            };
            let Some(texture_atlas) = texture_atlases.get(&atlas.layout) else {
                continue;
            };
            let num_frame = texture_atlas.textures.len();
            //hack to fix soem projectiles animating
            if num_frame >= 50 {
                continue;
            }
            if atlas.index == num_frame - 1 && remove_me_option.is_some() {
                commands.entity(e).despawn();
                if let Some(proj) = proj_option {
                    if proj == &Projectile::PlasmaBall {
                        // despawn plasmaball projectile and spawn explosion
                        ranged_att_event.write(RangedAttackEvent {
                            projectile: Projectile::PlasmaExplosion,
                            direction: proj_state_option
                                .map(|state| state.direction)
                                .unwrap_or(Vec2::ZERO),
                            from_entity: None,
                            from_enemy: false,
                            is_followup_proj: true,
                            mana_cost: None,
                            mana_cost_heirloom: None,
                            dmg_override: None,
                            pos_override: Some(transform.translation().truncate()),
                            spawn_delay: 0.01,
                        });
                        continue;
                    }
                }
                continue;
            }

            if let Some(atlas) = sprite.texture_atlas.as_mut() {
                atlas.index = (atlas.index + 1) % num_frame;
            }
            if let Some(children) = children_option {
                for child in children.iter() {
                    let Some(arc_data) = proj_arc_option else {
                        continue;
                    };

                    let index = sprite.texture_atlas.as_ref().map(|a| a.index).unwrap_or(0);
                    let angle = arc_data.col_points[index];
                    let x_offset =
                        (angle.cos() * (arc_data.size.x) + angle.cos() * (arc_data.size.y)) / 2.;
                    let y_offset = ((angle.sin() * (arc_data.size.x))
                        + (angle.sin() * (arc_data.size.y)))
                        / 2.;
                    let Ok(mut t) = children_txfm_query.get_mut(child) else {
                        continue;
                    };
                    t.translation.x = x_offset; //* (angle.cos() * arc_data.arc.x) - arc_data.size.x / 2.;
                    t.translation.y = y_offset;
                    t.rotation = Quat::from_rotation_z(arc_data.col_points[index] - PI / 2.);
                }
            }
            timer.reset();
        }
    }
}

#[derive(Default, Reflect, Clone, Serialize, Deserialize, Component, Debug)]
#[reflect(Component)]
pub struct FadeOpacity;

/// Smoothly fades foliage (trees) translucent when the player walks behind them by lerping the
/// sprite's color alpha toward a target. This keeps sprites batched (no per-tree material/mesh)
/// and works for every tree without needing pre-baked `_fade.png` textures.
fn animate_foliage_opacity(
    time: Res<Time>,
    mut tree_query: Query<(&GlobalTransform, &mut Sprite), (With<FadeOpacity>, Without<Sapling>)>,
    player: Query<&GlobalTransform, With<Player>>,
) {
    const NORMAL_ALPHA: f32 = 1.0;
    const FADED_ALPHA: f32 = 0.35;
    /// Seconds for a full opaque <-> faded transition.
    const FADE_DURATION: f32 = 0.15;

    let p_txfm = match player.single() {
        Ok(t) => t,
        Err(_) => return,
    };
    let player_pos = p_txfm.translation().truncate();
    let step = ((NORMAL_ALPHA - FADED_ALPHA) / FADE_DURATION) * time.delta_secs();

    for (txfm, mut sprite) in tree_query.iter_mut() {
        let delta_t = player_pos - txfm.translation().truncate();
        let target =
            if delta_t.x <= 65. && delta_t.x >= -65. && delta_t.y <= 80. && delta_t.y >= -26. {
                FADED_ALPHA
            } else {
                NORMAL_ALPHA
            };
        let current = sprite.color.to_srgba().alpha;
        if (current - target).abs() <= f32::EPSILON {
            continue;
        }
        let new_alpha = if current < target {
            (current + step).min(target)
        } else {
            (current - step).max(target)
        };
        sprite.color = sprite.color.with_alpha(new_alpha);
    }
}

// fn handle_add_foliage_material(
//     mut commands: Commands,
//     mut materials: ResMut<Assets<FoliageMaterial>>,
//     mut tree_query: Query<(Entity, &Foliage, &FoliageSize), Added<Foliage>>,
//     graphics: Res<Graphics>,
//     mut meshes: ResMut<Assets<Mesh>>,
//     asset_server: Res<AssetServer>,
// ) {
//     let l = tree_query.iter().count();
//     for (e, foliage, size) in tree_query.iter_mut() {
//         // let foliage_material = graphics
//         //     .foliage_material_map
//         //     .as_ref()
//         //     .unwrap()
//         //     .get(foliage)
//         //     .unwrap();

//         // let handle = materials.add(foliage_material.clone());
//         // println!("NEW TREE {:?} | {l:?}", foliage.to_string());
//         let size = size.0;
//         commands
//             .entity(e)
//             // .insert(SpriteBundle {
//             //     custom_size: Some(size),
//             //     ..Default::default()
//             // })
//             // .insert(
//             //     asset_server
//             //         .load::<Handle<Image>>(format!("{}.png", foliage.to_string().to_lowercase())),
//             // )
//             .insert(Name::new("FOLIAGE:GREEN_TREE"));
//         // .insert(Mesh2d::from(meshes.add(Mesh::from(shape::Quad {
//         //     size,
//         //     ..Default::default()
//         // }))));
//         // .insert((handle).clone());
//     }
// }
