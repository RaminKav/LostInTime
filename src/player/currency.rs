use bevy::prelude::*;

use crate::ui::CurrencyIcon;

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

pub struct ModifyTimeFragmentsEvent {
    pub delta: i32,
}

pub fn handle_modify_time_fragments(
    mut time_fragments: Query<&mut TimeFragmentCurrency>,
    mut events: EventReader<ModifyTimeFragmentsEvent>,
    mut icon: Query<&mut Transform, With<CurrencyIcon>>,
    time: Res<Time>,
) {
    let mut time_fragments = time_fragments.single_mut();
    for event in events.iter() {
        time_fragments.time_fragments = (time_fragments.time_fragments + event.delta).max(0);
        time_fragments.bounce_timer.reset();
    }
    if !time_fragments.bounce_timer.finished() {
        time_fragments.bounce_timer.tick(time.delta());
    }
    if time_fragments.bounce_timer.percent() <= 0.5 {
        icon.single_mut().scale += Vec2::splat(0.011).extend(0.);
    } else if time_fragments.bounce_timer.percent() > 0.5
        && time_fragments.bounce_timer.percent() < 1.
    {
        icon.single_mut().scale -= Vec2::splat(0.011).extend(0.);
    }
    if time_fragments.bounce_timer.just_finished() {
        icon.single_mut().scale = Vec3::ONE;
    }
}
