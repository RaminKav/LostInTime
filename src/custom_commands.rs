use crate::{
    item::{Foliage, WorldObject},
    proto::proto_param::{self, ProtoParam},
    world::world_helpers::camera_pos_to_tile_pos,
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

            if let Some(proto_data) = params.get_item_data(obj) {
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

            let Some(proto_data) = params.get_projectile_state(obj) else {return None};
            // modify the direction of projectile
            let mut proto_data = proto_data.clone();
            proto_data.direction = dir;
            //TODO: make these prototype data
            spawned_entity_commands
                .insert(proto_data)
                .insert(ActiveEvents::COLLISION_EVENTS)
                .insert(Name::new("Rock"))
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
        let p = format!("{}Obj", <T as Into<&str>>::into(obj.clone()).to_owned());
        if !prototypes.is_ready(&p) {
            println!("Prototype {} is not ready", p);
            return None;
        }
        //TODO: add parent to spawned entity
        let world_object = proto_param.get_world_object(obj).unwrap();
        let spawned_entity = self.spawn(p).id();

        let mut spawned_entity_commands = self.commands().entity(spawned_entity);
        let tile_pos = camera_pos_to_tile_pos(&pos);
        let pos = Vec3::new(
            (tile_pos.x as i32 * 32) as f32,
            (tile_pos.y as i32 * 32) as f32,
            0.,
        );
        println!("updated to {:?} {}", tile_pos, pos);
        spawned_entity_commands.insert(TransformBundle::from_transform(
            Transform::from_translation(pos),
        ));
        match world_object {
            WorldObject::Foliage(Foliage::Tree) => {
                let foliage_material = &proto_param
                    .graphics
                    .foliage_material_map
                    .as_ref()
                    .unwrap()
                    .get(&WorldObject::Foliage(Foliage::Tree))
                    .unwrap()
                    .0;
                spawned_entity_commands
                    .insert(Mesh2dHandle::from(proto_param.meshes.add(Mesh::from(
                        shape::Quad {
                            size: Vec2::new(32., 40.),
                            ..Default::default()
                        },
                    ))))
                    .insert(foliage_material.clone());
            }
            WorldObject::Wall(_) => {
                println!("ADDING WALL VISUALS");
                spawned_entity_commands
                    .insert(
                        proto_param
                            .graphics
                            .wall_texture_atlas
                            .as_ref()
                            .unwrap()
                            .clone(),
                    )
                    .insert(TextureAtlasSprite::default());
            }
            _ => {}
        }
        Some(spawned_entity)
    }
}
