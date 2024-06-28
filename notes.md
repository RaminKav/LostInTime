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
[x] Items: add random roll stats, clean up naming of stats too
[x] Items: add lots of items/recipies
[x] Items: shrink walls, simplify top borders so its easier to add new wall types
[x] Add colliders to water (use corners)
[x] Mobs: Add more mob types, maybe a boss, goblins, passive mobs
[x] Survival: Food system
[x] UI: add static tooltips on the side of inv
[x] Gameplay: mana system
[] Aesthetics: Make grass foliage shader
[] Aesthetics: Add random animals that don't do anything,
[] Aesthetics: add randomized full-block types
[x] Aesthetics: Particle system for easy non-damaging animations/particles
[x] Dungeons: add exit method, add chest generation
[] ## Items: Trinkets
[] ## Gameplay: weapon upgrades
[x] # Gameplay: weapon scrolls
[x] # Gameplay: weapon orbs of alteration
[x] FIX: fix dash
[x] Passive mobs that give leather
[x] add leather armor
[x] # mobs dont spawn around player
[x] # add elite mobs
[x] ### add resource UI (furnace, item upgrade station)
[x] ### FIX: lag in dungeons use bevy_spacial?
[] ## save/load on exit
[x] cleanup proto obj files, add parent
[] # bushlings have chance to spawn from bush
[x] ## add cursor tile highlight/world tooltip
[] enemy attack hit indicator/colliders
[] multi attack swings player animations, bow anim, etc
[x] bridges to cross water

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
  TODO: Maybe there is a way to tick all timers in one system rather than each handle their own timers?
  TODO: upgrade collider animation proto to use more detail: each frame can take a [shape, size, rotation, position]

## Mob Ideas

- Neutral Slime: basic early game slime mob, basic charge attack/follow
- Goblin archers: basic early game aggro, keep at distance and fire arrows
- Goblin Fighters(?): aggro, spawn in hordes, sometimes with archers, are faster, fight by slashing a sword when in range
- Mushroom: basic early game aggro mob, hops towards player, attacks with poison smoke thing (make this into a type of action, use proto for specifications to size of attack and asset), leaves poison cloud when dead that damages?

## Attribute Ideas

- All weapons have Attack (maybe called DMG?): 10 Attack or 11-24 Attack
- All weapons have Hit Speed (maybe called DMG?): 1.5 Hits/s, etc
- All armor have Health: +10 Health (or HP)
- Unbreaking (durability)
- Crit chance: +3% Crit
- Crit DMG: +10% Crit DMG
- Bonus Damage: +10 DMG
- Health Regen: +10 HP Regen or HP/s
- Healing: +10% Healing
- Thorns: +10% Thorns
- Dodge: +10% Dodge
- Speed: +10% Speed
- Lifesteal: +1 Lifesteal
- Defence: +10 Defence
- Loot Rate: +10% Loot
- Venom
- Burn
- Mana
- Mana Leech

- Experience Rate: +10% XP

## Biomes + Mobs + Themes

- [x] Grass Plains biome (Slimes, Slugs, Boar, Pigs, Bees/Bugs), gather pebbles on ground
  - rare healing flower
- [x] Forest, (Slugs, Mushroom, Goblins, Bugs,), gather pebbles on ground
  - rare healing flower
- [xx] Desert, a little more dangerous, but has dead branch that gives sticks early on
  - rare burried treasure, digs up random item from loottable

## New Weapons

[] Spear - melee, hits a pierce animation in a straight line
[] Iron/Wood Sword - stronger, slightly slower
[] continuous beam magic weapon
[] venom bow
[] multi-throwing projectile
[] claw arch-type: Throws ninja stars, can be upgraded to throw multi stars (MS style)

## Weapon Upgrades

# lightning staff

[] longer lightning - new art, new proto stuff
[x] chain lightning - match to find direction angle
[] faster cast speed - ez

# fire staff

[] leave burn field DoT - new art, new type of hitbox with ticking timer?
[x] AoE - new on-proj-hit event, new art, spawn hitbox/proj anim
[] faster cast speed

# magic whip

[] faster cast speed
[] larger whip - new art, proto stuff
[x] whip hits proc venom? - same as AoE, no hitbox

# bow

[x] Spread shot - similar to claw multi shot
[x] Faster Arrows - easy
[] piercing arrows
[] arrow bounce off objects
[] Faster hit speed
[] Homing Arrows? - math stuff

# sword

[] faster hit speed
[] attacks send a sonic wave - easy, new projectile
[x] attacks proc burn DoT - same as AoE
[] attacks do leech? - easy, add more leech stat
[] attacks multi-hit

# dagger

[] faster hit rate
[x] hits proc venom - same as AoE
[x] change to lethal hit targets below 25% HP ? - Same as AoE
[] attacks multi-hit - similar to claw
[] more crit ?

# claw

[] faster attack rate
[x] multi-star - done
[] large piercing star (Avenger <3) - new art, same as big lightning

## Progression Notes

# Mats/Drops/Gear Aquisition

- Limited weapon types: 6-8 (2-3 meleee, 2 ranged, 3 magic)
  <!-- - Weapons drop at random from mobs, some have higher rates to drop specific ones -->
  <!-- - armor/trinkets/accessories drop at random as well from mobs -->
- some armor can be crafted from materials or mob drops
<!-- - weapons can be crafted from mat/drops too, -->
- Dungeons will have ores, overworld other mats like leather/wood
- Wooden sword will be everyones starter weapon, mats to craft found in overworld
  - maybe rebrand this to a `sturdy stick`
- out-of-run progression could be you get to pick a new starter weapon
- generic armor can be crafted fairly early on (leather, chain)
  - leather from animals, chain from metal fragments from boulders?
  <!-- - how can weapons be crafted?
  - swords/daggers/bows are easy, what about magic weapons? -->
  <!-- - magic runes or w/e drop from mobs rarely, each maps to one elemental magic weapon -->
- loot drops as unidentified weapons -> need to be identified in a block with money?

# What Will Be Crafting

- Bandages
- Utility Blocks: Chest, anvils, upgrading stone, Identification stone
- Tools,
- Building: Walls, doors, presure plate, bed, walking planks, boats,
- Utility blocks: WarpStone
- plant/farming/food related things
- towns can have quests for items -> get money
- perhapse armor can be crafted in unique block
- fishing?

# Upgrades

- weapons/armor can be upgraded with scrolls to boost base dmg
  - scrolls are rare drops from mobs/chests/merchants/bosses
- Orbs of alteration - can reconfigure bonus stats on anything
  - also rare drop
- Maybe both of these can be applied at an anvil-like item
  - 2 slots, first for scrolls/orbs, second for the item
  - same UI can be used for repairs maybe

# Level Progression

- Mobs give exp. Leveling increasingly harder as mob exp will generally not increase.
- After every boss, world gets harder and mob exp increases
- Encourages player to fight bosses eventually as grinding levels increasingly less rewarding
- Each Level grants an opportunity to increase stats
  - STR: dmg, armor, DEX: crit, crit dmg, AGI: speed, dodge, VIT: hp, hp/s
- Leveling makes player decently stronger. All stats are applicable to all weapons, but some might benefit from certain stats more

# Trinkets

- Give larger buffs, boss drops, dungeon loot, very rare mob drops (elites)
- things like: bonus leech, crit, dmg, DoT stuff, dash modifiers, etc

# World Difficulty Progression

- clearing a boss makes the world harder
- mobs have more hp, speed, dmg, maybe new mechanics
- maybe new mobs can be found
- higher chance for elites

# World Generation Progression

- start in plains/dessert/beach areas
- forrest is next, maybe swamp,
- snow/lava?
- dungeons change theme too based on the biome theyre from?

# Recipes and Drops

- Rocks: flint, rock shard, coal*, metal*?
  - Walls, arrows,
- Coal: Coal
  - smelt things, lighting later
- Metal: rock, metal
  - armor, throwing stars, etc

Melee weapons == starter weapons/unlimited resource weapons
bow/claw are op bc of range, but need resources (Arrow/stars)
magic needs mana??

mob drops = make potions? (mana/hp) -> this is the mats to get magic resources

Night cycle?? night time mobs become hostile or start to spawn more hostile ones?
some type of daily market event => most reliable way to get scrolls + orbs, rare item drop?

Interactable chests spawn mobs to kill which give loot
Dungeons are smaller, with an unbreakable wall barrier around them, might reduce lag

- should they have a timer?

# Crafting

## Crafting Table

- Stick
- Wood Wall
- Bandage
- Stone Wall
- Chest,
- String
- Door,
- Arrow,
- Small Potion
- Large Potion
- Wood Axe
- Wood Pickaxe

## Furnace

- Metal Shard -> Metal Bar
- Raw Food -> Cooked Food

## Anvil

- Armor Sets (4 slots each x 3 total = 12)
- Bow
- Sword
- Dagger
- Throwing Star

##

[x] add velocity to dmg numbers, and fade out
add acceleration to player
cape + clothes
[] fix desc of all items
[x] bunch of recipes are missing, of course
[] tweak rarity of upgrade tome/orbs

[x] fix upgrade tome for equipment
[x] add esc to close container
[x] check if player spawn thing works

use tile coordinate multiplied by big number and module down to check if obj spawns. deterministic spawning.
use air blocks to determin a broken tile, regen world
make objects be defined by a 2x2 grid rather than 1x1 or 2x2 allow 2x1 and 1x2
add mobs to world gen params?
[x] make dungeon spawn with mobs initially, and hostile
particles w gradient, or tweak idk
dungeon chest has chance to spawn mobs
big brain: move the grid down until its at 0,0, not move the player
sapplings!!!
add juice explosion to projectiles on hit
can shift click crafting items to inv...
forrest armor too strong...

<!-- reloading chunk needs to remove water colliders from bridge -->
<!-- Player placed chests re-spawn as loot chests... -->

arrows double-hit
arrows can dmg objects
cant shoot proj over water
pick up item existign in inv when its full doesnt work
make mobs get stronger slightly every day.
MAGIC LIGHTING 2D
SOUNDS!

<!-- FRAME START -> (gen objs (prev frame), send chunk event -> spawn chunk, spawn dim event)  -> {CF} -> update walls -->

quick impls to feel out

- make dash faster
- animation startup for sword feels weird?
- should items have a level that scales with stats?

  - solves issue of getting random rng good drops in the early game and being op + items not scaling well at all + everything was the same power
  - also solves the fact that i only have a few item variations, this way items dont have to have to be balanced in different "power levels" (armor for example)
  - means that player will want to continue to upgrade their gear, via new drops, or upgrades
  - upgrade tome will just upgrade the level, which will guide the stat increase too.
    BUG: miss-matched items unstackable (add log)
    BUG: Slime proj dont hit, get destroyed by other colliders
  - mana regen and mana steal
  - lower drop rate of arrows and starss

  muck progression notes:

- progression is done through 2 things: chests/relics and higher tiers of gear
  - chests are gained through gold: incentivises killing mobs, and also exploring for free chests
  - higher tiers of gear gained through mining/trees: incentivises foraging/exploring
- gold is the true progression, since mobs give gold rather than gear/upgrades.
- all gear is crafted
- urgency comes from: mobs get stronger every night, very quickly, gotta explore to find chests/higher tiers of materials
- only get strong from 2 sources: relics and gear

notes for game:

- mobs get stronger quickly, higher health + dmg
- level up to add stat points -> agency to pick direction for build
- need lvl req to wear equipment
- equipment gets stronger as days progress (higher lvl mobs drop higher lvl equipment)
- 2 sources of progression: stat points, gear
- both gear and stat points incentivise killing mobs
- shelter, food, resources, and upgrading gear incentivise exploration/material gathering
- exploration incentivised to find dungeons
  - dungeon enemies match strenght of enemeis by day count

island variation:

- world generation is no longer infinite open world
- spawn on a randomly generated island, surrounded by water
- later, could introduce sections that are blocked off by unbreakable trees (can signify tiers of gear)
  - progress to harder area by getting strongest tools in previous area
- 1 dungeon spawner per area (or full island if no areas)
- every new day, the dungeon its different, repopulated with loot and mobs, maybe you can only enter once per day too
- on certain days, it could be a boss arena too
- would imply limited resources, 2 ways to solve this
  - add farming/plant growth: a way to regenerate plants/mushrooms
  - stone/ores can be later farmed in dungeons once they run out in the overworld

# Main Menu

Buttons:

- Play: opens window to pick a save (3 slots). default empty icon, then a designed one with some data on the run (night, level)
- Settings: opens settings menu -> one system to handle all options, match on enum, buttons just call an event with that enum type
  - music volume: [-] [+]
  - save frequency: [15s] [2]
- Quit.

- [x] shrink hit box on mobs
- [x] add sapplings asap
- [x] make breaking trees, etc give exp (Add component mobs get)
- [x] stats bug
- [x] stats page re-render bug (its not getting purged, stats menu)
- [x] chests generate as non-players so they spawn garbage loot :/
- [x] lower mob stat/dmg scaling
- sapplings reset timer when despawned... set up resource to track them, or dont despawn them lol...
- [x] dungeon exiting is broken i think
- [x] neg stats showing "+ -"
- [x] unify stat lines in gear
- add special inv slots for gear/acc
- add options menu...
- hitting trees always gives brown or white particles (different from minimap color...)
- [x] make sword hitbox slightly bigger/forward?
- [x] weird attack animation/sound bug (multi hits)
- fade intro step after hitting start
- [x] only take dmg from enemy when they're in attack anim
- proj double fire bug!!!
- helper ui keybind icons for stats, fairy, inv, etc
- [x] add a way to get weapons
      -> `Essence of the Forrest` (some way to translate monster killing to get a random piece of loot)
      -> All monsters can drop essense
      -> Or, Mob drops can be exchanged for essence

  - -> Right clicking Essence drops you a random piece of gear
  - -> Right clicking Essence opens a menu to spend on various things (Random gear, Skill Point, dungeon key?, boss summon?)
  - Genius prairie ida: Fairies!!
  - -> fairies spawn randomly and want to trade you random loot stuff for essence
  - -> they have a chance to sell you gear you lost in previous runs!!!!

- [x]buff mobs (hp, dmg, hit box)
- [x] fix inv bug where you drag and then clsoe and it gets stuck lol
- [x] cant reopen fairy shop (it breaks ui again lol)
- [x] i dont think item levels are doign anything lol
- [x] remove durability, causes inv bug and its useless
- add way to grow food
- [x] decrease cost to craft w stone, or new way to obtain it (stone mob?)
- [x] decrease dagger speed?
- [ ] upgrade station needs to reset tooltips when task is done
- BUG commo ngear dropping w 4 lines of stats
- Problem: You can run aroudn the island to despawn mobs
- [x] mushroom doesnt do dmg
- [x] make dmg numbers stray more
- Risk vs Reward:
  - shrine to pay resource( hp, essence?) for a reward (resources, food, item?)
  - [x] shrine to engage in combat to get reward (resource, food, item) (summons a bunch of mobs to kill, with an Elite)
- Relics/Buffs: Buffs that grant you unique bonuses: gain one after every night?

  - regen faster, dash twice, dash further, aoe burst when hit, split dmg in two hits, Knockback enemies on hit, food goes down slower, attack speed up, poison on hit, mp faster, mp discount, -1 damage on every hit, night time aggro distance decreased, more ammo drops, chance to not use ammo, weapon upgrades (drops a weapon of that type), upgrade tomes drop more often, bonus xp, +25% stat bonus (all 4), more chests in dungeons, defelct proj on dash,
  - Cursed relics: satisfy a condition to gain a powerful buff.

  -BUG shift clicking gear stacks them no matter what

- [x] bug only give ability to weapons
- [x] inv toggle bugs (pressing tab to close doesnt reset state to closed)
- [x] inv hotbar bug
- [x] bed texture bug
- [x] lvl text offset after double digits3
- [x] fairy is not spawning...
- [x] bug: mobs spawn in water
- TODO: extract game window size and other options into a resource
- ui element that sticks to screen edges to show where fairy is
- [x] can summon inf number of combat shrine mobs until you win
- change item abilities to use % total dmg instead of fixed
- mobs shoudl be able to hit w non-weapons
- duplicate audio triggers on attack
- [] design a better mob spawner system, too many spawn in dungeons instantly.
- weird tracking on the boss follow
  Notes: day 4, 15-35 dmg, day 6 45dmg, lvl 11-12

-[x] BUG: spawners need to get added after generation of a chunk is complete, not after a chunk is added

Stations: CT, Furnace, Cauldron, || Anvil, || Upgrade St.,|| Alchemy T

Progression flow:
Day 1: CT

- gather first tools
- gather some food / explore
- gather some resources (wood, stone)
- get to lvl 2
- if youre fast, get a crafting table + some walls

Night 1:

- get a level or two

Day 2: Furn / Caul

- get lots of resoruces
- finish building base, a chest
- explore more
- build towards furnace or cauldron, maybe both
- allows for some better food
- plant sapplings for food/grow other food

Day 3:

- should have enough essence/resources for a dungeon run

Boss 1:

- Unique Drops: Poison dagger,

prairie gameplay notes:
keybind icons
rare drops have a animation + sound
items drop more spread out
exp particles

### Time Travel idea

## Design Notes

- Player sent back to early times before the destruction of the island (maybe starts with a cutscene)
- Goal is to work towards the present where the final boss is
- Each era:
  - gets harder in general, through new enemies and flora/fauna
  - has more decay/damage in the form of less lush foliage, and eventually real damage (fire, destruction, etc)
  - gets more grim and destroyed thematically
  - can have vastly different climates/biomes (winter,)
  - each era will be gatekept by a boss, upon defeat will allow access to the next era
  - player can use `time currency` to access previous eras
    - currency from bosses? not sure.
- eras can also be partially random eventually, when we have a larger pool of eras, split into early/mid/endgame pools
- can introduce time based item abilities: short range teleports/blinks, freeze enemies in time, rewind time?
- item abilities should be on right click now, and optionally consume mana
- The same base resources (wood/stone) should be available in all eras probably
- varrying levels of food

## Technical Notes

- Need to rework generation code. Upon loading into a new era for the first time (including start of a run), we generate and cache the world in this order:
  - generate all the terrain/tiles/chunks first in a ISLAND_SIZE + 1 area
  - generate unique objs in viable spots, cache
  - generate normal objs, cache
- Eras can be indexed in order of discovery:
  - all cache systems now cache a vec of the same thing.
  - also track a vec of discovered eras, in order
  - internally, traveling to an era just looks up the relevant cache for that era, and loads them.
  - if its the first time traveling, we generate as normal.
- need world gen proto files for each era
- save system needs to match this system, store vec of obj data as well as vec of eras.
- new fairy in each world, spawns after boss dies, spawns FROM the boss when it dies. Time fairy or something

## Era Designs

- First Era: lush, colorful, saturated, cute enemies, peaceful music, lots of vegetation
- Second Era: A little more baren, some life has been lost from the island, slightly darker tileset, darker trees, less lush foliage, More stumps and dead branches, bushes with less foliage, more wild animals, boars, perhapse somewhat hostile, food is harder to find so need to
  - spider enemy: some form of slow effect with cobweb, maybe it creates a cobweb tile
  - maybe fur devil varient that is more undead, bones: drops bones
  - skeleton
- PLANTS:
  - taller more droopy fruit bush
  - spike-y tree
  - bamboo (near water)
  -
