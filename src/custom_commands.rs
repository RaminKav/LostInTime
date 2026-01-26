use crate::{
    animations::AnimationTimer,
    assets::{SpriteAnchor, SpriteSize},
    attributes::ItemLevel,
    inventory::ItemStack,
    item::{
        projectile::{ArcProjectileData, Projectile},
        EquipmentType, ItemDrop, Wall, WorldObject,
    },
    player::mage_skills::Electricity,
    proto::proto_param::ProtoParam,
    world::{
        wall_auto_tile::Dirty,
        world_helpers::{tile_pos_to_world_pos, world_pos_to_chunk_relative_tile_pos},
        WallTextureData,
    },
};
use bevy::prelude::*;
use bevy_aseprite::{anim::AsepriteAnimation, Aseprite};
use bevy_proto::prelude::{ProtoCommands, Prototypes, Schematic};
use bevy_rapier2d::prelude::{ActiveCollisionTypes, ActiveEvents, Collider, Sensor};
use core::fmt::Display;
use std::f32::consts::PI;
pub trait CommandsExt<'w, 's> {
    fn spawn_item_from_proto<'a, T: Display + Schematic + Clone + Into<&'a str>>(
        &mut self,
        obj: T,
        params: &ProtoParam,
        pos: Vec2,
        count: usize,
        level: Option<u8>,
    ) -> Option<Entity>;
    fn spawn_projectile_from_proto<'a, T: Display + Schematic + Clone + Into<&'a str>>(
        &mut self,
        obj: T,
        params: &ProtoParam,
        pos: Vec2,
        dir: Vec2,
        mana_bar_full: bool,
        asset_server: &AssetServer,
        scale_up: f32,
    ) -> Option<Entity>;
    fn spawn_from_proto<'a, T: Display + Schematic + Clone + Into<&'a str>>(
        &mut self,
        obj: T,
        prototypes: &Prototypes,
        pos: Vec2,
    ) -> Option<Entity>;
    fn spawn_object_from_proto<'a, T: Display + Schematic + Clone + Into<&'a str>>(
        &mut self,
        obj: T,
        pos: Vec2,
        prototypes: &Prototypes,
        proto_param: &mut ProtoParam,
        is_dirty: bool,
    ) -> Option<Entity>;
}

impl<'w, 's> CommandsExt<'w, 's> for ProtoCommands<'w, 's> {
    fn spawn_item_from_proto<'a, T: Display + Schematic + Clone + Into<&'a str>>(
        &mut self,
        obj: T,
        params: &ProtoParam,
        pos: Vec2,
        count: usize,
        level: Option<u8>,
    ) -> Option<Entity> {
        if let Some(spawned_entity) = self.spawn_from_proto(obj.clone(), &params.prototypes, pos) {
            // Check entity exists immediately after spawn (can be picked up instantly)
            let Some(mut spawned_entity_commands) = self.commands().get_entity(spawned_entity)
            else {
                return None; // Entity was already despawned
            };

            if let Some(proto_data) = params.get_item_data(obj.clone()) {
                // modify the item stack count
                let mut proto_data = proto_data.clone();
                proto_data.count = count;
                spawned_entity_commands.insert(proto_data).insert(ItemDrop);
                // Add despawn timer to reduce lag in endless mode
                spawned_entity_commands.insert(crate::item::ItemDropDespawnTimer(
                    Timer::from_seconds(300.0, TimerMode::Once), // Despawn after 60 seconds
                ));
                let eqp_type = params
                    .get_component::<EquipmentType, _>(obj.clone())
                    .unwrap_or(&EquipmentType::None);
                if let Some(level) = level {
                    if eqp_type.is_weapon() || (eqp_type.is_equipment() && !eqp_type.is_accessory())
                    {
                        spawned_entity_commands.insert(ItemLevel(level));
                    }
                }
            }

            // Fix item graphics immediately - replace proto atlas with shared game atlas
            // This ensures items have correct graphics from spawn, not relying on update_graphics
            if let Some(sprite_map) = &params.graphics.spritesheet_map {
                if let Some(obj_type) = params.get_component::<WorldObject, _>(obj.clone()) {
                    if let Some(sprite) = sprite_map.get(obj_type) {
                        spawned_entity_commands
                            .insert(params.graphics.texture_atlas.as_ref().unwrap().clone())
                            .insert(sprite.clone());
                    }
                }
            }

            return Some(spawned_entity);
        }
        None
    }
    fn spawn_projectile_from_proto<'a, T: Display + Schematic + Clone + Into<&'a str>>(
        &mut self,
        obj: T,
        params: &ProtoParam,
        pos: Vec2,
        dir: Vec2,
        mana_bar_full: bool,
        asset_server: &AssetServer,
        scale_up: f32,
    ) -> Option<Entity> {
        let obj_type = <T as Into<&str>>::into(obj.clone()).to_owned(); // Get obj_type for logging
        if let Some(spawned_entity) = self.spawn_from_proto(obj.clone(), &params.prototypes, pos) {
            // Check entity exists immediately after spawn (projectiles can despawn quickly)
            let Some(mut spawned_entity_commands) = self.commands().get_entity(spawned_entity)
            else {
                return None; // Entity was already despawned
            };

            let Some(proj_state) = params.get_projectile_state(obj.clone()) else {
                return None;
            };
            // modify the direction and offset of projectile
            let mut proto_data = proj_state.clone();
            proto_data.direction = dir;
            let sprite_size = if let Some(sprite_data) = params.get_sprite_sheet_data(obj.clone()) {
                sprite_data.size
            } else {
                Vec2::new(16., 16.)
            } * scale_up;
            let mut x_offset = 0.;
            let mut y_offset = 0.;
            let angle = proto_data.direction.y.atan2(proto_data.direction.x);
            if dir != Vec2::ZERO {
                x_offset = (angle.cos() * (sprite_size.x) + angle.cos() * (sprite_size.y)) / 2.;
                y_offset = (angle.sin() * (sprite_size.x) + angle.sin() * (sprite_size.y)) / 2.;
            }
            let custom_rotation =
                if let Some(proj) = params.get_component::<Projectile, _>(obj.clone()) {
                    proj.get_custom_rotation()
                } else {
                    None
                };
            proto_data.mana_bar_full = mana_bar_full;
            //TODO: make these prototype data
            spawned_entity_commands
                .insert(proto_data)
                .insert(Transform {
                    translation: pos.extend(0.)
                        + Vec3::new(
                            x_offset + (angle.cos() * proj_state.spawn_offset.x * scale_up),
                            y_offset + (angle.sin() * proj_state.spawn_offset.y * scale_up),
                            0.,
                        ),
                    rotation: Quat::from_rotation_z(angle + custom_rotation.unwrap_or(0.)),
                    scale: Vec3::splat(scale_up),
                    ..default()
                })
                .insert(ActiveEvents::COLLISION_EVENTS)
                .insert(Name::new("Projectile"))
                .insert(ActiveCollisionTypes::all())
                .remove::<ItemStack>();

            // Fix projectile graphics immediately - replace proto atlas with shared game atlas
            // This is needed because projectiles are excluded from update_graphics to prevent crashes
            if let Some(sprite_map) = &params.graphics.spritesheet_map {
                if let Some(obj_type) = params.get_component::<WorldObject, _>(obj.clone()) {
                    if let Some(sprite) = sprite_map.get(obj_type) {
                        spawned_entity_commands
                            .insert(params.graphics.texture_atlas.as_ref().unwrap().clone())
                            .insert(sprite.clone());
                    }
                }
            }
            if let Some(arc_data) = params.get_component::<ArcProjectileData, _>(obj.clone()) {
                spawned_entity_commands.with_children(|parent| {
                    let angle = arc_data.col_points[0];
                    parent.spawn((
                        TransformBundle::from_transform(Transform {
                            translation: (Vec3::new(
                                (angle.cos() * (arc_data.size.x) + angle.cos() * (arc_data.size.y))
                                    / 2.,
                                (angle.sin() * (arc_data.size.x) + angle.sin() * (arc_data.size.y))
                                    / 2.,
                                0.,
                            )),
                            rotation: Quat::from_rotation_z((arc_data.col_points[0]) - PI / 2.),
                            ..default()
                        }),
                        Sensor,
                        Collider::cuboid(
                            arc_data.col_size.x * scale_up,
                            arc_data.col_size.y * scale_up,
                        ),
                        ActiveEvents::COLLISION_EVENTS,
                        ActiveCollisionTypes::all(),
                    ));
                });
            }
            if let Some(proj) = params.get_component::<Projectile, _>(obj.clone()) {
                if proj == &Projectile::Electricity {
                    spawned_entity_commands
                        .insert(AsepriteAnimation::from(Electricity::tags::ELECTRICITY))
                        .insert(asset_server.load::<Aseprite, _>(Electricity::PATH))
                        .remove::<TextureAtlasSprite>()
                        .remove::<Handle<TextureAtlas>>()
                        .remove::<AnimationTimer>();
                }
            }

            return Some(spawned_entity);
        }
        None
    }
    fn spawn_from_proto<'a, T: Display + Schematic + Clone + Into<&'a str>>(
        &mut self,
        mob: T,
        prototypes: &Prototypes,
        pos: Vec2,
    ) -> Option<Entity> {
        let p = <T as Into<&str>>::into(mob).to_owned();
        let p_clone = p.clone(); // Clone for logging
        if !prototypes.is_ready(&p) {
            print!("Prototype {} is not ready", p);
            return None;
        }
        let spawned_entity = self.spawn(p).id();
        // Check entity exists immediately after spawn
        let Some(mut spawned_entity_commands) = self.commands().get_entity(spawned_entity) else {
            return None; // Entity was already despawned
        };

        spawned_entity_commands
            .insert(Transform::from_translation(pos.extend(0.)))
            .insert(ActiveEvents::COLLISION_EVENTS);
        Some(spawned_entity)
    }
    fn spawn_object_from_proto<'a, T: Display + Schematic + Clone + Into<&'a str>>(
        &mut self,
        obj: T,
        pos: Vec2,
        prototypes: &Prototypes,
        proto_param: &mut ProtoParam,
        is_dirty: bool,
    ) -> Option<Entity> {
        let p = <T as Into<&str>>::into(obj.clone()).to_owned();
        if !prototypes.is_ready(&p) {
            error!("Prototype {} is not ready", p);
            return None;
        }
        //TODO: add parent to spawned entity
        let spawned_entity = self.spawn(p.clone()).id();
        // Check entity exists immediately after spawn
        let Some(mut spawned_entity_commands) = self.commands().get_entity(spawned_entity) else {
            return None; // Entity was already despawned
        };
        let relative_tile_pos = world_pos_to_chunk_relative_tile_pos(pos);
        let should_center = proto_param
            .get_component::<SpriteSize, _>(obj.clone())
            .unwrap_or(&SpriteSize::Small)
            .is_medium();
        let pos = tile_pos_to_world_pos(relative_tile_pos, should_center).extend(0.);
        spawned_entity_commands.insert(TransformBundle::from_transform(
            Transform::from_translation(pos),
        ));

        if let Some(anchor) = proto_param.get_component::<SpriteAnchor, _>(obj.clone()) {
            spawned_entity_commands.insert(TransformBundle::from_transform(
                Transform::from_translation(pos + anchor.0.extend(0.)),
            ));
        }
        if let Some(_wall) = proto_param.get_component::<Wall, _>(obj.clone()) {
            let sprite_data = proto_param
                .get_component::<WallTextureData, _>(obj.clone())
                .unwrap();
            spawned_entity_commands
                .insert(
                    proto_param
                        .graphics
                        .wall_texture_atlas
                        .as_ref()
                        .unwrap()
                        .clone(),
                )
                .insert(TextureAtlasSprite {
                    index: (sprite_data.obj_bit_index + sprite_data.texture_offset * 32) as usize,
                    ..default()
                });
            if is_dirty {
                spawned_entity_commands.insert(Dirty);
            }
        } else {
            // Fix world object graphics immediately - replace proto atlas with shared game atlas
            // This ensures world objects have correct graphics from spawn
            if let Some(sprite_map) = &proto_param.graphics.spritesheet_map {
                if let Some(obj_type) = proto_param.get_component::<WorldObject, _>(obj.clone()) {
                    if obj_type != &WorldObject::CombatShrine
                        && obj_type != &WorldObject::CombatShrineDone
                        && obj_type != &WorldObject::PinkFlower
                    {
                        if let Some(sprite) = sprite_map.get(obj_type) {
                            spawned_entity_commands
                                .insert(
                                    proto_param.graphics.texture_atlas.as_ref().unwrap().clone(),
                                )
                                .insert(sprite.clone());
                        }
                    }
                }
            }
        }

        Some(spawned_entity)
    }
}
