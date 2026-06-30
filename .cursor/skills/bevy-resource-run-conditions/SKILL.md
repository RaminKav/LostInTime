---
name: bevy-resource-run-conditions
description: Prevent Bevy 0.10 runtime panics when using resource_changed or Res<T> in system run conditions. Use when adding or editing .run_if(...), resource_changed, resource_exists, init_resource, insert_resource on schedule, or when a panic says "Resource requested by ... does not exist".
---

# Bevy Resource Run Conditions

In Bevy 0.10, **`resource_changed::<T>()` and `Res<T>` inside a `run_if` closure panic if `T` is not in the world** when the condition is evaluated — even if the system itself would never run.

The follow-on `RecvError` panic from `bevy_render::pipelined_rendering` is a cascade after the main-thread panic; fix the missing resource, not the render thread.

## Panic signature

```
Resource requested by bevy_ecs::schedule::condition::common_conditions::resource_changed<...::MyResource>::{{closure}}
does not exist: ...::MyResource
```

## Rules

1. **Register the resource before any system/condition can see it**
   - Prefer `app.init_resource::<T>()` (or `insert_resource(T::default())`) in the plugin's `build`, not only in `OnEnter` / a one-shot setup system.
   - If a later system loads persisted state, **update** with `ResMut<T>` — do not rely on first `insert_resource` from a schedule hook.

2. **Guard optional or late-created resources in run conditions**
   ```rust
   my_system
       .run_if(resource_exists::<MyResource>())
       .run_if(resource_changed::<MyResource>()),
   ```
   Never use `resource_changed::<T>()` alone when `T` might not exist yet.

3. **Do not use `Res<T>` in a `run_if` closure unless `T` is always present**
   - Safe: resource initialized in the same plugin at startup.
   - Safer: combine with `resource_exists::<T>()` before the closure.
   - Alternative: use `resource_equals` / `resource_exists_and_equals` from `bevy::ecs::schedule::common_conditions`.

4. **System params vs run conditions**
   - `Option<Res<T>>` in the **system body** is fine (system skipped or param optional depending on design).
   - `run_if` closures are **not** optional — missing `Res<T>` still panics.

## Checklist when adding a new resource + conditional system

- [ ] `init_resource::<T>()` (or `insert_resource`) in plugin `build`
- [ ] Any `OnEnter` loader uses `ResMut<T>`, not first-time `insert_resource`, unless the resource is truly ephemeral
- [ ] Every `resource_changed::<T>()` chained with `resource_exists::<T>()` first (unless guaranteed at app start)
- [ ] Every `run_if(|x: Res<T>| ...)` has `resource_exists::<T>()` first, or `T` is init'd at startup
- [ ] Grep the codebase for `resource_changed::<T>` and `Res<T>` in run_ifs after introducing `T`

## Example (this repo: main-menu leaderboard visibility)

```rust
// UIPlugin::build — resource always exists (default false)
app.init_resource::<MainMenuLeaderboardVisible>();

// OnEnter(MainMenu) — load persisted preference
fn init_main_menu_leaderboard_visibility(
    mut visible: ResMut<MainMenuLeaderboardVisible>,
    game_data: Option<Res<GameData>>,
) {
    visible.0 = game_data
        .as_ref()
        .and_then(|d| d.show_main_menu_leaderboard)
        .unwrap_or(false);
}

// Schedule — safe run conditions
sync_main_menu_leaderboard_ui
    .run_if(in_state(GameState::MainMenu))
    .run_if(in_state(UIState::Closed))
    .run_if(resource_exists::<MainMenuLeaderboardVisible>())
    .run_if(resource_changed::<MainMenuLeaderboardVisible>()),
```

## Related

- System-param access conflicts (B0001/B0002): see [bevy-system-param-conflicts](../bevy-system-param-conflicts/SKILL.md)
