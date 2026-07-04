use crate::{
    assets::Graphics,
    audio::{AudioSoundEffect, SoundSpawner},
    client::ClientState,
    colors::{DARK_WOOD_BROWN, WHITE},
    cursor::CursorPos,
    datafiles,
    ui::{
        interactions::Interaction, minimap::IslandMapOpen, ui_helpers, Interactable, UIElement,
        UIState,
    },
    GameState,
};

use bevy::{prelude::*, render::view::RenderLayers, sprite::Anchor};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs::File,
    io::{BufReader, BufWriter},
};

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tip {
    Recipes,
    Pets,
    UpgradingGear,
    InventoryStats,
    Chaos,
    EndlessMode,
    PeacefulPeriod,
    Night,
    /// Any tip type that existed in an older build but was removed still deserializes
    /// here so `game_data.json` keeps loading. Stripped from disk on the next save
    /// (see [`crate::client::GameData::try_from_json_reader`]).
    #[serde(other)]
    Obsolete,
}

impl Tip {
    pub fn get_tip_text(&self) -> &'static str {
        match self {
            Tip::Recipes => "Gather materials to craft\nuseful items right in your\ninventory. Tools, food, bridges,\nand more can aid you in\nyour journey!",
            Tip::Pets => "Pets can assist you in\ncombat and provide buffs and\npassives. They can also use\nyour extra weapons for you to\nfight! Place a weapon in your\npet equipment slot for them!",
            Tip::UpgradingGear => "Gear can be upgraded in the\ninventory. Upgrade tomes\nincrease the item's level\n by 1, and Orbs re-roll\nthe item's attributes!",
            Tip::InventoryStats => "Extra gear can be placed in\nyour inventory. They will grant you\nthe highlighted stat shown in\ntheir tooltip, passively.",
            Tip::Chaos => "The island is getting more\nchaotic as time goes on. Some\nactions and choices can add\nchaos as well! Chaos increases\nthe strength of enemies.",
            Tip::EndlessMode => "Pay attention to the timer.\nDefeat the era boss and return\nto the portal before it runs\nout. If time runs out, the chaos\nwill overcome you...",
            Tip::PeacefulPeriod => "Time left over after defeating\nan era boss grants a peaceful\nperiod, where mobs will not\nspawn! Explore, gather\nresources, get stronger, and\nprepare for the next era!",
            Tip::Night => "Night time on the island is\ndangerous! Mob swarms will\nspawn, so make sure to be\nprepared!",
            Tip::Obsolete => "",
        }
    }
}

pub struct TipEvent {
    pub tip: Tip,
    pub pos: Vec3,
}

#[derive(Component)]
pub struct TipBox {
    pub tip: Tip,
}

#[derive(Component)]
pub struct TipOkButton;

/// Resource to track which tips have been seen by the player
#[derive(Resource, Default, Debug, Clone)]
pub struct SeenTips {
    pub seen: HashSet<Tip>,
}

impl SeenTips {
    pub fn has_seen(&self, tip: &Tip) -> bool {
        self.seen.contains(tip)
    }

    pub fn mark_seen(&mut self, tip: Tip) {
        self.seen.insert(tip);
    }
}

/// Persist seen tips to game_data.json immediately
pub fn persist_seen_tips(seen_tips: &SeenTips) {
    let path = datafiles::game_data();
    let mut game_data = if let Ok(file) = File::open(&path) {
        let reader = BufReader::new(file);
        crate::client::GameData::try_from_json_reader(reader).unwrap_or_default()
    } else {
        crate::client::GameData::default()
    };

    game_data.seen_tips = seen_tips.seen.clone();

    match File::create(&path) {
        Ok(file) => {
            let writer = BufWriter::new(file);
            if let Err(err) = serde_json::to_writer(writer, &game_data) {
                error!("Failed to persist seen tips to game_data.json: {err:?}");
            }
        }
        Err(err) => error!("Failed to create game_data.json while saving seen tips: {err:?}"),
    }
}

pub struct TipPlugin;
impl Plugin for TipPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SeenTips>()
            .add_event::<TipEvent>()
            .add_systems(
                (
                    spawn_tip_handler,
                    handle_tip_ok_button_click,
                    test_tip,
                    handle_tip_box_pause_state,
                )
                    .in_set(OnUpdate(GameState::Main)),
            );
    }
}

pub fn spawn_tip_handler(
    mut events: EventReader<TipEvent>,
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    seen_tips: Res<SeenTips>,
) {
    for event in events.iter() {
        let tip = &event.tip;

        // if seen_tips.has_seen(tip) {
        //     continue;
        // }

        let tip_text = tip.get_tip_text();

        let tipbox = commands
            .spawn(SpriteBundle {
                texture: graphics.get_ui_element_texture(UIElement::TipBox),
                sprite: Sprite {
                    custom_size: Some(Vec2::new(198.5, 102.5)),
                    ..Default::default()
                },
                transform: Transform {
                    translation: event.pos,
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..Default::default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .insert(TipBox { tip: tip.clone() })
            .insert(Name::new(format!("TIP {:?}", tip)))
            .id();

        commands
            .spawn(Text2dBundle {
                text: Text::from_section(
                    "Tip!",
                    TextStyle {
                        font: asset_server.load("fonts/slkscrbold.ttf"),
                        font_size: 8.4,
                        color: DARK_WOOD_BROWN,
                    },
                )
                .with_alignment(TextAlignment::Left),
                text_anchor: Anchor::CenterLeft,
                transform: Transform {
                    translation: Vec3::new(-14., 31., 1.),
                    ..Default::default()
                },
                ..Default::default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .set_parent(tipbox);
        // Spawn tip text
        commands
            .spawn(Text2dBundle {
                text: Text::from_section(
                    tip_text,
                    TextStyle {
                        font: asset_server.load("fonts/slkscr.ttf"),
                        font_size: 8.4,
                        color: DARK_WOOD_BROWN,
                    },
                )
                .with_alignment(TextAlignment::Left),
                text_anchor: Anchor::CenterLeft,
                transform: Transform {
                    translation: Vec3::new(-78., -4., 1.),
                    ..Default::default()
                },
                ..Default::default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .set_parent(tipbox);

        // Spawn OK button
        let ok_button = commands
            .spawn(SpriteBundle {
                texture: graphics.get_ui_element_texture(UIElement::BackButton),
                sprite: Sprite {
                    custom_size: Some(Vec2::new(50., 20.)),
                    ..Default::default()
                },
                transform: Transform {
                    translation: Vec3::new(0., -50., 1.),
                    ..Default::default()
                },
                ..Default::default()
            })
            .insert(Interactable::default())
            .insert(UIElement::BackButton)
            .insert(TipOkButton)
            .insert(crate::ui::focus::OverlayFocusable { index: 0 })
            .insert(RenderLayers::from_layers(&[3]))
            .insert(Name::new("TIP OK BUTTON"))
            .id();

        commands
            .spawn(Text2dBundle {
                text: Text::from_section(
                    "OK",
                    TextStyle {
                        font: asset_server.load("fonts/alagard.ttf"),
                        font_size: 15.0,
                        color: WHITE,
                    },
                ),
                text_anchor: Anchor::Center,
                transform: Transform::from_translation(Vec3::new(0., 0., 1.)),
                ..Default::default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .set_parent(ok_button);

        commands.entity(tipbox).push_children(&[ok_button]);
    }
}

pub fn handle_tip_ok_button_click(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    ui_focus: Res<crate::ui::focus::UiFocus>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut ok_buttons: Query<(Entity, &mut Interactable, &Parent), With<TipOkButton>>,
    tip_boxes: Query<(Entity, &TipBox)>,
    mut commands: Commands,
    graphics: Res<Graphics>,
    mut seen_tips: ResMut<SeenTips>,
) {
    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);

    for (button_entity, mut interactable, parent) in ok_buttons.iter_mut() {
        let is_hit = matches!(hit_test, Some(hit_ent) if hit_ent.0 == button_entity);
        let is_focused = ui_focus.is_focused(button_entity);
        let confirm_pressed =
            (is_hit && left_mouse_pressed) || (is_focused && ui_focus.confirm_just_pressed);

        if is_hit || is_focused {
            match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonHover, 0.05));
                    commands
                        .entity(button_entity)
                        .insert(UIElement::BackButtonHover)
                        .insert(graphics.get_ui_element_texture(UIElement::BackButtonHover));
                }
                Interaction::Hovering => {
                    if confirm_pressed {
                        commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonClick, 0.2));
                        if let Ok((tip_box_entity, tip_box)) = tip_boxes.get(parent.get()) {
                            seen_tips.mark_seen(tip_box.tip.clone());

                            persist_seen_tips(&seen_tips);

                            commands.entity(tip_box_entity).despawn_recursive();
                        }
                    }
                }
                _ => {}
            }
        } else if matches!(interactable.current(), Interaction::Hovering) {
            interactable.change(Interaction::None);
            commands
                .entity(button_entity)
                .insert(UIElement::BackButton)
                .insert(graphics.get_ui_element_texture(UIElement::BackButton));
        }
    }
}

pub fn test_tip(mut events: EventWriter<TipEvent>, mut done: Local<bool>) {
    // if !*done {
    //     *done = true;
    //     events.send(TipEvent {
    //         tip: Tip::Chaos,
    //         pos: Vec3::new(-200., -100., 30.),
    //     });
    //     events.send(TipEvent {
    //         tip: Tip::EndlessMode,
    //         pos: Vec3::new(0., -100., 30.),
    //     });
    //     events.send(TipEvent {
    //         tip: Tip::PeacefulPeriod,
    //         pos: Vec3::new(196., -100., 30.),
    //     });

    //     events.send(TipEvent {
    //         tip: Tip::Night,
    //         pos: Vec3::new(-200., 120., 30.),
    //     });
    //     events.send(TipEvent {
    //         tip: Tip::PinkFlowers,
    //         pos: Vec3::new(0., 120., 30.),
    //     });
    //     events.send(TipEvent {
    //         tip: Tip::InventoryStats,
    //         pos: Vec3::new(196., 120., 30.),
    //     });
    // }
}

/// Sync pause when tips or the island minimap appear or go away without a `UIState` transition.
///
/// `handle_new_ui_state` only runs when `NextState<UIState>` is set, so closing the last tip
/// or toggling only the minimap never reaches that path and would otherwise leave pause wrong.
pub fn handle_tip_box_pause_state(
    tip_boxes: Query<Entity, With<TipBox>>,
    mut next_client_state: ResMut<NextState<ClientState>>,
    curr_ui_state: Res<State<UIState>>,
    curr_client_state: Res<State<ClientState>>,
    next_ui_state: Res<NextState<UIState>>,
    minimap_open: Res<IslandMapOpen>,
) {
    let has_tip_boxes = !tip_boxes.is_empty();
    let ui_is_closed = curr_ui_state.0 == UIState::Closed;
    // `State<UIState>` may still be Closed this frame while input already queued a menu open.
    let pending_opens_ui = next_ui_state
        .0
        .as_ref()
        .map_or(false, |s| *s != UIState::Closed);

    if has_tip_boxes || minimap_open.0 {
        if curr_client_state.0 != ClientState::Paused {
            next_client_state.set(ClientState::Paused);
        }
    } else if ui_is_closed && !pending_opens_ui {
        if curr_client_state.0 != ClientState::Unpaused {
            next_client_state.set(ClientState::Unpaused);
        }
    }
}
