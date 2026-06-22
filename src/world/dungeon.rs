use bevy::prelude::*;

use super::TileMapPosition;

/// Marker component for the active dungeon dimension. The room layout, colliders,
/// shrine, waves and rewards are all handled by [`super::dungeon_room`].
#[derive(Component)]
pub struct Dungeon;
pub struct DungeonPlugin;
impl Plugin for DungeonPlugin {
    fn build(&self, _app: &mut App) {}
}

/// Brief marker storing the player's previous position across a dimension
/// swap. Inserted when leaving a dimension and removed shortly after — stored
/// `SparseSet` so dimension swaps don't churn the persisted dimension entity
/// through extra archetypes.
#[derive(Component, Default)]
#[component(storage = "SparseSet")]
pub struct CachedPlayerPos(pub TileMapPosition);
