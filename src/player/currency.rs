use bevy::prelude::*;

#[derive(Component, Default)]
pub struct TimeFragmentCurrency {
    pub time_fragments: i32,
    pub bounce_timer: Timer,
}
impl TimeFragmentCurrency {
    pub fn new(amount: i32) -> Self {
        TimeFragmentCurrency {
            time_fragments: amount,
            bounce_timer: Timer::from_seconds(0.5, TimerMode::Once),
        }
    }
}

#[derive(Event)]
pub struct ModifyTimeFragmentsEvent {
    pub delta: i32,
}

pub fn handle_modify_time_fragments(
    mut time_fragments: Query<&mut TimeFragmentCurrency>,
    mut events: EventReader<ModifyTimeFragmentsEvent>,
) {
    let mut time_fragments = time_fragments.single_mut();
    for event in events.iter() {
        time_fragments.time_fragments = (time_fragments.time_fragments + event.delta).max(0);
    }
}
