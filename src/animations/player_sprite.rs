// idle
// walk
// run (run stop)
// roll
// parry (parry success)
// attack (2 forms)
// sprint
// bowbasic

//when direction changes, we need to assign a new animation handle
//
use bevy::prelude::*;
use bevy_aseprite::{anim::AsepriteAnimation, aseprite, Aseprite};

use crate::{
    attributes::AttributeChangeEvent,
    inputs::FacingDirection,
    item::WorldObject,
    player::skills::{ActiveSkillChoiceState, HeirloomRarity, PlayerClass},
    FairyPetSprite, SlimePetSprite,
};

/// Timer to track how long an attack animation has been playing. Added
/// whenever the player triggers an attack animation and removed when the
/// animation finishes — stored `SparseSet` so attack animations don't move
/// the player to a new archetype on every swing.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct AttackAnimationTimer(pub Timer);

aseprite!(pub PlayerRedAseprite, "textures/player/player_red.aseprite");
aseprite!(pub PlayerBlueAseprite, "textures/player/player_blue.aseprite");
aseprite!(pub PlayerGreyAseprite, "textures/player/player_grey.aseprite");
aseprite!(pub PlayerGreenAseprite, "textures/player/player_green.aseprite");
aseprite!(pub PlayerRogueAseprite, "textures/player/player_rogue.aseprite");

#[derive(Resource)]
pub struct PlayerSpriteHandles {
    pub grey: Handle<Aseprite>,
    pub red: Handle<Aseprite>,
    pub green: Handle<Aseprite>,
    pub blue: Handle<Aseprite>,
    pub rogue: Handle<Aseprite>,

    pub slime_pet: Handle<Aseprite>,
    pub fairy_pet: Handle<Aseprite>,
}
aseprite!(pub PlayerDeadAseprite, "textures/player/player_dead.aseprite");

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
        let dir_str = dir.get_anim_dir_str();
        match self {
            PlayerAnimation::Idle => format!("Idle{}", dir_str),
            PlayerAnimation::Walk => format!("Walk{}", dir_str),
            PlayerAnimation::Run => format!("Run{}", dir_str),
            PlayerAnimation::Roll => format!("Roll{}", dir_str),
            PlayerAnimation::Parry => format!("Parry{}", dir_str),
            PlayerAnimation::ParryHit => format!("ParryHit{}", dir_str),
            PlayerAnimation::Spear => format!("Spear{}", dir_str),
            PlayerAnimation::Attack => format!("Attack2{}", dir_str),
            PlayerAnimation::Bow => format!("Bow{}", dir_str),
            PlayerAnimation::Lunge => format!("Lunge{}", dir_str),
            PlayerAnimation::RunAttack2 => format!("RunAttack2{}", dir_str),
            PlayerAnimation::Teleport => format!("Teleport{}", dir_str),
            PlayerAnimation::SpinAttack => "SpinAttackFront".to_string(),
        }
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
}
impl PlayerAnimationState {
    pub fn new() -> Self {
        Self {
            prev_dir: FacingDirection::Down,
        }
    }
}

pub fn handle_anim_change_when_player_dir_changes(
    mut new_dir_query: Query<
        (
            &FacingDirection,
            &PlayerAnimation,
            &mut AsepriteAnimation,
            &mut PlayerAnimationState,
            &mut TextureAtlasSprite,
        ),
        Or<(Changed<FacingDirection>, Changed<PlayerAnimation>)>,
    >,
) {
    for (new_dir, curr_anim, mut prev_anim_state, mut prev_dir, mut sprite) in
        new_dir_query.iter_mut()
    {
        if curr_anim.is_dir_locked() && !prev_anim_state.just_finished() {
            // Left/right share the same Aseprite tag (`…Side`); only `flip_x` differs.
            // Locked animations skip full tag swaps mid-playback, but during rapid
            // attack chains (e.g. auto attack) we must still refresh horizontal flip.
            let side_to_side = matches!(
                (&prev_dir.prev_dir, new_dir),
                (
                    FacingDirection::Left | FacingDirection::Right,
                    FacingDirection::Left | FacingDirection::Right,
                )
            );
            if side_to_side {
                sprite.flip_x = new_dir == &FacingDirection::Left;
                prev_dir.prev_dir = new_dir.clone();
            }
            continue;
        }

        match new_dir {
            FacingDirection::Up => {
                *prev_anim_state = AsepriteAnimation::from(curr_anim.get_str(new_dir.clone()));

                sprite.flip_x = false;
            }
            FacingDirection::Down => {
                *prev_anim_state = AsepriteAnimation::from(curr_anim.get_str(new_dir.clone()));
                sprite.flip_x = false;
            }
            FacingDirection::Left | FacingDirection::Right => {
                *prev_anim_state = AsepriteAnimation::from(curr_anim.get_str(new_dir.clone()));
                if new_dir == &FacingDirection::Left {
                    sprite.flip_x = true;
                } else {
                    sprite.flip_x = false;
                }
            }
        }

        prev_dir.prev_dir = new_dir.clone();
    }
}

pub fn handle_player_animation_change(
    mut query: Query<
        (
            &PlayerAnimation,
            &mut AsepriteAnimation,
            &mut PlayerAnimationState,
            &FacingDirection,
        ),
        Changed<PlayerAnimation>,
    >,
) {
    for (curr_anim, mut aseprite_anim, mut prev_dir, dir) in query.iter_mut() {
        *aseprite_anim = AsepriteAnimation::from(curr_anim.get_str(dir.clone()));
        prev_dir.prev_dir = dir.clone();
    }
}

pub fn cleanup_one_time_animations(
    mut query: Query<(
        Entity,
        &PlayerAnimation,
        &AsepriteAnimation,
        Option<&crate::attributes::AttackCooldown>,
        Option<&mut AttackAnimationTimer>,
    )>,
    mut commands: Commands,
    time: Res<Time>,
) {
    for (e, curr_anim, anim_state, attack_cooldown_option, attack_timer_option) in query.iter_mut()
    {
        if curr_anim.is_one_time_anim() {
            let mut animation_finished = anim_state.just_finished();

            // Allow attack animations to be interrupted early if attack cooldown is very low
            // This prevents animation duration from being the bottleneck for attack speed
            if curr_anim.is_an_attack() || curr_anim.is_shooting_bow() {
                if let Some(cooldown) = attack_cooldown_option {
                    if let Some(mut attack_timer) = attack_timer_option {
                        attack_timer.0.tick(time.delta());
                        // If attack cooldown is less than 0.25s (typical animation duration),
                        // allow the animation to finish after the cooldown duration has elapsed
                        if cooldown.0 < 0.25 && attack_timer.0.elapsed_secs() >= cooldown.0 * 0.8 {
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
}

pub fn preload_player_sprites(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(PlayerSpriteHandles {
        grey: asset_server.load(PlayerGreyAseprite::PATH),
        red: asset_server.load(PlayerRedAseprite::PATH),
        green: asset_server.load(PlayerGreenAseprite::PATH),
        rogue: asset_server.load(PlayerRogueAseprite::PATH),
        blue: asset_server.load(PlayerBlueAseprite::PATH),
        slime_pet: asset_server.load(SlimePetSprite::PATH),
        fairy_pet: asset_server.load(FairyPetSprite::PATH),
    });
}

pub fn change_player_class_visuals(
    mut player: Query<
        (Entity, &mut crate::player::skills::PlayerSkills),
        (Without<PlayerClass>, With<crate::Player>),
    >,
    mut commands: Commands,
    mut att_event: EventWriter<AttributeChangeEvent>,
    player_class: Res<PlayerClass>,
    sprite_handles: Res<PlayerSpriteHandles>,
    graphics: Res<crate::assets::Graphics>,
    unlocked_skills: Res<crate::player::unlocks::UnlockedSkills>,
    cheat_settings: Res<crate::ui::CheatSettings>,
) {
    for (e, mut player_skills) in player.iter_mut() {
        let class = &player_class.class;
        let (handle, anim) = class.get_anim_data(&sprite_handles);

        // Get class data to access all 4 active skills
        let class_data = graphics.get_class_data(class.clone());
        let active_skills = &class_data.active_skills;

        // Assign each class skill slot only if it's unlocked. Slots 0-1 are
        // always free; slots 2-3 require purchasing with Time Fragments
        // unless `bypass_class_unlocks` cheat is set.
        let bypass = cheat_settings.bypass_class_unlocks;
        let slot_choice = |i: usize| -> Option<ActiveSkillChoiceState> {
            if bypass || unlocked_skills.is_unlocked(class, i) {
                Some(ActiveSkillChoiceState::new(
                    active_skills[i].clone(),
                    HeirloomRarity::Common,
                ))
            } else {
                None
            }
        };
        player_skills.active_skill_slot_0 = slot_choice(0);
        player_skills.active_skill_slot_1 = slot_choice(1);
        player_skills.active_skill_slot_2 = slot_choice(2);
        player_skills.active_skill_slot_3 = slot_choice(3);

        // Only register skill components for slots the player actually has unlocked.
        for (i, skill) in active_skills.iter().enumerate() {
            if bypass || unlocked_skills.is_unlocked(class, i) {
                skill.add_skill_components(e, &mut commands);
            }
        }

        //att update event
        commands
            .entity(e)
            .remove::<TextureAtlasSprite>()
            .insert(class.clone())
            .insert(player_class.clone())
            .insert(handle)
            .insert(AsepriteAnimation::from(anim));

        att_event.send(AttributeChangeEvent);
    }
}
