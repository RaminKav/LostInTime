use crate::{
    assets::Graphics,
    item::WorldObject,
    systems::{Interactable, LastHoveredSlot},
    InventoryItemStack,
};
use bevy::prelude::*;
use bevy_rapier2d::{prelude::Collider, rapier::prelude::NarrowPhase};
use kayak_ui::prelude::{widgets::*, KStyle, *};

#[derive(Component, Debug, Default, Clone, PartialEq)]
pub struct InventorySlot {
    pub item: Option<InventoryItemStack>,
    pub slot: usize,
}

pub fn render_inventory_slot(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    slot_query: Query<&InventorySlot>,
    graphics: Res<Graphics>,
) -> bool {
    let state_entity = widget_context.use_state(
        &mut commands,
        inv_slot_entity,
        InventorySlotState {
            hovering: false,
            dragging: false,
            offset: Vec2::ZERO,
            position: Vec2::ZERO,
        },
    );
    // convert images to imageHandles
    let slot_item = slot_query.get(inv_slot_entity).unwrap().item;
    let mut item_handle = None;
    if let Some(item) = slot_item {
        item_handle = Some(
            graphics
                .world_obj_image_handles
                .as_ref()
                .unwrap()
                .get(&item.item_stack.0)
                .unwrap_or_else(|| panic!("No graphic for object Flint"))
                .clone(),
        );
    }

    let slot_image: Handle<Image> = asset_server.load("ui/inventory_slot.png");
    let slot_image_hover = asset_server.load("ui/inventory_slot_hover.png");

    let on_event = OnEvent::new(
        move |In((mut event_dispatcher_context, _, mut event, entity)): In<(
            EventDispatcherContext,
            WidgetState,
            Event,
            Entity,
        )>,
              mut state_query: Query<&mut InventorySlotState>,
              slot_query: Query<&InventorySlot>,
              mut last_hovered_item: ResMut<LastHoveredSlot>| {
            if let Ok(mut slot_state) = state_query.get_mut(state_entity) {
                event.stop_propagation();
                match event.event_type {
                    EventType::MouseIn(..) => {
                        slot_state.hovering = true;
                        if let Ok(slot) = slot_query.get(inv_slot_entity) {
                            println!("UPDATED LAST HOVERED SLOT: {:?}", slot.slot);
                            last_hovered_item.slot = Some(slot.slot)
                        }
                    }
                    EventType::MouseOut(..) => {
                        slot_state.hovering = false;
                        last_hovered_item.slot = None
                    }
                    EventType::MouseDown(data) => {
                        slot_state.dragging = true;
                        //Set up system in drag_Ssytem to read init cursor pos, and apply an offset based on
                        //the diff of the current cursor pos. Then update position

                        // slot_state.offset = Vec2::new(
                        //     slot_state.position.x - data.position.0,
                        //     slot_state.position.y - data.position.1,
                        // );
                        if let Ok(slot) = slot_query.get(inv_slot_entity) {
                            last_hovered_item.slot = Some(slot.slot);
                            println!("DRAG START: {:?} {:?}", slot_state.offset, slot);
                        }
                    }
                    EventType::MouseUp(..) => {
                        if let Ok(slot) = slot_query.get(inv_slot_entity) {
                            last_hovered_item.slot = Some(slot.slot);
                            println!("DRAG STOP: {:?} {:?}", slot_state, slot);
                        }
                    }
                    // EventType::Hover(data) => {
                    //     if slot_state.dragging {
                    //         slot_state.position = Vec2::new(
                    //             slot_state.offset.x + data.position.0,
                    //             slot_state.offset.y + data.position.1,
                    //         );
                    //         if let Ok(slot) = slot_query.get(inv_slot_entity) {
                    //             println!("DRAGGING...: {:?} {:?}", slot_state.position, slot);
                    //         }
                    //     } else {
                    //         event_dispatcher_context.release_cursor(entity);
                    //         slot_state.hovering = true;
                    //         if let Ok(slot) = slot_query.get(inv_slot_entity) {
                    //             println!("2 UPDATED LAST HOVERED SLOT: {:?}", slot.slot);
                    //             last_hovered_slot.slot_index = Some(slot.slot)
                    //         }
                    //     }
                    // }
                    _ => {}
                }
            }
            (event_dispatcher_context, event)
        },
    );
    if let Ok(slot_state) = state_query.get(state_entity) {
        let slot_image_handle = if slot_state.hovering {
            slot_image_hover
        } else {
            slot_image
        };
        let parent_id = Some(inv_slot_entity);
        rsx! {
            <ElementBundle on_event={on_event.clone()}
            styles={KStyle {
                z_index: StyleProp::Value(0),
                position_type: StyleProp::Value(KPositionType::SelfDirected),
                width: Units::Stretch(1.0).into(),
                height: Units::Stretch(1.0).into(),
                ..KStyle::default()
            }}>
                <KImageBundle
                    image={KImage(slot_image_handle)}
                    styles={KStyle {
                        z_index: StyleProp::Value(0),
                        position_type: StyleProp::Value(KPositionType::SelfDirected),
                        width: Units::Stretch(1.0).into(),
                        height: Units::Stretch(1.0).into(),
                        ..KStyle::default()
                    }}
                />
                {if let Some(item) = item_handle.clone() {
                    constructor!{
                        <KImageBundle
                            image={KImage(item)}
                            styles={KStyle {
                                z_index: StyleProp::Value(1),
                                position_type: StyleProp::Value(KPositionType::SelfDirected),
                                width: Units::Percentage(60.).into(),
                                height: Units::Percentage(60.).into(),
                                left: Units::Stretch(0.5).into(),
                                right: Units::Stretch(0.5).into(),
                                bottom: Units::Stretch(0.5).into(),
                                top: Units::Stretch(0.5).into(),
                                ..KStyle::default()
                            }}
                        />
                    }
                    constructor!{
                        <TextWidgetBundle
                            styles={KStyle {
                                z_index: StyleProp::Value(2),
                                font_size: StyleProp::Value(60.),
                                top: StyleProp::Value(Units::Percentage(60.)),
                                left: StyleProp::Value(Units::Percentage(62.)),
                                ..Default::default()
                            }}
                            text={TextProps {
                                content: slot_item.unwrap().item_stack.1.to_string(),
                                ..Default::default()
                            }}
                        />
                    }

                    // {if slot_state.hovering {
                    //     constructor! {
                    //         <BackgroundBundle styles={KStyle {
                    //             background_color: Color::rgba(1., 0.0, 0.0, 1.).into(),
                    //             position_type: StyleProp::Value(KPositionType::SelfDirected),
                    //             width: Units::Pixels(600.0).into(),
                    //             height: Units::Pixels(250.0).into(),
                    //             top: Units::Pixels(-250.).into(),
                    //             ..KStyle::default()}} >
                    //             {if let Some(item) = item_handle {
                    //                 constructor! {
                    //                     <ElementBundle styles={KStyle{layout_type: StyleProp::Value(LayoutType::Row),..default()}}>
                    //                         <KImageBundle
                    //                             image={KImage(item)}
                    //                             styles={KStyle {
                    //                                 width: Units::Pixels(200.0).into(),
                    //                                 height: Units::Pixels(200.0).into(),
                    //                                 ..KStyle::default()
                    //                             }}
                    //                         />
                    //                         <TextWidgetBundle
                    //                         styles={KStyle {
                    //                             font_size: StyleProp::Value(200.),
                    //                             ..Default::default()
                    //                         }}
                    //                         text={TextProps {
                    //                             content: if let Some(i) = slot_item {i.item_stack.1.to_string()} else {0.to_string()},
                    //                             ..Default::default()
                    //                         }}
                    //                         />
                    //                     </ElementBundle>
                    //                 }
                    //             }
                    //             }
                    //             </BackgroundBundle>
                    //     }}
                    // }
                }}

            </ElementBundle>

        };
    }
    true
}
