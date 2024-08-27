use std::marker::PhantomData;

use bevy::{
    ecs::system::SystemParam,
    prelude::{EventWriter, Res, ResMut, State},
};

use crate::{
    container::Container,
    item::{CraftedItemEvent, CraftingTracker, Recipes},
};

use super::{
    crafting_ui::CraftingContainer, scrapper_ui::ScrapperContainer, ChestContainer,
    FurnaceContainer, UIState,
};

#[derive(SystemParam)]
pub struct UIContainersParam<'w, 's> {
    pub chest_option: Option<ResMut<'w, ChestContainer>>,
    pub scrapper_option: Option<ResMut<'w, ScrapperContainer>>,
    pub furnace_option: Option<ResMut<'w, FurnaceContainer>>,
    pub crafting_option: Option<ResMut<'w, CraftingContainer>>,

    pub crafted_event: EventWriter<'w, CraftedItemEvent>,
    pub crafting_tracker: Res<'w, CraftingTracker>,
    pub recipes: Res<'w, Recipes>,
    pub ui_state: Res<'w, State<UIState>>,

    #[system_param(ignore)]
    marker: PhantomData<&'s ()>,
}

impl<'w, 's> UIContainersParam<'w, 's> {
    pub fn get_active_ui_container(&self) -> Option<&Container> {
        match self.ui_state.0 {
            UIState::Chest => self.chest_option.as_ref().map(|c| &c.items),
            UIState::Furnace => self.furnace_option.as_ref().map(|c| &c.items),
            UIState::Crafting => self.crafting_option.as_ref().map(|c| &c.items),
            _ => None,
        }
    }
    pub fn get_active_ui_container_mut(&mut self) -> Option<&mut Container> {
        match self.ui_state.0 {
            UIState::Chest => self.chest_option.as_mut().map(|c| &mut c.items),
            UIState::Furnace => self.furnace_option.as_mut().map(|c| &mut c.items),
            UIState::Crafting => self.crafting_option.as_mut().map(|c| &mut c.items),
            UIState::Scrapper => self.scrapper_option.as_mut().map(|c| &mut c.items),
            _ => None,
        }
    }
}
