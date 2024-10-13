use bevy::prelude::*;
use rand::seq::IteratorRandom;
use strum_macros::Display;

use crate::{
    animations::player_sprite::PlayerAnimation,
    combat::{AttackTimer, HitEvent, ObjBreakEvent},
    enemy::Mob,
    handle_attack_cooldowns,
    item::WorldObject,
    juice::UseItemEvent,
    player::Player,
    ui::UIState,
    GameParam, GameState,
};

#[derive(Component)]
pub struct HitSound;
pub struct AudioPlugin;

#[derive(Resource, Debug)]
pub struct BGMPicker {
    pub current_track: String,
    pub current_handle: Option<Handle<AudioSink>>,
}

pub struct UpdateBGMTrackEvent {
    pub asset_path: String,
}

#[derive(Component, Display)]
pub enum AudioSoundEffect {
    IceStaffCast,
    IceStaffHit,
    LightningStaffCast,
    LightningStaffHit,
    IceExplosion,
    DefaultEnemyHit,
    Teleport,
    TeleportShock,
    AirWaveAttack,
    Parry,
    CombatStatueActivate,
    GambleStatueActivate,
    GambleStatueFail,
    GambleStatueSuccess,
    CurrencyPickup,
    RareDrop1,
    RareDrop2,
    LegendaryDrop1,
    LegendaryDrop2,
    Roll,
    UISlotHover,
    UIEquipSlotClick,
    UISkillSelection,
    UISkillHover,
    UISkillReRoll,
    ItemPickup,
    SkillCooldown,
    IncorrectAction,
    PortalAura,
    PortalActivate,
    Spear,
    SpearPull,
    Lunge,
    Claw,
    Bow,
    DungeonEntranceLoading,
    ButtonClick,
    ButtonHover,
    Anvil,
    CaveAmbience,
    GainExp,
    LevelUp,
    PlayerHit,
}
#[derive(Component)]

pub struct SoundSpawner {
    pub sound: AudioSoundEffect,
    pub volume: f32,
    pub delay: Option<Timer>,
}

impl SoundSpawner {
    pub fn new(sound: AudioSoundEffect, volume: f32) -> Self {
        SoundSpawner {
            sound,
            volume,
            delay: None,
        }
    }
    pub fn with_delay(mut self, delay: f32) -> Self {
        self.delay = Some(Timer::from_seconds(delay, TimerMode::Once));
        self
    }
}

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(BGMPicker {
            current_track: "sounds/bgm_day.ogg".to_owned(),
            current_handle: None,
        })
        .add_event::<UpdateBGMTrackEvent>()
        .add_system(bgm_audio)
        .add_system(handle_sound_spawners)
        .add_systems(
            (
                sword_swing_sound.after(handle_attack_cooldowns),
                use_item_audio,
                break_item_audio,
                hit_collision_audio,
            )
                .in_set(OnUpdate(GameState::Main)),
        );
    }
}
pub fn handle_sound_spawners(
    mut sounds: Query<(Entity, &mut SoundSpawner)>,
    asset_server: Res<AssetServer>,
    audio: Res<Audio>,
    time: Res<Time>,
    mut commands: Commands,
) {
    for (e, mut sound) in sounds.iter_mut() {
        let mut play_sound = false;
        if let Some(delay) = sound.delay.as_mut() {
            delay.tick(time.delta());
            if delay.finished() {
                play_sound = true;
            }
        } else {
            play_sound = true;
        }
        if play_sound {
            let sound_handle = asset_server.load(format!("sounds/{}.ogg", sound.sound));
            audio.play_with_settings(
                sound_handle.clone(),
                PlaybackSettings::ONCE.with_volume(sound.volume),
            );
            commands.entity(e).despawn();
        }
    }
}
pub fn sword_swing_sound(
    asset_server: Res<AssetServer>,
    audio: Res<Audio>,
    mouse_button_input: Res<Input<MouseButton>>,
    player_query: Query<(Option<&AttackTimer>, &PlayerAnimation), With<Player>>,
    curr_ui_state: Res<State<UIState>>,
    game: GameParam,
) {
    let (attack_timer_option, player_anim) = player_query.single();
    if mouse_button_input.pressed(MouseButton::Left)
        && curr_ui_state.0 == UIState::Closed
        && player_anim == &PlayerAnimation::Attack
    {
        if attack_timer_option.is_some() {
            return;
        }
        trace!("AUDIO!!");
        let is_dagger = if let Some(main_hand_state) = game.game.player_state.main_hand_slot.clone()
        {
            main_hand_state.get_obj() == WorldObject::Dagger
        } else {
            false
        };
        let swing1 = asset_server.load("sounds/swing.ogg");
        let swing2 = asset_server.load("sounds/swing2.ogg");
        let swing3 = asset_server.load("sounds/swing3.ogg");
        let dagger1 = asset_server.load("sounds/Dagger1.ogg");
        let dagger2 = asset_server.load("sounds/Dagger2.ogg");
        let swings = if is_dagger {
            vec![dagger1, dagger2]
        } else {
            vec![swing1, swing2, swing3]
        };
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
            audio.play_with_settings(bgm1.clone(), PlaybackSettings::LOOP.with_volume(0.75)),
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
