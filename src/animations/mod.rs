pub mod enemy_sprites;
mod game_over;

use std::cmp::max;
use std::f32::consts::PI;

use bevy::reflect::TypeUuid;
use bevy::render::render_resource::ShaderRef;
use bevy::sprite::{Material2d, Material2dPlugin};
use bevy::{prelude::*, render::render_resource::AsBindGroup};
use bevy_proto::prelude::{ReflectSchematic, Schematic};
use bevy_rapier2d::prelude::KinematicCharacterController;
use game_over::{handle_game_over_fadeout, tick_game_over_overlay};
use player_sprite::{
    cleanup_one_time_animations, handle_anim_change_when_player_dir_changes,
    handle_player_animation_change, PlayerAnimation,
};
use serde::{Deserialize, Serialize};
pub mod player_sprite;

use crate::ai::LeapAttackState;
use crate::enemy::{EnemyMaterial, Mob};
use crate::inputs::{mouse_click_system, FacingDirection, MovementVector};
use crate::item::projectile::ArcProjectileData;
use crate::item::{Equipment, MainHand, WorldObject, PLAYER_EQUIPMENT_POSITIONS};
use crate::player::Limb;
use crate::sappling::Sappling;
use crate::world::chunk::Chunk;
use crate::{inventory::ItemStack, Game, Player};
use crate::{GameParam, GameState};

use self::enemy_sprites::{
    animate_character_spritesheet_animations,
    change_anim_offset_when_character_action_state_changes, change_character_anim_direction,
    CharacterAnimationSpriteSheetData, EnemyAnimationState,
};

pub struct AnimationsPlugin;

#[derive(Component, Reflect, FromReflect, Schematic, Default)]
#[reflect(Schematic)]
pub struct AnimationPosTracker(pub f32, pub f32, pub f32);

#[derive(Component, Schematic, Reflect, FromReflect, Default)]
#[reflect(Schematic)]
pub struct AnimationFrameTracker(pub i32, pub i32);

#[derive(Component, Clone, Deref, DerefMut, Schematic, Reflect, FromReflect)]
#[reflect(Schematic)]
pub struct AnimationTimer(pub Timer);
#[derive(Component, Debug, Clone)]
pub struct HitAnimationTracker {
    pub timer: Timer,
    pub knockback: f32,
    pub dir: Vec2,
}

#[derive(Component, Reflect, FromReflect, Schematic, Debug)]
#[reflect(Schematic)]
pub struct DoneAnimation;

#[derive(Debug, Clone, Default)]
pub struct AttackEvent {
    pub direction: Vec2,
    pub ignore_cooldown: bool,
}
#[derive(AsBindGroup, TypeUuid, Debug, Clone)]
#[uuid = "f690fdae-d598-45ab-8225-97e2a3f056f0"]
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

    // fn alpha_mode(&self) -> AlphaMode {
    //     self.alpha_mode
    // }
}

impl Plugin for AnimationsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(Material2dPlugin::<AnimatedTextureMaterial>::default())
            // .add_systems(
            //     ().in_set(CoreGameSet::Main)
            //         .in_schedule(CoreSchedule::FixedUpdate),
            // )
            .add_systems(
                (
                    change_anim_offset_when_character_action_state_changes,
                    change_character_anim_direction,
                    animate_character_spritesheet_animations,
                    animate_enemies,
                    animate_dropped_items,
                    handle_held_item_direction_change,
                    move_player_attack_collider,
                    animate_hit,
                    animate_spritesheet_animations.after(mouse_click_system),
                    animate_foliage_opacity,
                    handle_game_over_fadeout,
                    handle_anim_change_when_player_dir_changes,
                    handle_player_animation_change,
                    cleanup_one_time_animations,
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(tick_game_over_overlay);
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
        &Handle<EnemyMaterial>,
        &Mob,
        Option<&LeapAttackState>,
    )>,
) {
    for (mut tracker, mut timer, enemy_handle, _enemy, att_option) in enemy_query.iter_mut() {
        let enemy_material = materials.get_mut(enemy_handle);
        timer.tick(time.delta());
        if let Some(mat) = enemy_material {
            if timer.just_finished() {
                tracker.0 = max((tracker.0 + 1) % (tracker.1 - 1), 0);
            }
            mat.source_texture = Some(asset_server.load(format!(
                "textures/slime/{}-move-{}.png",
                "slime",
                //enemy.to_string().to_lowercase(),
                tracker.0
            )));
            if let Some(attack) = att_option {
                mat.is_attacking = if attack.attack_startup_timer.finished()
                    && !attack.attack_duration_timer.finished()
                {
                    1.
                } else {
                    0.
                };
            }
        }

        // else if let Ok(mut t) = eq_query.get_mut(*l) {
        //     // t.translation.y = (t.translation.y + 1.) % 2.;
        //     // t.translation.x = (t.translation.y + 1.) % 2.;
        // }
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
    anim_state: Query<(&CharacterAnimationSpriteSheetData, &TextureAtlasSprite)>,
    time: Res<Time>,
) {
    let (p_e, mut kcc, _mv) = player.single_mut();
    for (e, mut hit, mob_option) in hit_tracker.iter_mut() {
        if let Some(state) = mob_option {
            if state != &EnemyAnimationState::Hit && state != &EnemyAnimationState::Attack {
                commands.entity(e).insert(EnemyAnimationState::Hit);
            }
        }
        hit.timer.tick(time.delta());

        if hit.timer.percent() <= 0.25 {
            if e == p_e {
                let d = hit.dir * hit.knockback * time.delta_seconds();
                kcc.translation = Some(d);
            } else if let Ok(mut hit_t) = transforms.get_mut(e) {
                hit_t.translation += hit.dir.extend(0.) * hit.knockback * time.delta_seconds();
            }
        }

        if hit.timer.finished() {
            if mob_option.is_some() {
                let (anim_data, sprite) = anim_state.get(e).unwrap();
                if sprite.index == anim_data.get_starting_frame_for_animation(mob_option.unwrap())
                    && mob_option.unwrap() == &EnemyAnimationState::Hit
                {
                    commands
                        .entity(e)
                        .remove::<HitAnimationTracker>()
                        .insert(EnemyAnimationState::Walk);
                }
            } else {
                commands.entity(e).remove::<HitAnimationTracker>();
            }
        }
    }
    //TODO: move to hit_handler fn
}
fn handle_held_item_direction_change(
    game: GameParam,
    mut tool_query: Query<
        (&WorldObject, &mut Transform, &mut TextureAtlasSprite),
        (With<MainHand>, Without<Chunk>),
    >,
) {
    if let Ok((obj, mut t, mut sprite)) = tool_query.get_single_mut() {
        let obj_data = game.world_obj_data.properties.get(obj).unwrap();
        let anchor = obj_data.anchor.unwrap_or(Vec2::ZERO);

        let is_facing_left = game.player().direction == FacingDirection::Left;

        t.translation.x = PLAYER_EQUIPMENT_POSITIONS[&Limb::Hands].x
            + anchor.x * obj_data.size.x
            + if is_facing_left { 0. } else { 11. };
        sprite.flip_x = is_facing_left;
    }
}
fn move_player_attack_collider(
    game: GameParam,
    mut tool_query: Query<(&WorldObject, &mut Transform), (With<Equipment>, Without<Chunk>)>,
    mut attack_event: EventReader<AttackEvent>,
    mut dir_state: Local<Vec2>,
    player_anim: Query<&PlayerAnimation>,
) {
    if let Ok((obj, mut t)) = tool_query.get_single_mut() {
        let attack_option = attack_event.iter().next();
        if let Some(attack) = attack_option {
            *dir_state = attack.direction;
        }

        if attack_option.is_some() || player_anim.single().is_an_attack() {
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
    texture_atlases: Res<Assets<TextureAtlas>>,
    mut query: Query<
        (
            Entity,
            &mut AnimationTimer,
            &mut TextureAtlasSprite,
            &Handle<TextureAtlas>,
            Option<&Children>,
            Option<&ArcProjectileData>,
            Option<&DoneAnimation>,
        ),
        (
            Without<ItemStack>,
            Without<CharacterAnimationSpriteSheetData>,
        ),
    >,
    mut children_txfm_query: Query<&mut Transform>,
) {
    for (
        e,
        mut timer,
        mut sprite,
        texture_atlas_handle,
        children_option,
        proj_arc_option,
        remove_me_option,
    ) in &mut query
    {
        timer.tick(time.delta());
        if timer.just_finished() {
            let texture_atlas = texture_atlases.get(texture_atlas_handle).unwrap();
            let num_frame = texture_atlas.textures.len();
            //hack to fix soem projectiles animating
            if num_frame >= 50 {
                continue;
            }
            if sprite.index == num_frame - 1 && remove_me_option.is_some() {
                commands.entity(e).despawn_recursive();
                continue;
            }

            sprite.index = (sprite.index + 1) % num_frame;
            if let Some(children) = children_option {
                for child in children.iter() {
                    let Some(arc_data) = proj_arc_option else {
                        continue;
                    };

                    let angle = arc_data.col_points[sprite.index];
                    let x_offset =
                        (angle.cos() * (arc_data.size.x) + angle.cos() * (arc_data.size.y)) / 2.;
                    let y_offset = ((angle.sin() * (arc_data.size.x))
                        + (angle.sin() * (arc_data.size.y)))
                        / 2.;
                    let mut t = children_txfm_query.get_mut(*child).unwrap();
                    t.translation.x = x_offset; //* (angle.cos() * arc_data.arc.x) - arc_data.size.x / 2.;
                    t.translation.y = y_offset;
                    t.rotation = Quat::from_rotation_z(arc_data.col_points[sprite.index] - PI / 2.);
                }
            }
            timer.reset();
        }
    }
}

#[derive(FromReflect, Default, Reflect, Clone, Serialize, Deserialize, Component, Schematic)]
#[reflect(Component, Schematic)]
pub struct FadeOpacity;

fn animate_foliage_opacity(
    mut commands: Commands,
    mut tree_query: Query<
        (Entity, &GlobalTransform, &WorldObject),
        (With<FadeOpacity>, Without<Sappling>),
    >,
    player: Query<&GlobalTransform, With<Player>>,
    asset_server: Res<AssetServer>,
) {
    for (e, txfm, obj) in tree_query.iter_mut() {
        let p_txfm = player.single();
        // check if player is behind tree
        let delta_t = p_txfm.translation().truncate() - txfm.translation().truncate();
        if delta_t.x <= 40. && delta_t.x >= -40. && delta_t.y <= 54. && delta_t.y >= -20. {
            commands.entity(e).insert(
                asset_server
                    .load::<Image, _>(format!("{}_fade.png", obj.to_string().to_lowercase())),
            );
            // sprite.color = sprite.color.with_a(0.0);
        } else {
            commands.entity(e).insert(
                asset_server.load::<Image, _>(format!("{}.png", obj.to_string().to_lowercase())),
            );
            // sprite.color = sprite.color.with_a(1.0);
        }
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
//         // .insert(Mesh2dHandle::from(meshes.add(Mesh::from(shape::Quad {
//         //     size,
//         //     ..Default::default()
//         // }))));
//         // .insert((handle).clone());
//     }
// }
