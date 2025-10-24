# Pet System

This module implements a comprehensive pet animation and AI system for the survival rogue-like game.

## Features

- **State Machine AI**: Pets use a state machine with multiple states (Idle, Follow, Attack, Return)
- **Animation System**: Support for sprite-based animations (currently using simple sprites, can be extended to Aseprite)
- **Targeting System**: Pets can target and follow enemies or the player
- **Combat Integration**: Pets can attack enemies with weapons

## Components

### Pet States

- `PetIdleState`: Random wandering behavior when no target is nearby
- `PetFollowState`: Following a target (player or enemy)
- `PetAttackState`: Attacking a target when in range
- `PetReturnState`: Returning to the player when too far away

### Pet Configuration

- `PetState`: Main configuration component with targeting distances, speeds, and weapon settings
- `Pet`: Enum for different pet types

## Usage

### Spawning a Pet

Press `Z` to spawn a test pet. The pet will:

1. Start in idle state with random movement
2. Follow the player when they get close (100 units)
3. Return to idle when player moves away (120 units)
4. Follow the player at night (night time aggro)

### Pet Behavior

- **Idle**: Random movement in different directions
- **Follow**: Moves towards target at configured speed
- **Attack**: Moves close to target and performs attacks
- **Return**: Returns to player when too far away

## Configuration

The pet behavior can be configured through the `PetState` component:

- `max_distance_from_player`: Maximum distance before pet returns to player
- `max_target_distance`: Maximum distance to detect enemies
- `min_target_distance`: Minimum distance to maintain from target
- `follow_speed`: Speed when following targets
- `weapon_slot`: Optional weapon for the pet to use

## Animation System

The current implementation uses simple sprites, but the system is designed to support Aseprite animations. To add Aseprite support:

1. Create an Aseprite file with animation tags (IDLE, WALK, ATTACK)
2. Uncomment the aseprite macro in `pet_animation.rs`
3. Update the animation handlers to use AsepriteAnimation

## Future Enhancements

- Aseprite animation support
- Multiple pet types with different behaviors
- Pet leveling and stat progression
- Pet inventory system
- Pet commands (stay, follow, attack, etc.)
