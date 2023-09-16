use std::marker::PhantomData;

use bevy::{
    ecs::system::SystemParam,
    prelude::{EventWriter, Res, ResMut},
};

use crate::item::{CraftedItemEvent, CraftingTracker, Recipes};

use super::{crafting_ui::CraftingContainer, ChestContainer, FurnaceContainer};

#[derive(SystemParam)]
pub struct UIContainersParam<'w, 's> {
    pub chest_option: Option<ResMut<'w, ChestContainer>>,
    pub furnace_option: Option<ResMut<'w, FurnaceContainer>>,
    pub crafting_option: Option<ResMut<'w, CraftingContainer>>,

    pub crafted_event: EventWriter<'w, CraftedItemEvent>,
    pub crafting_tracker: Res<'w, CraftingTracker>,
    pub recipes: Res<'w, Recipes>,

    #[system_param(ignore)]
    marker: PhantomData<&'s ()>,
}
