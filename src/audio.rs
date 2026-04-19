use std::collections::HashSet;

use bevy::{
    asset::{HandleId, LoadState},
    prelude::*,
    utils::HashMap,
};
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
    GameState,
};

const MAX_HIT_SOUNDS_PER_FRAME: usize = 3;
const SFX_CLEANUP_DELAY_SECS: f32 = 5.0;

/// Controls the global volume for music and sound effects independently.
/// Values range from 0 (muted) to 10 (full volume).
#[derive(Resource, Debug, Clone)]
pub struct AudioVolume {
    pub music: u8,
    pub sfx: u8,
}

impl Default for AudioVolume {
    fn default() -> Self {
        Self { music: 7, sfx: 7 }
    }
}

impl AudioVolume {
    pub fn music_fraction(&self) -> f32 {
        self.music as f32 / 10.0
    }

    pub fn sfx_fraction(&self) -> f32 {
        self.sfx as f32 / 10.0
    }
}

pub struct AudioPlugin;

#[derive(Resource, Debug)]
pub struct BGMPicker {
    pub current_track: String,
    pub current_handle: Option<Handle<AudioSink>>,
}

/// Resource to track sound cooldowns to prevent spam
#[derive(Resource, Default)]
pub struct SoundCooldowns {
    pub cooldowns: HashMap<AudioSoundEffect, Timer>,
}

pub struct UpdateBGMTrackEvent {
    pub asset_path: String,
}

/// Caches loaded sound handles and tracks paths that failed to load.
/// Prevents Bevy's internal audio queue from accumulating unresolvable
/// playback commands for missing sound files.
#[derive(Resource, Default)]
pub struct SoundCache {
    handles: HashMap<String, Handle<AudioSource>>,
    failed: HashSet<String>,
}

impl SoundCache {
    fn get_or_load(
        &mut self,
        path: &str,
        asset_server: &AssetServer,
    ) -> Option<Handle<AudioSource>> {
        if self.failed.contains(path) {
            return None;
        }
        Some(
            self.handles
                .entry(path.to_string())
                .or_insert_with(|| asset_server.load(path))
                .clone(),
        )
    }

    fn play(
        &mut self,
        path: &str,
        volume: f32,
        global_volume: f32,
        asset_server: &AssetServer,
        audio: &Audio,
        tracker: &mut SinkCleanupTracker,
    ) {
        if let Some(handle) = self.get_or_load(path, asset_server) {
            let sink_handle = audio.play_with_settings(
                handle,
                PlaybackSettings::ONCE.with_volume(volume * global_volume),
            );
            tracker.pending.push((
                sink_handle.id(),
                Timer::from_seconds(SFX_CLEANUP_DELAY_SECS, TimerMode::Once),
            ));
        }
    }
}

/// Tracks AudioSink handles with timers so we can remove them from
/// Assets<AudioSink> after playback is guaranteed to have finished.
/// Bevy 0.10 does not auto-cleanup ONCE sinks and lacks `empty()`.
#[derive(Resource, Default)]
pub struct SinkCleanupTracker {
    pending: Vec<(HandleId, Timer)>,
}

#[derive(Component, Display, Clone, Copy, PartialEq, Eq, Hash)]
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
    SwordSwing,
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
        .init_resource::<AudioVolume>()
        .init_resource::<SoundCooldowns>()
        .init_resource::<SoundCache>()
        .init_resource::<SinkCleanupTracker>()
        .add_event::<UpdateBGMTrackEvent>()
        .add_system(bgm_audio)
        .add_system(update_bgm_volume)
        .add_system(handle_sound_spawners)
        .add_system(tick_sound_cooldowns)
        .add_system(update_sound_cache)
        .add_system(cleanup_finished_audio_sinks)
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
/// Tick all sound cooldowns
pub fn tick_sound_cooldowns(time: Res<Time>, mut cooldowns: ResMut<SoundCooldowns>) {
    for timer in cooldowns.cooldowns.values_mut() {
        timer.tick(time.delta());
    }
}

/// Detects sound files that failed to load and marks them in the cache
/// so we never queue playback for them again.
fn update_sound_cache(mut cache: ResMut<SoundCache>, asset_server: Res<AssetServer>) {
    let mut newly_failed = Vec::new();
    for (path, handle) in &cache.handles {
        if asset_server.get_load_state(handle.clone()) == LoadState::Failed {
            newly_failed.push(path.clone());
        }
    }
    for path in newly_failed {
        warn!("Sound file failed to load, skipping future plays: {}", path);
        cache.handles.remove(&path);
        cache.failed.insert(path);
    }
}

/// Ticks sink cleanup timers and removes expired AudioSink assets.
/// Sound effects are short so SFX_CLEANUP_DELAY_SECS is enough headroom.
fn cleanup_finished_audio_sinks(
    time: Res<Time>,
    mut tracker: ResMut<SinkCleanupTracker>,
    mut audio_sinks: ResMut<Assets<AudioSink>>,
) {
    let mut i = 0;
    while i < tracker.pending.len() {
        tracker.pending[i].1.tick(time.delta());
        if tracker.pending[i].1.finished() {
            let (id, _) = tracker.pending.swap_remove(i);
            audio_sinks.remove(id);
        } else {
            i += 1;
        }
    }
}

/// Get the cooldown duration for a specific sound effect (in seconds)
/// Sounds with cooldowns will be rate-limited to prevent audio spam and performance issues
fn get_sound_cooldown_duration(sound: &AudioSoundEffect) -> Option<f32> {
    match sound {
        // Combat hit sounds - very frequent, short cooldown
        AudioSoundEffect::DefaultEnemyHit => Some(0.05), // 50ms
        AudioSoundEffect::IceStaffHit => Some(0.05),     // 50ms
        AudioSoundEffect::LightningStaffHit => Some(0.05), // 50ms
        AudioSoundEffect::PlayerHit => Some(0.1),        // 100ms

        // Attack sounds - can be spammed with fast attack speed
        AudioSoundEffect::IceStaffCast => Some(0.1), // 100ms
        AudioSoundEffect::LightningStaffCast => Some(0.1), // 100ms
        AudioSoundEffect::Bow => Some(0.1),          // 100ms
        AudioSoundEffect::Claw => Some(0.1),         // 100ms
        AudioSoundEffect::AirWaveAttack => Some(0.15), // 150ms
        AudioSoundEffect::SwordSwing => Some(0.15),  // 150ms - prevent spam with Rapidfire

        // Explosion/AOE sounds - prevent overlapping
        AudioSoundEffect::IceExplosion => Some(0.15), // 150ms

        // UI sounds - prevent spam on hover/interaction
        AudioSoundEffect::UISlotHover => Some(0.05), // 50ms
        AudioSoundEffect::ButtonHover => Some(0.05), // 50ms
        AudioSoundEffect::UISkillHover => Some(0.05), // 50ms

        // Pickup sounds - frequent in combat
        AudioSoundEffect::ItemPickup => Some(0.08), // 80ms
        AudioSoundEffect::CurrencyPickup => Some(0.1), // 100ms
        AudioSoundEffect::GainExp => Some(0.08),    // 80ms

        // Skill sounds
        AudioSoundEffect::Teleport => Some(0.2), // 200ms
        AudioSoundEffect::TeleportShock => Some(0.2), // 200ms
        AudioSoundEffect::Roll => Some(0.15),    // 150ms
        AudioSoundEffect::Lunge => Some(0.2),    // 200ms
        AudioSoundEffect::Spear => Some(0.2),    // 200ms
        AudioSoundEffect::Parry => Some(0.2),    // 200ms

        // Rare/Legendary drops - no spam possible but add cooldown for safety
        AudioSoundEffect::RareDrop1 => Some(0.3),
        AudioSoundEffect::RareDrop2 => Some(0.3),
        AudioSoundEffect::LegendaryDrop1 => Some(0.3),
        AudioSoundEffect::LegendaryDrop2 => Some(0.3),

        // Other sounds - no cooldown needed (infrequent or one-time)
        _ => Some(0.2),
    }
}

pub fn handle_sound_spawners(
    mut sounds: Query<(Entity, &mut SoundSpawner)>,
    asset_server: Res<AssetServer>,
    audio: Res<Audio>,
    time: Res<Time>,
    mut commands: Commands,
    mut cooldowns: ResMut<SoundCooldowns>,
    mut cache: ResMut<SoundCache>,
    mut tracker: ResMut<SinkCleanupTracker>,
    volume: Res<AudioVolume>,
) {
    if *crate::NO_AUDIO {
        for (e, _) in sounds.iter() {
            commands.entity(e).despawn();
        }
        return;
    }
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
            if let Some(cooldown_duration) = get_sound_cooldown_duration(&sound.sound) {
                let can_play = cooldowns
                    .cooldowns
                    .get(&sound.sound)
                    .map(|timer| timer.finished())
                    .unwrap_or(true);

                if !can_play {
                    commands.entity(e).despawn();
                    continue;
                }

                cooldowns.cooldowns.insert(
                    sound.sound,
                    Timer::from_seconds(cooldown_duration, TimerMode::Once),
                );
            }

            let sfx_vol = volume.sfx_fraction();
            if sound.sound == AudioSoundEffect::SwordSwing {
                use rand::seq::SliceRandom;
                let paths = ["sounds/swing.ogg", "sounds/swing2.ogg", "sounds/swing3.ogg"];
                if let Some(path) = paths.choose(&mut rand::thread_rng()) {
                    cache.play(
                        path,
                        sound.volume,
                        sfx_vol,
                        &asset_server,
                        &audio,
                        &mut tracker,
                    );
                }
            } else {
                let path = format!("sounds/{}.ogg", sound.sound);
                cache.play(
                    &path,
                    sound.volume,
                    sfx_vol,
                    &asset_server,
                    &audio,
                    &mut tracker,
                );
            }
            commands.entity(e).despawn();
        }
    }
}
pub fn sword_swing_sound(
    mut commands: Commands,
    mouse_button_input: Res<Input<MouseButton>>,
    player_query: Query<(Option<&AttackTimer>, &PlayerAnimation), With<Player>>,
    curr_ui_state: Res<State<UIState>>,
) {
    if *crate::NO_AUDIO {
        return;
    }
    let (attack_timer_option, player_anim) = player_query.single();
    if mouse_button_input.pressed(MouseButton::Left)
        && curr_ui_state.0 == UIState::Closed
        && player_anim == &PlayerAnimation::Attack
    {
        if attack_timer_option.is_some() {
            return;
        }
        trace!("AUDIO!!");
        // Use SoundSpawner to get automatic cooldown rate limiting
        commands.spawn(SoundSpawner::new(AudioSoundEffect::SwordSwing, 0.5));
    }
}
pub fn bgm_audio(
    asset_server: Res<AssetServer>,
    audio: Res<Audio>,
    mut bgm_tracker: ResMut<BGMPicker>,
    audio_handles: Res<Assets<AudioSink>>,
    mut bgm_update_events: EventReader<UpdateBGMTrackEvent>,
    mut cache: ResMut<SoundCache>,
    volume: Res<AudioVolume>,
) {
    if *crate::NO_AUDIO {
        bgm_update_events.clear();
        return;
    }
    for event in bgm_update_events.iter() {
        if let Some(prev_handle) = bgm_tracker.current_handle.as_ref() {
            if let Some(prev_audio) = audio_handles.get(prev_handle) {
                prev_audio.stop();
            }
        }
        let path = event.asset_path.clone();
        bgm_tracker.current_track = path.clone();
        if let Some(bgm_handle) = cache.get_or_load(&path, &asset_server) {
            let new_handle = audio_handles.get_handle(audio.play_with_settings(
                bgm_handle,
                PlaybackSettings::LOOP.with_volume(0.75 * volume.music_fraction()),
            ));
            bgm_tracker.current_handle = Some(new_handle);
        }
    }
}

/// Adjusts the currently-playing BGM sink whenever the music volume changes.
pub fn update_bgm_volume(
    volume: Res<AudioVolume>,
    bgm_tracker: Res<BGMPicker>,
    audio_sinks: Res<Assets<AudioSink>>,
) {
    if !volume.is_changed() {
        return;
    }
    if let Some(handle) = bgm_tracker.current_handle.as_ref() {
        if let Some(sink) = audio_sinks.get(handle) {
            sink.set_volume(0.75 * volume.music_fraction());
        }
    }
}

pub fn use_item_audio(
    asset_server: Res<AssetServer>,
    audio: Res<Audio>,
    mut use_item_event: EventReader<UseItemEvent>,
    mut cache: ResMut<SoundCache>,
    mut tracker: ResMut<SinkCleanupTracker>,
    volume: Res<AudioVolume>,
) {
    if *crate::NO_AUDIO {
        use_item_event.clear();
        return;
    }
    let sfx_vol = volume.sfx_fraction();
    for item in use_item_event.iter() {
        if [
            WorldObject::Apple,
            WorldObject::BrownMushroomBlock,
            WorldObject::RedMushroomBlock,
            WorldObject::RedStew,
            WorldObject::PinkFlowerStew,
            WorldObject::YellowFlowerStew,
            WorldObject::BerryJam,
            WorldObject::RawMeat,
            WorldObject::CookedMeat,
            WorldObject::Berries,
        ]
        .contains(&item.0)
        {
            let paths = [
                "sounds/crunch.ogg",
                "sounds/crunch2.ogg",
                "sounds/crunch3.ogg",
            ];
            if let Some(path) = paths.iter().choose(&mut rand::thread_rng()) {
                cache.play(path, 0.2, sfx_vol, &asset_server, &audio, &mut tracker);
            }
        } else {
            let path = format!("sounds/{}.ogg", item.0);
            cache.play(&path, 0.2, sfx_vol, &asset_server, &audio, &mut tracker);
        }
    }
}

pub fn break_item_audio(
    asset_server: Res<AssetServer>,
    audio: Res<Audio>,
    mut obj_break_events: EventReader<ObjBreakEvent>,
    mut cache: ResMut<SoundCache>,
    mut tracker: ResMut<SinkCleanupTracker>,
    volume: Res<AudioVolume>,
) {
    if *crate::NO_AUDIO {
        obj_break_events.clear();
        return;
    }
    let sfx_vol = volume.sfx_fraction();
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
            let paths = [
                "sounds/rustle.ogg",
                "sounds/rustle2.ogg",
                "sounds/rustle3.ogg",
                "sounds/rustle4.ogg",
                "sounds/rustle5.ogg",
                "sounds/rustle6.ogg",
                "sounds/rustle7.ogg",
            ];
            if let Some(path) = paths.iter().choose(&mut rand::thread_rng()) {
                cache.play(path, 0.15, sfx_vol, &asset_server, &audio, &mut tracker);
            }
        } else {
            let path = format!("sounds/{}.ogg", item.obj);
            cache.play(&path, 0.15, sfx_vol, &asset_server, &audio, &mut tracker);
        }
    }
}

pub fn hit_collision_audio(
    asset_server: Res<AssetServer>,
    audio: Res<Audio>,
    mut hit_events: EventReader<HitEvent>,
    world_objects: Query<&WorldObject>,
    mobs: Query<&Mob>,
    mut cache: ResMut<SoundCache>,
    mut tracker: ResMut<SinkCleanupTracker>,
    volume: Res<AudioVolume>,
) {
    if *crate::NO_AUDIO {
        hit_events.clear();
        return;
    }
    let sfx_vol = volume.sfx_fraction();
    let mut played = 0;
    for hit in hit_events.iter() {
        if played >= MAX_HIT_SOUNDS_PER_FRAME {
            break;
        }
        if let Ok(obj) = world_objects.get(hit.hit_entity) {
            let path = format!("sounds/{}.ogg", obj);
            cache.play(&path, 0.15, sfx_vol, &asset_server, &audio, &mut tracker);
            played += 1;
        } else if let Ok(mob) = mobs.get(hit.hit_entity) {
            let path = format!("sounds/{}.ogg", mob);
            cache.play(&path, 0.15, sfx_vol, &asset_server, &audio, &mut tracker);
            played += 1;
        }
    }
}
