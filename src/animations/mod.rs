pub mod enemy_sprites;

use std::f32::consts::PI;
use std::{cmp::max, time::Duration};

use bevy::reflect::TypeUuid;
use bevy::render::render_resource::ShaderRef;
use bevy::sprite::{Material2d, Material2dPlugin};
use bevy::{prelude::*, render::render_resource::AsBindGroup};
use bevy_proto::prelude::{ReflectSchematic, Schematic};
use interpolation::lerp;

use crate::ai::LeapAttackState;
use crate::enemy::{EnemyMaterial, Mob};
use crate::inputs::{FacingDirection, InputsPlugin, MovementVector};
use crate::item::projectile::ArcProjectileData;
use crate::item::{Equipment, MainHand, WorldObject, PLAYER_EQUIPMENT_POSITIONS};
use crate::player::Limb;
use crate::world::chunk::Chunk;
use crate::{inventory::ItemStack, Game, Player, TIME_STEP};
use crate::{GameParam, GameState, RawPosition};

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
#[derive(Component, Clone)]
pub struct HitAnimationTracker {
    pub timer: Timer,
    pub knockback: f32,
    pub dir: Vec2,
}

#[derive(Component, Debug)]
pub struct AttackAnimationTimer(pub Timer, pub f32);

#[derive(Component, Reflect, FromReflect, Schematic, Debug)]
#[reflect(Schematic)]
pub struct DoneAnimation;

#[derive(Debug, Clone, Default)]
pub struct AttackEvent;
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
            .add_systems(
                (
                    change_anim_offset_when_character_action_state_changes,
                    animate_character_spritesheet_animations,
                    change_character_anim_direction,
                    Self::animate_limbs,
                    Self::animate_enemies,
                    Self::animate_dropped_items,
                    Self::handle_held_item_direction_change,
                    Self::animate_attack,
                    Self::animate_hit,
                    Self::animate_spritesheet_animations.after(InputsPlugin::mouse_click_system),
                )
                    .in_set(OnUpdate(GameState::Main)),
            );
    }
}

impl AnimationsPlugin {
    fn animate_limbs(
        time: Res<Time>,
        game: Res<Game>,
        asset_server: Res<AssetServer>,
        mut materials: ResMut<Assets<AnimatedTextureMaterial>>,
        mut player_query: Query<(&mut AnimationTimer, &Children), With<Player>>,
        is_hit: Query<&HitAnimationTracker, With<Player>>,
        mut limb_query: Query<(
            &mut AnimationFrameTracker,
            &Handle<AnimatedTextureMaterial>,
            &Limb,
        )>,
        // mut eq_query: Query<&mut Transform, With<Equipment>>,
    ) {
        for (mut timer, limb_children) in &mut player_query {
            let d = time.delta();
            timer.tick(if game.player_state.is_dashing {
                Duration::new(
                    (d.as_secs() as f32 * 4.) as u64,
                    (d.subsec_nanos() as f32 * 4.) as u32,
                )
            } else {
                d
            });
            // if timer.just_finished() && game.player.is_moving {
            for l in limb_children {
                if let Ok((mut tracker, limb_handle, limb)) = limb_query.get_mut(*l) {
                    let limb_material = materials.get_mut(limb_handle);
                    if let Some(mat) = limb_material {
                        if timer.just_finished() && game.player_state.is_moving {
                            tracker.0 = max((tracker.0 + 1) % (tracker.1 - 1), 0);
                        } else if !game.player_state.is_moving {
                            tracker.0 = 0;
                        }
                        mat.source_texture = Some(asset_server.load(format!(
                            "textures/player/player-run-down/player-{}-run-down-source-{}.png",
                            limb.to_string().to_lowercase(),
                            tracker.0
                        )));
                        mat.opacity = if is_hit.get_single().is_ok() { 0.5 } else { 1. };
                    }
                }
                // else if let Ok(mut t) = eq_query.get_mut(*l) {
                //     // t.translation.y = (t.translation.y + 1.) % 2.;
                //     // t.translation.x = (t.translation.y + 1.) % 2.;
                // }
            }

            // } else if !game.player.is_moving {
            //     for l in limb_children {
            //         if let Ok((mut limb_sprite, _)) = limb_query.get_mut(*l) {
            //             limb_sprite.index = 0
            //         } else if let Ok(mut t) = eq_query.get_mut(*l) {
            //             // t.translation.y = -1.;
            //             // t.translation.x = 0.;
            //         }
            //     }
            // }
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
            &ItemStack,
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
        mut player: Query<(Entity, &mut RawPosition, &mut MovementVector), With<Player>>,
        anim_state: Query<(&CharacterAnimationSpriteSheetData, &TextureAtlasSprite)>,
        time: Res<Time>,
    ) {
        let (p_e, mut p_rp, mut mv) = player.single_mut();
        for (e, mut hit, mob_option) in hit_tracker.iter_mut() {
            if let Some(state) = mob_option {
                if state != &EnemyAnimationState::Hit {
                    commands.entity(e).insert(EnemyAnimationState::Hit);
                }
            }
            hit.timer.tick(time.delta());

            if hit.timer.percent() <= 0.25 {
                if e == p_e {
                    let d = hit.dir * hit.knockback * TIME_STEP;
                    p_rp.x += d.x;
                    p_rp.y += d.y;
                    mv.0 = d;
                } else {
                    if let Ok(mut hit_t) = transforms.get_mut(e) {
                        hit_t.translation += hit.dir.extend(0.) * hit.knockback * TIME_STEP;
                    }
                }
            }

            if hit.timer.finished() {
                if mob_option.is_some() {
                    let (anim_data, sprite) = anim_state.get(e).unwrap();
                    if sprite.index
                        == anim_data.get_starting_frame_for_animation(mob_option.unwrap()) as usize
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
        mut tool_query: Query<(&WorldObject, &mut Transform), (With<MainHand>, Without<Chunk>)>,
    ) {
        if let Ok((obj, mut t)) = tool_query.get_single_mut() {
            let obj_data = game.world_obj_data.properties.get(&obj).unwrap();
            let anchor = obj_data.anchor.unwrap_or(Vec2::ZERO);

            let is_facing_left = game.player().direction == FacingDirection::Left;

            t.translation.x = PLAYER_EQUIPMENT_POSITIONS[&Limb::Hands].x
                + anchor.x * obj_data.size.x
                + if is_facing_left { 0. } else { 11. }
        }
    }
    fn animate_attack(
        mut game: GameParam,
        time: Res<Time>,
        mut tool_query: Query<
            (&WorldObject, &mut Transform, &mut AttackAnimationTimer),
            (With<Equipment>, Without<Chunk>),
        >,
        attack_event: EventReader<AttackEvent>,
    ) {
        if let Ok((obj, mut t, mut at)) = tool_query.get_single_mut() {
            let is_facing_left = if game.player().direction == FacingDirection::Left {
                1.
            } else {
                -1.
            };

            if attack_event.len() > 0 || !at.0.elapsed().is_zero() {
                game.player_mut().is_attacking = true;

                let d = time.delta();
                at.0.tick(d);
                if !at.0.just_finished() {
                    at.1 = PI / 2.;
                    // at.1 = lerp(
                    //     &0.,
                    //     &PI,
                    //     &(at.0.elapsed().as_secs_f32() / at.0.duration().as_secs_f32()),
                    // );
                    t.rotation = Quat::from_rotation_z(is_facing_left * at.1);
                    // t.translation.x = f32::min(t.translation.x.lerp(&5., &at.1), 5.);
                    t.translation.y = -4.;
                    t.translation.x = lerp(
                        &(-5. * is_facing_left),
                        &(-15. * is_facing_left),
                        &(at.0.elapsed().as_secs_f32() / at.0.duration().as_secs_f32()),
                    );
                } else {
                    at.0.reset();
                    at.1 = 0.;
                    t.rotation = Quat::from_rotation_z(-at.1);
                    let obj_data = game.world_obj_data.properties.get(&obj).unwrap();
                    let anchor = obj_data.anchor.unwrap_or(Vec2::ZERO);
                    t.translation.x =
                        PLAYER_EQUIPMENT_POSITIONS[&Limb::Hands].x + anchor.x * obj_data.size.x;
                    t.translation.y =
                        PLAYER_EQUIPMENT_POSITIONS[&Limb::Hands].y + anchor.y * obj_data.size.y;
                }
            } else {
                game.player_mut().is_attacking = false;
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
                if sprite.index == num_frame - 1 && remove_me_option.is_some() {
                    commands.entity(e).despawn_recursive();
                    continue;
                }

                sprite.index = (sprite.index + 1) % num_frame;
                if let Some(children) = children_option {
                    for child in children.iter() {
                        let Some(arc_data) = proj_arc_option else {
                            continue
                        };

                        let angle = arc_data.col_points[sprite.index];
                        let x_offset = (angle.cos() * (arc_data.size.x)
                            + angle.cos() * (arc_data.size.y))
                            / 2.;
                        let y_offset = ((angle.sin() * (arc_data.size.x))
                            + (angle.sin() * (arc_data.size.y)))
                            / 2.;
                        let mut t = children_txfm_query.get_mut(*child).unwrap();
                        t.translation.x = x_offset; //* (angle.cos() * arc_data.arc.x) - arc_data.size.x / 2.;
                        t.translation.y = y_offset;
                        t.rotation =
                            Quat::from_rotation_z(arc_data.col_points[sprite.index] - PI / 2.);
                    }
                }
                timer.reset();
            }
        }
    }
}
