use crate::{
    animations::AnimationTimer,
    assets::{SpriteAnchor, SpriteSize},
    attributes::ItemLevel,
    inventory::ItemStack,
    item::{
        projectile::{ArcProjectileData, Projectile},
        EquipmentType, ItemDrop, Wall,
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
            let mut spawned_entity_commands = self.commands().entity(spawned_entity);

            if let Some(proto_data) = params.get_item_data(obj.clone()) {
                // modify the item stack count
                let mut proto_data = proto_data.clone();
                proto_data.count = count;
                spawned_entity_commands.insert(proto_data).insert(ItemDrop);
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
    ) -> Option<Entity> {
        if let Some(spawned_entity) = self.spawn_from_proto(obj.clone(), &params.prototypes, pos) {
            let mut spawned_entity_commands = self.commands().entity(spawned_entity);

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
            };
            let mut x_offset = 0.;
            let mut y_offset = 0.;
            let angle = proto_data.direction.y.atan2(proto_data.direction.x);
            if dir != Vec2::ZERO {
                x_offset = (angle.cos() * (sprite_size.x) + angle.cos() * (sprite_size.y)) / 2.;
                y_offset = (angle.sin() * (sprite_size.x) + angle.sin() * (sprite_size.y)) / 2.;
            }
            proto_data.mana_bar_full = mana_bar_full;
            //TODO: make these prototype data
            spawned_entity_commands
                .insert(proto_data)
                .insert(Transform {
                    translation: pos.extend(0.)
                        + Vec3::new(
                            x_offset + (angle.cos() * proj_state.spawn_offset.x),
                            y_offset + (angle.sin() * proj_state.spawn_offset.y),
                            0.,
                        ),
                    rotation: Quat::from_rotation_z(angle),
                    ..default()
                })
                .insert(ActiveEvents::COLLISION_EVENTS)
                .insert(Name::new("Projectile"))
                .insert(ActiveCollisionTypes::all())
                .remove::<ItemStack>();
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
                        Collider::cuboid(arc_data.col_size.x, arc_data.col_size.y),
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
        if !prototypes.is_ready(&p) {
            print!("Prototype {} is not ready", p);
            return None;
        }
        let spawned_entity = self.spawn(p).id();
        let mut spawned_entity_commands = self.commands().entity(spawned_entity);
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
        let spawned_entity = self.spawn(p).id();
        let mut spawned_entity_commands = self.commands().entity(spawned_entity);
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
                .get_component::<WallTextureData, _>(obj)
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
        }

        Some(spawned_entity)
    }
}
