# Changelog

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

### Bugfixes

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
- Nerfed fire staff (decreased explosion damage and hitbox size)
- Wave attack, fire attack, and teleport shock damage now scales with attack (varying amounts)
- Manual Save button moved to U instead of ESC so it doesnt lag when you close inventories
- Nerfed drop rates of equipment and some weapons, small potions, and tomes/orbs
- Buffed arrow/throwing star drop rate

### Bugfixes

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

### Bugfixes

- Menu buttons are now disabled when the info modal is open
- Loot level now matches player level
- Fixed the game not being able to run on MacOS unless you had OpenSSL installed via Homebrew
- Fixed a crash involving doors
