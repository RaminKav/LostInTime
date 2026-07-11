use bevy::prelude::*;
use rand::Rng;

use crate::{
    assets::SpriteAnchor,
    gamepad_input::GamepadAction,
    inventory::{Inventory, ItemStack},
    item::{shrine_visuals::ShrineNeedsRepair, WorldObject},
    keybinds::InputMappings,
    player::Player,
    ui::key_input_guide::InteractionGuideTrigger,
    world::{generation::WorldObjectCache, world_helpers::world_pos_to_tile_pos, TileMapPosition},
};
use leafwing_input_manager::prelude::ActionState;

/// Average material requirement for repairing a shrine. Actual cost rolls ±25%.
#[derive(Clone, Copy, Debug)]
pub struct ShrineRepairMaterial {
    pub item: WorldObject,
    pub average: u32,
}

/// Rolled repair requirements attached while a shrine is broken.
#[derive(Component, Clone, Debug)]
pub struct ShrineRepairCosts {
    pub materials: Vec<(WorldObject, u32)>,
}

pub const SHRINE_BROKEN_CHANCE: f64 = 0.30;
const REPAIR_COST_VARIANCE: f32 = 0.25;

pub fn repair_materials_for(obj: WorldObject) -> &'static [ShrineRepairMaterial] {
    match obj {
        WorldObject::GambleShrine => &[
            ShrineRepairMaterial {
                item: WorldObject::StoneChunk,
                average: 8,
            },
            ShrineRepairMaterial {
                item: WorldObject::Log,
                average: 20,
            },
        ],
        WorldObject::MicrowaveShrine => &[
            ShrineRepairMaterial {
                item: WorldObject::StoneChunk,
                average: 10,
            },
            ShrineRepairMaterial {
                item: WorldObject::Log,
                average: 12,
            },
        ],
        WorldObject::ActiveSkillShrine => &[
            ShrineRepairMaterial {
                item: WorldObject::Log,
                average: 24,
            },
            ShrineRepairMaterial {
                item: WorldObject::Stick,
                average: 6,
            },
            ShrineRepairMaterial {
                item: WorldObject::PlantFibre,
                average: 16,
            },
        ],
        WorldObject::BlacksmithMerchant => &[
            ShrineRepairMaterial {
                item: WorldObject::Log,
                average: 24,
            },
            ShrineRepairMaterial {
                item: WorldObject::Stick,
                average: 6,
            },
            ShrineRepairMaterial {
                item: WorldObject::PlantFibre,
                average: 16,
            },
        ],
        WorldObject::HeirloomShrine => &[
            ShrineRepairMaterial {
                item: WorldObject::StoneChunk,
                average: 12,
            },
            ShrineRepairMaterial {
                item: WorldObject::Log,
                average: 18,
            },
            ShrineRepairMaterial {
                item: WorldObject::Stick,
                average: 4,
            },
        ],
        WorldObject::CombatShrine => &[ShrineRepairMaterial {
            item: WorldObject::StoneChunk,
            average: 20,
        }],
        WorldObject::ChaosTotem => &[ShrineRepairMaterial {
            item: WorldObject::StoneChunk,
            average: 16,
        }],
        WorldObject::CauldronShrine => &[ShrineRepairMaterial {
            item: WorldObject::StoneChunk,
            average: 20,
        }],
        WorldObject::WellShrine => &[
            ShrineRepairMaterial {
                item: WorldObject::StoneChunk,
                average: 12,
            },
            ShrineRepairMaterial {
                item: WorldObject::PlantFibre,
                average: 8,
            },
            ShrineRepairMaterial {
                item: WorldObject::Stick,
                average: 10,
            },
        ],
        _ => &[],
    }
}

pub fn can_be_broken(obj: WorldObject) -> bool {
    !repair_materials_for(obj).is_empty()
}

pub fn roll_repair_costs(obj: WorldObject, rng: &mut impl Rng) -> Vec<(WorldObject, u32)> {
    repair_materials_for(obj)
        .iter()
        .map(|entry| {
            let variance = 1.0 + rng.gen_range(-REPAIR_COST_VARIANCE..=REPAIR_COST_VARIANCE);
            let amount = ((entry.average as f32) * variance).round().max(1.0) as u32;
            (entry.item, amount)
        })
        .collect()
}

/// Roll broken state for a newly placed shrine tile (pre-roll or first procedural spawn).
/// Empty vec in the cache means "rolled healthy" so chunk reloads do not re-roll.
pub fn maybe_mark_shrine_broken(
    cache: &mut WorldObjectCache,
    tile_pos: TileMapPosition,
    obj: WorldObject,
    rng: &mut impl Rng,
) {
    if !can_be_broken(obj) {
        return;
    }
    if cache.broken_shrine_costs.contains_key(&tile_pos) {
        return;
    }
    if rng.gen_bool(SHRINE_BROKEN_CHANCE) {
        cache
            .broken_shrine_costs
            .insert(tile_pos, roll_repair_costs(obj, rng));
    } else {
        cache.broken_shrine_costs.insert(tile_pos, Vec::new());
    }
}

fn repair_guide_trigger(costs: &[(WorldObject, u32)]) -> InteractionGuideTrigger {
    let icon = costs
        .first()
        .map(|(item, amount)| ItemStack::crate_icon_stack(*item).copy_with_count(*amount as usize));
    InteractionGuideTrigger {
        text: Some("Repair".to_string()),
        activation_distance: 32.,
        icon_stack: icon,
    }
}

/// When a shrine entity spawns, attach broken state from the world cache (rolling for chaos if needed).
pub fn apply_broken_shrine_state_on_spawn(
    mut commands: Commands,
    new_shrines: Query<
        (Entity, &WorldObject, &Transform, Option<&SpriteAnchor>),
        Added<WorldObject>,
    >,
    mut cache: ResMut<WorldObjectCache>,
) {
    let mut rng = rand::thread_rng();
    for (entity, obj, transform, anchor) in new_shrines.iter() {
        if !can_be_broken(*obj) {
            continue;
        }
        let world_pos =
            transform.translation.truncate() - anchor.map(|a| a.0).unwrap_or(Vec2::ZERO);
        let tile_pos = world_pos_to_tile_pos(world_pos);

        if !cache.broken_shrine_costs.contains_key(&tile_pos) {
            maybe_mark_shrine_broken(&mut cache, tile_pos, *obj, &mut rng);
        }

        let Some(costs) = cache.broken_shrine_costs.get(&tile_pos) else {
            continue;
        };
        if costs.is_empty() {
            continue;
        }

        commands.entity(entity).insert((
            ShrineNeedsRepair,
            ShrineRepairCosts {
                materials: costs.clone(),
            },
            repair_guide_trigger(costs),
        ));
    }
}

/// Attempt to pay repair costs. Returns true if the shrine was repaired.
fn try_repair_shrine(
    commands: &mut Commands,
    inv: &mut Inventory,
    cache: &mut WorldObjectCache,
    shrine_entity: Entity,
    tile_pos: TileMapPosition,
    costs: &ShrineRepairCosts,
) -> bool {
    for (item, amount) in costs.materials.iter() {
        if inv.items.get_item_count_in_container(*item) < *amount as usize {
            return false;
        }
    }
    for (item, amount) in costs.materials.iter() {
        if let Err(err) = inv.items.remove_from_inventory(*amount as usize, *item) {
            error!("Failed to remove repair material {:?}: {:?}", item, err);
            return false;
        }
    }

    cache.broken_shrine_costs.insert(tile_pos, Vec::new());
    commands
        .entity(shrine_entity)
        .remove::<ShrineNeedsRepair>()
        .remove::<ShrineRepairCosts>();

    true
}

/// Intercept interact on broken shrines: repair if possible, otherwise block normal use.
pub fn handle_broken_shrine_interact(
    mut commands: Commands,
    broken: Query<
        (
            Entity,
            &GlobalTransform,
            &WorldObject,
            &SpriteAnchor,
            &ShrineRepairCosts,
        ),
        With<ShrineNeedsRepair>,
    >,
    mut player_query: Query<(&GlobalTransform, &mut Inventory), With<Player>>,
    mut cache: ResMut<WorldObjectCache>,
    key_input: Res<Input<KeyCode>>,
    mouse_input: Res<Input<MouseButton>>,
    keybinds: Res<InputMappings>,
    gamepad_action_q: Query<&ActionState<GamepadAction>, With<Player>>,
) {
    let gamepad_pressed = gamepad_action_q
        .get_single()
        .map(|a| a.just_pressed(GamepadAction::Interact))
        .unwrap_or(false);
    if !keybinds.check_interact_input(&key_input, &mouse_input) && !gamepad_pressed {
        return;
    }

    let Ok((player_t, mut inv)) = player_query.get_single_mut() else {
        return;
    };
    let player_pos = player_t.translation().truncate();

    for (entity, transform, obj, anchor, costs) in broken.iter() {
        let obj_pos = transform.translation().truncate() - anchor.0;
        if obj_pos.distance(player_pos) > 32. {
            continue;
        }
        let tile_pos = world_pos_to_tile_pos(obj_pos);
        if try_repair_shrine(&mut commands, &mut inv, &mut cache, entity, tile_pos, costs) {
            restore_shrine_guide_after_repair(&mut commands, entity, *obj);
        }
        // Consume the interact so normal shrine use does not also run this frame.
        return;
    }
}

fn restore_shrine_guide_after_repair(commands: &mut Commands, entity: Entity, obj: WorldObject) {
    let guide = match obj {
        WorldObject::CombatShrine => InteractionGuideTrigger {
            text: Some("Activate Shrine".to_string()),
            activation_distance: 32.,
            icon_stack: Some(ItemStack::crate_icon_stack(WorldObject::ChestBlock)),
        },
        WorldObject::GambleShrine => InteractionGuideTrigger {
            text: Some("Activate Shrine".to_string()),
            activation_distance: 32.,
            icon_stack: Some(ItemStack::crate_icon_stack(WorldObject::Coin)),
        },
        WorldObject::BlacksmithMerchant => InteractionGuideTrigger {
            text: Some("Activate Shrine".to_string()),
            activation_distance: 32.,
            icon_stack: Some(ItemStack::crate_icon_stack(WorldObject::Coin)),
        },
        WorldObject::ActiveSkillShrine => InteractionGuideTrigger {
            text: Some("Activate Shrine".to_string()),
            activation_distance: 32.,
            icon_stack: None,
        },
        WorldObject::HeirloomShrine => InteractionGuideTrigger {
            text: Some("Activate Shrine".to_string()),
            activation_distance: 32.,
            icon_stack: None,
        },
        WorldObject::MicrowaveShrine => InteractionGuideTrigger {
            text: Some("Activate Shrine".to_string()),
            activation_distance: 32.,
            icon_stack: None,
        },
        WorldObject::ChaosTotem => InteractionGuideTrigger {
            text: Some("Activate Shrine".to_string()),
            activation_distance: 32.,
            icon_stack: None,
        },
        WorldObject::CauldronShrine | WorldObject::WellShrine => {
            commands.entity(entity).remove::<InteractionGuideTrigger>();
            return;
        }
        _ => {
            commands.entity(entity).remove::<InteractionGuideTrigger>();
            return;
        }
    };
    commands.entity(entity).insert(guide);
}
