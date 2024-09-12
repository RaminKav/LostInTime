use bevy::prelude::*;

#[derive(Component)]
pub struct StatsUI;

#[derive(Component)]
pub struct StatsText {
    pub index: usize,
}
#[derive(Component)]
pub struct SPText;

#[derive(Component)]
pub struct StatsButtonState {
    pub index: usize,
}

// STATS ARE DEPRIECATED
