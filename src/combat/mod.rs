use bevy::prelude::*;

use bevy_proto::prelude::ProtoCommands;
use rand::Rng;

pub mod collisions;

use crate::{
    animations::{AnimationTimer, AttackEvent, DoneAnimation, HitAnimationTracker},
    assets::SpriteAnchor,
    attributes::{AttackCooldown, CurrentHealth, InvincibilityCooldown, LootRateBonus},
    custom_commands::CommandsExt,
    item::{BreaksWith, LootTable, LootTablePlugin, MainHand, WorldObject},
    proto::proto_param::ProtoParam,
    world::{world_helpers::world_pos_to_tile_pos, TileMapPosition},
    AppExt, CustomFlush, GameParam, GameState, Player, YSort,
};

use self::collisions::CollisionPlugion;

#[derive(Debug, Clone)]
pub struct HitEvent {
    pub hit_entity: Entity,
    pub damage: i32,
    pub dir: Vec2,
    pub hit_with: Option<WorldObject>,
}

#[derive(Component, Debug, Clone)]
pub struct MarkedForDeath;
#[derive(Debug, Clone)]

pub struct EnemyDeathEvent {
    pub entity: Entity,
    pub enemy_pos: Vec2,
}
#[derive(Debug, Clone)]

pub struct ObjBreakEvent {
    pub entity: Entity,
    pub obj: WorldObject,
    pub pos: TileMapPosition,
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
        app.with_default_schedule(CoreSchedule::FixedUpdate, |app| {
            app.add_event::<HitEvent>().add_event::<EnemyDeathEvent>();
        })
        .add_event::<ObjBreakEvent>()
        .add_plugin(CollisionPlugion)
        .add_systems(
            (
                Self::handle_hits,
                Self::cleanup_marked_for_death_entities,
                Self::handle_attack_cooldowns.before(CustomFlush),
                Self::spawn_hit_spark_effect.after(Self::handle_hits),
                Self::handle_invincibility_frames.after(Self::handle_hits),
                Self::handle_enemy_death.after(Self::handle_hits),
            )
                .in_set(OnUpdate(GameState::Main)),
        )
        .add_system(apply_system_buffers.in_set(CustomFlush));
    }
}

impl CombatPlugin {
    fn handle_attack_cooldowns(
        mut commands: Commands,
        time: Res<Time>,
        tool_query: Query<Entity, With<MainHand>>,
        attack_event: EventReader<AttackEvent>,
        mut player: Query<(Entity, &AttackCooldown, Option<&mut AttackTimer>), With<Player>>,
    ) {
        let (player_e, cooldown, timer_option) = player.single_mut();

        if attack_event.len() > 0 && timer_option.is_none() {
            let mut attack_cd_timer = AttackTimer(Timer::from_seconds(cooldown.0, TimerMode::Once));
            attack_cd_timer.0.tick(time.delta());
            commands.entity(player_e).insert(attack_cd_timer);
            if let Ok(tool) = tool_query.get_single() {
                commands.entity(tool).remove::<HitMarker>();
            }
        }
        if let Some(mut t) = timer_option {
            t.0.tick(time.delta());
            if t.0.finished() {
                commands.entity(player_e).remove::<AttackTimer>();
            }
        }
    }
    fn handle_enemy_death(
        mut commands: Commands,
        proto_param: ProtoParam,
        mut death_events: EventReader<EnemyDeathEvent>,
        asset_server: Res<AssetServer>,
        mut texture_atlases: ResMut<Assets<TextureAtlas>>,
        loot_tables: Query<&LootTable>,
        mut proto_commands: ProtoCommands,
        loot_bonus: Query<&LootRateBonus>,
    ) {
        for death_event in death_events.iter() {
            let t = death_event.enemy_pos;
            let texture_handle = asset_server.load("textures/effects/hit-particles.png");
            let texture_atlas =
                TextureAtlas::from_grid(texture_handle, Vec2::new(32.0, 32.0), 7, 1, None, None);
            let texture_atlas_handle = texture_atlases.add(texture_atlas);
            commands.spawn((
                SpriteSheetBundle {
                    texture_atlas: texture_atlas_handle,
                    transform: Transform::from_translation(t.extend(0.)),
                    ..default()
                },
                AnimationTimer(Timer::from_seconds(0.075, TimerMode::Repeating)),
                YSort,
                DoneAnimation,
                Name::new("Hit Spark"),
            ));
            if let Ok(loot_table) = loot_tables.get(death_event.entity) {
                for drop in LootTablePlugin::get_drops(loot_table, loot_bonus.single().0) {
                    proto_commands.spawn_item_from_proto(
                        drop.obj_type,
                        &proto_param,
                        death_event.enemy_pos,
                        drop.count,
                    );
                }
            }
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
        transforms: Query<&GlobalTransform>,
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
            let hit_pos = transforms.get(hit.0).unwrap().translation();

            commands.spawn((
                SpriteSheetBundle {
                    texture_atlas: texture_atlas_handle,
                    transform: Transform::from_translation(Vec3::new(
                        hit_pos.x + x_rng as f32,
                        hit_pos.y - 5. + y_rng as f32,
                        1.,
                    )),
                    ..default()
                },
                AnimationTimer(Timer::from_seconds(0.075, TimerMode::Repeating)),
                YSort,
                DoneAnimation,
                Name::new("Hit Spark"),
            ));

            commands.entity(hit.0).remove::<JustGotHit>();
        }
    }

    pub fn handle_hits(
        mut commands: Commands,
        game: GameParam,
        mut health: Query<(
            Entity,
            &mut CurrentHealth,
            &GlobalTransform,
            Option<&WorldObject>,
            Option<&InvincibilityCooldown>,
        )>,
        mut hit_events: EventReader<HitEvent>,
        mut enemy_death_events: EventWriter<EnemyDeathEvent>,
        mut obj_death_events: EventWriter<ObjBreakEvent>,
        in_i_frame: Query<&InvincibilityTimer>,
        breaks_with_query: Query<&BreaksWith>,
        proto_param: ProtoParam,
    ) {
        for hit in hit_events.iter() {
            // is in invincibility frames from a previous hit
            if in_i_frame.get(hit.hit_entity).is_ok() {
                return;
            }
            if let Ok((e, mut hit_health, t, obj_option, i_frame_option)) =
                health.get_mut(hit.hit_entity)
            {
                // don't shoot a dead horse...
                if hit_health.0 <= 0 {
                    continue;
                }
                if let Some(obj) = obj_option {
                    let anchor = proto_param
                        .get_component::<SpriteAnchor, _>(obj.clone())
                        .unwrap_or(&SpriteAnchor(Vec2::ZERO));
                    let pos = world_pos_to_tile_pos(t.translation().truncate() - anchor.0);

                    //TODO: create breaks with tool component, instead of using properties
                    if let Some(main_hand_tool) = hit.hit_with {
                        if let Ok(breaks_with) = breaks_with_query.get(hit.hit_entity) {
                            if main_hand_tool != breaks_with.0 {
                                continue;
                            }
                        }
                    }
                    hit_health.0 -= hit.damage as i32;

                    println!("HP {:?} {:?}", e, hit_health.0);
                    if hit_health.0 <= 0 {
                        obj_death_events.send(ObjBreakEvent {
                            entity: e,
                            obj: *obj,
                            pos,
                        });
                    }
                } else {
                    hit_health.0 -= hit.damage as i32;
                    println!("HP {:?}", hit_health.0);

                    // let has_i_frames = has_i_frames.get(hit.hit_entity);
                    commands.entity(hit.hit_entity).insert(HitAnimationTracker {
                        timer: Timer::from_seconds(
                            //TODO: once we create builders for creatures, add this as a default to all creatures that can be hit
                            0.2,
                            TimerMode::Once,
                        ),
                        knockback: 400.,
                        dir: hit.dir,
                    });
                    if let Some(i_frames) = i_frame_option {
                        commands.entity(hit.hit_entity).insert(InvincibilityTimer(
                            Timer::from_seconds(i_frames.0, TimerMode::Once),
                        ));
                    }
                    if hit_health.0 <= 0 && game.player_query.single().0 != e {
                        commands.entity(e).insert(MarkedForDeath);
                        println!("SENDING DEATH EVENT {e:?}");
                        enemy_death_events.send(EnemyDeathEvent {
                            entity: e,
                            enemy_pos: t.translation().truncate(),
                        })
                    }
                }

                if let Some(mut hit_e) = commands.get_entity(hit.hit_entity) {
                    hit_e.insert(JustGotHit);
                }
            }
        }
    }
    fn cleanup_marked_for_death_entities(
        mut commands: Commands,
        dead_query: Query<Entity, With<MarkedForDeath>>,
    ) {
        for e in dead_query.iter() {
            println!("DESPAWNING {e:?}");
            commands.entity(e).despawn_recursive();
        }
    }
}
