use bevy::prelude::*;
use bevy_ecs_tilemap::tiles::TilePos;
use bevy_rapier2d::prelude::RapierContext;
use rand::Rng;

use crate::{
    animations::{AnimationTimer, AttackEvent, DoneAnimation, HitAnimationTracker},
    attributes::{Attack, AttackCooldown, AttributeModifier, Health, InvincibilityCooldown},
    inventory::Inventory,
    item::{LootTable, LootTablePlugin, MainHand, WorldObject},
    ui::InventoryState,
    world::world_helpers::{camera_pos_to_block_pos, camera_pos_to_chunk_pos},
    CoreGameSet, CustomFlush, Game, GameParam, Player, YSort,
};

#[derive(Debug, Clone)]
pub struct HitEvent {
    pub hit_entity: Entity,
    pub damage: i32,
    pub dir: Vec2,
    pub hit_with: Option<WorldObject>,
}

#[derive(Debug, Clone)]

pub struct EnemyDeathEvent {
    pub entity: Entity,
    pub enemy_pos: Vec2,
}
#[derive(Debug, Clone)]

pub struct ObjBreakEvent {
    pub entity: Entity,
    pub obj: WorldObject,
    pub tile_pos: TilePos,
    pub chunk_pos: IVec2,
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
            .add_event::<ObjBreakEvent>()
            .add_systems(
                (
                    Self::handle_hits,
                    Self::handle_attack_cooldowns.before(CustomFlush),
                    Self::spawn_hit_spark_effect.after(Self::handle_hits),
                    Self::handle_invincibility_frames.after(Self::handle_hits),
                    Self::handle_enemy_death.after(Self::handle_hits),
                    Self::check_hit_collisions,
                )
                    .in_base_set(CoreGameSet::Main),
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
        mut game: GameParam,
        mut death_events: EventReader<EnemyDeathEvent>,
        asset_server: Res<AssetServer>,
        mut texture_atlases: ResMut<Assets<TextureAtlas>>,
        loot_tables: Query<&LootTable>,
    ) {
        for death_event in death_events.iter() {
            let t = death_event.enemy_pos;
            commands.entity(death_event.entity).despawn();
            let enemy_chunk_pos = camera_pos_to_chunk_pos(&Vec2::new(t.x, t.y));
            let enemy_tile_pos = camera_pos_to_block_pos(&Vec2::new(t.x, t.y));
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
                for drop in LootTablePlugin::get_drops(loot_table) {
                    drop.obj_type.spawn_item_drop(
                        &mut commands,
                        &mut game,
                        enemy_tile_pos,
                        enemy_chunk_pos,
                        drop.count,
                        Some(drop.attributes),
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

    fn handle_hits(
        mut commands: Commands,
        game: GameParam,
        mut health: Query<(
            Entity,
            &mut Health,
            &GlobalTransform,
            Option<&WorldObject>,
            Option<&InvincibilityCooldown>,
        )>,
        mut hit_events: EventReader<HitEvent>,
        mut enemy_death_events: EventWriter<EnemyDeathEvent>,
        mut obj_death_events: EventWriter<ObjBreakEvent>,
        in_i_frame: Query<&InvincibilityTimer>,
    ) {
        for hit in hit_events.iter() {
            // is in invincibility frames from a previous hit
            if in_i_frame.get(hit.hit_entity).is_ok() {
                return;
            }
            if let Ok((e, mut hit_health, t, obj_option, i_frame_option)) =
                health.get_mut(hit.hit_entity)
            {
                if let Some(obj) = obj_option {
                    let obj_data = game.world_obj_data.properties.get(&obj).unwrap();
                    let anchor = obj_data.anchor.unwrap_or(Vec2::ZERO);
                    let obj_chunk_pos = camera_pos_to_chunk_pos(
                        &(t.translation().truncate() - anchor * obj_data.size),
                    );
                    let obj_tile_pos = camera_pos_to_block_pos(
                        &(t.translation().truncate() - anchor * obj_data.size),
                    );

                    if let Some(data) = game.world_obj_data.properties.get(&obj) {
                        if let Some(main_hand_tool) = hit.hit_with {
                            if let Some(breaks_with) = data.breaks_with {
                                if main_hand_tool != breaks_with {
                                    continue;
                                }
                            }
                        }
                        hit_health.0 -= hit.damage as i32;

                        println!("HP {hit_health:?}");
                        if hit_health.0 <= 0 {
                            obj_death_events.send(ObjBreakEvent {
                                entity: e,
                                obj: *obj,
                                tile_pos: obj_tile_pos,
                                chunk_pos: obj_chunk_pos,
                            });
                        }
                    }
                } else {
                    hit_health.0 -= hit.damage as i32;

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
    fn check_hit_collisions(
        mut commands: Commands,
        context: ResMut<RapierContext>,
        weapons: Query<(Entity, &Parent, &WorldObject), (Without<HitMarker>, With<MainHand>)>,
        parent_attack: Query<&Attack>,
        mut hit_event: EventWriter<HitEvent>,
        game: Res<Game>,
        inv_state: Query<&InventoryState>,
        mut inv: Query<&mut Inventory>,
        world_obj: Query<Entity, (With<WorldObject>, Without<MainHand>)>,
    ) {
        if let Ok(weapon) = weapons.get_single() {
            let weapon_parent = weapon.1;
            if let Some(hit) = context.intersection_pairs().find(|c| {
                (c.0 == weapon.0 && c.1 != weapon_parent.get())
                    || (c.1 == weapon.0 && c.0 != weapon_parent.get())
            }) {
                let hit_entity = if hit.0 == weapon.0 { hit.1 } else { hit.0 };
                if !game.player_state.is_attacking || world_obj.get(hit_entity).is_ok() {
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
                    hit_entity,
                    damage: parent_attack.get(**weapon_parent).unwrap().0,
                    dir: Vec2::new(0., 0.),
                    hit_with: Some(*weapon.2),
                });
            }
        }
    }
}
