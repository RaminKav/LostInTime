use bevy::prelude::*;
use rand::seq::IteratorRandom;

use crate::{
    combat::{AttackTimer, HitEvent, ObjBreakEvent},
    enemy::Mob,
    handle_attack_cooldowns,
    item::WorldObject,
    juice::UseItemEvent,
    player::Player,
    ui::UIState,
    GameState,
};

#[derive(Component)]
pub struct HitSound;
pub struct AudioPlugin;

#[derive(Resource, Debug)]
pub struct BGMPicker {
    pub current_track: String,
    pub current_handle: Option<Handle<AudioSink>>,
}

#[derive(Event)]
pub struct UpdateBGMTrackEvent {
    pub asset_path: String,
}

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(BGMPicker {
            current_track: "sounds/bgm_day.ogg".to_owned(),
            current_handle: None,
        })
        .add_event::<UpdateBGMTrackEvent>()
        .add_system(bgm_audio)
        .add_systems(
            (
                sword_swing_sound.after(handle_attack_cooldowns),
                use_item_audio,
                break_item_audio,
                hit_collision_audio,
            )
                .in_set(Update(GameState::Main)),
        );
    }
}

pub fn sword_swing_sound(
    asset_server: Res<AssetServer>,
    audio: Res<Audio>,
    mouse_button_input: Res<Input<MouseButton>>,
    player_query: Query<Option<&AttackTimer>, With<Player>>,
    curr_ui_state: Res<State<UIState>>,
) {
    if mouse_button_input.pressed(MouseButton::Left) && curr_ui_state.get() == UIState::Closed {
        let attack_timer_option = player_query.single();
        if attack_timer_option.is_some() {
            return;
        }
        trace!("AUDIO!!");

        let swing1 = asset_server.load("sounds/swing.ogg");
        let swing2 = asset_server.load("sounds/swing2.ogg");
        let swing3 = asset_server.load("sounds/swing3.ogg");
        let swings = [swing1, swing2, swing3];
        swings.iter().choose(&mut rand::thread_rng()).map(|sound| {
            audio.play_with_settings(sound.clone(), PlaybackSettings::ONCE.with_volume(0.5))
        });
    }
}
pub fn bgm_audio(
    asset_server: Res<AssetServer>,
    audio: Res<Audio>,
    mut bgm_tracker: ResMut<BGMPicker>,
    audio_handles: Res<Assets<AudioSink>>,
    mut bgm_update_events: EventReader<UpdateBGMTrackEvent>,
) {
    for event in bgm_update_events.iter() {
        //TODO: Fade out music rather than just stopping it
        if let Some(prev_handle) = bgm_tracker.current_handle.as_ref() {
            if let Some(prev_audio) = audio_handles.get(prev_handle) {
                prev_audio.stop();
            }
        }
        let path = event.asset_path.clone();
        bgm_tracker.current_track = path.clone();
        let bgm1 = asset_server.load(path);

        let new_handle = audio_handles.get_handle(
            audio.play_with_settings(bgm1.clone(), PlaybackSettings::LOOP.with_volume(1.0)),
        );
        bgm_tracker.current_handle = Some(new_handle);
    }
}

pub fn use_item_audio(
    asset_server: Res<AssetServer>,
    audio: Res<Audio>,
    mut use_item_event: EventReader<UseItemEvent>,
) {
    for item in use_item_event.iter() {
        if [
            WorldObject::Apple,
            WorldObject::BrownMushroomBlock,
            WorldObject::RedMushroomBlock,
            WorldObject::RedStew,
            WorldObject::RawMeat,
            WorldObject::CookedMeat,
            WorldObject::Berries,
        ]
        .contains(&item.0)
        {
            let crunch1 = asset_server.load("sounds/crunch.ogg");
            let crunch2 = asset_server.load("sounds/crunch2.ogg");
            let crunch3 = asset_server.load("sounds/crunch3.ogg");
            let crunchs = [crunch1, crunch2, crunch3];
            crunchs.iter().choose(&mut rand::thread_rng()).map(|sound| {
                audio.play_with_settings(sound.clone(), PlaybackSettings::ONCE.with_volume(0.5))
            });
        } else {
            let sound = asset_server.load(format!("sounds/{}.ogg", item.0));
            audio.play_with_settings(sound.clone(), PlaybackSettings::ONCE.with_volume(0.5));
        }
    }
}

pub fn break_item_audio(
    asset_server: Res<AssetServer>,
    audio: Res<Audio>,
    mut obj_break_events: EventReader<ObjBreakEvent>,
) {
    for item in obj_break_events.iter() {
        if [
            WorldObject::Grass,
            WorldObject::Grass2,
            WorldObject::Grass3,
            WorldObject::Bush,
            WorldObject::Bush2,
            WorldObject::BerryBush,
            WorldObject::Lillypad,
            WorldObject::Cattail,
        ]
        .contains(&item.obj)
        {
            let rustle1 = asset_server.load("sounds/rustle.ogg");
            let rustle2 = asset_server.load("sounds/rustle2.ogg");
            let rustle3 = asset_server.load("sounds/rustle3.ogg");
            let rustle4 = asset_server.load("sounds/rustle4.ogg");
            let rustle5 = asset_server.load("sounds/rustle5.ogg");
            let rustle6 = asset_server.load("sounds/rustle6.ogg");
            let rustle7 = asset_server.load("sounds/rustle7.ogg");
            let rustles = vec![
                rustle1, rustle2, rustle3, rustle4, rustle5, rustle6, rustle7,
            ];
            rustles.iter().choose(&mut rand::thread_rng()).map(|sound| {
                audio.play_with_settings(sound.clone(), PlaybackSettings::ONCE.with_volume(0.5))
            });
        } else {
            let sound = asset_server.load(format!("sounds/{}.ogg", item.obj));
            audio.play_with_settings(sound.clone(), PlaybackSettings::ONCE.with_volume(0.5));
        }
    }
}

pub fn hit_collision_audio(
    asset_server: Res<AssetServer>,
    audio: Res<Audio>,
    mut hit_events: EventReader<HitEvent>,
    world_objects: Query<&WorldObject>,
    mobs: Query<&Mob>,
) {
    for hit in hit_events.iter() {
        if let Ok(obj) = world_objects.get(hit.hit_entity) {
            let sound = asset_server.load(format!("sounds/{}.ogg", obj));
            audio.play_with_settings(sound.clone(), PlaybackSettings::ONCE.with_volume(0.5));
        } else if let Ok(mob) = mobs.get(hit.hit_entity) {
            let sound = asset_server.load(format!("sounds/{}.ogg", mob));
            audio.play_with_settings(sound.clone(), PlaybackSettings::ONCE.with_volume(0.5));
        }
    }
}
