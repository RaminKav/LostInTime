pub mod chest_ui;
pub mod crafting_ui;
pub mod damage_numbers;
pub mod screen_effects;
pub mod ui_container_param;
use bevy::sprite::Material2dPlugin;
use damage_numbers::handle_clamp_screen_locked_icons;
use screen_effects::{handle_add_screen_effects, setup_screen_effects, ScreenEffectMaterial};
pub use ui_container_param::*;
mod enemy_health_bar;
mod fps_text;
pub mod furnace_ui;
mod interactions;
mod inventory_ui;
pub mod minimap;
mod player_hud;
pub mod stats_ui;
mod tile_hover;
mod tooltips;
mod ui_helpers;
pub use chest_ui::*;
pub use enemy_health_bar::*;
use fps_text::*;
pub use furnace_ui::*;
pub use interactions::*;
pub use inventory_ui::*;
pub use player_hud::*;
pub use tooltips::*;
mod main_menu;
pub use main_menu::*;
mod essence_ui;
pub use essence_ui::*;

use crate::{
    client::load_state, combat::handle_hits, item::item_actions::ActionSuccessEvent, CustomFlush,
    GameState, DEBUG_MODE,
};

use self::{
    crafting_ui::{change_ui_state_to_crafting_when_resource_added, CraftingContainer},
    // crafting_ui::setup_crafting_slots_ui,
    damage_numbers::{
        add_previous_health, handle_add_damage_numbers_after_hit, handle_add_dodge_text,
        tick_damage_numbers, DodgeEvent,
    },
    minimap::MinimapPlugin,
    stats_ui::{setup_stats_ui, toggle_stats_visibility, update_sp_text, update_stats_text},
    tile_hover::spawn_tile_hover_on_cursor_move,
};

pub const INVENTORY_UI_SIZE: Vec2 = Vec2::new(172., 135.);
pub const STATS_UI_SIZE: Vec2 = Vec2::new(79., 104.);
pub const ESSENCE_UI_SIZE: Vec2 = Vec2::new(109., 151.);
pub const TOOLTIP_UI_SIZE: Vec2 = Vec2::new(93., 120.5);
pub const CHEST_INVENTORY_UI_SIZE: Vec2 = Vec2::new(127., 142.);
pub const CRAFTING_INVENTORY_UI_SIZE: Vec2 = Vec2::new(171., 166.);
pub const FURNACE_INVENTORY_UI_SIZE: Vec2 = Vec2::new(171., 166.);
pub const UI_SLOT_SIZE: f32 = 20.0;

pub struct UIPlugin;
//TODO: extract out ui darken overlay into a helper function
impl Plugin for UIPlugin {
    fn build(&self, app: &mut App) {
        app.add_state::<UIState>()
            .insert_resource(LastHoveredSlot { slot: None })
            .insert_resource(InventoryState::default())
            .insert_resource(TooltipsManager {
                timer: Timer::from_seconds(0.3, TimerMode::Once),
            })
            .add_event::<ActionSuccessEvent>()
            .add_event::<DropOnSlotEvent>()
            .add_event::<DodgeEvent>()
            .add_event::<RemoveFromSlotEvent>()
            .add_event::<ToolTipUpdateEvent>()
            .add_event::<TooltipTeardownEvent>()
            .add_event::<ShowInvPlayerStatsEvent>()
            .add_event::<SubmitEssenceChoice>()
            .add_event::<DropInWorldEvent>()
            .add_event::<MenuButtonClickEvent>()
            .add_plugin(Material2dPlugin::<ScreenEffectMaterial>::default())
            .register_type::<InventorySlotState>()
            .add_plugin(MinimapPlugin)
            .add_system(spawn_fps_text.in_schedule(OnEnter(GameState::Main)))
            .add_systems((
                setup_inv_ui
                    .before(CustomFlush)
                    .run_if(state_changed::<UIState>().and_then(in_state(UIState::Inventory))),
                setup_inv_ui
                    .before(CustomFlush)
                    .run_if(state_changed::<UIState>().and_then(in_state(UIState::Chest))),
                setup_inv_ui
                    .before(CustomFlush)
                    .run_if(state_changed::<UIState>().and_then(in_state(UIState::Crafting))),
                setup_inv_ui
                    .before(CustomFlush)
                    .run_if(state_changed::<UIState>().and_then(in_state(UIState::Furnace))),
            ))
            .add_systems(
                (
                    setup_hotbar_hud,
                    setup_xp_bar_ui.after(load_state),
                    setup_bars_ui.after(load_state),
                )
                    .in_schedule(OnEnter(GameState::Main)),
            )
            .add_systems(
                (
                    create_enemy_health_bar,
                    add_previous_health,
                    update_xp_bar,
                    handle_enemy_health_bar_change,
                    handle_enemy_health_visibility,
                    add_ui_icon_for_elite_mobs,
                    handle_add_damage_numbers_after_hit.after(handle_hits),
                    handle_add_dodge_text,
                    tick_damage_numbers,
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_systems(
                (
                    setup_inv_slots_ui,
                    setup_chest_slots_ui.run_if(in_state(UIState::Chest)),
                    change_ui_state_to_chest_when_resource_added
                        .before(CustomFlush)
                        .run_if(resource_added::<ChestContainer>()),
                    text_update_system,
                    handle_item_drop_clicks,
                    handle_drop_dragged_items_on_inv_close,
                    handle_dragging,
                    handle_drop_on_slot_events.after(handle_item_drop_clicks),
                    handle_drop_in_world_events.after(handle_item_drop_clicks),
                    handle_interaction_clicks
                        .before(handle_item_drop_clicks)
                        .run_if(not(in_state(UIState::Closed))),
                    handle_spawn_inv_item_tooltip,
                    update_inventory_ui.after(CustomFlush),
                    update_healthbar,
                    handle_update_inv_item_entities,
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_systems(
                (
                    add_inv_to_new_chest_objs,
                    add_container_to_new_furnace_objs,
                    setup_furnace_slots_ui.run_if(in_state(UIState::Furnace)),
                    update_foodbar,
                    update_furnace_bar,
                    update_mana_bar,
                    handle_spawn_inv_player_stats.after(CustomFlush),
                    handle_cursor_stats_buttons.run_if(in_state(UIState::Stats)),
                    toggle_stats_visibility,
                    spawn_tile_hover_on_cursor_move,
                    setup_stats_ui
                        .before(CustomFlush)
                        .run_if(state_changed::<UIState>().and_then(in_state(UIState::Stats))),
                    change_ui_state_to_crafting_when_resource_added
                        .before(CustomFlush)
                        .run_if(resource_added::<CraftingContainer>()),
                    change_ui_state_to_furnace_when_resource_added
                        .before(CustomFlush)
                        .run_if(resource_added::<FurnaceContainer>()),
                    update_stats_text.run_if(in_state(UIState::Stats)),
                    update_sp_text.run_if(in_state(UIState::Stats)),
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_systems(
                (
                    tick_tooltip_timer,
                    handle_tooltip_teardown,
                    handle_submit_essence_choice,
                    handle_populate_essence_shop_on_new_spawn,
                    handle_cursor_essence_buttons,
                    handle_add_screen_effects,
                    setup_screen_effects,
                    handle_clamp_screen_locked_icons,
                    setup_essence_ui
                        .before(CustomFlush)
                        .run_if(resource_added::<EssenceShopChoices>()),
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                handle_new_ui_state
                    .in_base_set(CoreSet::PostUpdate)
                    .run_if(in_state(GameState::Main)),
            )
            .add_system(handle_hovering.run_if(ui_hover_interactions_condition))
            .add_system(handle_cursor_main_menu_buttons.in_set(OnUpdate(GameState::MainMenu)))
            .add_system(apply_system_buffers.in_set(CustomFlush));
    }
}

fn ui_hover_interactions_condition(state: Res<State<GameState>>) -> bool {
    state.0 == GameState::Main || state.0 == GameState::MainMenu
}

pub fn handle_new_ui_state(
    mut next_ui_state: ResMut<NextState<UIState>>,
    curr_ui_state: Res<State<UIState>>,
    old_ui: Query<(Entity, &UIState), With<UIState>>,
    mut commands: Commands,
    chest_option: Option<Res<ChestContainer>>,
    furnace_option: Option<Res<FurnaceContainer>>,
    mut hotbar_slots: Query<(&mut Visibility, &mut InventorySlotState), Without<Interactable>>,
) {
    if !next_ui_state.0.is_some() {
        return;
    }
    let next_ui = next_ui_state.0.as_ref().unwrap().clone();
    if *DEBUG_MODE {
        println!("UI State Changed: {:?} -> {:?}", curr_ui_state.0, next_ui);
    }
    let mut should_close_self = false;
    if next_ui == curr_ui_state.0 {
        next_ui_state.set(UIState::Closed);
        should_close_self = true;
    }
    for (e, ui) in old_ui.iter() {
        if *ui != next_ui || should_close_self {
            commands.entity(e).despawn_recursive();
        }
    }
    if let Some(chest) = chest_option {
        if let Some(mut chest_parent) = commands.get_entity(chest.parent) {
            chest_parent.insert(chest.to_owned());
        }
        if next_ui != UIState::Chest {
            commands.remove_resource::<ChestContainer>();
        }
    }
    if let Some(furnace) = furnace_option {
        if let Some(mut furnace_parent) = commands.get_entity(furnace.parent) {
            furnace_parent.insert(furnace.to_owned());
        }
        if next_ui != UIState::Furnace {
            commands.remove_resource::<FurnaceContainer>();
        }
    }
    if !next_ui.is_inv_open() || should_close_self {
        commands.remove_resource::<CraftingContainer>();
    }
    if next_ui != UIState::Essence {
        commands.remove_resource::<EssenceShopChoices>();
    }
    if let Some(next_ui) = &next_ui_state.0 {
        for (mut hbv, mut state) in hotbar_slots.iter_mut() {
            if !next_ui.is_inv_open() {
                state.dirty = true;
            }
            *hbv = if next_ui.is_inv_open() {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            };
        }
    }
}
