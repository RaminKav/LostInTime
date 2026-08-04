//! Thin helpers for native `bevy_aseprite_ultra` usage.
//!
//! Prefer these over rebuilding [`AseAnimation`] when changing tags — that wipes the
//! aseprite handle. Finish reactions go through [`AsepriteFinished`] (bridged from
//! ultra's [`AnimationEvents`] message).

use std::collections::HashSet;

use bevy::prelude::*;
use bevy_aseprite_ultra::prelude::{
    Animation, AnimationEvents, AnimationRepeat, AseAnimation, Aseprite,
};

/// Entity-targeted finish signal, bridged from ultra's [`AnimationEvents::Finished`].
#[derive(Event, Clone, Copy, Debug)]
pub struct AsepriteFinished(pub Entity);

pub struct AsepriteHelpersPlugin;

impl Plugin for AsepriteHelpersPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, bridge_animation_finished)
            .add_observer(despawn_done_animation_on_finished);
    }
}

/// Re-trigger ultra's finish message as an observer [`Event`] so game code can react
/// without one-frame mirror flags.
fn bridge_animation_finished(mut events: MessageReader<AnimationEvents>, mut commands: Commands) {
    for event in events.read() {
        if let AnimationEvents::Finished(entity) = event {
            commands.trigger(AsepriteFinished(*entity));
        }
    }
}

fn despawn_done_animation_on_finished(
    trigger: On<AsepriteFinished>,
    done: Query<(), With<crate::animations::DoneAnimation>>,
    mut commands: Commands,
) {
    let entity = trigger.0;
    if done.get(entity).is_ok() {
        if let Ok(mut entity_commands) = commands.get_entity(entity) {
            entity_commands.despawn();
        }
    }
}

/// Build a native [`AseAnimation`]. Empty `tag` uses `tag: None` (never `Some("")`).
pub fn ase_animation(handle: Handle<Aseprite>, tag: &str, once: bool) -> AseAnimation {
    let repeat = if once {
        AnimationRepeat::Count(1)
    } else {
        AnimationRepeat::Loop
    };
    let animation = if tag.is_empty() {
        Animation::default().with_repeat(repeat)
    } else {
        Animation::tag(tag).with_repeat(repeat)
    };
    AseAnimation {
        aseprite: handle,
        animation,
    }
}

/// Spawn bundle replacing old `AsepriteBundle::into_bundle()`.
///
/// [`AnimationState`] is required by ultra's `AseAnimation`; only [`Sprite`] is added here.
pub fn aseprite_bundle(
    handle: Handle<Aseprite>,
    tag: &str,
    transform: Transform,
    visibility: Visibility,
    once: bool,
) -> impl Bundle {
    (
        ase_animation(handle, tag, once),
        Sprite::default(),
        transform,
        GlobalTransform::default(),
        visibility,
        InheritedVisibility::default(),
        ViewVisibility::default(),
    )
}

/// Switch tag without dropping the aseprite handle.
pub fn play_loop(ase: &mut AseAnimation, tag: &str) {
    if tag.is_empty() {
        ase.animation.tag = None;
        ase.animation.repeat = AnimationRepeat::Loop;
        ase.animation.playing = true;
        ase.animation.queue.clear();
    } else {
        ase.animation.play_loop(tag);
    }
}

/// Switch tag once without dropping the aseprite handle.
pub fn play_once(ase: &mut AseAnimation, tag: &str) {
    if tag.is_empty() {
        ase.animation.tag = None;
        ase.animation.repeat = AnimationRepeat::Count(1);
        ase.animation.playing = true;
        ase.animation.queue.clear();
    } else {
        ase.animation.play(tag, AnimationRepeat::Count(1));
    }
}

pub fn is_paused(ase: &AseAnimation) -> bool {
    !ase.animation.playing
}

pub fn pause(ase: &mut AseAnimation) {
    ase.animation.pause();
}

pub fn start(ase: &mut AseAnimation) {
    ase.animation.start();
}

/// Collect entities that finished a one-shot clip this frame (from ultra messages).
pub fn collect_finished(events: &mut MessageReader<AnimationEvents>) -> HashSet<Entity> {
    let mut finished = HashSet::new();
    for event in events.read() {
        if let AnimationEvents::Finished(entity) = event {
            finished.insert(*entity);
        }
    }
    finished
}
