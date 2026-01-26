pub mod chest_ui;
pub mod class_selection;
pub mod crafting_ui;
pub mod damage_numbers;
pub mod guide_hud;
pub mod item_chest;
mod loading_screen;
pub mod scrapper_ui;
pub mod screen_effects;
use class_selection::*;
use class_selection::{ClassUnlockConfirmState, ClassUnlockHoverState};
use guide_hud::*;
use item_chest::*;
pub mod ui_container_param;
use bevy::sprite::Material2dPlugin;
use damage_numbers::FloatingTextQueue;
use scrapper_ui::{
    add_inv_to_new_scrapper_objs, change_ui_state_to_scrapper_when_resource_added,
    handle_scrap_items_in_scrapper, setup_scrapper_slots_ui, ScrapperContainer, ScrapperEvent,
};
use screen_effects::{handle_screen_effects, setup_screen_effects, ScreenEffectMaterial};
pub use ui_container_param::*;
pub mod boss_health_bar;
mod enemy_health_bar;
mod fps_text;
pub mod key_input_guide;
use key_input_guide::*;
pub mod furnace_ui;
pub use skill_choice_ui::*;
mod achievement_banner;
mod active_skill_shrine_ui;
mod interactions;
mod inventory_ui;
pub mod minimap;
mod player_hud;
mod skill_choice_ui;
pub mod stats_ui;
pub use active_skill_shrine_ui::*;
mod tile_hover;
mod tooltips;
pub mod ui_helpers;
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
mod unlocks_ui;
pub use unlocks_ui::*;
mod options_ui;
pub use options_ui::*;
mod leaderboard_ui;
pub use leaderboard_ui::*;
mod name_entry_ui;
pub use name_entry_ui::*;
mod achievements_ui;
use crate::run_once_per_run;
use crate::ui::achievement_banner::{
    debug_trigger_achievement_banner, handle_achievement_banner_events, update_achievement_banners,
};
use crate::ui::damage_numbers::{
    handle_clamp_screen_locked_icons_worldpos, BeaconGuidanceRegistry,
};
pub use achievements_ui::*;
use loading_screen::*;

use crate::{
    attributes::clamp_health,
    client::{is_not_paused, leaderboard::auto_fetch_leaderboard_on_menu, load_state, ClientState},
    handle_hits,
    item::{
        active_skill_shrine::ActiveSkillShrineOverwrite,
        heirloom_shrine::handle_heirloom_shrine_ui_setup, item_actions::ActionSuccessEvent,
    },
    night::NightTracker,
    player::skills::HeirloomChoiceQueue,
    player::unlocks::RunUnlockState,
    player::RunScore,
    CustomFlush, Game, GameState, DEBUG,
};

use self::{
    crafting_ui::{change_ui_state_to_crafting_when_resource_added, CraftingContainer},
    damage_numbers::{
        add_previous_health, handle_add_damage_numbers_after_hit, handle_add_dodge_text,
        handle_queued_floating_texts, tick_damage_numbers, DodgeEvent,
    },
    minimap::MinimapPlugin,
    tile_hover::spawn_tile_hover_on_cursor_move,
};

pub const INVENTORY_UI_SIZE: Vec2 = Vec2::new(218., 145.);
pub const SKILLS_CHOICE_UI_SIZE: Vec2 = Vec2::new(96., 120.);
pub const ESSENCE_UI_SIZE: Vec2 = Vec2::new(157., 130.5);
pub const TOOLTIP_UI_SIZE: Vec2 = Vec2::new(133., 160.5);
pub const CHEST_INVENTORY_UI_SIZE: Vec2 = Vec2::new(127., 142.);
pub const CRAFTING_INVENTORY_UI_SIZE: Vec2 = Vec2::new(171., 166.);
pub const FURNACE_INVENTORY_UI_SIZE: Vec2 = Vec2::new(171., 166.);
pub const UI_SLOT_SIZE: f32 = 20.0;

pub struct UIPlugin;
//TODO: extract out ui darken overlay into a helper function
impl Plugin for UIPlugin {
    fn build(&self, app: &mut App) {
        app.add_state::<UIState>()
            .insert_resource(InventoryState::default())
            .insert_resource(ClassSelectionState::default())
            .init_resource::<ClassUnlockHoverState>()
            .init_resource::<ClassUnlockConfirmState>()
            .init_resource::<CheatSettings>()
            .insert_resource(RunUnlockState::default())
            .init_resource::<AchievementsPagination>()
            .insert_resource(crate::keybinds::InputMappings::load())
            .init_resource::<CurrentNameInput>()
            .init_resource::<CursorBlinkTimer>()
            .insert_resource(FloatingTextQueue::new(0.8))
            .insert_resource(TooltipsManager {
                timer: Timer::from_seconds(0.7, TimerMode::Once),
            })
            .add_event::<ActionSuccessEvent>()
            .add_event::<ScrapperEvent>()
            .add_event::<FlashExpBarEvent>()
            .add_event::<ItemChestAnimChangeEvent>()
            .add_event::<DropOnSlotEvent>()
            .add_event::<DodgeEvent>()
            .add_event::<RemoveFromSlotEvent>()
            .add_event::<ToolTipUpdateEvent>()
            .init_resource::<BeaconGuidanceRegistry>()
            .init_resource::<BlacksmithPurchaseTracker>()
            .add_event::<TooltipTeardownEvent>()
            .add_event::<ShowInvPlayerStatsEvent>()
            .add_event::<SubmitEssenceChoice>()
            .add_event::<DropInWorldEvent>()
            .add_event::<MenuButtonClickEvent>()
            .add_plugin(Material2dPlugin::<ScreenEffectMaterial>::default())
            .register_type::<InventorySlotState>()
            .add_plugin(MinimapPlugin)
            .add_system(setup_loading_screen.in_schedule(OnEnter(GameState::Initializing)))
            .add_system(
                check_initialization_complete
                    .run_if(in_state(GameState::Initializing)),
            )
            .add_system(spawn_fps_text.run_if(run_once_per_run()).in_schedule(OnEnter(GameState::Main)))
            .add_system(
                setup_leaderboard_ui
                    .in_schedule(OnEnter(GameState::MainMenu))
                    .after(auto_fetch_leaderboard_on_menu)  // Ensure fetch happens first
            )
            .add_system(cleanup_leaderboard_ui.in_schedule(OnExit(GameState::MainMenu)))
            .add_system(reset_blacksmith_tracker.in_schedule(OnEnter(GameState::MainMenu)))
            .add_system(cleanup_run_state.in_base_set(CoreSet::PreUpdate).run_if(not(in_state(GameState::MainMenu))))
            .add_systems((
                // Clean up leaderboard when entering other UI states to avoid duplicates
                cleanup_leaderboard_ui
                    .run_if(in_state(GameState::MainMenu)
                        .and_then(state_changed::<UIState>())
                        .and_then(not(in_state(UIState::Closed)))),
                // Recreate leaderboard when returning to main menu view (but not on initial entry)
                setup_leaderboard_ui
                    .after(cleanup_leaderboard_ui)
                    .run_if(in_state(GameState::MainMenu)
                        .and_then(state_changed::<UIState>())
                        .and_then(in_state(UIState::Closed))),
            ))
            .add_systems((
                update_leaderboard_display
                    .run_if(in_state(GameState::MainMenu))
                    .run_if(in_state(UIState::Closed)),
                ensure_leaderboard_entries
                    .run_if(in_state(GameState::MainMenu))
                    .run_if(in_state(UIState::Closed)),
            ))
            .add_systems((
                setup_inv_ui
                    .before(CustomFlush)
                    .run_if(state_changed::<UIState>().and_then(in_state(UIState::Inventory))),
                setup_inv_ui
                    .before(CustomFlush)
                    .run_if(state_changed::<UIState>().and_then(in_state(UIState::Chest))),
                setup_inv_ui
                    .before(CustomFlush)
                    .run_if(state_changed::<UIState>().and_then(in_state(UIState::Scrapper))),
                setup_inv_ui
                    .before(CustomFlush)
                    .run_if(state_changed::<UIState>().and_then(in_state(UIState::Crafting))),
                setup_inv_ui
                    .before(CustomFlush)
                    .run_if(state_changed::<UIState>().and_then(in_state(UIState::Furnace))),
            ))
            // Check for pending level-up rewards when any menu closes during gameplay
            .add_system(
                check_pending_levelup_rewards_on_menu_close
                    .run_if(in_state(GameState::Main))
                    .run_if(state_changed::<UIState>())
                    .run_if(in_state(UIState::Closed))
            )
            .add_systems(
                (
                    setup_hotbar_hud.run_if(run_once_per_run()),
                    setup_xp_bar_ui.after(load_state).run_if(run_once_per_run()),
                    setup_bars_ui.after(load_state).run_if(run_once_per_run()),
                    setup_currency_ui.run_if(run_once_per_run()),
                    setup_clock_hud.run_if(run_once_per_run()),
                    setup_era_timer_hud.run_if(run_once_per_run()),
                    setup_chaos_ui.run_if(run_once_per_run()),
                )
                    .in_schedule(OnEnter(GameState::Main)),
            )
            .add_system(
                setup_screen_effects
                    .in_set(OnUpdate(GameState::Main))
                    .before(handle_screen_effects),
            )
            .add_system(handle_screen_effects.in_set(OnUpdate(GameState::Main)))
            .add_systems(
                (
                    add_previous_health,
                    handle_flash_bars,
                    update_xp_bar_rainbow.before(update_xp_bar),
                    update_xp_bar,
                    update_decorative_xp_shards.after(update_xp_bar_rainbow),
                    handle_skill_choice_ui_close.after(update_xp_bar),
                    handle_enemy_health_bar_change,
                    add_ui_icon_for_elite_mobs,
                    handle_add_dodge_text,
                    boss_health_bar::spawn_boss_health_bar,
                    boss_health_bar::update_boss_health_bar,
                    boss_health_bar::cleanup_boss_health_bar_on_despawn,
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_systems((handle_queued_floating_texts.run_if(in_state(GameState::Main).or_else(in_state(GameState::BlessingChoice))),
                        tick_damage_numbers.run_if(in_state(GameState::Main).or_else(in_state(GameState::BlessingChoice)))))
            .add_system(
                handle_add_damage_numbers_after_hit
                    .before(handle_hits)
                    .in_set(OnUpdate(GameState::Main))
                    .run_if(resource_exists::<Game>()),
            )
            .add_systems(
                (
                    handle_item_drop_clicks,
                    handle_drop_dragged_items_on_inv_close,
                    handle_dragging,
                    handle_drop_on_slot_events.after(handle_item_drop_clicks),
                    handle_drop_in_world_events.after(handle_item_drop_clicks),
                    handle_interaction_clicks
                        .before(handle_item_drop_clicks)
                        .run_if(not(in_state(UIState::Closed))),
                    handle_hotbar_slot_clicks_when_inv_closed
                        .run_if(in_state(UIState::Closed)),
                    handle_spawn_inv_item_tooltip,
                    update_inventory_ui.after(CustomFlush),
                    handle_update_inv_item_entities,
                    update_hotbar_ammo_bar,
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_systems(
                (
                    change_ui_state_to_chest_when_resource_added
                        .before(CustomFlush)
                        .run_if(resource_added::<ChestContainer>()),
                    change_ui_state_to_scrapper_when_resource_added
                        .before(CustomFlush)
                        .run_if(resource_added::<ScrapperContainer>()),
                    handle_scrap_items_in_scrapper.run_if(in_state(UIState::Scrapper)),
                    text_update_system,
                    add_inv_to_new_scrapper_objs,
                    add_container_to_new_furnace_objs,
                    update_foodbar,
                    update_healthbar,
                    update_shieldbar,
                    change_ui_state_to_crafting_when_resource_added
                        .before(CustomFlush)
                        .run_if(resource_added::<CraftingContainer>()),
                    change_ui_state_to_furnace_when_resource_added
                        .before(CustomFlush)
                        .run_if(resource_added::<FurnaceContainer>()),
                    handle_update_clock_hud.run_if(
                        resource_exists::<NightTracker>()
                            .and_then(resource_changed::<NightTracker>()),
                    ),
                    handle_update_era_timer_hud.run_if(
                        resource_exists::<crate::night::EraTimer>(),
                    ),
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                add_inv_to_new_chest_objs
                    .in_base_set(CoreSet::PreUpdate)
                    .run_if(in_state(GameState::Main)),
            )
            .add_system(handle_spawn_inv_player_stats.in_base_set(CoreSet::PostUpdate))
            .add_systems(
                (
                    setup_unlocks_ui
                        .before(CustomFlush)
                        .run_if(state_changed::<UIState>().and_then(in_state(UIState::Unlocks))),
                    cleanup_unlocks_ui
                        .run_if(state_changed::<UIState>().and_then(not(in_state(UIState::Unlocks)))),
                    update_unlocks_currency_text.run_if(in_state(UIState::Unlocks)),
                    refresh_unlock_button_states.run_if(in_state(UIState::Unlocks)),
                    
                    setup_options_ui
                        .before(CustomFlush)
                        .run_if(state_changed::<UIState>().and_then(in_state(UIState::Options))),
                    cleanup_options_ui
                        .run_if(state_changed::<UIState>().and_then(not(in_state(UIState::Options)))),
                    
                    setup_achievements_ui
                        .before(CustomFlush)
                        .run_if(state_changed::<UIState>().and_then(in_state(UIState::Achievements))),
                    cleanup_achievements_ui
                        .run_if(state_changed::<UIState>().and_then(not(in_state(UIState::Achievements)))),
                    update_achievements_page_display
                        .after(CustomFlush)
                        .run_if(in_state(UIState::Achievements).and_then(
                            state_changed::<UIState>()
                                .or_else(resource_changed::<AchievementsPagination>())
                                .or_else(resource_changed::<crate::player::achievements::Achievements>())
                        )),
                    update_achievements_navigation_buttons
                        .run_if(in_state(UIState::Achievements)),
                )
                    .in_set(OnUpdate(GameState::MainMenu)),
            )
            .add_systems(
                (
                    setup_unlocks_ui
                        .before(CustomFlush)
                        .run_if(state_changed::<UIState>().and_then(in_state(UIState::Unlocks))),
                    cleanup_unlocks_ui
                        .run_if(state_changed::<UIState>().and_then(not(in_state(UIState::Unlocks)))),
                   
                    setup_options_ui
                        .before(CustomFlush)
                        .run_if(state_changed::<UIState>().and_then(in_state(UIState::Options))),
                    cleanup_options_ui
                        .run_if(state_changed::<UIState>().and_then(not(in_state(UIState::Options)))),
                   
                    setup_achievements_ui
                        .before(CustomFlush)
                        .run_if(state_changed::<UIState>().and_then(in_state(UIState::Achievements))),
                    cleanup_achievements_ui
                        .run_if(state_changed::<UIState>().and_then(not(in_state(UIState::Achievements)))),
                    update_achievements_page_display
                        .after(CustomFlush)
                        .run_if(in_state(UIState::Achievements).and_then(
                            state_changed::<UIState>()
                                .or_else(resource_changed::<AchievementsPagination>())
                                .or_else(resource_changed::<crate::player::achievements::Achievements>())
                        )),
                    update_achievements_navigation_buttons
                        .run_if(in_state(UIState::Achievements)),
                    
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_systems(
                (
                    setup_name_entry_ui
                        .before(CustomFlush)
                        .run_if(state_changed::<UIState>().and_then(in_state(UIState::EnterName))),
                    cleanup_name_entry_ui
                        .run_if(state_changed::<UIState>().and_then(not(in_state(UIState::EnterName)))),
                    handle_name_entry_input
                        .run_if(in_state(UIState::EnterName)),
                    update_name_entry_text
                        .run_if(in_state(UIState::EnterName)),
                    update_cursor_blink
                        .run_if(in_state(UIState::EnterName)),
                    handle_name_entry_ok_button
                        .run_if(in_state(UIState::EnterName)),
                )
                    .in_set(OnUpdate(GameState::MainMenu)),
            )
            .add_systems((
                    handle_unlocks_clicks.run_if(in_state(UIState::Unlocks)),
                    update_unlocks_currency_text.run_if(in_state(UIState::Unlocks)),
                    refresh_unlock_button_states.run_if(in_state(UIState::Unlocks)),
                    handle_options_clicks.run_if(in_state(UIState::Options)),
                    handle_key_rebind_input.run_if(in_state(UIState::Options)),
                    update_keybind_text
                        .run_if(in_state(UIState::Options))
                        .after(handle_key_rebind_input),
                    handle_cheat_checkbox_click.run_if(in_state(UIState::Options)),
                    update_cheat_checkbox_visual.run_if(in_state(UIState::Options)),
                    handle_achievement_row_clicks.run_if(in_state(UIState::Achievements)))
                )
            .add_system(
                handle_tooltip_teardown
                    .in_base_set(CoreSet::PreUpdate)
                    .run_if(in_state(GameState::Main)),
            )
            .add_systems(
                (
                    setup_inv_slots_ui,
                    setup_chest_slots_ui.run_if(in_state(UIState::Chest)),
                    setup_scrapper_slots_ui.run_if(in_state(UIState::Scrapper)),
                    tick_tooltip_timer,
                    handle_submit_essence_choice.run_if(resource_exists::<EssenceShopChoices>()),
                    handle_populate_essence_shop_on_new_spawn,
                    handle_cursor_essence_buttons,
                    handle_cursor_skills_buttons.run_if(in_state(UIState::Skills)),
                    update_furnace_bar,
                    setup_skill_choice_ui
                        .before(CustomFlush)
                        .run_if(state_changed::<UIState>().and_then(in_state(UIState::Skills))),
                    handle_heirloom_shrine_ui_setup
                        .before(setup_skill_choice_ui)
                        .run_if(state_changed::<UIState>().and_then(in_state(UIState::Skills))),
                    setup_item_chest_ui
                        .before(CustomFlush)
                        .run_if(state_changed::<UIState>().and_then(in_state(UIState::ItemChest))),

                    handle_cursor_item_chest_button.run_if(in_state(UIState::ItemChest)),
                    interactions::handle_cursor_heirloom_chest_button
                        .run_if(in_state(UIState::ItemChest)),
                    setup_essence_ui
                        .before(CustomFlush)
                        .run_if(resource_added::<EssenceShopChoices>()),
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                    handle_update_player_skills.after(clamp_health).run_if(in_state(GameState::Main).or_else(in_state(GameState::BlessingChoice))),
            )
            .add_systems(
                (
                    handle_heirloom_hud_tooltip,
                    player_hud::handle_active_skill_hud_tooltip,
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                update_active_skill_keybind_text
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                update_inventory_keybind_text
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                handle_essence_heirloom_tooltip
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                handle_item_chest_final_item_hover
                    .run_if(in_state(UIState::ItemChest))
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_systems(
                (
                    setup_active_skill_shrine_ui.before(CustomFlush).run_if(
                        state_changed::<UIState>().and_then(in_state(UIState::ActiveSkillShrine)),
                    ),
                    tick_active_skill_shrine_ui_interaction_lock_timers
                        .run_if(in_state(UIState::ActiveSkillShrine)),
                    handle_active_skill_shrine_ui_interaction
                        .run_if(in_state(UIState::ActiveSkillShrine)),
                    active_skill_shrine_ui::tick_active_skill_slot_choice_ui_interaction_lock_timers
                        .run_if(in_state(UIState::ActiveSkills)),
                    handle_active_skill_shrine_overwrite_interaction
                        .run_if(in_state(UIState::ActiveSkills)),
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                active_skill_shrine_ui::setup_active_skill_shrine_overwrite_ui
                    .before(CustomFlush)
                    .run_if(
                        state_changed::<UIState>()
                            .and_then(in_state(UIState::ActiveSkills))
                            .and_then(resource_exists::<ActiveSkillShrineOverwrite>()),
                    )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_systems((setup_class_selection_ui
                .before(CustomFlush)
                .run_if(state_changed::<UIState>().and_then(in_state(UIState::ClassSelection))),
                update_class_unlock_warnings
                    .run_if(in_state(UIState::ClassSelection).and_then(
                        resource_changed::<crate::player::achievements::Achievements>()
                            .or_else(resource_changed::<crate::player::UnlockedClasses>())
                    )),))
            .add_systems(
                (
                    handle_anim_events.run_if(in_state(UIState::ItemChest)),
                    tick_skill_choice_interaction_lock_timers,
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_systems(
                (
                    tick_skill_cooldown_overlays.run_if(is_not_paused),
                    handle_active_skill_event.run_if(is_not_paused),
                    tick_game_start_overlay,
                    handle_clamp_screen_locked_icons_worldpos,
                    spawn_shrine_interact_key_guide,
                    add_guide_to_unique_objs,
                    toggle_skills_visibility,
                    toggle_item_chest_visibility.run_if(resource_added::<ItemChestState>()),
                    update_mana_bar,
                    spawn_tile_hover_on_cursor_move,
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_systems(
                (
                    shuffle_items.run_if(in_state(UIState::ItemChest)),
                    handle_skill_reroll_after_flash.run_if(in_state(UIState::Skills)),
                    handle_cursor_reroll_dice_buttons.run_if(in_state(UIState::Skills)),
                    handle_cursor_banish_buttons.run_if(in_state(UIState::Skills)),
                    update_skill_choice_button_states.run_if(in_state(UIState::Skills)),
                    update_skill_choice_count_text.run_if(in_state(UIState::Skills)),
                    handle_cursor_inventory_upgrade_button.run_if(in_state(UIState::Inventory)),
                    setup_furnace_slots_ui.run_if(in_state(UIState::Furnace)),
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                handle_new_ui_state.in_base_set(CoreSet::PostUpdate), // .run_if(in_state(GameState::Main)),
            )
            .add_systems((
                handle_class_selection.run_if(in_state(UIState::ClassSelection)),
                handle_slot_deselection.run_if(in_state(UIState::ClassSelection)),
                update_preview_sprites.run_if(in_state(UIState::ClassSelection)),
                update_slot_visuals.run_if(in_state(UIState::ClassSelection)),
                update_class_option_icons.run_if(in_state(UIState::ClassSelection)),
                update_info_card.run_if(in_state(UIState::ClassSelection)),
                update_unlock_currency_text.run_if(in_state(UIState::ClassSelection)),
                update_class_unlock_panel.run_if(in_state(UIState::ClassSelection)),
                update_class_unlock_confirm_panel.run_if(in_state(UIState::ClassSelection)),
                handle_portal_animation.run_if(in_state(UIState::ClassSelection)),
            ))
            .add_system(init_goal_state.run_if(run_once_per_run()).in_schedule(OnEnter(GameState::Main)))
            .add_system(display_goal_text.run_if(resource_added::<GoalState>()).in_schedule(OnEnter(GameState::Main)))
            .add_systems(
                (
                    handle_goal_state_updates,
                    handle_goal_reset_on_era_change,
                    display_goal_text,
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(handle_hovering.run_if(ui_hover_interactions_condition))
            .add_system(handle_cursor_main_menu_buttons)
            .add_system(update_achievements_notification_icon.run_if(in_state(GameState::MainMenu)));

        app.add_systems(
            (
                debug_trigger_achievement_banner,
                handle_achievement_banner_events,
                update_achievement_banners,
            )
                .in_set(OnUpdate(GameState::Main)),
        );

        app.add_system(update_currency_text.run_if(in_state(GameState::Main)))
            .add_system(update_chaos_ui.run_if(in_state(GameState::Main)))
            .add_system(update_score_text.run_if(resource_changed::<RunScore>()))
            .add_system(player_hud::update_skill_charge_text.run_if(in_state(GameState::Main)))
            .add_system(apply_system_buffers.in_set(CustomFlush));
    }
}

fn ui_hover_interactions_condition(state: Res<State<GameState>>) -> bool {
    state.0 == GameState::Main || state.0 == GameState::MainMenu
}

pub fn handle_new_ui_state(
    mut next_ui_state: ResMut<NextState<UIState>>,
    mut next_client_state: ResMut<NextState<ClientState>>,
    curr_ui_state: Res<State<UIState>>,
    old_ui: Query<(Entity, &UIState), With<UIState>>,
    mut commands: Commands,
    chest_option: Option<Res<ChestContainer>>,
    scrapper_option: Option<Res<ScrapperContainer>>,
    furnace_option: Option<Res<FurnaceContainer>>,
    mut hotbar_slots: Query<(&mut Visibility, &mut InventorySlotState), Without<Interactable>>,
) {
    if next_ui_state.0.is_none() {
        return;
    }
    let next_ui = next_ui_state.0.as_ref().unwrap().clone();

    let mut should_close_self = false;
    let should_reset_crafting_container =
        next_ui != curr_ui_state.0 && curr_ui_state.0.is_inv_open();
    if *DEBUG {
        debug!(
            "UI State Changed: {:?} -> {:?} | should reset: {should_reset_crafting_container:?}",
            curr_ui_state.0, next_ui
        );
    }
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
    if let Some(scrapper) = scrapper_option {
        if let Some(mut scrapper_parent) = commands.get_entity(scrapper.parent) {
            scrapper_parent.insert(scrapper.to_owned());
        }
        if next_ui != UIState::Scrapper {
            commands.remove_resource::<ScrapperContainer>();
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
    if !next_ui.is_inv_open() || should_close_self || should_reset_crafting_container {
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
    info!("{:?}", next_ui);
    if next_ui_state.0.as_ref().unwrap() != &UIState::Closed {
        next_client_state.set(ClientState::Paused);
    } else {
        next_client_state.set(ClientState::Unpaused);
    }
}

/// System that checks for pending level-up rewards when closing menus.
/// If there are pending heirloom choices from level-ups, redirect to the Skills UI instead of closing.
/// This prevents players from accidentally missing their level-up rewards when they level up
/// while in another menu.
pub fn check_pending_levelup_rewards_on_menu_close(
    heirloom_queue: Res<HeirloomChoiceQueue>,
    mut next_inv_state: ResMut<NextState<UIState>>,
) {
    // This system runs when we just entered UIState::Closed (via run conditions)
    // Check if there are pending heirloom choices from level-ups
    if !heirloom_queue.queue.is_empty() {
        info!(
            "Detected {} pending level-up rewards! Redirecting to Skills UI.",
            heirloom_queue.queue.len()
        );
        // Override the transition to Closed - go to Skills instead
        next_inv_state.set(UIState::Skills);
    }
}
