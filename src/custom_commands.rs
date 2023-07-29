use crate::{
    assets::{SpriteAnchor, SpriteSize},
    item::{Foliage, Wall},
    proto::proto_param::ProtoParam,
    world::{
        wall_auto_tile::Dirty,
        world_helpers::{tile_pos_to_world_pos, world_pos_to_chunk_relative_tile_pos},
    },
};
use bevy::{prelude::*, sprite::Mesh2dHandle};
use bevy_proto::prelude::{ProtoCommands, Prototypes, Schematic};
use bevy_rapier2d::prelude::{ActiveCollisionTypes, ActiveEvents};
use core::fmt::Display;
pub trait CommandsExt<'w, 's> {
    fn spawn_item_from_proto<'a, T: Display + Schematic + Clone + Into<&'a str>>(
        &mut self,
        obj: T,
        params: &ProtoParam,
        pos: Vec2,
        count: usize,
    ) -> Option<Entity>;
    fn spawn_projectile_from_proto<'a, T: Display + Schematic + Clone + Into<&'a str>>(
        &mut self,
        obj: T,
        params: &ProtoParam,
        pos: Vec2,
        dir: Vec2,
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
    ) -> Option<Entity>;
}

impl<'w, 's> CommandsExt<'w, 's> for ProtoCommands<'w, 's> {
    fn spawn_item_from_proto<'a, T: Display + Schematic + Clone + Into<&'a str>>(
        &mut self,
        obj: T,
        params: &ProtoParam,
        pos: Vec2,
        count: usize,
    ) -> Option<Entity> {
        if let Some(spawned_entity) = self.spawn_from_proto(obj.clone(), &params.prototypes, pos) {
            let mut spawned_entity_commands = self.commands().entity(spawned_entity);

            if let Some(proto_data) = params.get_item_data(obj.clone()) {
                // modify the item stack count
                let mut proto_data = proto_data.clone();
                proto_data.count = count;
                spawned_entity_commands.insert(proto_data);
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
    ) -> Option<Entity> {
        if let Some(spawned_entity) = self.spawn_from_proto(obj.clone(), &params.prototypes, pos) {
            let mut spawned_entity_commands = self.commands().entity(spawned_entity);

            let Some(proto_data) = params.get_projectile_state(obj.clone()) else {return None};
            // modify the direction and offset of projectile
            let mut proto_data = proto_data.clone();
            proto_data.direction = dir;
            let angle = proto_data.direction.y.atan2(proto_data.direction.x);
            let sprite_size = if let Some(sprite_data) = params.get_sprite_sheet_data(obj) {
                sprite_data.size
            } else {
                Vec2::new(16., 16.)
            };
            let x_offset = (angle.cos() * (sprite_size.x) + angle.cos() * (sprite_size.y)) / 2.;
            let y_offset = (angle.sin() * (sprite_size.x) + angle.sin() * (sprite_size.y)) / 2.;

            //TODO: make these prototype data
            spawned_entity_commands
                .insert(proto_data)
                .insert(Transform {
                    translation: pos.extend(0.) + Vec3::new(x_offset, y_offset, 0.),
                    rotation: Quat::from_rotation_z(angle),
                    ..default()
                })
                .insert(ActiveEvents::COLLISION_EVENTS)
                .insert(Name::new("Projectile"))
                .insert(ActiveCollisionTypes::all());

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
    ) -> Option<Entity> {
        let p = <T as Into<&str>>::into(obj.clone()).to_owned();
        if !prototypes.is_ready(&p) {
            println!("Prototype {} is not ready", p);
            return None;
        }
        //TODO: add parent to spawned entity
        let spawned_entity = self.spawn(p).id();
        let mut spawned_entity_commands = self.commands().entity(spawned_entity);
        let relative_tile_pos = world_pos_to_chunk_relative_tile_pos(pos);
        let should_center = proto_param
            .get_component::<SpriteSize, _>(obj.clone())
            .unwrap()
            .is_medium();
        let pos = tile_pos_to_world_pos(relative_tile_pos, should_center).extend(0.);
        spawned_entity_commands.insert(TransformBundle::from_transform(
            Transform::from_translation(pos),
        ));

        if let Some(foliage) = proto_param.get_component::<Foliage, _>(obj.clone()) {
            let foliage_material = &proto_param
                .graphics
                .foliage_material_map
                .as_ref()
                .unwrap()
                .get(foliage)
                .unwrap();
            spawned_entity_commands
                .insert(Mesh2dHandle::from(proto_param.meshes.add(Mesh::from(
                    shape::Quad {
                        size: Vec2::new(32., 40.),
                        ..Default::default()
                    },
                ))))
                .insert((*foliage_material).clone());
        }
        if let Some(anchor) = proto_param.get_component::<SpriteAnchor, _>(obj.clone()) {
            spawned_entity_commands.insert(TransformBundle::from_transform(
                Transform::from_translation(pos + anchor.0.extend(0.)),
            ));
        }
        if let Some(_wall) = proto_param.get_component::<Wall, _>(obj) {
            spawned_entity_commands
                .insert(
                    proto_param
                        .graphics
                        .wall_texture_atlas
                        .as_ref()
                        .unwrap()
                        .clone(),
                )
                .insert(TextureAtlasSprite::default())
                .insert(Dirty);
        }

        Some(spawned_entity)
    }
}
