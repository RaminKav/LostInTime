# Patches

Local crate checkouts used via `path` deps in `Cargo.toml`.

## `bevy_aseprite_ultra-0.9.0`

Vendored copy of [`bevy_aseprite_ultra`](https://crates.io/crates/bevy_aseprite_ultra) 0.9 for Bevy 0.19 aseprite animation loading.

```toml
bevy_aseprite_ultra = { path = "patches/bevy_aseprite_ultra-0.9.0" }
```

## Removed (Bevy 0.10 era)

These were dropped with the Bevy 0.19 bump — crates.io `bevy_ecs` / `bevy_ecs_tilemap` 0.19 are used directly:

- `bevy_ecs-0.10.1` (chunk-despawn insert panic → warn)
- `bevy_ecs_tilemap-0.10.0` (render extract after despawn)
