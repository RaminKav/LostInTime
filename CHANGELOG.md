# Changelog

## 0.2.0

### Features

- new player animations: Roll, bow attack, sprint, lunge, sprint attack
- new sprint skill: overrides roll just like teleport, has skill upgrades that let you do a lunge attack while sprinting, reset the cooldown when getting a kill, and sprint faster.
- Day/Night Clock HUD widget
- over 25 new skills
- Skill Re-rolls: each skill choice slot can be re-rolled once per level-up!
- New enemy attack warnings (! above their head)
- Minimap!
- New UI improvments for item and currency pickup
- New Item: Beacons. Place them down to always find your way back to them
- Lots of new Audio/SFX

### Changes

- shrines no longer spawn near/in the starting clearing
- full internal player animation refactor
- Fire staff is now Ice Staff
- Mana and Mana Regen are new attributes that show up on some gear
- Night is slightly darker, and day/night cycle is slightly shorter
- non-weapons or empty hotbar slots always do 1 damage
- icestaff costs less mana
- default player sword/swing animations are white instead of blue
- Lethal Blow skill nerfed to 20%
- Magic Tusk now teleports you to the portal, and the bed item is removed.
- Increase spawn rate of pebbles and all stoen boulder types

### Bug Fixes

- fixed spawns being bricked due to not enough stone
- fixed bow arrow spread beign off-centred
- fixed enemies getting stuck in water
- fixed being able to teleport into the water
- coal and metal boulders make correct sound now
- fix various dungeon bugs
- No more lag spikes when game saves. As a result, game saves every 10s now.

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

## 0.13.0

- bug fixes
- Fix Mushking leap attack (he now actually leaps..)
- Mana rework:
  - The goal is to make the mana resoruce more exciting and rewarding to build into regardless of class. There will be more uses for mana, and thus more reason to pick mana heirlooms, etc.
  - Various Heirlooms that provide combat effects or spawn damaging entities now cost mana to trigger. (ants, stone boulders, wave attack, ice explosions, extra attacks, etc)
  - Mana orbs no longer spawn by default
  - [NEW HEIRLOOM] Blue Mushroom: +25 Mana
  - [NEW HEIRLOOM] Mana Dust: +10% chance to drop mana orb that triggers mana regen
  - [NEW HEIRLOOM] Wizard Hat: Mana regeneration shoots a mana orb at an enemy, dealing dmg equal to the amoutn regenerated.
  - [NEW HEIRLOOM] Purple Card: triggering mana regen stores the amount regenerated. On next attack, deal bonus damage equal to stored mana regen.
  - Staffs cost less mana to cast to compensate for no more mana orbs on default.
- [NEX] Heirloom Chest: new kind of chest that drops from enemies, gives a random heirloom (similar to the equipment chests)
- Chaos & Mob scaling Rework
  - Goal: Make chaos mob scaling and difficulty more smooth and exciting. mobs shoudl spawn more often now, but have less HP, and scaling chaos greatly increases your score potential now, since it also increases Mob spawn rate.
  - Chaos now scales mob spawn count per spawn wave. The effect is increased during Night Time.
  - Chaos scales mob HP slightly less now.
  - Mobs also have a much lower base Health.

## 14.4

- Speed up the startup time for the Teleport skill, feels a lot quicker to cast and smoother now
- Added color blind more in options, which changes the color of the boss attack indicators. Will continue to flesh out accessability settings as development goes on (if there is something that would help you play the game easier, let me know!)
- add blueberries, they recover some MP when eatten!
- Reworked the two "summon" heirlooms a tad, and i am considering them a new arch type, with more support to be added for these builds soon
- [NEW HEIRLOOM]: Summoning Wand (Rare), Healing has a 20% chance to trigger all summons once (ant farm, boulder)
- Reworked poison damage scaling to also now scale with any sources of "bonus damage" (all damage gain that is NOT the base weapon damage)
- Added the damage tracker to the inventory ui, as well as the game over screen! Tracks damage from all sources across weapons, pets, skills, heirlooms, etc!
- Fixed some skill cooldown bugs (there are still more... rip)
- [NEW SHRINE] Heirloom Swap Shrine: Allows you to select a rarity, and then select an heirloom to gain an extra copy of, at the cost of losing a random other heirloom of that rarity. This should help unlock certain builds, and make builds more consistant (costs gold, which increases each time you use one of these shrines!)
- Made some small performance improvements, but still more to be done here...
- [NEW ENEMY] Crow: for now, spawns in Era 2 exclusivly, instead of the Spike Slime. The Crow has a close/medium range projectile attack that shoots feathers.

## 0.14.5

- fixed chaos tracker to include chaos from endless mode
- rescaled endless mode difficulty
- nerf ripe tomato from 5 -> 25 kills per hp gain
- nerf uncommon, rare, legendary heirloom rates
- add dev mode in options, adds useful buttons in inventory to test builds
- tweaked endless mode scaling, should feel more smooth, with enemies hitting the 2.5-4.5k hp range after ~9min as a reference, around 5min is ~250 - 450 hp
- small minimap fix (heirloom swap shrine didnt show up)
- removed unlock for "extra skill slot"
- changed mage passive to +1 mana regen
- fixed health bug, now should show ALL positive health gains
- fixed stuck in generation bug (hopfully this time)
- added more cheats for dev mode (gain gold, gain key)
- Fixed a bug where lightning heirlooms did not consume any mana... should feel more balanced maybe

## 0.15.0

General Changes:

- New Desert Biome in era2!!!!
- Nerfed life steal, crit rate, crit dmg, mana regen stats on equipment
- Nerfed dungeon key drop rate by about 50%
- Nerfed the bonus stats gain for rarities by ~50% (higher rarity gear now multiplies the base bonus stats less)
- Buff Dungeons, remove middle pillars, they now spawn a stone golem boss as well
- Nerf player invulnerability frames when being hit from 1.0s -> 0.5s
- Max level for gear set to lvl 30
- Can press F key during chest opening ui to progress the animation/accept the loot (same as pressing the button)
- Movement skills let you pass through enemies better
- Removed enemy hit particles in endless mode (to reduce lag and visual clutter)
- New settings option for toggling enemy damage numbers
- Entering the portal in the third Era during endless mode will now kill the run (allows you to end a run when you are ready, if you are too strong lol).
- Bridges can only be broken by Axes now, making them MUCH easier to use
- [New Pet] Golden Pig: Grants pickup range, and Drops gold every so often (unlocked by beating Era2)
- [New Pet] Porkipine: Grants lifesteal, and Damages you periodically (triggers on hit effects) (unlocked by beating Era2)
- Rescaled the unlocks reward costs
- Poison rework: Can now crit, and every 3s cuts stacks in half, instead of removing them all
- 2 new quick consume buttons to consume items in 2nd and 3rd hotbar slots (Z, X default)
- Starting weapon rarity is capped at Rare instead of Legendary
- Extra crit chance over 200 goes towards crit damage
- Added an Heirloom trigger count tracker in the hover tooltip at the top of the game HUD!
- Rescaled endless mob difficulty, should feel smoother now
- Add some improvements to the game over screen, with new stats
- Pet abilities added to damage tracker
- Added cooldown text to the class skill hovers
- Buffed Fire pillar damage from 55% -> 95%
- At max mobs spawned, the furthest mobs will start despawning, so you cant kite them forever.

Bug fixes

- Shops are now cached properly and shouldnt reset their inventory when despawned
- Fixed cooldown tracker UI being messed up, shoudl be in sync now
- Removed green health gain text when mobs spawned
- Fix inventory stat highlights not showing up
- Remove green squares behind shrine stars on minimap
- Fix various crashes
- small ui fixes
- dungeons no longer give over-leveled gear as rewards
- Red/Blue Telephone Heirlooms properly REDUCE regen timers rather than increase them (lol)

Heirlooms:

- BUFF [Hero Sword] proc chance from 25% -> 33%
- BUFF [Rose] 2 -> 4 Lifesteal
- BUFF [Dark Blade] 5 -> 10 Lifesteal -5 -> -10 Health Regen
- BUFF [Ancient Tome] Multiple copies now works properly
- [New Heirloom] Rare Arcane Tome: 6% chance on skill use to gain +1 skill power
- [New Heirloom] Rare Summon Ring: Summons a bouncing ring, new summon synergy piece
- NERF [Summoning Wand] Moved from Rare -> Legendary
- BUFF Ant and Boulder heirlooms, boulder moves outwards now as it rotates
- NERF [Mana Orbs] drop much less often on hit from elites and bosses
- NERF [Lost Dagger] Reworked to cap at 500 stacks, granting +100% crit dmg at max stacks
- NERF [Cursed Crown] Only triggers 20% of the time on lifesteal trigger
- BUFF [Lightning Ring] 5% -> 20% trigger chance on kill
- BUFF [Sturdy Crate] 1% -> 1.5% damage per crate
- NERF [Ice Wand] changed from On hit effect to on kill effect, and it no longer triggers Frozen Tear
- NERF [Chalice] and [Bob's Bell] to have an internal trigger timer of 0.1s
- BUFF [Spiked Club] Now grants 15 thorns as well
- BUFF [Spiked Ring] Now grants 15 thorns as well
- BUFF [Dragon Eye] From Legendary -> Rare rarity
- REMOVED [Deadly Mushroom] due to balance reasons

- fix lunge not moving player
- fix timer crash maybe
- fix shop prices not scaling,
- buff thorns heirlooms (they give thorns now),
- fix inventory crash (maybe)
- despawn enemies after some time,
- fix tracking on dragon eye -> buff to rare and fix tracking,
- fix dmg number bug

## 0.16.0

- Snow biome for Era3
- Add unique coloring in the minimap for each biome
