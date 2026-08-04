use crate::aseprite_assets::{
    FairyPetSprite, PlayerBlueAseprite, PlayerDeadAseprite, PlayerGreenAseprite,
    PlayerGreyAseprite, PlayerHunterAseprite, PlayerRedAseprite, PlayerRogueAseprite,
    PlayerThiefAseprite, PlayerWizardAseprite, SlimePetSprite,
};
use bevy::prelude::*;
use bevy_aseprite_ultra::prelude::{
    Animation, AnimationEvents, AnimationRepeat, AnimationState, AseAnimation, Aseprite,
};

use crate::{
    attributes::AttributeChangeEvent,
    inputs::FacingDirection,
    item::WorldObject,
    player::skills::{ActiveSkillChoiceState, HeirloomRarity, PlayerClass},
};

/// Timer to track how long an attack animation has been playing. Added
/// whenever the player triggers an attack animation and removed when the
/// animation finishes — stored `SparseSet` so attack animations don't move
/// the player to a new archetype on every swing.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct AttackAnimationTimer(pub Timer);

/// Force-restart the current attack/bow clip when a new swing starts while
/// [`PlayerAnimation`] is already `Attack`/`Bow` (no `Changed` trigger).
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct RestartPlayerAttackAnim;

#[derive(Resource)]
pub struct PlayerSpriteHandles {
    pub grey: Handle<Aseprite>,
    pub red: Handle<Aseprite>,
    pub green: Handle<Aseprite>,
    pub blue: Handle<Aseprite>,
    pub rogue: Handle<Aseprite>,
    pub thief: Handle<Aseprite>,
    pub wizard: Handle<Aseprite>,
    pub hunter: Handle<Aseprite>,

    pub slime_pet: Handle<Aseprite>,
    pub fairy_pet: Handle<Aseprite>,
}

#[derive(Component, Eq, PartialEq, Debug)]
pub enum PlayerAnimation {
    Idle,
    Walk,
    Run,
    Roll,
    Parry,
    ParryHit,
    Spear,
    Attack,
    Bow,
    Lunge,
    RunAttack2,
    Teleport,
    SpinAttack,
}
impl PlayerAnimation {
    pub fn get_str(&self, dir: FacingDirection) -> String {
        // Tag names must match the `.aseprite` file exactly (parsed from the binary).
        // A missing tag makes bevy_aseprite_ultra skip that entity's animation update.
        match self {
            PlayerAnimation::Idle => match dir {
                FacingDirection::Up => "IdleBack",
                FacingDirection::Down => "IdleFront",
                FacingDirection::Left | FacingDirection::Right => "IdleSide",
            },
            PlayerAnimation::Walk => match dir {
                FacingDirection::Up => "WalkBack",
                FacingDirection::Down => "WalkFront",
                FacingDirection::Left | FacingDirection::Right => "WalkSide",
            },
            PlayerAnimation::Run => match dir {
                FacingDirection::Up => "RunBack",
                FacingDirection::Down => "RunFront",
                FacingDirection::Left | FacingDirection::Right => "RunSide",
            },
            PlayerAnimation::Roll => match dir {
                FacingDirection::Up => "RollBack",
                FacingDirection::Down => "RollFront",
                FacingDirection::Left | FacingDirection::Right => "RollSide",
            },
            PlayerAnimation::Parry => match dir {
                FacingDirection::Up => "ParryBack",
                FacingDirection::Down => "ParryFront",
                FacingDirection::Left | FacingDirection::Right => "ParrySide",
            },
            PlayerAnimation::ParryHit => match dir {
                FacingDirection::Up => "ParryHitBack",
                FacingDirection::Down => "ParryHitFront",
                FacingDirection::Left | FacingDirection::Right => "ParryHitSide",
            },
            PlayerAnimation::Spear => match dir {
                FacingDirection::Up => "SpearBack",
                FacingDirection::Down => "SpearFront",
                FacingDirection::Left | FacingDirection::Right => "SpearSide",
            },
            PlayerAnimation::Attack => match dir {
                FacingDirection::Up => "Attack2Back",
                FacingDirection::Down => "Attack2Front",
                FacingDirection::Left | FacingDirection::Right => "Attack2Side",
            },
            PlayerAnimation::Bow => match dir {
                FacingDirection::Up => "BowBack",
                FacingDirection::Down => "BowFront",
                FacingDirection::Left | FacingDirection::Right => "BowSide",
            },
            PlayerAnimation::Lunge => match dir {
                FacingDirection::Up => "LungeBack",
                FacingDirection::Down => "LungeFront",
                FacingDirection::Left | FacingDirection::Right => "LungeSide",
            },
            PlayerAnimation::RunAttack2 => match dir {
                FacingDirection::Up => "RunAttack2Back",
                FacingDirection::Down => "RunAttack2Front",
                FacingDirection::Left | FacingDirection::Right => "RunAttack2Side",
            },
            PlayerAnimation::Teleport => match dir {
                FacingDirection::Up => "TeleportBack",
                FacingDirection::Down => "TeleportFront",
                FacingDirection::Left | FacingDirection::Right => "TeleportSide",
            },
            // Most class sheets only define the front clip; rogue/thief have more.
            PlayerAnimation::SpinAttack => "SpinAttackFront",
        }
        .to_string()
    }
    pub fn is_dir_locked(&self) -> bool {
        match self {
            PlayerAnimation::Roll => true,
            PlayerAnimation::Parry => true,
            PlayerAnimation::ParryHit => true,
            PlayerAnimation::Spear => true,
            PlayerAnimation::Attack => true,
            PlayerAnimation::Lunge => true,
            PlayerAnimation::Bow => true,
            PlayerAnimation::RunAttack2 => true,
            PlayerAnimation::Teleport => true,
            PlayerAnimation::SpinAttack => true,
            _ => false,
        }
    }

    pub fn action_movement_restriction(&self, main_hand: Option<WorldObject>) -> f32 {
        if self == &PlayerAnimation::Attack || self == &PlayerAnimation::Bow {
            if let Some(wep) = main_hand {
                match wep {
                    WorldObject::BasicStaff
                    | WorldObject::FireStaff
                    | WorldObject::IceStaff
                    | WorldObject::MagicWhip => return 0.5,
                    WorldObject::Sword => return 0.45,
                    WorldObject::Dagger => return 0.85,
                    WorldObject::Hammer => return 0.2,
                    WorldObject::Blowdart => return 0.95,
                    WorldObject::Gun => return 0.7,
                    WorldObject::WoodBow => return 0.37,
                    WorldObject::Claw => return 0.8,
                    WorldObject::Spear => return 0.3,
                    _ => return 1.,
                }
            }
            0.5
        } else {
            1.0
        }
    }

    pub fn is_sprinting(&self) -> bool {
        self == &PlayerAnimation::Run
    }
    pub fn is_lunging(&self) -> bool {
        self == &PlayerAnimation::Lunge
    }
    pub fn is_parrying(&self) -> bool {
        self == &PlayerAnimation::Parry || self == &PlayerAnimation::ParryHit
    }

    pub fn is_walking(&self) -> bool {
        self == &PlayerAnimation::Walk
    }

    pub fn is_shooting_bow(&self) -> bool {
        self == &PlayerAnimation::Bow
    }
    pub fn is_one_time_anim(&self) -> bool {
        match self {
            PlayerAnimation::Roll => true,
            PlayerAnimation::Parry => true,
            PlayerAnimation::ParryHit => true,
            PlayerAnimation::Spear => true,
            PlayerAnimation::Attack => true,
            PlayerAnimation::Bow => true,
            PlayerAnimation::Lunge => true,
            PlayerAnimation::RunAttack2 => true,
            PlayerAnimation::Teleport => true,
            PlayerAnimation::SpinAttack => true,
            _ => false,
        }
    }
    pub fn is_an_attack(&self) -> bool {
        match self {
            PlayerAnimation::Attack => true,
            // PlayerAnimation::Lunge => true,
            PlayerAnimation::RunAttack2 => true,
            _ => false,
        }
    }
}

#[derive(Component)]
pub struct PlayerAnimationState {
    pub prev_dir: FacingDirection,
    /// Previous **tag-relative** frame, used to detect when a one-time tag wraps.
    prev_aseprite_frame: usize,
    /// Set once the animation advances past its first relative frame.
    seen_aseprite_progress: bool,
    /// After a tag switch, ignore the next wrap sample — `AnimationState` still
    /// holds the previous tag's frame until ultra applies the new tag.
    suppress_wrap_detect: bool,
}
impl PlayerAnimationState {
    pub fn new() -> Self {
        Self {
            prev_dir: FacingDirection::Down,
            prev_aseprite_frame: 0,
            seen_aseprite_progress: false,
            suppress_wrap_detect: false,
        }
    }

    fn reset_aseprite_frame_tracker(&mut self) {
        self.prev_aseprite_frame = 0;
        self.seen_aseprite_progress = false;
        self.suppress_wrap_detect = true;
    }
}

fn play_player_tag(ase: &mut AseAnimation, tag: &str, once: bool) {
    if once {
        ase.animation.play(tag, AnimationRepeat::Count(1));
    } else {
        ase.animation.play_loop(tag);
    }
}

pub fn handle_anim_change_when_player_dir_changes(
    mut new_dir_query: Query<
        (
            &FacingDirection,
            &PlayerAnimation,
            &mut AseAnimation,
            &mut PlayerAnimationState,
            &mut Sprite,
        ),
        Or<(Changed<FacingDirection>, Changed<PlayerAnimation>)>,
    >,
) {
    for (new_dir, curr_anim, mut ase, mut anim_state, mut sprite) in new_dir_query.iter_mut() {
        if curr_anim.is_dir_locked() {
            // Left/right share the same Aseprite tag (`…Side`); only `flip_x` differs.
            let side_to_side = matches!(
                (&anim_state.prev_dir, new_dir),
                (
                    FacingDirection::Left | FacingDirection::Right,
                    FacingDirection::Left | FacingDirection::Right,
                )
            );
            if side_to_side {
                sprite.flip_x = new_dir == &FacingDirection::Left;
                anim_state.prev_dir = new_dir.clone();
            }
            continue;
        }

        let tag = curr_anim.get_str(new_dir.clone());
        play_player_tag(&mut ase, tag.as_str(), false);
        sprite.flip_x = matches!(new_dir, FacingDirection::Left);
        anim_state.prev_dir = new_dir.clone();
    }
}

pub fn handle_player_animation_change(
    mut query: Query<
        (
            &PlayerAnimation,
            &mut AseAnimation,
            &mut PlayerAnimationState,
            &FacingDirection,
        ),
        Changed<PlayerAnimation>,
    >,
) {
    for (curr_anim, mut ase, mut anim_state, dir) in query.iter_mut() {
        let tag = curr_anim.get_str(dir.clone());
        play_player_tag(&mut ase, tag.as_str(), curr_anim.is_one_time_anim());
        anim_state.prev_dir = dir.clone();
        anim_state.reset_aseprite_frame_tracker();
    }
}

/// Restart attack/bow clips when a new swing starts without a `PlayerAnimation` change
/// (same state already `Attack`/`Bow`). Needed so long class sheets (wizard) can cut
/// short and resync with weapon cadence like short warrior clips do naturally.
pub fn handle_restart_player_attack_anim(
    mut query: Query<
        (
            Entity,
            &PlayerAnimation,
            &mut AseAnimation,
            &mut PlayerAnimationState,
            &FacingDirection,
        ),
        With<RestartPlayerAttackAnim>,
    >,
    mut commands: Commands,
) {
    for (e, curr_anim, mut ase, mut anim_state, dir) in query.iter_mut() {
        if curr_anim.is_an_attack() || curr_anim.is_shooting_bow() {
            let tag = curr_anim.get_str(dir.clone());
            play_player_tag(&mut ase, tag.as_str(), true);
            anim_state.reset_aseprite_frame_tracker();
        }
        commands.entity(e).remove::<RestartPlayerAttackAnim>();
    }
}

pub fn cleanup_one_time_animations(
    mut query: Query<(
        Entity,
        &PlayerAnimation,
        &mut PlayerAnimationState,
        &AnimationState,
        Option<&crate::attributes::AttackCooldown>,
        Option<&mut AttackAnimationTimer>,
    )>,
    mut finished_events: MessageReader<AnimationEvents>,
    mut commands: Commands,
    time: Res<Time>,
) {
    let mut finished = std::collections::HashSet::new();
    for event in finished_events.read() {
        if let AnimationEvents::Finished(entity) = event {
            finished.insert(*entity);
        }
    }

    for (e, curr_anim, mut anim_state, ultra_state, attack_cooldown_option, attack_timer_option) in
        query.iter_mut()
    {
        if !curr_anim.is_one_time_anim() {
            continue;
        }

        // Use tag-relative frames. Global sheet indices made Walk→Attack look like a
        // wrap on classes whose Attack tags sit earlier in the sheet than Walk
        // (wizard): one attack frame, then false "finished" → Idle → Walk while moving.
        let current_frame = usize::from(ultra_state.relative_frame());
        let mut looped = false;
        if anim_state.suppress_wrap_detect {
            anim_state.prev_aseprite_frame = current_frame;
            anim_state.seen_aseprite_progress = false;
            anim_state.suppress_wrap_detect = false;
        } else {
            looped =
                anim_state.seen_aseprite_progress && current_frame < anim_state.prev_aseprite_frame;
            if current_frame > anim_state.prev_aseprite_frame {
                anim_state.seen_aseprite_progress = true;
            }
            anim_state.prev_aseprite_frame = current_frame;
        }

        let is_attack_clip = curr_anim.is_an_attack() || curr_anim.is_shooting_bow();
        // Attacks already end via Finished + AttackAnimationTimer cadence cut. Never
        // trust wrap detection for them — it's what caused the move+auto-attack glitch.
        let mut animation_finished = finished.contains(&e) || (!is_attack_clip && looped);

        // Weapon cadence wins over clip length. Warrior Attack2 ≈ 245ms so it
        // naturally ends near typical cooldowns; wizard Attack2 ≈ 500–600ms would
        // otherwise finish late, flash Idle, then start the next swing out of sync.
        // Cut the swing once we've played most of the cooldown window (any AS).
        if is_attack_clip {
            if let Some(cooldown) = attack_cooldown_option {
                if let Some(mut attack_timer) = attack_timer_option {
                    attack_timer.0.tick(time.delta());
                    // Ignore stale Finished from a previous swing that was just restarted.
                    if attack_timer.0.elapsed_secs() < 0.02 {
                        animation_finished = false;
                    } else if attack_timer.0.elapsed_secs() >= cooldown.0 * 0.8 {
                        animation_finished = true;
                    }
                }
            }
        }

        if animation_finished {
            commands.entity(e).insert(PlayerAnimation::Idle);
            commands.entity(e).remove::<AttackAnimationTimer>();
        }
    }
}

pub fn preload_player_sprites(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(PlayerSpriteHandles {
        grey: asset_server.load(PlayerGreyAseprite::PATH),
        red: asset_server.load(PlayerRedAseprite::PATH),
        green: asset_server.load(PlayerGreenAseprite::PATH),
        rogue: asset_server.load(PlayerRogueAseprite::PATH),
        thief: asset_server.load(PlayerThiefAseprite::PATH),
        wizard: asset_server.load(PlayerWizardAseprite::PATH),
        hunter: asset_server.load(PlayerHunterAseprite::PATH),
        blue: asset_server.load(PlayerBlueAseprite::PATH),
        slime_pet: asset_server.load(SlimePetSprite::PATH),
        fairy_pet: asset_server.load(FairyPetSprite::PATH),
    });
}

pub fn change_player_class_visuals(
    mut player: Query<
        (Entity, &mut crate::player::skills::PlayerSkills),
        // Gate on `AseAnimation`, not `PlayerClass`. In Bevy 0.19 `Resource` impls
        // `Component` but resources are unique — inserting `PlayerClass` onto the player
        // is stripped while the resource exists, so `Without<PlayerClass>` matched every
        // frame and re-inserted `AnimationState::default()`, freezing the sprite on frame 0.
        (Without<AseAnimation>, With<crate::Player>),
    >,
    mut commands: Commands,
    mut att_event: MessageWriter<AttributeChangeEvent>,
    player_class: Res<PlayerClass>,
    sprite_handles: Res<PlayerSpriteHandles>,
    graphics: Res<crate::assets::Graphics>,
    _unlocked_skills: Res<crate::player::unlocks::UnlockedSkills>,
    _cheat_settings: Res<crate::ui::CheatSettings>,
) {
    for (e, mut player_skills) in player.iter_mut() {
        let class = &player_class.class;
        let (handle, anim) = class.get_anim_data(&sprite_handles);

        // Get class data for `active_skills` (see `VISIBLE_CLASS_SKILL_COUNT`).
        let class_data = graphics.get_class_data(class.clone());
        let active_skills = &class_data.active_skills;

        // Slot 0 is the class movement skill; slots 1–4 may already hold run-start blessings.
        player_skills.active_skill_slot_0 = Some(ActiveSkillChoiceState::new(
            active_skills[0].clone(),
            HeirloomRarity::Common,
        ));

        active_skills[0].add_skill_components(e, &mut commands);

        // Do not `.insert(player_class.clone())` — `PlayerClass` is a Resource.
        commands
            .entity(e)
            .insert(class.clone())
            .insert((
                AseAnimation {
                    aseprite: handle,
                    animation: Animation::tag(anim).with_repeat(AnimationRepeat::Loop),
                },
                AnimationState::default(),
                Sprite::default(),
            ));

        att_event.write(AttributeChangeEvent);
    }
}
