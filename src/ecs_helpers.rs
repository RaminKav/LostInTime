use bevy::ecs::system::{Commands, EntityCommands};
use bevy::prelude::*;

/// Parent `child` under `parent` when commands flush. If `parent` no longer exists, despawn `child`.
pub fn safe_add_child(commands: &mut Commands, parent: Entity, child: Entity) {
    commands.queue(move |world: &mut World| {
        if world.get_entity(parent).is_ok() {
            if let Ok(mut parent_commands) = world.get_entity_mut(parent) {
                parent_commands.add_child(child);
            }
        } else if let Ok(mut child_commands) = world.get_entity_mut(child) {
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
    commands.queue(move |world: &mut World| {
        if world.get_entity(parent).is_ok() {
            if let Ok(mut parent_commands) = world.get_entity_mut(parent) {
                parent_commands.add_children(&children);
            }
        } else {
            for child in children {
                if let Ok(mut child_commands) = world.get_entity_mut(child) {
                    child_commands.despawn();
                }
            }
        }
    });
}

pub trait SafeHierarchyExt {
    /// Parent this entity under `parent` when commands flush.
    fn safe_set_parent(&mut self, parent: Entity) -> &mut Self;

    /// Add `child` under this entity when commands flush.
    fn safe_add_child(&mut self, child: Entity) -> &mut Self;
}

impl SafeHierarchyExt for EntityCommands<'_> {
    fn safe_set_parent(&mut self, parent: Entity) -> &mut Self {
        let child = self.id();
        self.commands().queue(move |world: &mut World| {
            if world.get_entity(parent).is_ok() {
                if let Ok(mut parent_commands) = world.get_entity_mut(parent) {
                    parent_commands.add_child(child);
                }
            } else if let Ok(mut child_commands) = world.get_entity_mut(child) {
                child_commands.despawn();
            }
        });
        self
    }

    fn safe_add_child(&mut self, child: Entity) -> &mut Self {
        let parent = self.id();
        self.commands().queue(move |world: &mut World| {
            if world.get_entity(parent).is_ok() {
                if let Ok(mut parent_commands) = world.get_entity_mut(parent) {
                    parent_commands.add_child(child);
                }
            } else if let Ok(mut child_commands) = world.get_entity_mut(child) {
                child_commands.despawn();
            }
        });
        self
    }
}
