---
name: bevy-system-param-conflicts
description: Detect and prevent Bevy 0.10 runtime system-param conflict panics (B0001/B0002) in this survival-rogue-like project. Use when adding or editing a Bevy system, adding a param to a SystemParam struct (e.g. GameParam, ProtoParam, ItemActionParam), adding multiple Queries that touch the same component, using ParamSet, or when a panic mentions "conflicts with a previous", "B0001", "B0002", "conflicting access", or "EraManager/Graphics/Player/Visibility ... conflicts".
---

# Bevy System-Param Conflicts (B0001 / B0002)

Bevy does NOT catch conflicting data access at compile time. Conflicting params compile
fine and only panic the first time the system runs.

The follow-on `RecvError` panic from `bevy_render::pipelined_rendering` is just the
render thread dying after the main-thread panic — ignore it; fix the B0001/B0002.

## Two error codes

| Code | Meaning | Typical fix |
|---|---|---|
| **B0001** | Two `Query`s access the same component type (often `&mut Visibility`) in overlapping ways | Put conflicting queries in a `ParamSet`, or make them disjoint with `Without<...>` on **both** sides |
| **B0002** | Duplicate resource access (e.g. `Res<T>` + `ResMut<T>`, or composite `SystemParam` + direct `Res<T>`) | Remove the duplicate param; read through the composite param instead |

Example B0001 panic:
```
error[B0001]: Query<..., &mut Visibility, With<AchievementNameText>> in system ...::update_achievements_page_display
accesses component(s) Visibility in a way that conflicts with a previous system parameter.
Consider using `Without<T>` to create disjoint Queries or merging conflicting Queries into a `ParamSet`.
```

Example B0002 panic:
```
error[B0002]: ResMut<...::EraManager> in system ...::drop_dungeon_rewards_on_return
conflicts with a previous Res<...::EraManager> access.
```

## B0001 — multiple queries on the same component

Common in UI systems that update many sibling entities (name text, desc text, row bg,
clickable hitbox) that all have `Visibility`.

**Conflict (panics):** one query outside `ParamSet`, others inside (or all outside).
```rust
fn update_ui(
    mut row_clickables: Query<(&Row, &mut Visibility), With<Interactable>>,
    mut param_set: ParamSet<(
        Query<(&Row, &mut Text, &mut Visibility), With<NameText>>,
        Query<(&Row, &mut Visibility), With<RowBg>>,
    )>,
) { /* B0001 — row_clickables conflicts with param_set queries on Visibility */ }
```

**Fixed:** move every `&mut Visibility` query into the same `ParamSet`; use `Without<...>`
filters so each query targets a distinct marker component.
```rust
fn update_ui(
    mut param_set: ParamSet<(
        Query<(&Row, &mut Text, &mut Visibility), With<NameText>>,
        Query<(&Row, &mut Visibility), With<RowBg>>,
        Query<
            (&Row, &mut Visibility),
            (With<Interactable>, Without<NameText>, Without<RowBg>),
        >,
    )>,
) {
    { let mut q = param_set.p0(); /* ... */ }
    { let mut q = param_set.p1(); /* ... */ }
    { let mut q = param_set.p2(); /* ... */ }
}
```

Real example in this repo: `src/ui/achievements_ui.rs` → `update_achievements_page_display`.

**UiShadow + recursive fade:** when fading UI hierarchies that include `UiShadowChild`
entities (mesh shadow rings) alongside `Text` nodes, both need `&mut Visibility` in the
same recursive helper. Put both queries in one `ParamSet` and borrow each with `{ ... }`
per entity visit — see `src/blessings/blessing_choice_ui.rs` →
`fade_blessing_card_descendants` / `transition_blessing_ui_after_choice`.

Rules:
- Any system with **2+ queries** that fetch `&mut` on the same component → use `ParamSet`.
- `With<MarkerA>` alone is NOT enough if Bevy still sees overlapping access — prefer
  `ParamSet` for UI bulk-update systems.
- Only one query from a `ParamSet` may be active (borrowed) at a time; wrap each in `{ ... }`.

## B0002 — duplicate resource access

### Root cause in this project

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
   - Same resource as both a direct param and inside a composite (**B0002**).
   - `ResMut<T>` anywhere + `Res<T>`/`ResMut<T>` anywhere else in the same system.
   - Two `Query`s over the same component where at least one is `&mut` (**B0001**).
3. Fix:
   - **B0002:** read through the composite param, or remove the duplicate.
   - **B0001:** merge queries into `ParamSet`, or add reciprocal `Without<...>` filters.
4. Verify by actually running the game far enough to execute the system — conflicts
   only fire at runtime; a clean `cargo build` is NOT sufficient.

## Verifying

`cargo build` passing does NOT prove there is no conflict. Run and exercise the system.
Scan the latest run log:

```bash
bash .cursor/skills/bevy-system-param-conflicts/scripts/check_conflicts.sh /path/to/run_output.log
```

If the user pasted a panic, fix the named system directly: open the system, find
the param named in the error, and resolve the duplicate/overlapping access.

## Examples

**B0002 conflict:** `GameParam` already has `EraManager`.
```rust
fn my_system(game: GameParam, era: Res<EraManager>) { /* B0002 */ }
```
**Fixed:**
```rust
fn my_system(game: GameParam) { let era = &game.era; }
```

**B0002 conflict:** two mutable player queries.
```rust
fn s(game: GameParam, mut q: Query<&mut Transform, With<Player>>) { /* may conflict */ }
```
**Fixed:** read through `game`, or make disjoint, or split the system.
