# Bevy Engine Patches

This directory contains local patches for Bevy crates to fix race conditions with chunk despawning.

## Why These Patches Are Needed

When chunks despawn, there's a race condition between:
1. Main world despawning tilemap entities
2. Render world extracting tilemap data

Even with 2-frame delays and `CoreSet::Last` scheduling, the render extraction can sometimes try to access entities that have been despawned, causing crashes.

## Patches Applied

### `bevy_ecs-0.10.1`
**File:** `src/system/commands/mod.rs`
**Change:** Suppressed panic when trying to insert/spawn into invalid entities
- Changed from: `panic!("error[B0003]: ...")`
- Changed to: `warn!("Failed to 'insert or spawn' bundle...")` and continue gracefully

### `bevy_ecs_tilemap-0.10.0`
**File:** `src/render/prepare.rs` (line 87)
**Change:** Gracefully handle case where tilemap entity has been despawned
- Changed from: `extracted_tilemaps.get(tile.tilemap_id.0).unwrap()`
- Changed to: `if let Ok(...) = extracted_tilemaps.get(...) { ... } else { continue }`

## Building With Patches

The patches are automatically applied via `Cargo.toml`:

```toml
[patch.crates-io]
bevy_ecs = { path = "patches/bevy_ecs-0.10.1" }
bevy_ecs_tilemap = { path = "patches/bevy_ecs_tilemap-0.10.0" }
```

**Important:** This `patches/` directory must be committed to git and present on all build machines (Mac, Windows, etc.) for builds to work correctly.

## When To Update

These patches are tied to specific versions:
- `bevy_ecs-0.10.1` 
- `bevy_ecs_tilemap-0.10.0`

If you upgrade Bevy to a newer version, you'll need to:
1. Check if the race condition still exists
2. If yes, re-apply the patches to the new version
3. If no, you can remove these patches entirely

## Testing

After applying patches, test by:
1. Running around and letting chunks despawn/respawn frequently
2. Check logs for warnings about invalid entities (expected and safe now)
3. Ensure no crashes occur during chunk transitions

