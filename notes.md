# Inventory TODO

<!-- - Add hotbar -->
<!-- - Bug: can lose an item if you pick up 2 very fast? -->

- Update assets
  <!-- - hotbar fades/greys when inventory is open, or add a faint overlay -->
  <!-- - Allow dragging out of inv to drop (use timer comp) -->
  - Bug: updating while draging item stack doesnt update text.
- Add double click to colelct all items of same type to a stack

# Components for UI

- `InventorySlotWidget`
  - Has state for what item metadata it holds, or a ref to its slot number that looks up a map for the data
  - On hover events, displays `TooltipWidget` with metadata, and makes the slot change color
  - Image can be different for Hotbar slots
- `InventoryWidget`
  - a grid of `InventorySlotWidgets`
  - opens on a keypress
- `ProgressBarWidget`
  - holds state for percentage, renders inner rectangle's size as percentage of full size
- `TooltipWidget`
  - rectangle (transparent?) with text and image (?) of metadata of item

# Implementation Notes for drag/hover system:

## Defs

- `Interactable:` Component, {state: Interaction, previous state} defines entity that can be interacted with this system
- `Interaction:` Enum, defines states None, hover, Drag
- `DropEvent:` Event, {held en, drop en, pos}

## System Fns

- a system that does a cursor raycast to the window and checks for any entity hits with `Interactable`. When raycast returns a hit, set its state to hover. If there was a click mouse event that frame, then set it to dragging, send click event.
- a system that clears all drags (resets `Interaction::None`), runs before above.
- Drag system that checks for any `Interactable`s that have state == dragging, sets their transform to the cursor transform
- system to track dragging changes, if dragging stopped, send `DropEvent`
- System that listens for `DropEvents` and swaps the dropped entities positions in the grid

# UI System

- Spawn `SpriteSheetBundles` with custom Component `Interactable` `UIElement`, and `InventorySlot`, etc.

  - Inventory spawned with slot entities as childern. Slots get position based on their slot index, which is iter from inventory.
  -

- `Interactable` entities are subject to Drag Plugin, which handles dragging/hover states,
- Input Plugin listens for mouse events while inventory is open.

  - Use aabb collider system to check if mouse is inside each `UI` entity.
  - Send `MouseIn`, `MouseOut` events when cursor enters/exits. These trigger `Hover`
  - Send `MouseDown`, `MouseUp` events which trigger click/drag states
  - Events send the entitiy that triggered them as part of data

  # Inventory System V2

  ## goals

  - Clicking on item picks it up and locks it to cursor. Left = whole stack, Right = half
  - Clicking again while dragging attempts to drop the stack in a slot
  - If the dropped slot has a stack, it gets added to the cursor next.

  ## technical notes

  - when interacting with UI, actions physically move inventory item stacks to your cursor.
  - allows for precise manipulation of the inventory state and ui.
  - less room for errors in syncing between "fake" pick up of stacks
  - Clicking 1st time removes IIS to cursor (attach a drag comp?)
  - Clicking 2nd time checks if any IIS have the drag component, and if they do, performs a drop
  - Dropping action looks at the hittest slot and checks if its empty. If its not, sets the current slots ISS to drag comp
  - While dragging, right clicking drops one item at a time (create new entity with count 1, and call drop.
    Fail if the target slot is not empty or the same type).
  - Right clicks work the same, but only half the slot is added to the cursor (create new entity with half the count)
  - If same slot, merge. If dropped outside of inv, spawn an ItemStack

  ## Tooltips

  - Items in `ItemStacks` can have `ItemMetaData` Component, which will holds things like:
    - Desc: Item lore/description
    - Vec of Attributes (need to_string impl): Stats like att, dur, etc
  - Tooltips will display the data
  - Items by default will have a default impl of `ItemMetaData`, or a randomly generated one by a drop system
  - Different gameplay systems can modify `Attributes` which will update `ItemMetaData` in a `Changed<>` query system thing

  ## Next Features:

  //TODO: add ron file for stats/metadata for WorldObjects
  [x] Impl `LootTable` for drops list on enemies and breakable items

  [x] Impl spawners that auto spawn mobs

  - UI:
    [x] Crafting UI
    - Equipment UI
    - Food Bar
  - World Generation:
    [x] Stone
    - Random Grass Foliage
    - Biomes ?
    - Spawn random structures from custom schema files ?
      [x] dungeons
  - Art/Items
    [x] Make borders of tops of blocks like stone merge properly/dynamically
    - Make new block types: New Stone, Wood blocks, fences, chests/loot box
    - Mob drops
    - Weapons, Tools, Armor
    - Food
  - Art/Mobs
    - More Enemies
    - Passive Mobs/animals
  - Misc.
    [x] Impl passive/neutral enemies AI
    [x] Fix crafting table bug
    - fix weapon Z fighting bug
    - Fix UI text bug regression
    - Bundle components for useful types
    - Animate drops better
    - fix neutral mob fighting bug

proto todo:

- make impl for mesh2dmaterial that takes input of asset, size, move txfm to .insert()
- add item attribute systemparam?

how to set up item prefabs:

- Equipment that enter the world as an Entity from a drop:
  - use prefab custom impl that takes attribute min/max, calculates an rng roll per stat
- spawn_item_drop already takes optional attributes override. use this for existing items that get dropped from inv
- creating an item directly into the inventory will currently not work with this design. if its not an entity, cant use prefab to get attributes

BUG: moving very far causes dash cooldown to double in speed? weird behaviour
BUG: move dash tick to animations.rs, also dash movement is mega glitched rn

TODO: `AttackState` remove attack related data to components, for proto? basically how do we
TODO: remove fireball from world object/sprite sheet, make own file, with animation
impl different att styles w seldum_state

## Misc. Design Ideas

- item identification
- item re-rolling
- mobs drop keys to bosses
  - progression dependant on level/area?
  - biomes spawn based on distance?
- small towns spawn
  - chance to have npcs that do actions for you, or sell gear?
    - item identifier
    - blacksmith - sells gear
    - dark auction house style dude
- rework inventory uis to use states
  - different states to easily change what UIs ar shown for npc trades, etc
- ui slot for off-hands (throwable or any right-clickable item)
- rework tooltips to be a large display on the side?
- possible UI refactor is include InventorySlotState in InventoryItemStack

## Features

[x] add healthbar to enemies
[x] add damage numbers
[x] Schematics: try out random structure spawns,
[x] `Blocked below` Dungeons: add item that tp to dungeon instance, spawned from schematic probably
[x] Items: add right-click ability items (potion, single use items)
[x] Items: add right-click interact objects (chests, alter, etc)
[x] Equipment: UI to equip, visuals pipeline, add restrictions for slots
[x] UI: Containers, allow different UI inventory states to allow for chests, npcs, icons for acc/equip slots, maybe change ui slot colors to be unique
[] Items: add random roll stats, clean up naming of stats too
[] Items: add lots of items/recipies
[] Items: shrink walls, simplify top borders so its easier to add new wall types
[] Add colliders to water (use corners)
[] Generation: Add random ore clusters in dungeons?
[] Mobs: Add more mob types, maybe a boss, goblins, passive mobs
[x] Survival: Food system
[] UI: add static tooltips on the side of inv
[] Aesthetics: Add random animals that don't do anything,
[] Aesthetics: add randomized full-block types
[] Dungeons: add exit method, add chest generation, etc `Blocked` by below
[] save/load/cache

- throwable items (ninja star,etc) (spawns proj, deducts 1 item count)
- new types of projectile

## Projectile/Magic ideas

- orb that spins around you as you walk, attack fires the orb, then respawns or comes back

## Foliage/object rework notes:

- convert tile object hash to a vec of 4 objects, 1 for each quadrant
  - objects like trees or walls take up all 4, grass/flowers/chests take up 1
- Add world generation for grass/flower clusters

## Gameloop ideas:

- Player starts weak in a tough environment. Hostile and Neutral Mobs need to be avoided for now.
- Early game revolves around getting grass/trees/rocks for early materials (sticks, plant fibre -> string, stone shards)
  - `Goal:` build first tools/weapons
- Kill animals for food/wool/leather `Goal:` get beginner armor, food
- From here options are open: Explore world, Try to kill mobs, Dungeon farming/mining
  - `Goal:` get progressivly stronger gear
- Find boss summon keys, various methods (mob drop, crafting, dungeon loot)
  - `Goal:` kill all bosses
- World changes in difficulty after each boss is defeataed. New Mobs/harder mobs, new dungeons/harder dungeons
