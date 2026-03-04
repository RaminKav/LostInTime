use bevy::{prelude::*, render::view::RenderLayers, sprite::Anchor};

use bevy_proto::prelude::ProtoCommands;

use crate::chaos::ChaosTracker;
use crate::colors::{DARK_WOOD_BROWN, UNCOMMON_GREEN};
use crate::cursor::CursorPos;
use crate::custom_commands::CommandsExt;
use crate::item::ammo::Ammo;
use crate::night::EraTimer;
use crate::proto::proto_param::ProtoParam;
use crate::world::dimension::{DimensionSpawnEvent, Era};
use crate::player::ModifyCurencyEvent;
use crate::GameParam;
use crate::{
    assets::Graphics,
    attributes::{add_item_glows, AttributeChangeEvent},
    inventory::{Inventory, InventoryItemStack, ItemStack},
    item::WorldObject,
    ui::{crafting_ui::UpgradeButton, FurnaceState, CHEST_INVENTORY_UI_SIZE, INVENTORY_UI_SIZE},
    ScreenResolution, GAME_HEIGHT,
};

use super::{
    crafting_ui::CraftingContainer,
    interactions::{Interactable, Interaction},
    options_ui::CheatSettings,
    player_hud::FlashExpBarEvent,
    ui_helpers::spawn_ui_overlay,
    ShowInvPlayerStatsEvent, UIContainersParam, UIElement, CRAFTING_INVENTORY_UI_SIZE,
    FURNACE_INVENTORY_UI_SIZE, UI_SLOT_SIZE,
};

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States, Component)]
pub enum UIState {
    #[default]
    Closed,
    Inventory,
    NPC,
    Chest,
    Skills,
    ItemChest,
    ActiveSkills,
    ActiveSkillShrine,
    MicrowaveShrine,
    Crafting,
    Furnace,
    Essence,
    Unlocks,
    Options,
    Achievements,
    Scrapper,
    ClassSelection,
    EnterName,
    BlessingChoice,
}
impl UIState {
    pub fn is_inv_open(&self) -> bool {
        self == &UIState::Inventory
            || self == &UIState::Chest
            || self == &UIState::Scrapper
            || self == &UIState::Crafting
            || self == &UIState::Furnace
    }
}

#[derive(Component, Default, Clone)]
pub struct InventoryUI;

/// Dev-only button shown in inventory when dev mode is enabled (Options > Dev Mode).
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum DevButtonAction {
    GrantXp,
    GrantMoreXp,
    SpawnChest,
    SpawnTome,
    SpawnOrb,
    TeleportEra2,
    TeleportEra3,
    TriggerEndless,
    AddChaos,
    AddGold,
    DropDungeonKey,
}
#[derive(Component, FromReflect, Reflect, Clone, Debug)]
pub struct InventorySlotState {
    pub slot_index: usize,
    pub item: Option<Entity>,
    pub count: Option<usize>,
    pub obj_type: Option<WorldObject>,
    pub r#type: InventorySlotType,
    pub dirty: bool,
}
#[derive(Resource, Default, Debug)]
pub struct InventoryState {
    pub active_hotbar_slot: usize,
    pub inv_size: Vec2,
    pub furnace_state: FurnaceState,
    pub hotbar_dirty: bool,
}
#[derive(FromReflect, PartialEq, Reflect, Debug, Clone, Copy)]
pub enum InventorySlotType {
    Normal,
    Hotbar,
    Crafting,
    Equipment,
    Accessory,
    Chest,
    Furnace,
    Scrapper,
    Trash,
}
impl InventorySlotType {
    pub fn is_crafting(self) -> bool {
        self == InventorySlotType::Crafting
    }
    pub fn is_furnace(self) -> bool {
        self == InventorySlotType::Furnace
    }
    pub fn is_hotbar(self) -> bool {
        self == InventorySlotType::Hotbar
    }
    pub fn is_equipment(self) -> bool {
        self == InventorySlotType::Equipment
    }
    pub fn is_accessory(self) -> bool {
        self == InventorySlotType::Accessory
    }
    pub fn is_inventory(self) -> bool {
        self == InventorySlotType::Normal
    }
    pub fn is_chest(self) -> bool {
        self == InventorySlotType::Chest
    }
    pub fn is_scrapper(self) -> bool {
        self == InventorySlotType::Scrapper
    }
    pub fn is_trash(self) -> bool {
        self == InventorySlotType::Trash
    }
}
pub fn setup_inv_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    mut inv_state: ResMut<InventoryState>,
    cur_inv_state: Res<State<UIState>>,
    mut stats_event: EventWriter<ShowInvPlayerStatsEvent>,
    resolution: Res<ScreenResolution>,
    asset_server: Res<AssetServer>,
    cheat_settings: Option<Res<CheatSettings>>,
) {
    // Title
    let _upgrade_text = commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                "Upgrade",
                TextStyle {
                    font: asset_server.load("fonts/alagard.ttf"),
                    font_size: 15.0,
                    color: DARK_WOOD_BROWN,
                },
            ),
            text_anchor: Anchor::Center,
            transform: Transform {
                translation: Vec3::new(106., resolution.game_height / 2. - 100., 10.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("CLASS TITLE"))
        .insert(UIState::Inventory)
        .id();

    let (size, texture, pos_offset) = match cur_inv_state.0 {
        UIState::Inventory => (
            INVENTORY_UI_SIZE,
            graphics.get_ui_element_texture(UIElement::Inventory),
            Vec2::new(22., 0.5),
        ),
        UIState::Chest => (
            CHEST_INVENTORY_UI_SIZE,
            graphics.get_ui_element_texture(UIElement::ChestInventory),
            Vec2::new(22.5, 0.),
        ),
        UIState::Scrapper => (
            CHEST_INVENTORY_UI_SIZE,
            graphics.get_ui_element_texture(UIElement::ChestInventory),
            Vec2::new(22.5, 0.),
        ),
        UIState::Crafting => (
            CRAFTING_INVENTORY_UI_SIZE,
            graphics.get_ui_element_texture(UIElement::CraftingInventory),
            Vec2::new(22.5, 0.),
        ),
        UIState::Furnace => (
            FURNACE_INVENTORY_UI_SIZE,
            graphics.get_ui_element_texture(UIElement::FurnaceInventory),
            Vec2::new(22.5, 0.),
        ),
        _ => return,
    };

    spawn_ui_overlay(
        &mut commands,
        Vec2::new(resolution.game_width + 10., GAME_HEIGHT + 20.),
        0.8,
        9.,
    );

    let inv = commands
        .spawn(SpriteBundle {
            texture,
            sprite: Sprite {
                custom_size: Some(size),
                ..Default::default()
            },
            transform: Transform {
                translation: Vec3::new(pos_offset.x, pos_offset.y, 10.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(InventoryUI)
        .insert(cur_inv_state.0.clone())
        .insert(Name::new("INVENTORY"))
        .insert(RenderLayers::from_layers(&[3]))
        .id();

    inv_state.inv_size = size;
    if cur_inv_state.0 != UIState::Scrapper {
        let upgrade_button = commands
            .spawn(SpriteBundle {
                texture: graphics
                    .get_ui_element_texture(UIElement::UpgradeButton)
                    .clone(),
                sprite: Sprite {
                    custom_size: Some(Vec2::new(13., 13.)),
                    ..Default::default()
                },
                transform: Transform {
                    translation: Vec3::new(95., 44., 10.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..Default::default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .insert(UIElement::UpgradeButton)
            .insert(Interactable::default())
            .insert(UIState::Inventory)
            .insert(UpgradeButton)
            .insert(Name::new("UPGRADE BUTTON"))
            .id();
        commands.entity(inv).push_children(&[upgrade_button]);
    }

    // Dev mode buttons (far left of inventory, only when Options > Dev Mode is on)
    let dev_mode = cheat_settings.map(|c| c.dev_mode).unwrap_or(false);
    if cur_inv_state.0 == UIState::Inventory && dev_mode {
        const DEV_BUTTON_WIDTH: f32 = 38.;
        const DEV_BUTTON_HEIGHT: f32 = 11.;
        const DEV_BUTTON_SPACING: f32 = 14.;
        // Left of inventory panel in local space (inv center is 22, 0.5 in world; panel half-width 109)
        let dev_x = -INVENTORY_UI_SIZE.x / 2. - DEV_BUTTON_WIDTH / 2. - 130.;
        let start_y = 48.0f32;
        let labels: [(DevButtonAction, &str); 11] = [
            (DevButtonAction::GrantXp, "+250 xp"),
            (DevButtonAction::GrantMoreXp, "+1000 xp"),
            (DevButtonAction::SpawnChest, "chest"),
            (DevButtonAction::SpawnTome, "tome"),
            (DevButtonAction::SpawnOrb, "orb"),
            (DevButtonAction::TeleportEra2, "era2"),
            (DevButtonAction::TeleportEra3, "era3"),
            (DevButtonAction::TriggerEndless, "endless"),
            (DevButtonAction::AddChaos, "+chaos"),
            (DevButtonAction::AddGold, "+50 gold"),
            (DevButtonAction::DropDungeonKey, "key"),
        ];
        for (i, (action, label)) in labels.iter().enumerate() {
            let y = start_y - i as f32 * DEV_BUTTON_SPACING;
            let btn = commands
                .spawn(SpriteBundle {
                    texture: graphics.get_ui_element_texture(UIElement::XLKey).clone(),
                    sprite: Sprite {
                        custom_size: Some(Vec2::new(DEV_BUTTON_WIDTH, DEV_BUTTON_HEIGHT)),
                        ..Default::default()
                    },
                    transform: Transform::from_xyz(dev_x, y, 10.),
                    ..Default::default()
                })
                .insert(RenderLayers::from_layers(&[3]))
                .insert(UIState::Inventory)
                .insert(Interactable::default())
                .insert(*action)
                .insert(Name::new(format!("Dev Button {:?}", action)))
                .id();
            commands
                .spawn((
                    Text2dBundle {
                        text: Text::from_section(
                            *label,
                            TextStyle {
                                font: asset_server.load("fonts/4x5.ttf"),
                                font_size: 5.0,
                                color: DARK_WOOD_BROWN,
                            },
                        )
                        .with_alignment(TextAlignment::Center),
                        text_anchor: Anchor::Center,
                        transform: Transform::from_xyz(0., 0.5, 1.),
                        ..Default::default()
                    },
                    RenderLayers::from_layers(&[3]),
                    UIState::Inventory,
                    Name::new("Dev Button Label"),
                ))
                .set_parent(btn);
            commands.entity(inv).add_child(btn);
        }
    }

    stats_event.send(ShowInvPlayerStatsEvent {
        stat: None,
        ignore_timer: true,
    });
}

pub fn setup_inv_slots_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    inv_query: Query<Entity, With<InventoryUI>>,
    inv_state_res: Res<InventoryState>,
    inv_state: Res<State<UIState>>,
    inv_spawn_check: Query<Entity, Added<InventoryUI>>,

    asset_server: Res<AssetServer>,
    mut inv: Query<&mut Inventory>,
    crafting_container: Option<Res<CraftingContainer>>,
) {
    if inv_spawn_check.get_single().is_err() {
        return;
    }
    let (should_spawn_equipment, crafting_items_option) = match inv_state.0 {
        UIState::Inventory => (true, Some(inv.single().crafting_items.clone())),
        UIState::Crafting => (true, Some(crafting_container.unwrap().items.clone())),
        UIState::Chest => (false, None),
        UIState::Scrapper => (false, None),
        UIState::Furnace => (true, None),
        _ => return,
    };
    for (slot_index, item) in inv.single_mut().items.items.iter().enumerate() {
        spawn_inv_slot(
            &mut commands,
            &inv_state,
            &graphics,
            slot_index,
            Interaction::None,
            &inv_state_res,
            &inv_query,
            &asset_server,
            InventorySlotType::Normal,
            item.clone(),
        );

        // equipment slots
        if slot_index < 4 && should_spawn_equipment {
            spawn_inv_slot(
                &mut commands,
                &inv_state,
                &graphics,
                slot_index,
                Interaction::None,
                &inv_state_res,
                &inv_query,
                &asset_server,
                InventorySlotType::Equipment,
                None,
            );
        }
        // accessoyr slots
        if slot_index < 3 && should_spawn_equipment {
            spawn_inv_slot(
                &mut commands,
                &inv_state,
                &graphics,
                slot_index,
                Interaction::None,
                &inv_state_res,
                &inv_query,
                &asset_server,
                InventorySlotType::Accessory,
                None,
            );
        }
    }
    if let Some(crafting_items) = crafting_items_option {
        for (slot_index, item) in crafting_items.items.iter().enumerate() {
            spawn_inv_slot(
                &mut commands,
                &inv_state,
                &graphics,
                slot_index,
                Interaction::None,
                &inv_state_res,
                &inv_query,
                &asset_server,
                InventorySlotType::Crafting,
                item.to_owned(),
            );
        }
    }
    if inv_state.0 != UIState::Scrapper {
        if let Some(furnace_items) = inv.single_mut().furnace_items.clone().into() {
            for (slot_index, item) in furnace_items.items.iter().enumerate() {
                spawn_inv_slot(
                    &mut commands,
                    &inv_state,
                    &graphics,
                    slot_index,
                    Interaction::None,
                    &inv_state_res,
                    &inv_query,
                    &asset_server,
                    InventorySlotType::Furnace,
                    item.to_owned(),
                );
            }
        }
        // Spawn trash slot (only in regular inventory, not scrapper)
        if inv_state.0 == UIState::Inventory || inv_state.0 == UIState::Crafting {
            let trash_item = inv
                .single_mut()
                .trash_items
                .items
                .get(0)
                .and_then(|x| x.clone());
            spawn_inv_slot(
                &mut commands,
                &inv_state,
                &graphics,
                0,
                Interaction::None,
                &inv_state_res,
                &inv_query,
                &asset_server,
                InventorySlotType::Trash,
                trash_item,
            );
        }
    }
}

pub fn spawn_inv_slot(
    commands: &mut Commands,
    inv_ui_state: &Res<State<UIState>>,
    graphics: &Graphics,
    slot_index: usize,
    interactable_state: Interaction,
    inv_state: &InventoryState,
    inv_query: &Query<Entity, With<InventoryUI>>,
    asset_server: &AssetServer,
    slot_type: InventorySlotType,
    item_stack: Option<InventoryItemStack>,
) -> Entity {
    // spawns an inv slot, with an item icon as its child if an item exists in that inv slot.
    // the slot's parent is set to the inv ui entity.
    let inv_slot_offset = match inv_ui_state.0 {
        UIState::Chest => Vec2::new(0., 0.),
        UIState::Crafting => Vec2::new(0., -4.),
        _ => Vec2::new(0., 0.),
    };

    let mut x = ((slot_index % 6) as f32 * UI_SLOT_SIZE) - (inv_state.inv_size.x) / 2.
        + UI_SLOT_SIZE / 2.
        + 4.;
    let mut y = ((slot_index / 6) as f32).trunc() * UI_SLOT_SIZE - (inv_state.inv_size.y) / 2.
        + 7.
        + UI_SLOT_SIZE / 2.;

    if slot_type.is_hotbar() {
        y = -GAME_HEIGHT / 2. + 14.;
        x = ((slot_index % 6) as f32 * UI_SLOT_SIZE) - 2. * UI_SLOT_SIZE;
    } else if slot_type.is_crafting() {
        x = ((slot_index % 8) as f32 * UI_SLOT_SIZE) - (inv_state.inv_size.x) / 2.
            + UI_SLOT_SIZE / 2.
            + 6.;

        y = -((slot_index / 8) as f32).trunc() * (UI_SLOT_SIZE + 1.) - (inv_state.inv_size.y) / 2.
            + 7. * UI_SLOT_SIZE
            + 16.;
        if inv_ui_state.0 == UIState::Inventory {
            x -= 2.;
            y -= 29.;
        }
    } else if slot_type.is_equipment() {
        x = UI_SLOT_SIZE - (inv_state.inv_size.x) / 2. + UI_SLOT_SIZE / 2. + 7. + 5. * UI_SLOT_SIZE;
        y = slot_index as f32 * UI_SLOT_SIZE - (inv_state.inv_size.y + UI_SLOT_SIZE) / 2.
            + UI_SLOT_SIZE
            + 1. * slot_index as f32
            + 4.;
    } else if slot_type.is_accessory() {
        x = UI_SLOT_SIZE - (inv_state.inv_size.x) / 2. + UI_SLOT_SIZE / 2. + 8. + 6. * UI_SLOT_SIZE;
        y = slot_index as f32 * UI_SLOT_SIZE - (inv_state.inv_size.y + UI_SLOT_SIZE) / 2.
            + UI_SLOT_SIZE
            + 1. * slot_index as f32
            + 4.;
    } else if slot_type.is_chest() || slot_type.is_scrapper() {
        y += 4. * UI_SLOT_SIZE + 11.;
    } else if slot_type.is_furnace() {
        if slot_index == 0 {
            x = 76.;
            y = 33.5;
        } else if slot_index == 1 {
            x = 76.;
            y = 54.5;
        }
    } else if slot_type.is_trash() {
        x = UI_SLOT_SIZE - (inv_state.inv_size.x) / 2. + UI_SLOT_SIZE / 2. + 4.;
        y = -inv_state.inv_size.y / 2. - UI_SLOT_SIZE / 2. - 2.;
    } else if ((slot_index / 6) as f32).trunc() == 0. {
        y -= 3.;
    }
    let translation = Vec3::new(x, y, 1.) + inv_slot_offset.extend(0.);
    let mut item_icon_option = None;
    let mut item_type_option = None;
    let mut item_count_option = None;
    // check if we need to spawn an item icon for this slot
    if let Some(item) = item_stack {
        // player has item in this slot

        let obj_type = *item.get_obj();
        item_type_option = Some(obj_type);
        item_count_option = Some(item.item_stack.count);
        item_icon_option = Some(spawn_item_stack_icon(
            commands,
            graphics,
            &item.item_stack,
            asset_server,
            Vec2::ZERO,
            if slot_index == 5 || slot_index == 2 {
                Vec2::new(0.5, 0.)
            } else {
                Vec2::ZERO
            },
            3,
        ));
    }

    // Inv Slot Icon //
    let mut slot_icon = match slot_type {
        InventorySlotType::Equipment => match slot_index {
            3 => Some("ui/icons/CapeSlotIcon.png"),
            2 => Some("ui/icons/ChestSlotIcon.png"),
            1 => Some("ui/icons/PantsSlotIcon.png"),
            0 => Some("ui/icons/ShoesSlotIcon.png"),
            _ => None,
        },
        InventorySlotType::Accessory => match slot_index {
            2 => Some("ui/icons/RingSlotIcon.png"),
            1 => Some("ui/icons/RingSlotIcon.png"),
            0 => Some("ui/icons/NecklaceSlotIcon.png"),
            _ => None,
        },
        InventorySlotType::Trash => Some("ui/icons/TrashSlotIcon.png"),
        InventorySlotType::Furnace => match slot_index {
            1 => Some("ui/icons/SwordSlotIcon.png"),
            0 => Some("ui/icons/OrbSlotIcon.png"),
            _ => None,
        },
        _ => None,
    };

    if item_icon_option.is_some() {
        slot_icon = None;
    }

    let icon_entity_option = slot_icon.map(|slot_icon| {
        commands
            .spawn(SpriteBundle {
                texture: asset_server.load(slot_icon),
                transform: Transform {
                    translation: Vec3::new(0., 0., 1.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                sprite: Sprite {
                    custom_size: Some(Vec2::new(16., 16.)),
                    ..Default::default()
                },
                ..Default::default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .id()
    });

    let mut slot_entity = commands.spawn(SpriteBundle {
        texture: graphics.get_ui_element_texture(
            if slot_type.is_hotbar() && inv_state.active_hotbar_slot == slot_index {
                UIElement::InventorySlotHover
            } else if slot_type.is_hotbar() {
                UIElement::InventorySlotHotbar
            } else {
                UIElement::InventorySlot
            },
        ),

        transform: Transform {
            translation,
            scale: Vec3::new(1., 1., 1.),
            ..Default::default()
        },
        sprite: Sprite {
            custom_size: Some(Vec2::new(UI_SLOT_SIZE, UI_SLOT_SIZE)),
            ..Default::default()
        },
        ..Default::default()
    });
    slot_entity
        .insert(RenderLayers::from_layers(&[3]))
        .insert(InventorySlotState {
            slot_index,
            item: item_icon_option,
            obj_type: item_type_option,
            count: item_count_option,
            dirty: false,
            r#type: slot_type,
        })
        .insert(UIElement::InventorySlot)
        .insert(Name::new(if slot_type.is_crafting() {
            "CRAFTING SLOT"
        } else {
            "SLOT"
        }));
    if let Some(i) = item_icon_option {
        slot_entity.push_children(&[i]);
    }
    if !slot_type.is_hotbar() {
        let inv_e = inv_query.single();
        slot_entity
            .set_parent(inv_e)
            .insert(Interactable::from_state(interactable_state));
    } else {
        // Hotbar slots also get Interactable for click detection when inventory is closed
        slot_entity.insert(Interactable::from_state(interactable_state));
    }

    if let Some(icon_entity) = icon_entity_option {
        slot_entity.push_children(&[icon_entity]);
    }
    let id = slot_entity.id();
    // Attach an ammo bar to the active hotbar slot (UI only shows for active weapon)
    if slot_type.is_hotbar() && inv_state.active_hotbar_slot == slot_index {
        commands
            .spawn(SpriteBundle {
                sprite: Sprite {
                    color: UNCOMMON_GREEN,
                    custom_size: Some(Vec2::new(0., 1.)),
                    anchor: Anchor::CenterLeft,
                    ..Default::default()
                },
                transform: Transform {
                    translation: Vec3::new(-6., -6.5, 4.),
                    ..Default::default()
                },
                ..Default::default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .insert(Name::new("AMMO BAR"))
            .insert(AmmoBarHotbar)
            .set_parent(id);
    }

    id
}

#[derive(Component)]
pub struct AmmoBarHotbar;

pub fn update_hotbar_ammo_bar(
    game: GameParam,
    mut bars: Query<&mut Sprite, With<AmmoBarHotbar>>,
    ammo_query: Query<&Ammo>,
) {
    if bars.is_empty() {
        return;
    }
    if let Some(main_hand) = game.player().main_hand_slot.clone() {
        if let Ok(ammo) = ammo_query.get(main_hand.entity) {
            let percent = if ammo.reloading {
                ammo.reload.percent()
            } else if ammo.max > 0 {
                ammo.current as f32 / ammo.max as f32
            } else {
                0.
            };
            for mut sprite in bars.iter_mut() {
                sprite.custom_size = Some(Vec2::new(12. * percent.clamp(0.0, 1.0), 1.));
            }
        } else {
            for mut sprite in bars.iter_mut() {
                sprite.custom_size = Some(Vec2::new(0., 1.));
            }
        }
    }
}
pub fn spawn_item_stack_icon(
    commands: &mut Commands,
    graphics: &Graphics,
    item_stack: &ItemStack,
    asset_server: &AssetServer,
    icon_offset: Vec2,
    text_offset: Vec2,
    render_layer: u8,
) -> Entity {
    let has_icon = graphics.icons.as_ref().unwrap().get(&item_stack.obj_type);
    let sprite = if let Some(icon) = has_icon {
        icon.clone()
    } else {
        graphics
            .spritesheet_map
            .as_ref()
            .unwrap()
            .get(&item_stack.obj_type)
            .unwrap_or_else(|| panic!("No graphic for object {:?}", item_stack.obj_type))
            .clone()
    };
    let item_entity = commands
        .spawn(SpriteSheetBundle {
            sprite,
            texture_atlas: graphics.texture_atlas.as_ref().unwrap().clone(),
            transform: Transform {
                translation: Vec3::new(icon_offset.x, icon_offset.y, 2.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(item_stack.clone())
        .insert(RenderLayers::from_layers(&[render_layer]))
        .id();
    // glow effect for rarity
    if let Some(glow_e) = add_item_glows(commands, graphics, item_entity, item_stack.rarity.clone())
    {
        commands
            .entity(glow_e)
            .insert(RenderLayers::from_layers(&[3]));
    }

    if item_stack.count > 1 {
        let text = commands
            .spawn((
                Text2dBundle {
                    text: Text::from_section(
                        item_stack.count.to_string(),
                        TextStyle {
                            font: asset_server.load("fonts/4x5.ttf"),
                            font_size: 5.0,
                            color: Color::WHITE,
                        },
                    )
                    .with_alignment(TextAlignment::Center),
                    transform: Transform {
                        translation: Vec3::new(7., -5.5, 3.) + text_offset.extend(0.),
                        scale: Vec3::new(1., 1., 1.),
                        ..Default::default()
                    },
                    ..default()
                },
                Name::new("ITEM STACK TEXT"),
                RenderLayers::from_layers(&[render_layer]),
            ))
            .id();
        commands.entity(item_entity).push_children(&[text]);
    }
    item_entity
}
//TODO: make event?
pub fn change_hotbar_slot(
    slot: usize,
    inv_state: &mut InventoryState,
    inv_slots: &mut Query<&mut InventorySlotState>,
) {
    mark_slot_dirty(
        inv_state.active_hotbar_slot,
        InventorySlotType::Hotbar,
        inv_slots,
    );
    inv_state.active_hotbar_slot = slot;
    mark_slot_dirty(slot, InventorySlotType::Hotbar, inv_slots);
}
pub fn update_inventory_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    mut ui_elements: Query<(Entity, &mut InventorySlotState)>,
    interactables: Query<&Interactable>,
    inv_ui_state: Res<State<UIState>>,
    inv_query: Query<Entity, With<InventoryUI>>,
    asset_server: Res<AssetServer>,
    inv: Query<&mut Inventory>,
    cont_param: UIContainersParam,
) {
    for (e, mut slot_state) in ui_elements.iter_mut() {
        // check current inventory state against that slot's state
        // if they do not match, delete and respawn

        // hotbars are hidden when inventory is open, so defer update
        // until inv is closed again. But still mark dirty if count changed
        // so they update properly when inv closes.
        if inv_ui_state.0.is_inv_open() && slot_state.r#type.is_hotbar() {
            // Check if hotbar slot needs updating and mark dirty for later
            let hotbar_item = inv
                .single()
                .get_items_from_slot_type(slot_state.r#type)
                .items[slot_state.slot_index]
                .clone();
            let real_count = hotbar_item.as_ref().map(|i| i.item_stack.count);
            if slot_state.count != real_count {
                slot_state.dirty = true;
            }
            continue;
        }

        let interactable_option = interactables.get(e);
        let item_option = if slot_state.r#type.is_chest() {
            cont_param.chest_option.as_ref().unwrap().items.items[slot_state.slot_index].clone()
        } else if slot_state.r#type.is_scrapper() {
            cont_param.scrapper_option.as_ref().unwrap().items.items[slot_state.slot_index].clone()
        } else if slot_state.r#type.is_furnace() {
            inv.single()
                .get_items_from_slot_type(slot_state.r#type)
                .items[slot_state.slot_index]
                .clone()
        } else if slot_state.r#type.is_trash() {
            inv.single()
                .get_items_from_slot_type(slot_state.r#type)
                .items[slot_state.slot_index]
                .clone()
        } else if slot_state.r#type.is_crafting() && cont_param.crafting_option.is_some() {
            cont_param.crafting_option.as_ref().unwrap().items.items[slot_state.slot_index].clone()
        } else {
            inv.single()
                .get_items_from_slot_type(slot_state.r#type)
                .items[slot_state.slot_index]
                .clone()
        };
        let real_count = if let Some(item) = item_option.clone() {
            Some(item.item_stack.count)
        } else {
            None
        };

        if slot_state.dirty || slot_state.count != real_count {
            commands.entity(e).despawn_recursive();
            spawn_inv_slot(
                &mut commands,
                &inv_ui_state,
                &graphics,
                slot_state.slot_index,
                if let Ok(i) = interactable_option {
                    i.current().clone()
                } else {
                    Interaction::None
                },
                &cont_param.inv_state,
                &inv_query,
                &asset_server,
                slot_state.r#type,
                item_option.clone(),
            );
        }
    }
}
/// when items in the inventory state change, update the matching entities in the UI
pub fn handle_update_inv_item_entities(
    mut inv: Query<&mut Inventory, Changed<Inventory>>,
    mut inv_slot_state: Query<&mut InventorySlotState>,
    mut att_event: EventWriter<AttributeChangeEvent>,
    mut commands: Commands,
    ui_state: Res<State<UIState>>,
) {
    if !ui_state.0.is_inv_open() {
        return;
    }
    if let Ok(inv) = inv.get_single_mut() {
        att_event.send(AttributeChangeEvent);
        for inv_item_option in inv.clone().items.items.iter() {
            if let Some(inv_item) = inv_item_option {
                let item = inv_item.item_stack.clone();
                for slot_state in inv_slot_state.iter_mut() {
                    if slot_state.slot_index == inv_item.slot
                        && (slot_state.r#type.is_inventory() || slot_state.r#type.is_hotbar())
                    {
                        if let Some(item_e) = slot_state.item {
                            commands.entity(item_e).insert(item.clone());
                        }
                    }
                }
            }
        }
    }
}

pub fn mark_slot_dirty(
    slot_index: usize,
    slot_type: InventorySlotType,
    inv_slots: &mut Query<&mut InventorySlotState>,
) {
    for mut state in inv_slots.iter_mut() {
        if state.slot_index == slot_index && (state.r#type == slot_type || state.r#type.is_hotbar())
        {
            state.dirty = true;
        }
    }
}

/// Handles clicks on dev mode buttons (only runs when inventory is open and dev mode is on).
pub fn handle_dev_button_clicks(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut dev_buttons: Query<(Entity, &DevButtonAction, &mut Interactable)>,
    mut commands: Commands,
    mut flash_event: EventWriter<FlashExpBarEvent>,
    mut game: GameParam,
    mut proto_commands: ProtoCommands,
    proto: ProtoParam,
    mut dimension_spawn: EventWriter<DimensionSpawnEvent>,
    mut era_timer: ResMut<EraTimer>,
    mut chaos_tracker: ResMut<ChaosTracker>,
    mut currency_event: EventWriter<ModifyCurencyEvent>,
) {
    let hit_test = super::ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);

    for (e, action, mut interactable) in dev_buttons.iter_mut() {
        let hit = match &hit_test {
            Some((hit_ent, _, _)) => *hit_ent == e,
            None => false,
        };
        if hit {
            interactable.change(Interaction::Hovering);
            if left_mouse_pressed {
                let player_pos = game.player().position.truncate();
                let spawn_pos = player_pos + Vec2::new(0., -20.);

                match action {
                    DevButtonAction::GrantXp => {
                        let player_skills = game.get_player_skills();
                        let mut player_level = game.get_player_level_mut();
                        let did_level =
                            player_level.add_xp(250, &player_skills, &mut chaos_tracker);
                        flash_event.send(FlashExpBarEvent {
                            amount: 250,
                            did_level,
                        });
                    }
                    DevButtonAction::GrantMoreXp => {
                        let player_skills = game.get_player_skills();
                        let mut player_level = game.get_player_level_mut();
                        let did_level =
                            player_level.add_xp(1000, &player_skills, &mut chaos_tracker);
                        flash_event.send(FlashExpBarEvent {
                            amount: 1000,
                            did_level,
                        });
                    }
                    DevButtonAction::SpawnChest => {
                        let _ = proto_commands.spawn_item_from_proto(
                            WorldObject::ChestBlock,
                            &proto,
                            spawn_pos,
                            1,
                            None,
                        );
                    }
                    DevButtonAction::SpawnTome => {
                        let _ = proto_commands.spawn_item_from_proto(
                            WorldObject::UpgradeTome,
                            &proto,
                            spawn_pos,
                            1,
                            None,
                        );
                    }
                    DevButtonAction::SpawnOrb => {
                        let _ = proto_commands.spawn_item_from_proto(
                            WorldObject::OrbOfTransformation,
                            &proto,
                            spawn_pos,
                            1,
                            None,
                        );
                    }
                    DevButtonAction::TeleportEra2 => {
                        dimension_spawn.send(DimensionSpawnEvent {
                            swap_to_dim_now: true,
                            new_era: Some(Era::Second),
                        });
                    }
                    DevButtonAction::TeleportEra3 => {
                        dimension_spawn.send(DimensionSpawnEvent {
                            swap_to_dim_now: true,
                            new_era: Some(Era::Third),
                        });
                    }
                    DevButtonAction::TriggerEndless => {
                        era_timer.remaining_seconds = 5.0;
                    }
                    DevButtonAction::AddChaos => {
                        chaos_tracker.add_chaos(1.0);
                    }
                    DevButtonAction::AddGold => {
                        currency_event.send(ModifyCurencyEvent {
                            delta: 50,
                            obj: WorldObject::Coin,
                        });
                    }
                    DevButtonAction::DropDungeonKey => {
                        let _ = proto_commands.spawn_item_from_proto(
                            WorldObject::Key,
                            &proto,
                            spawn_pos,
                            1,
                            None,
                        );
                    }
                }
                commands.spawn(crate::audio::SoundSpawner::new(
                    crate::audio::AudioSoundEffect::ButtonClick,
                    0.2,
                ));
            }
        } else if matches!(interactable.current(), Interaction::Hovering) {
            interactable.change(Interaction::None);
        }
    }
}
