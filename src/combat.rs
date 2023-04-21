use bevy::{prelude::*, time::FixedTimestep};
use bevy_rapier2d::prelude::RapierContext;
use rand::Rng;

use crate::{
    animations::{AnimationTimer, DoneAnimation, HitAnimationTracker},
    attributes::{Attack, AttributeModifier, Health, InvincibilityCooldown},
    inventory::Inventory,
    item::{MainHand, Placeable, WorldObject},
    ui::InventoryState,
    world_generation::WorldGenerationPlugin,
    Game, GameParam, GameState, Player, YSort, TIME_STEP,
};

#[derive(Debug, Clone)]
pub struct HitEvent {
    pub hit_entity: Entity,
    pub damage: i32,
    pub dir: Vec2,
}

#[derive(Debug, Clone)]

pub struct EnemyDeathEvent {
    pub entity: Entity,
    pub enemy_pos: Vec2,
}

#[derive(Component, Debug, Clone)]
pub struct AttackTimer(pub Timer);

#[derive(Component, Debug, Clone)]
pub struct InvincibilityTimer(pub Timer);
#[derive(Component, Debug, Clone)]

pub struct HitMarker;

#[derive(Component, Debug)]
pub struct JustGotHit;
pub struct CombatPlugin;
impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<HitEvent>()
            .add_event::<EnemyDeathEvent>()
            .add_system_set(
                SystemSet::on_update(GameState::Main)
                    .with_run_criteria(FixedTimestep::step(TIME_STEP as f64))
                    .with_system(Self::handle_hits)
                    .with_system(Self::spawn_hit_spark_effect.after(Self::handle_hits))
                    .with_system(Self::handle_invincibility_frames.after(Self::handle_hits))
                    .with_system(Self::handle_enemy_death.after(Self::handle_hits))
                    .with_system(Self::check_hit_collisions),
            );
    }
}

impl CombatPlugin {
    fn handle_enemy_death(
        mut commands: Commands,
        mut game: GameParam,
        mut death_events: EventReader<EnemyDeathEvent>,
    ) {
        for death_event in death_events.iter() {
            let t = death_event.enemy_pos;
            commands.entity(death_event.entity).despawn();
            let enemy_chunk_pos =
                WorldGenerationPlugin::camera_pos_to_chunk_pos(&Vec2::new(t.x, t.y));
            let enemy_tile_pos =
                WorldGenerationPlugin::camera_pos_to_block_pos(&Vec2::new(t.x, t.y));

            WorldObject::Placeable(Placeable::Flint).spawn_item_drop(
                &mut commands,
                &mut game,
                enemy_tile_pos,
                enemy_chunk_pos,
                2,
                None,
            );
        }
    }
    fn handle_invincibility_frames(
        mut commands: Commands,
        mut i_frames: Query<(Entity, &mut InvincibilityTimer)>,
        time: Res<Time>,
    ) {
        for mut i_frame in i_frames.iter_mut() {
            i_frame.1 .0.tick(time.delta());
            if i_frame.1 .0.just_finished() {
                commands.entity(i_frame.0).remove::<InvincibilityTimer>();
            }
        }
    }
    fn spawn_hit_spark_effect(
        mut commands: Commands,
        hits: Query<(Entity, Added<JustGotHit>, Option<&Player>)>,
        asset_server: Res<AssetServer>,
        mut texture_atlases: ResMut<Assets<TextureAtlas>>,
    ) {
        // add spark animation entity as child, will animate once and remove itself.
        for hit in hits.iter() {
            if hit.2.is_some() {
                commands.entity(hit.0).remove::<JustGotHit>();
                continue;
            }
            let texture_handle = asset_server.load("textures/effects/hit-particles.png");
            let texture_atlas =
                TextureAtlas::from_grid(texture_handle, Vec2::new(32.0, 32.0), 7, 1, None, None);
            let texture_atlas_handle = texture_atlases.add(texture_atlas);
            let mut rng = rand::thread_rng();
            let s = 5;
            let x_rng = rng.gen_range(-s..s);
            let y_rng = rng.gen_range(-s..s);

            let hit_spark_entity = commands
                .spawn((
                    SpriteSheetBundle {
                        texture_atlas: texture_atlas_handle,
                        transform: Transform::from_translation(Vec3::new(
                            0. + x_rng as f32,
                            -5. + y_rng as f32,
                            1.,
                        )),
                        ..default()
                    },
                    AnimationTimer(Timer::from_seconds(0.05, TimerMode::Repeating)),
                    YSort,
                    DoneAnimation,
                    Name::new("Hit Spark"),
                ))
                .id();

            commands
                .entity(hit.0)
                .remove::<JustGotHit>()
                .add_child(hit_spark_entity);
        }
    }

    fn handle_hits(
        mut commands: Commands,
        mut health: Query<(
            Entity,
            &mut Health,
            &Transform,
            Option<&WorldObject>,
            Option<&InvincibilityCooldown>,
        )>,
        mut hit_events: EventReader<HitEvent>,
        mut death_events: EventWriter<EnemyDeathEvent>,
        player: Query<Entity, With<Player>>,
        in_i_frame: Query<&InvincibilityTimer>,
        // has_i_frames: Query<&InvincibilityCooldown>,
    ) {
        for hit in hit_events.iter() {
            // is in invincibility frames from a previous hit
            if in_i_frame.get(hit.hit_entity).is_ok() {
                return;
            }
            if let Ok((e, mut hit_health, t, obj_option, i_frame_option)) =
                health.get_mut(hit.hit_entity)
            {
                if obj_option.is_none() {
                    // let has_i_frames = has_i_frames.get(hit.hit_entity);
                    commands
                        .entity(hit.hit_entity)
                        .insert(HitAnimationTracker {
                            timer: Timer::from_seconds(
                                //TODO: once we create builders for creatures, add this as a default to all creatures that can be hit
                                0.2,
                                TimerMode::Once,
                            ),
                            knockback: 400.,
                            dir: hit.dir,
                        })
                        .insert(JustGotHit);
                    if let Some(i_frames) = i_frame_option {
                        commands.entity(hit.hit_entity).insert(InvincibilityTimer(
                            Timer::from_seconds(i_frames.0, TimerMode::Once),
                        ));
                    }
                }

                hit_health.0 -= hit.damage as i32;
                if hit_health.0 <= 0 && player.single() != e {
                    death_events.send(EnemyDeathEvent {
                        entity: e,
                        enemy_pos: t.translation.truncate(),
                    })
                }
            }
        }
    }
    fn check_hit_collisions(
        mut commands: Commands,
        context: ResMut<RapierContext>,
        weapons: Query<(Entity, &Parent), (Without<HitMarker>, With<MainHand>)>,
        parent_attack: Query<&Attack>,
        mut hit_event: EventWriter<HitEvent>,
        game: Res<Game>,
        inv_state: Query<&InventoryState>,
        mut inv: Query<&mut Inventory>,
    ) {
        if let Ok(weapon) = weapons.get_single() {
            let weapon_parent = weapon.1;
            if let Some(hit) = context.intersection_pairs().find(|c| {
                (c.0 == weapon.0 && c.1 != weapon_parent.get())
                    || (c.1 == weapon.0 && c.0 != weapon_parent.get())
            }) {
                if !game.player_state.is_attacking {
                    return;
                }
                if let Some(Some(wep)) = inv
                    .single()
                    .clone()
                    .items
                    .get(inv_state.single().active_hotbar_slot)
                {
                    wep.modify_attributes(
                        AttributeModifier {
                            modifier: "durability".to_owned(),
                            delta: -1,
                        },
                        &mut inv,
                    );
                }
                commands.entity(weapon.0).insert(HitMarker);
                hit_event.send(HitEvent {
                    hit_entity: if hit.0 == weapon.0 { hit.1 } else { hit.0 },
                    damage: parent_attack.get(**weapon_parent).unwrap().0,
                    dir: Vec2::new(0., 0.),
                });
            }
        }
    }
}
