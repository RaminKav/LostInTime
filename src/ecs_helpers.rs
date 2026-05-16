use bevy::ecs::system::{Commands, EntityCommands};
use bevy::prelude::*;

/// Parent `child` under `parent` when commands flush. If `parent` no longer exists, despawn `child`.
pub fn safe_add_child(commands: &mut Commands, parent: Entity, child: Entity) {
    commands.add(move |world: &mut World| {
        if world.get_entity(parent).is_some() {
            if let Some(mut parent_commands) = world.get_entity_mut(parent) {
                parent_commands.add_child(child);
            }
        } else if let Some(mut child_commands) = world.get_entity_mut(child) {
            child_commands.despawn();
        }
    });
}

/// Reparent `child` under `parent` when commands flush. If `parent` no longer exists, despawn `child`.
pub fn safe_set_parent(commands: &mut Commands, child: Entity, parent: Entity) {
    safe_add_child(commands, parent, child);
}

/// Parent each child under `parent` when commands flush. If `parent` no longer exists, despawn orphans.
pub fn safe_push_children(commands: &mut Commands, parent: Entity, children: &[Entity]) {
    let children = children.to_vec();
    commands.add(move |world: &mut World| {
        if world.get_entity(parent).is_some() {
            if let Some(mut parent_commands) = world.get_entity_mut(parent) {
                parent_commands.push_children(&children);
            }
        } else {
            for child in children {
                if let Some(mut child_commands) = world.get_entity_mut(child) {
                    child_commands.despawn();
                }
            }
        }
    });
}

pub trait SafeHierarchyExt<'w, 's> {
    /// Parent this entity under `parent` when commands flush.
    fn safe_set_parent(&mut self, parent: Entity) -> &mut Self;

    /// Add `child` under this entity when commands flush.
    fn safe_add_child(&mut self, child: Entity) -> &mut Self;
}

impl<'w, 's, 'a> SafeHierarchyExt<'w, 's> for EntityCommands<'w, 's, 'a> {
    fn safe_set_parent(&mut self, parent: Entity) -> &mut Self {
        let child = self.id();
        self.commands().add(move |world: &mut World| {
            if world.get_entity(parent).is_some() {
                if let Some(mut parent_commands) = world.get_entity_mut(parent) {
                    parent_commands.add_child(child);
                }
            } else if let Some(mut child_commands) = world.get_entity_mut(child) {
                child_commands.despawn();
            }
        });
        self
    }

    fn safe_add_child(&mut self, child: Entity) -> &mut Self {
        let parent = self.id();
        self.commands().add(move |world: &mut World| {
            if world.get_entity(parent).is_some() {
                if let Some(mut parent_commands) = world.get_entity_mut(parent) {
                    parent_commands.add_child(child);
                }
            } else if let Some(mut child_commands) = world.get_entity_mut(child) {
                child_commands.despawn();
            }
        });
        self
    }
}
