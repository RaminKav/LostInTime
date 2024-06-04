use bevy::{prelude::*, render::view::RenderLayers, sprite::Anchor};

use crate::{
    assets::Graphics,
    attributes::{
        Attack, BonusDamage, CritChance, CritDamage, Defence, Dodge, Healing, HealthRegen,
        ItemAttributes, Lifesteal, LootRateBonus, MaxHealth, Speed, Thorns, XpRateBonus,
    },
    colors::{BLACK, GOLD, GREY, LIGHT_GREEN},
    inventory::ItemStack,
    item::{Recipes, WorldObject},
    player::Player,
    ui::{spawn_item_stack_icon, STATS_UI_SIZE, TOOLTIP_UI_SIZE},
};

use super::{
    stats_ui::StatsUI, EssenceUI, InventoryUI, ShowInvPlayerStatsEvent, UIElement, UIState,
    CHEST_INVENTORY_UI_SIZE, CRAFTING_INVENTORY_UI_SIZE, ESSENCE_UI_SIZE,
    FURNACE_INVENTORY_UI_SIZE, INVENTORY_UI_SIZE,
};
#[derive(Component)]
pub struct PlayerStatsTooltip;
#[derive(Component)]
pub struct ItemOrRecipeTooltip;

#[derive(Component)]
pub struct RecipeIngredientTooltipIcon;

#[derive(Resource, Clone)]
pub struct TooltipsManager {
    pub timer: Timer,
}

#[derive(Debug, Clone)]

pub struct ToolTipUpdateEvent {
    pub item_stack: ItemStack,
    pub is_recipe: bool,
}

#[derive(Default)]
pub struct TooltipTeardownEvent;

pub fn tick_tooltip_timer(time: Res<Time>, mut tooltip_manager: ResMut<TooltipsManager>) {
    if tooltip_manager.timer.finished() {
        return;
    }
    tooltip_manager.timer.tick(time.delta());
}

pub fn handle_tooltip_teardown(
    mut commands: Commands,
    mut updates: EventReader<TooltipTeardownEvent>,
    tooltip: Query<Entity, With<ItemOrRecipeTooltip>>,
    mut tooltip_manager: ResMut<TooltipsManager>,
) {
    if updates.iter().count() > 0 {
        for t in tooltip.iter() {
            commands.entity(t).despawn_recursive();
        }

        // begin delay for next tooltip
        if tooltip.iter().count() > 0 {
            tooltip_manager.timer.reset();
        }
    }
}

pub fn handle_spawn_inv_item_tooltip(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    mut updates: EventReader<ToolTipUpdateEvent>,
    inv: Query<Entity, With<InventoryUI>>,
    essence: Query<Entity, With<EssenceUI>>,
    cur_inv_state: Res<State<UIState>>,
    recipes: Res<Recipes>,
) {
    for item in updates.iter() {
        let parent_inv_size = match cur_inv_state.0 {
            UIState::Inventory => INVENTORY_UI_SIZE,
            UIState::Chest => CHEST_INVENTORY_UI_SIZE,
            UIState::Crafting => CRAFTING_INVENTORY_UI_SIZE,
            UIState::Furnace => FURNACE_INVENTORY_UI_SIZE,
            UIState::Essence => ESSENCE_UI_SIZE,
            _ => unreachable!(),
        };
        let attributes = item.item_stack.attributes.get_tooltips();
        // let durability = item.item_stack.attributes.get_durability_tooltip();
        let level = item.item_stack.metadata.level;
        let should_show_attributes = attributes.len() > 0 && !item.is_recipe;
        let size = Vec2::new(93., 120.5);
        let tooltip = commands
            .spawn((
                SpriteBundle {
                    texture: graphics
                        .get_ui_element_texture(item.item_stack.rarity.get_tooltip_ui_element()),
                    transform: Transform {
                        translation: Vec3::new(-(parent_inv_size.x + size.x + 2.) / 2., 0., 4.),
                        scale: Vec3::new(1., 1., 1.),
                        ..Default::default()
                    },
                    sprite: Sprite {
                        custom_size: Some(size),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                RenderLayers::from_layers(&[3]),
                item.item_stack.rarity.get_tooltip_ui_element(),
                Name::new("TOOLTIP"),
                ItemOrRecipeTooltip,
            ))
            .id();

        let mut tooltip_text: Vec<(String, f32)> = vec![];
        tooltip_text.push((item.item_stack.metadata.name.clone(), 0.));

        if should_show_attributes {
            for a in attributes.iter().clone() {
                tooltip_text.push((a.to_string(), 0.));
            }
            if let Some(level) = level {
                tooltip_text.push((
                    "Level ".to_string() + &level.to_string(),
                    size.y - (tooltip_text.len() + 1) as f32 * 10. - 14.,
                ));
            }
        } else {
            for desc_string in item.item_stack.metadata.desc.iter().clone() {
                tooltip_text.push((desc_string.to_string(), 0.));
            }
        }

        for (i, (text, d)) in tooltip_text.iter().enumerate() {
            let text_pos = if i == 0 {
                Vec3::new(
                    -(f32::ceil((text.chars().count() * 6 - 1) as f32 / 2.)) + 0.5,
                    size.y / 2. - 12.,
                    1.,
                )
            } else {
                Vec3::new(
                    -size.x / 2. + 8. + if item.is_recipe { 4. } else { 0. },
                    size.y / 2. - 14. - (i as f32 * if item.is_recipe { 14. } else { 10. }) - d,
                    1.,
                )
            };
            let text = commands
                .spawn((
                    Text2dBundle {
                        text: Text::from_section(
                            text,
                            TextStyle {
                                font: asset_server.load("fonts/Kitchen Sink.ttf"),
                                font_size: 8.0,
                                color: if i == 0 {
                                    item.item_stack.rarity.get_color()
                                } else if i > 1 && i == tooltip_text.len() - 1 {
                                    GREY
                                } else if i > 2 && should_show_attributes {
                                    GOLD
                                } else {
                                    GREY
                                },
                            },
                        ),
                        text_anchor: Anchor::CenterLeft,
                        transform: Transform {
                            translation: text_pos,
                            scale: Vec3::new(1., 1., 1.),
                            ..Default::default()
                        },
                        ..default()
                    },
                    Name::new("TOOLTIP TEXT"),
                    RenderLayers::from_layers(&[3]),
                ))
                .id();
            if item.is_recipe && i > 0 {
                let ingredient_world_obj: Vec<WorldObject> = recipes
                    .crafting_list
                    .get(&item.item_stack.obj_type)
                    .unwrap()
                    .0
                    .iter()
                    .map(|r| r.item)
                    .collect();
                let icon_e = spawn_item_stack_icon(
                    &mut commands,
                    &graphics,
                    &ItemStack {
                        obj_type: ingredient_world_obj[i - 1],
                        count: 1,
                        ..Default::default()
                    },
                    &asset_server,
                );
                commands
                    .entity(icon_e)
                    .insert(RecipeIngredientTooltipIcon)
                    .insert(Transform {
                        translation: text_pos + Vec3::new(20., 0., 0.),
                        ..Default::default()
                    });
                commands.entity(tooltip).add_child(icon_e);
            }
            commands.entity(tooltip).add_child(text);
        }
        // add tooltip to inventory or essence ui
        //TODO: maybe we dont need to add as parent here and avoid this
        if let Ok(inv) = inv.get_single() {
            commands.entity(inv).add_child(tooltip);
        } else {
            commands
                .entity(essence.get_single().unwrap())
                .add_child(tooltip);
        }
    }
}

pub fn handle_spawn_inv_player_stats(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    mut updates: EventReader<ShowInvPlayerStatsEvent>,
    curr_ui_state: Res<State<UIState>>,
    player_stats: Query<
        (
            &Attack,
            &MaxHealth,
            &Defence,
            &CritChance,
            &CritDamage,
            &BonusDamage,
            &HealthRegen,
            &Healing,
            &Thorns,
            &Dodge,
            &Speed,
            &Lifesteal,
            &XpRateBonus,
            &LootRateBonus,
        ),
        With<Player>,
    >,
    inv: Query<Entity, With<InventoryUI>>,
    stats: Query<Entity, With<StatsUI>>,
    tooltip_manager: Res<TooltipsManager>,
    old_tooltips: Query<Entity, With<PlayerStatsTooltip>>,
) {
    if updates.iter().len() > 0 && tooltip_manager.timer.finished() {
        for t in old_tooltips.iter() {
            commands.entity(t).despawn_recursive();
        }
        let (Ok(parent_e), translation) = (if curr_ui_state.0 == UIState::Inventory {
            (
                inv.get_single(),
                Vec3::new(-(INVENTORY_UI_SIZE.x + TOOLTIP_UI_SIZE.x + 2.) / 2., 0., 2.),
            )
        } else if curr_ui_state.0 == UIState::Stats {
            (
                stats.get_single(),
                Vec3::new(-(STATS_UI_SIZE.x + TOOLTIP_UI_SIZE.x + 2.) / 2., 0., 2.),
            )
        } else {
            return;
        }) else {
            return;
        };
        let (
            attack,
            max_health,
            defence,
            crit_chance,
            crit_damage,
            bonus_damage,
            health_regen,
            healing,
            thorns,
            dodge,
            speed,
            lifesteal,
            xp_rate_bonus,
            loot_rate_bonus,
        ) = player_stats.single();
        let attributes = ItemAttributes {
            attack: attack.0,
            health: max_health.0,
            defence: defence.0,
            crit_chance: crit_chance.0,
            crit_damage: crit_damage.0,
            bonus_damage: bonus_damage.0,
            health_regen: health_regen.0,
            healing: healing.0,
            thorns: thorns.0,
            dodge: dodge.0,
            speed: speed.0,
            lifesteal: lifesteal.0,
            xp_rate: xp_rate_bonus.0,
            loot_rate: loot_rate_bonus.0,
            ..default()
        }
        .get_stats_summary();

        let tooltip = commands
            .spawn((
                SpriteBundle {
                    texture: graphics.get_ui_element_texture(UIElement::LargeTooltipCommon),
                    transform: Transform {
                        translation,
                        scale: Vec3::new(1., 1., 1.),
                        ..Default::default()
                    },
                    sprite: Sprite {
                        custom_size: Some(TOOLTIP_UI_SIZE),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                RenderLayers::from_layers(&[3]),
                UIElement::LargeTooltipCommon,
                PlayerStatsTooltip,
                Name::new("TOOLTIP"),
            ))
            .id();

        let mut tooltip_text: Vec<((String, String), f32)> = vec![];
        tooltip_text.push((("Stats".to_string(), "".to_string()), 0.));
        for (_i, a) in attributes.iter().enumerate().clone() {
            tooltip_text.push(((a.0.clone(), a.1.clone()), 0.));
        }

        for (i, (text, d)) in tooltip_text.iter().enumerate() {
            let text_pos = if i == 0 {
                Vec3::new(
                    -(f32::ceil((text.0.chars().count() * 6 - 1) as f32 / 2.)) + 0.5,
                    TOOLTIP_UI_SIZE.y / 2. - 12.,
                    1.,
                )
            } else {
                Vec3::new(
                    -TOOLTIP_UI_SIZE.x / 2. + 8.,
                    TOOLTIP_UI_SIZE.y / 2. - 12. - (i as f32 * 8.) - d - 2.,
                    1.,
                )
            };
            let text = commands
                .spawn((
                    Text2dBundle {
                        text: Text::from_sections(vec![
                            TextSection {
                                value: text.0.to_string(),
                                style: TextStyle {
                                    font: asset_server.load("fonts/Kitchen Sink.ttf"),
                                    font_size: 8.0,
                                    color: if i == 0 { BLACK } else { GREY },
                                },
                            },
                            TextSection {
                                value: text.1.to_string(),
                                style: TextStyle {
                                    font: asset_server.load("fonts/Kitchen Sink.ttf"),
                                    font_size: 8.0,
                                    color: LIGHT_GREEN,
                                },
                            },
                        ]),
                        text_anchor: Anchor::CenterLeft,
                        transform: Transform {
                            translation: text_pos,
                            scale: Vec3::new(1., 1., 1.),
                            ..Default::default()
                        },
                        ..default()
                    },
                    Name::new("TOOLTIP TEXT"),
                    RenderLayers::from_layers(&[3]),
                ))
                .id();
            commands.entity(tooltip).add_child(text);
        }
        commands.entity(parent_e).add_child(tooltip);
    }
}
