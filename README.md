# A Top Down 2D Survival Rogue-like Game

A survival game for the exploration of Bevy/Rust and game development.

Feature-rich rogueliek survival gameplay:

[x] Diablo-style loot with randomized attribute rolls
[x] Stat points upon level, tons of attributes to increase
[x] HP, Mana, Food resources
[x] Full procedural world generation, with tons of foliage, plants, etc
[x] Random dungeon generation (Dimention support, like minecraft)
[x] Chunking optimizations
[x] Day/Night cycle
[x] 3 enemies and 1 boss with animations and different attack styles/flavor
[x] Tons of weapon types: melee, ranged, and magic with diferent play styles
[x] Weapon upgrades (bows shoot in a spread, chain lightning upgrade, etc)
[x] Fully custom UI implementation: Inventory, Containers(chests), Stats page, tooltips, HUD, etc
[x] Damage numbers!
[x] Tons of different items for crafting, building, and utility (like potions, food)
[x] Beautiful art pack and UI (still a WIP)
[x] Internal tooling for easy addition of new enemies, items, world generation items/params, etc all with the power of `bevy_proto` and custom tooling to make further development fast and easy!
[x] Simple enemy AI, expanding to mroe complex behaviors soon
[] Saving/Loading gameplay -> blocked upstream for `bevy_ecs_tilemap`, a WIP
[] Gameplay integration for mentioned weapon upgrades
[] More bosses/enemies! -> (huge art bottleneck)
[] Improved world generation algorithems
[] Add configuration options for device specific settings (for example screen resolution)

run using `cargo run --release`, if not using a retina Display, game might render too large, change `HEIGHT` constant in `main.rs`

### Controls

- `WASD` to move
- `I` opens inventory
- `B` opens stats page
- Mouse buttons to attack/use item
- `SPACE` to dash
