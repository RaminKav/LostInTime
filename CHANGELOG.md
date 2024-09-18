# Changelog

## 0.2.0

### Features

- new player animations: Roll, bow attack, sprint, lunge, sprint attack
- new sprint skill: overrides roll just like teleport, has skill upgrades that let you do a lunge attack while sprinting, reset the cooldown when getting a kill, and sprint faster.
- Day/Night Clock HUD widget
- over 25 new skills
- Skill Re-rolls: each skill choice slot can be re-rolled once per level-up!

### Changes

- shrines no longer spawn near/in the starting clearing
- full internal player animation refactor
- Fire staff is now Ice Staff
- Mana and Mana Regen are new attributes that show up on some gear
- Night is slightly darker, and day/night cycle is slightly shorter
- non-weapons or empty hotbar slots always do 1 damage

### Bug Fixes

- fixed spawns being bricked due to not enough stone
- fixed bow arrow spread beign off-centred
- fixed enemies getting stuck in water
- fixed being able to teleport into the water
- coal and metal boulders make correct sound now

## 0.1.4

### Features

- Shift+hotbar number auto-consumes items without swapping the hotbar slot
- Consuming a consumable item in teh hotbar slot will attempt to replenish the slot with a matching consumable type if the slot becomes empty.

### Changes

### Bug fixes

- Fix macos going crazy with the hotbar trackpad sensitivity

## 0.1.3

### Changes

- Assets are now embedded in the binary
- Crates drop slightly better loot
- EXP curve adjusted to be quadratic
- nerf: lightnign staff chain lightning upgrade does half of original dmg
- nerf: Lifesteal is not longer a repeatable skill

### Features

- Allow placing some objects on water blocks
- Pause during any Inventory/menus!

### Bug fixes

- fix skill points carrying over to the next run
- fix end of run crash
- fix water collider not returning after breaking bridges
- skills now give weapons that scale with player level
- actually fix shrine depth...
- fix spacing on guide input hovers

## 0.1.2

### Features

- Added a changelog
- Added wayland support
- Added log files
- Added an error popup when the game crashes
- Added Anvil, Upgrade Station, and Alchemy Table recipes to the crafting table (forgot to enable them, whoops)

### Changes

- Increased pickup distance for item drops
- Nerfed Ice Staff (decreased explosion damage and hitbox size)
- Wave attack, fire attack, and teleport shock damage now scales with attack (varying amounts)
- Manual Save button moved to U instead of ESC so it doesnt lag when you close inventories
- Nerfed drop rates of equipment and some weapons, small potions, and tomes/orbs
- Buffed arrow/throwing star drop rate

### Bug fixes

- Fixed a bug where player would teleport to the spawn portal with the teleport skill
- Fixed a crash pertaining to status effects
- Fixed a bug where you could not teleport without a item in your hand
- Fixed some z-fighting issues with the gamble shrine
- Fixed the crash on Windows that happened after you died

## 0.1.1

### Features

- Decreased Forest Density

### Changes

- Increased visibility radius slightly
- Decreased autosave frequency
- Game will now automatically save on pause

### Bug fixes

- Menu buttons are now disabled when the info modal is open
- Loot level now matches player level
- Fixed the game not being able to run on MacOS unless you had OpenSSL installed via Homebrew
- Fixed a crash involving doors
