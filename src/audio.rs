use bevy::prelude::*;
use rand::seq::IteratorRandom;

use crate::{
    combat::{AttackTimer, HitEvent, ObjBreakEvent},
    enemy::Mob,
    item::WorldObject,
    juice::UseItemEvent,
    player::Player,
    ui::InventoryState,
    GameState,
};

#[derive(Component)]
pub struct HitSound;
pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_startup_system(bgm_audio).add_systems(
            (
                sword_swing_sound,
                use_item_audio,
                break_item_audio,
                hit_collision_audio,
            )
                .in_set(OnUpdate(GameState::Main)),
        );
    }
}

pub fn sword_swing_sound(
    asset_server: Res<AssetServer>,
    audio: Res<Audio>,
    mouse_button_input: Res<Input<MouseButton>>,
    player_query: Query<Option<&AttackTimer>, With<Player>>,
    inv_state: Res<InventoryState>,
) {
    if mouse_button_input.pressed(MouseButton::Left) && !inv_state.open {
        let attack_timer_option = player_query.single();
        if attack_timer_option.is_some() {
            return;
        }

        let swing1 = asset_server.load("sounds/swing.ogg");
        let swing2 = asset_server.load("sounds/swing2.ogg");
        let swing3 = asset_server.load("sounds/swing3.ogg");
        let swings = vec![swing1, swing2, swing3];
        swings.iter().choose(&mut rand::thread_rng()).map(|sound| {
            audio.play_with_settings(sound.clone(), PlaybackSettings::ONCE.with_volume(0.5))
        });
    }
}
pub fn bgm_audio(asset_server: Res<AssetServer>, audio: Res<Audio>) {
    let bgm1 = asset_server.load("sounds/bgm_1.ogg");

    audio.play_with_settings(bgm1.clone(), PlaybackSettings::LOOP.with_volume(1.0));
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
            let crunchs = vec![crunch1, crunch2, crunch3];
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
