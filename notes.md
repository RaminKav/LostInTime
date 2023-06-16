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

- make weapons proto?
- make impl for mesh2dmaterial that takes input of asset, size, move txfm to .insert()

- weapon/tool protos need:
  \*\*\*\* remove sprite_desc entries for weps, this should replace all of those
  -basic misc stuff (collider, ysort, markers, etc)

  - WorldObject
  - attributes(attack, durability, other stats)
  - sprite image
  - Equipman(Limb)
  - Melee or Ranged attack types (maybe ranged needs a projectile?)
    - projectile needs: sprite, speed, lerp_Type (linear speed, ease in, etc),
  - ToolToughness (replaces breaks_with)

- Item Attributes:
-
