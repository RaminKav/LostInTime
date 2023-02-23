use crate::{
    inventory::INVENTORY_SIZE,
    systems::Interactable,
    ui::{InventorySlot, InventorySlotBundle},
    InventoryItemStack,
};
use bevy::prelude::*;
use kayak_ui::prelude::{widgets::*, KStyle, *};

#[derive(Component, Debug, Default, Clone, PartialEq)]
pub struct Inventory {
    pub open: bool,
    pub items: [Option<InventoryItemStack>; INVENTORY_SIZE],
}
impl Widget for Inventory {}
#[derive(Component, Default, Debug, Clone, PartialEq, Eq)]
pub struct InventoryState {
    pub open: bool,
}

#[derive(Bundle)]
pub struct InventoryBundle {
    pub inventory: Inventory,
    pub styles: KStyle,
    pub widget_name: WidgetName,
}

impl Default for InventoryBundle {
    fn default() -> Self {
        Self {
            inventory: Default::default(),
            styles: KStyle {
                render_command: StyleProp::Value(RenderCommand::Quad),
                ..KStyle::default()
            },
            widget_name: Inventory::default().get_name(),
        }
    }
}

pub fn render_inventory(
    In((widget_context, inv_entity)): In<(KayakWidgetContext, Entity)>,
    mut commands: Commands,
    props_query: Query<&Inventory>,
    q: Query<(Entity, &Interactable)>,
) -> bool {
    let state_entity =
        widget_context.use_state(&mut commands, inv_entity, InventoryState { open: true });

    let on_event = OnEvent::new(
        move |In((event_dispatcher_context, _, event, _entity)): In<(
            EventDispatcherContext,
            WidgetState,
            Event,
            Entity,
        )>,
              mut query: Query<&mut InventoryState>| {
            if let Ok(mut inv_state) = query.get_mut(state_entity) {
                match event.event_type {
                    EventType::KeyUp(key_event) => {
                        if key_event.key() == KeyCode::I {
                            inv_state.open = !inv_state.open;
                        }
                    }
                    _ => {}
                }
            }
            (event_dispatcher_context, event)
        },
    );
    if let Ok(inv_state) = props_query.get(inv_entity) {
        let parent_id = Some(inv_entity);
        if inv_state.open {
            rsx! {
                <BackgroundBundle on_event={on_event} styles={KStyle {
                    background_color: Color::rgba(1., 0.9, 0.4, 0.5).into(),
                    width: Units::Pixels(160.0).into(),
                    height: Units::Pixels(160.0).into(),
                    left: Units::Stretch(1.0).into(),
                    right: Units::Stretch(1.0).into(),
                    top: Units::Stretch(1.0).into(),
                    bottom: Units::Stretch(1.0).into(),
                    ..KStyle::default()
                }}>
                    <ElementBundle
                        styles={KStyle{
                            background_color: Color::rgba(1., 0.0, 0.4, 0.5).into(),
                            width: Units::Pixels(25. * 6.).into(),
                            height: Units::Pixels(25. * 4.).into(),
                            left: Units::Stretch(1.0).into(),
                            right: Units::Stretch(1.0).into(),
                            top: Units::Percentage (45.).into(),
                            // bottom: Units::Stretch(0.5).into(),
                            layout_type: LayoutType::Grid.into(),
                            grid_rows: vec![Units::Stretch(1.0), Units::Stretch(1.0), Units::Stretch(1.0), Units::Stretch(1.0)].into(),
                            grid_cols: vec![Units::Stretch(1.0), Units::Stretch(1.0),Units::Stretch(1.0),Units::Stretch(1.0),Units::Stretch(1.0),Units::Stretch(1.0)].into(),
                            render_command: StyleProp::Value(RenderCommand::Quad),
                            ..KStyle::default()}}
                    >
                        {inv_state.items.iter().enumerate().for_each(|(index, item_stack)|{
                            constructor! {
                                <ElementBundle
                                    styles={KStyle{
                                        row_index: StyleProp::Value(((index) as f32 /6 as f32).trunc() as usize),
                                        col_index: ((index) % 6).into(),
                                        ..KStyle::default()}}
                                >
                                    <InventorySlotBundle inventory_slot={InventorySlot{item: *item_stack, slot: index}}/>
                                </ElementBundle>

                            }

                        })}
                    </ElementBundle>
                </BackgroundBundle>
            };
        } else {
            for i in q.iter() {
                commands.entity(i.0).despawn();
            }
        }
    }
    true
}
