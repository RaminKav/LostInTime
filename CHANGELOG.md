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

- shrine generation rebuilt around per-era min/max counts that are pre-rolled at world init and distributed across non-center chunks, replacing the per-chunk probability rolls
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

## 0.17.0

- Remove [Jam] Heirloom (hunger no longer a thing)
- Nerf [Vampiric Ring] heirloom (25% -> 15% heal on crit)
- Add volume buttons in options menu
- Add auto attack keybind in options (default T) (auto casts weapon attack in direction of cursor)
- Nerf Chaos increase from Era 2 and 3 (they should feel easier)
- Nerf boss and stone golem hp
- New desert enemies! Small cactus, Big cactus and Bull
- Performance improvements (hopfully, but after some testing i might have made it worse lol, lmk)
- New inventory and crafting system, new tooltips, new art, all new
- New hotbar system and keybinds
- new food items/recipes (just a quick palceholder, will add better art and tons more food items soon)
- Pet auto selects in class selection

## 0.17.2

- Fix bridges breaking on attack
- Fix getting stuck in dungeon walls bug,
- Drops dont follow the player anymore when inventory is full
- Added Sort button in inventory
- Fix boss despawn bug
- Auto equip gear when left in upgrade slot,
- Audio settings persist Now
- Fix score getting stuck/not updating
- Fix crash when killing mushking boss
- Fix HUD slots/skills UI being too low
- New desert Crates
- New Chest sprites
- Tweak spawn rates of shrines
- Increase pickup range a bit
- Reduce max gear lvl from 30 -> 10
- Defeating the era boss now pulls all loot on the map to the player
- Remove movement penalty on weapon attacks (nerf bow slightly to compensate)
- Attempt once again to fix performance

## 0.17.4

- [Frozen Tear] Moved from Uncommon -> Rare
- [Ice Lantern] Moved from Rare -> Uncommon
- [Ripe Tomato] Capped at 250hp
- [Arcane Tome] Capped at 1000 skill power
- [Stanley] Nerfed 15% -> 8% cooldown reduction
- Add toggles for animations for Weapon attack, skills, and heirloom effects
- Add different starting base stats for classes (mainly hp, mp, crit)
- Add hp regen and dmg multi in stats view
- Bosses dont inflate very large anymore lol
- Stagger desert era mobs properly when first entering the era
- No longer grant heirloom level up rewards past level 50
- Buff attack scaling in endless mode
- Tweak some heirloom mana costs (overall things cost more)
- Allow ice explosion and echo heirlooms to multi cast if you have more than 1 copy
- Stone golem spawns in endless every 8min :)
- buff move speeds of cactus mobs
- Fix bug for auto attack and multi projectile attacks
- Fix bug with staffs granting +1 on cast instead of -1 mana
- Tweak drop rates for tomes, keys, small potions
- More trees in snow era

## 0.17.5

- nerf xp gain from mobs, and environment objects
- [Cursed Crown] nerfed from 20 -> 10% chance
- rework lunge, feels smoother now, with a slightly new animation
- [Possessed Blade] NEW skill for rogue
- [Arrow Volley] NEW skill for Hunter
- Boss always drops loot
- Boss scaling tiers, re-summons of bosses cause it to summon an uncommon, then rare, then legandary varient that scales hp, attack, and abilities
- Added a toggle button in inventory to turn off ALL drops from objects (grass, stone, trees, flowers, food, etc).
- Some classes get starting speed bonus
- Fix [Sturdy Crate] to apply from era2/3 crates
- make stone walls unbreakable in dungeon
- increase player reach distance to break objects or place bridges

## 0.17.6

- Rework mana orbs, now drops at a fixed 10% rate on kill and regens a fixed 10 MP, [Mana Dust] now increases mana orb regen by 5
- Nerf xp gain by about 20% again
- Buff endless: HP Scales slower, enemy dmg scales faster. Max mob count increased, spawn rate increased, mob speed increased
- [Vampiric Ring] Nerfed from Uncommon -> Rare
- Rebalanced a few skills damage values
- Buffed heal skill cooldown from 45s -> 20s
- Fix active skill shrine locking you out if you exit early
- BUG: Sheild does not regen while paused anymore

## 0.17.7

- Make shrine generation mroe consistant, with set min/max spawn counts per era
- Increase island size from 5 to 6, map will feel a bit bigger
- Remove collider from pet, wont blocking your way
- World objects like trees and boulders will scale their health with chaos in the same way mobs do. this is an experiment
- Increase heirlooms shown in each row of the player HUD
- Rework shop scaling to be based on level of the player
- Rebalance class and pet passives
- Add scaling mechanics to the last skill of Warrior, Rogue, and Hunter.
- Increase teleport distance by 12.5%
- Tweak summon times (mostly buffs) of the 3 summon heirlooms
- Buff [Reaper] damage by 100%
- Add Set bonuses to gear!
- Remove starting tomes/orbs for balance reasons
- Fix bug with starting hp/mp of some classes not being full

## 0.18.0

- [Feature] Added new meta progression system: Time Crystals. Will make sense when the lore is finalized. Collect Shards by doing runs, shards complete Time Crytals, which will unlock new heirlooms that you can find in your run. Added a new Time Crystals menu button to view unlock progress, AND to view all unlocked/locked heirlooms. This UI is a placeholder.
- [Art] Add new rogue player sprite/animations!
- [Art] Add new boulder asset, its chunky now.
- [QoL] Can now drag and rearrange skills directly in the player HUD
- [QoL] Fix teleport skill to place you on a nearby tile, if possible, if you teleport directly on top of an object
- Shift + Left Click now moves items to and from the hotbar, or equipment slots based on the type of item
- [Boulder] Update rotations and movement of boulder
- [Piercing Ring] Buff duration of rings
- [Wizard] Now scales with Mana instead of Mana Regen
- Buff dagger damage, nerf claw damage
- Add Duration scaling to Fire Pillar
- Reduce starting speed of endless mobs a bit
- Improve skill descriptions to show scaling dynamic values, based on the stat they scale with (description of the scaled value will update as you scale)
- Fix Attack speed bug with Rapid Fire skill
- Clean up some tips and pop up boxes that were outdated

## 0.18.1

- [Feature] New Special Food Items and crafting improvements: Foods that grant perm stat buffs, using materials from different eras (biomes) Added new food materials to Desert and Tundra to make this work.
- [Feature] Banished Heirlooms tracker
- [Feature] Cosmetic Grass and dirt patches added to Forest (Era1). Will add to rest soon!
- [Feature] Mob stats tracker underneat Damage Tracker. Tracks kills and damage taken by mob type
- Buff xp gain by ~10%
- Nerf [Rose] 4 -> 3% lifesteal
- Nerf [Dark Blade] 10 -> 7% Lifesteal
- Nerf [Lightning Ring] 20% -> 15% proc chance
- Nerf [Chalice] 20% -> 10% proc chance
- Free Heirloom shrines per era dropped from 7 -> 5
- [QoL] Add new stats to Stats tooltip (Defence mitigation %, regen timers)
- [QoL] Improve Shift + Click interactions in inventory
- [QoL] Only consumables can go in the hotbar
- [QoL] Auto-Attack is on by default, and is now a checkbox toggle
- [Bug] Fix animation for Rogue movement
- [Bug] Fix overlays when opening inventory during heirloom selection

## 0.18.2

- [Feature] Classes start with last 2 skills Locked by default. Unlock them with Time Fragments. Unlock Classes checkbox bypasses this in options
- [Feature] First Time User Tutorial with visuals! Added "Show Tutorial" button in Options, check it out
- [QoL] Auto-Attack on by default, changed to a toggle checkbox in options. Left mouse button mapped to a skill now by default
- [QoL] Improve font used for in-game text hovers (damage numbers, item pickups, shrine text).
- [Bug] Fixed crit detection for damage numbers, should be consistant now
- Wipe Game Data button in options, for those who want to try out the new meta progression stuff added recently.

## 0.19.0

- [Feature] Beastiary & Monster Cards. Collect rare monster cards, and unlock more information and stats about that monster in the Bestiary. UI is a work in progress (placeholder).
- [Feature] Added Scorpion Boss to Desert Era! Spawns instead of the mushking now in era2
- [Balance] Nerf chaos gain per level from 0.2 -> 0.1
- [New Skill] Shadow Step: Rogue skill, dash to your position 0.75s ago, damaging all enemies in your path, gain stealth after for 1s.
- [New Heirloom] Blue Card: Uncommon, +3% chance on weapon or skill damage to regen 1 Mana.
- [New Heirloom] Purple Card: Rare, Rework of old purple card, doubles drop rate of mana orbs. (default is 10% drop).
- [Balance] Reduce classes to 3 skills total, 2 unlocked by default, 3rd one purchased with Time Fragments.
- [Balance] Boulder balance: Increase time between spawns, but increase duration of the boulder.
- [Balance] Reduce drop rate of Time Fragments
- [Bug] Fix text on some monitors, text should be more clear, please send screenshots if any text looks off on your monitor (non-uniform pixel sizes, etc)
- Removed Gun weapon from game pool
- [QoL] Added more helpful text in the floating text tooltips (boss summon, dungone), and portal, mostly for new players.
- [QoL] Added coins counter in the shop UI

## 0.19.1

- [Feature] New main HUD UI! This is the first iteration of this, will likely change and adapt in the coming days, but wanted to get it out there asap! Let me know your thoughts on it so far.
- [New Heirloom] Gravity Scales: Rare, converts 25% of pickup range into size
- [Balance] Nerf dmg of some desert enemies
- [Balance] Buff desert Scorpion boss attack pattern and hp
- [Balance] Buff the knockback from shout by like 150%
- [Balance] Brown Card (skills trigger mana regen) moved to Common rarity, and reduced from 20% -> 8% chance
- Remove CDZ heirloom (and shield as a result)
- [QoL] Add tracker for movement skill cooldowns on top of player
- [QoL] Remove mana cost from staffs
- [QoL] More option in settings, switch to smaller dmg numbers, disable player numbers (hp/mp), and change interact button
- [QoL] Remove some of the lame skills that are not part of any class base kit, from the shrine pool
- [Bug] Fix the sudden night overlay change when entering a new Era
- [Bug] Fix crash related to poison, again

## 0.19.2

- Improved Game HUD layout and art
- Add hover tooltip system, first versions added to the 3 buttons on the left of inventory
- Nerf Claw dmg and attack speed

## 0.19.3

- fix desert scorpion crash
- negative hp regen can no longer kill you
- add global text pop ups at start of run and after boss kill to guide player objectives
- fix fury scaling, now scales purly on bonus attack speed stat, not weapon base attack speed

## 0.19.4

- [Feature] Minimap! Let me know what you think!
- [Feature] Improved Item Filters UI, can now pick individual items to filter from a menu
- [Feature] Tornados now spawn in the desert biome (Era 2), which knock the player up briefly.
- [New Heirloom] Underworld's Hat: Legendary, shoots a homing fireball every 150 dmg you deal. Part of the base pool for now
- [QoL] Upgrade System Reworked: drag and drop tomes and orbs on the equipment to apply them now, removed the ui slots for upgrading
- [QoL] Ensure some items dont make it into the hotbar on pickup
- [QoL] Tweak and improve the HUD ui a bit
- [QoL] Click to Consume items in your hotbar in the HUD! Alternative to using the keybinds.
- [QoL] Improve text spacing on descriptions and recipes
- [QoL] Speed up bow animation, improves feel
- [Balance] Buff Piercing Star Skill: Goes further, and boomerangs back to you now! Feels a lot more fun, try it out
- [Balance] Claw spacing of 2 stars per attack tightened, feels more like a "double throw" now
- [Balance] Pet weapon slot now grants the bonus stats to you
- [Bug] Thorns properly triggers off negative hp regen and pet damage (Porkipine)
- [Bug] Desert Golem triggers magnet effect on death properly
- [Bug] Fix issue where Heirloom Swap Shrine did not show Star on maps
- [Bug] Fix a bug where chests would get deleted on pickup if you had more than 1 underneath you at once.
- [Bug] Fixed an issue where some heirlooms did not get turned off from the toggle in options

## 0.19.5

- [Balance] Buff Metal armor to grant way more hp/armor
- [Bug] Increasing rarity of gear with Orbs properly grants the Base stat bonus of the new rarity.
- [QoL] Armor and accessories are weighted to show up more often in chests now
- [QoL] Clean up colors and spacing of gear tooltips
- [Bug] Added missing Blueberry to filter list
- [Bug] Enemy projectiles no longer speed up with player projectile speed heirloom.
- [Bug] Item drop filter now works on crates

## 0.20.0

- [Feature] New Thief skin is here! Check it out. Last 2 class skins coming very soon!!
- [UI] New chest sprites, and polished Chest UI! Equip gear directly in the chest, banish heirlooms directly from chest, also view currently equiped gear to compare!
- [Feature] New Skill: Meteor Shower! Replaces Ice Wall for Wizard. Rain down meteors around you, meteor count increases with every cast!
- [Feature] New Enemy: Lizard! Found in Desert Era as the first enemy (replaces Fur Devils here). This concludes the Desert enemies!
- [Feature] New Enemy: Void Crawler! Special enemy that spawns during endless. Endless mode now only spawns Void enemies, Void Crawler is the first to be added.
- [Balance] Buff base Mana regen timer from 6s -> 2s
- [Balance] Buff base defence/hp stats on armor
- [Balance] Buff defence formula: defence now mitigates more % of dmg
- [Balance] Buff Lightning Ring mana cost 7 -> 5
- [Balance] Buff Summoning Wand from LEgendary -> Rare, 20% proc -> 10% proc
- [Balance] Warrior now scales +2 Size per level, and Gravitational Spear scales with size.
- [Balance] Buff drop rates of cards a tiny bit
- [Balance] Reduce shop prices/scaling.
- [QoL/Bug] Fix text rendering on windows/larger monitors. Text should look clear now without being warped or "chizzled", etc
- [QoL] Fixed Bow arrow firing delay. Bow should feel a lot more responsive to use now.
- [QoL] Add Heirloom picker in dev tools: add any heirloom instantly, to test builds/theory craft
- [QoL] Added setting to adjust game and UI render scale separatly! Zoom in our out the game view, and make ui smaller, or standard
- [QoL] Pet skill now shows "PET" label for clarity/completeness
- [QoL] Chunkers boulder now orbits further away as it scales with size
- [QoL] Reskin Spike slime enemy to purple to stand out more
- [Bug] Reposition cursor every frame, so if you dont move mouse and walk past your cursor, the targetting doesnt drift
- [Bug] Assets were missing on Windows for Era3 (snow biome), should now see the leafless trees as intended!
- [Bug] Piercing Star thief skill now properly tracks in damage tracker
- [Bug] Some wep/animations were not hiding when toggling hid attacks/animations, fixed
- [Bug] Fix Magic Whip collider being too small

## 0.21.0

- [Feature] Big overhaul to heirloom car text, as well as info boxes next to the tooltip explaining what key terms mean
- [Feature] NEW night time effect! No longer dims the entire game, now slowly adds a gradient of darkness around the player. Much nicer than before!
- [Feature] HUGE merchant rework. merchant shrine now offers 3 heirlooms, 2 equipments, and 2 materials. you can use a reroll on each of these 3 categories to refresh them.
- [Feature] 8 New large cactus varients for teh desert, as well as some dark patches underneath objects to add contrast (same idea as in the forest). The desert will continue to improve to bring it to par with the forest biome.
- [UI] New overlay system for UI (dark, soft gradient)
- [UI] Add dark background on the big text pop ups in-game
- [QoL] HUGE buff to bridge placement: Now, pressing the hotbar button puts you into bridge mode, and you can click and drag to place lots of bridges at once. press input again, or dont click for ~2s to exit bridge mode.
- [QoL] Player icon for the map, as well as a icon legend
- [QoL] Reworked starting tutorial to show in segments, as the player encounters the relevant tips, instead of displaying everything at once
- [QoL] Add big text that shows the current day, and the biome when you enter a new era
- [QoL] Heirloom hovers added to the swap shrine
- [QoL] Magnet shrine cost changed from 1 time fragment to 10 coins
- [QoL] Enable all pets by default for the purposes of playtesting
- [Balance] EXPERIMENT: All classes now only have 1 class movement skill (unique to the class, cant find in the skill shrine), the 2nd and 3rd slots are EMPTY at the start, you can fill them up by visiting the skill shrine. (there are other ways to gain skills planned). Let me know how this one feels, the idea is to let the player discover and have more reason to try different skill combos, rather than giving the most broken combos right out of the gate.
- [Balance] Nerf base mana regen from 2s -> 3.5s
- [Balance] Spike slime shows up 1 day earlier in Era1
- [Balance] Arrows only pierce 3 objects now, so you dont destroy the entire map (still pierce infinite enemies)
- [Balance] Slow down fur devils and bushlings a tiny bit, to make era1 a bit easier to explore
- [Balance] Buff Blue Card from 3% -> 4% proc chance (gain +1 mana on hit)
- [Bug] Fix heirloom tooltips in inventory, and end game screen
- [Bug] If taking a wep from the chest, it will put your old one into the pet slot if its empty
- [Bug] Fix a bug where dungeon would reset the Blake Boulder spawn timer
- [Bug] Bug fixes and improvements to poison: Going over 100% poison chance now lets you do multipel stacks of poison on each hit
- [Bug] Drop filters now reset properly after a run ends

## 0.21.1

- [Feature] Added keybind to toggle Weapon Auto Attack! Attacks can now auto aim to nearest enemy
- [Feature] Health and Mana trackers, hover over the health or mana orbs in the HUD to view useful information about health gain sources, health regen/s, or mana draining sources, and mana regen/s. It resets every 60s.
- [Balance] Buff class passive stat amounts significantly. For example, warrior gives +5 size isntead of +2. Should make the class passives feel very significant, and give them more identity, now that the base skills were removed.
- [Balance] Fur devils and lizards spawn rate increased from 2.5 -> 2.0s per spawn, this is hand-in-hand with the change last patch that made some enemies slower.
- [QoL] Add info boxes explaining Size, Luck, Chaos, and Attack stats
- [QoL] Add % Poison chance stat to the stat viewer in inventory
- [QoL] Decreased density of trees and big cactuses in all 3 biomes. Forest (Era 1) especially should feel more open
- [Art] tweaked visuals for desert dirt patches
