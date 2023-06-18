use bevy::prelude::*;
use bevy_proto::prelude::{ProtoCommands, Prototypes};
pub trait CommandsExt<'w, 's> {
    fn spawn_item_from_proto<'a>(
        &'a mut self,
        p: String,
        prototypes: &'a Prototypes,
        pos: Vec2,
    ) -> Option<Entity>;
}

impl<'w, 's> CommandsExt<'w, 's> for ProtoCommands<'w, 's> {
    fn spawn_item_from_proto<'a>(
        &'a mut self,
        p: String,
        prototypes: &'a Prototypes,
        pos: Vec2,
    ) -> Option<Entity> {
        if !prototypes.is_ready(&p) {
            return None;
        }

        let spawned_entity = self.spawn(p).id();

        let mut spawned_entity_commands = self.commands().entity(spawned_entity);
        spawned_entity_commands.insert(Transform::from_translation(pos.extend(0.)));

        Some(spawned_entity)
    }
}
