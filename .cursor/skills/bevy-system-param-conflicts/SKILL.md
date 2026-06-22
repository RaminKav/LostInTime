---
name: bevy-system-param-conflicts
description: Detect and prevent Bevy 0.10 runtime system-param conflict panics (B0002) in this survival-rogue-like project. Use when adding or editing a Bevy system, adding a param to a SystemParam struct (e.g. GameParam, ProtoParam, ItemActionParam), or when a panic mentions "conflicts with a previous", "B0002", "conflicting access", or "EraManager/Graphics/Player ... conflicts".
---

# Bevy System-Param Conflicts (B0002)

Bevy does NOT catch conflicting data access at compile time. A system that takes
two conflicting params (e.g. `Res<T>` + something that also accesses `T` mutably)
compiles fine and only panics the first time the system runs:

```
error[B0002]: ResMut<...::EraManager> in system ...::drop_dungeon_rewards_on_return
conflicts with a previous Res<...::EraManager> access. Consider removing the duplicate access.
```

The follow-on `RecvError` panic from `bevy_render::pipelined_rendering` is just the
render thread dying after the main-thread panic — ignore it; fix the B0002.

## Root cause in this project

The big composite `#[derive(SystemParam)]` structs already pull in many resources
and queries. Taking the same thing again in the same system conflicts.

| Composite param | Already accesses (non-exhaustive) |
|---|---|
| `GameParam` | `Res<EraManager>`, `Res<Graphics>`, `Res<WorldObjectCache>`, player `Query<&mut ...>`, chunk/object queries |
| `ProtoParam` | `ProtoCommands`, `Prototypes`, `Res<Graphics>`, `Res<AssetServer>`, `ResMut<Assets<Mesh>>` |
| `ItemActionParam` | many `EventWriter`s + `ResMut<NextState<...>>` + player queries |

Rule of thumb: if you already take `GameParam`, do NOT also take `Res<EraManager>`,
`Res<Graphics>`, or another `Query<..., With<Player>>` that overlaps mutably — read
them through `game.*` instead (e.g. `game.era.current_era`, `game.graphics`).

## Workflow when adding/editing a system

1. List every param the system takes, expanding composite `SystemParam` structs.
2. Check for duplicates / mutable-vs-any overlaps:
   - Same resource as both a direct param and inside a composite (most common here).
   - `ResMut<T>` anywhere + `Res<T>`/`ResMut<T>` anywhere else in the same system.
   - Two `Query`s over the same component where at least one is `&mut` and the
     archetypes overlap, without `Without<...>` disjointness.
3. Prefer reading shared state through the composite param you already have.
   If you truly need separate access, make the queries disjoint with `Without<...>`
   or split into two systems.
4. Verify by actually running the game far enough to execute the system — B0002
   only fires at runtime, so a clean `cargo build` is NOT sufficient.

## Verifying

`cargo build` passing does NOT prove there is no conflict. To catch B0002 you must
run and exercise the system. Use the helper to scan the latest run log for B0002:

```bash
bash .cursor/skills/bevy-system-param-conflicts/scripts/check_conflicts.sh /path/to/run_output.log
```

If the user pasted a panic, fix the named system directly: open the system, find
the param named in the error, and remove the duplicate (route it through the
composite param instead).

## Examples

**Conflict (panics):** `GameParam` already has `EraManager`.
```rust
fn my_system(game: GameParam, era: Res<EraManager>) { /* B0002 */ }
```
**Fixed:**
```rust
fn my_system(game: GameParam) { let era = &game.era; }
```

**Conflict:** two mutable player queries.
```rust
fn s(game: GameParam, mut q: Query<&mut Transform, With<Player>>) { /* may conflict */ }
```
**Fixed:** read through `game`, or make disjoint, or split the system.
