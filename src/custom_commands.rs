use crate::{proto::proto_param::ProtoParam, world::world_helpers::camera_pos_to_tile_pos};
use bevy::prelude::*;
use bevy_proto::prelude::{ProtoCommands, Prototypes, Schematic};
use core::fmt::Display;
pub trait CommandsExt<'w, 's> {
    fn spawn_item_from_proto<'a, T: Display + Schematic + Clone + Into<&'a str>>(
        &mut self,
        obj: T,
        params: &ProtoParam,
        pos: Vec2,
        count: usize,
    ) -> Option<Entity>;
    fn spawn_mob_from_proto<'a, T: Display + Schematic + Clone + Into<&'a str>>(
        &mut self,
        obj: T,
        prototypes: &Prototypes,
        pos: Vec2,
    ) -> Option<Entity>;
    fn spawn_object_from_proto<'a, T: Display + Schematic + Clone + Into<&'a str>>(
        &mut self,
        obj: T,
        prototypes: &Prototypes,
        pos: Vec2,
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
        let p = <T as Into<&str>>::into(obj).to_owned();

        if !params.prototypes.is_ready(&p) {
            print!("Prototype {} is not ready!", p);
            return None;
        }
        let spawned_entity = self.spawn(p.clone()).id();
        let mut spawned_entity_commands = self.commands().entity(spawned_entity);
        if let Some(proto_data) = params.get_item_data(&p) {
            // modify item stack count
            let mut proto_data = proto_data.clone();
            proto_data.count = count;
            spawned_entity_commands.insert(proto_data);
        }

        spawned_entity_commands.insert(Transform::from_translation(pos.extend(0.)));
        Some(spawned_entity)
    }
    fn spawn_mob_from_proto<'a, T: Display + Schematic + Clone + Into<&'a str>>(
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
        spawned_entity_commands.insert(Transform::from_translation(pos.extend(0.)));
        Some(spawned_entity)
    }
    fn spawn_object_from_proto<'a, T: Display + Schematic + Clone + Into<&'a str>>(
        &mut self,
        obj: T,
        prototypes: &Prototypes,
        pos: Vec2,
    ) -> Option<Entity> {
        let p = <T as Into<&str>>::into(obj.clone()).to_owned();
        if !prototypes.is_ready(&p) {
            println!("Prototype {} is not ready", p);
            return None;
        }
        //TODO: add parent to spawned entity
        let spawned_entity = self.spawn(p).id();

        let mut spawned_entity_commands = self.commands().entity(spawned_entity);
        let tile_pos = camera_pos_to_tile_pos(&pos);
        let pos = Vec3::new(
            (tile_pos.x as i32 * 32) as f32,
            (tile_pos.y as i32 * 32) as f32,
            0.,
        );
        spawned_entity_commands.insert(TransformBundle::from_transform(
            Transform::from_translation(pos),
        ));
        Some(spawned_entity)
    }
}
