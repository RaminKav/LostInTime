// stop clicks through UI

```fn set_active_tile_target(
    mut tile: Query<&mut ActiveTile>,
    cursor: Res<Input<MouseButton>>,
    event_context: Query<&EventDispatcher, With<GameUI>>,
    camera_transform: Query<&GlobalTransform, With<WorldCamera>>,
    windows: Res<Windows>,
) {
if !cursor.just_pressed(MouseButton::Left) {
// Only run this system when the mouse button is clicked
return;
}

    if event_context.single().contains_cursor() {
        // This is the important bit:
        // If the cursor is over a part of the UI, then we should not allow clicks to pass through to the world
        return;
    }
```

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
