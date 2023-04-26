use std::f32::consts::PI;
use std::{cmp::max, time::Duration};

use bevy::reflect::TypeUuid;
use bevy::render::render_resource::ShaderRef;
use bevy::sprite::{Material2d, Material2dPlugin};
use bevy::time::FixedTimestep;
use bevy::{prelude::*, render::render_resource::AsBindGroup};
use bevy_inspector_egui::quick::ResourceInspectorPlugin;
use interpolation::lerp;

use crate::ai::AttackState;
use crate::attributes::AttackCooldown;
use crate::combat::{AttackTimer, HitMarker};
use crate::enemy::{Enemy, EnemyMaterial};
use crate::inputs::{FacingDirection, InputsPlugin, MovementVector};
use crate::item::Equipment;
use crate::{inventory::ItemStack, Game, GameState, Player, TIME_STEP};
use crate::{Limb, RawPosition};

pub struct AnimationsPlugin;

#[derive(Component)]
pub struct AnimationPosTracker(pub f32, pub f32, pub f32);

// #[derive(Component)]
// pub struct CameraOffsetTracker(Vec2, Vec2);

#[derive(Component)]
pub struct AnimationFrameTracker(pub i32, pub i32);

#[derive(Component, Clone, Deref, DerefMut)]
pub struct AnimationTimer(pub Timer);
#[derive(Component, Clone)]
pub struct HitAnimationTracker {
    pub timer: Timer,
    pub knockback: f32,
    pub dir: Vec2,
}

#[derive(Component, Debug)]
pub struct AttackAnimationTimer(pub Timer, pub f32);

#[derive(Component, Debug)]
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
            .add_system_set(
                SystemSet::on_update(GameState::Main)
                    .with_run_criteria(FixedTimestep::step(TIME_STEP as f64))
                    .with_system(Self::animate_limbs)
                    .with_system(Self::animate_enemies)
                    .with_system(Self::animate_dropped_items)
                    .with_system(Self::animate_attack)
                    .with_system(Self::animate_hit)
                    .with_system(Self::animate_spritesheet_animations)
                    .after(InputsPlugin::mouse_click_system),
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
        game: Res<Game>,
        asset_server: Res<AssetServer>,
        mut materials: ResMut<Assets<EnemyMaterial>>,
        mut enemy_query: Query<(
            &mut AnimationFrameTracker,
            &mut AnimationTimer,
            &Handle<EnemyMaterial>,
            &Enemy,
            Option<&AttackState>,
        )>,
    ) {
        for (mut tracker, mut timer, enemy_handle, enemy, att_option) in enemy_query.iter_mut() {
            let enemy_material = materials.get_mut(enemy_handle);
            timer.tick(time.delta());
            if let Some(mat) = enemy_material {
                if timer.just_finished() && game.player_state.is_moving {
                    tracker.0 = max((tracker.0 + 1) % (tracker.1 - 1), 0);
                } else if !game.player_state.is_moving {
                    tracker.0 = 0;
                }
                mat.source_texture = Some(asset_server.load(format!(
                    "textures/slime/{}-move-{}.png",
                    enemy.to_string().to_lowercase(),
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
        mut hit_tracker: Query<(Entity, &mut HitAnimationTracker)>,
        mut player: Query<(Entity, &mut RawPosition, &mut MovementVector), With<Player>>,
        time: Res<Time>,
    ) {
        let (p_e, mut p_rp, mut mv) = player.single_mut();
        for (e, mut hit) in hit_tracker.iter_mut() {
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
                commands.entity(e).remove::<HitAnimationTracker>();
            }
        }
        //TODO: move to hit_handler fn
    }
    fn animate_attack(
        mut commands: Commands,
        mut game: ResMut<Game>,
        time: Res<Time>,
        mut tool_query: Query<
            (
                Entity,
                &mut Transform,
                &mut AttackAnimationTimer,
                Option<&mut AttackTimer>,
            ),
            With<Equipment>,
        >,
        attack_event: EventReader<AttackEvent>,
        player: Query<(&FacingDirection, Option<&AttackCooldown>), With<Player>>,
    ) {
        if let Ok((e, mut t, mut at, timer_option)) = tool_query.get_single_mut() {
            let (dir, cooldown) = player.single();
            let is_facing_left = if *dir == FacingDirection::Left {
                1.
            } else {
                -1.
            };
            let on_cooldown = timer_option.is_some();
            if on_cooldown {
                let mut t = timer_option.unwrap();
                t.0.tick(time.delta());
                if t.0.finished() {
                    commands
                        .entity(e)
                        .remove::<AttackTimer>()
                        .remove::<HitMarker>();
                }
            }

            if attack_event.len() > 0 || !at.0.elapsed().is_zero() {
                if !game.player_state.is_attacking {
                    if let Some(cooldown) = cooldown {
                        let mut attack_cd_timer =
                            AttackTimer(Timer::from_seconds(cooldown.0, TimerMode::Once));
                        attack_cd_timer.0.tick(time.delta());
                        commands.entity(e).insert(attack_cd_timer);
                    }
                }
                game.player_state.is_attacking = true;

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
                        &(5. * is_facing_left),
                        &(-15. * is_facing_left),
                        &(at.0.elapsed().as_secs_f32() / at.0.duration().as_secs_f32()),
                    );
                } else {
                    at.0.reset();
                    at.1 = 0.;
                    t.rotation = Quat::from_rotation_z(-at.1);
                    t.translation.x = -5.;
                    t.translation.y = -1.;
                }
            } else {
                game.player_state.is_attacking = false;
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
                Option<&DoneAnimation>,
            ),
            Without<ItemStack>,
        >,
    ) {
        for (e, mut timer, mut sprite, texture_atlas_handle, remove_me_option) in &mut query {
            timer.tick(time.delta());
            if timer.just_finished() {
                let texture_atlas = texture_atlases.get(texture_atlas_handle).unwrap();
                if sprite.index == texture_atlas.textures.len() - 1 && remove_me_option.is_some() {
                    commands.entity(e).despawn();
                    return;
                }
                sprite.index = (sprite.index + 1) % texture_atlas.textures.len();
                timer.reset();
            }
        }
    }
}
