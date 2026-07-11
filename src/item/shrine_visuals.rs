use bevy::prelude::*;
use bevy_aseprite::{anim::AsepriteAnimation, aseprite, AsepriteBundle};

use crate::assets::Graphics;

use super::WorldObject;

aseprite!(pub ShrineEye, "textures/shrines/shrine_eye.ase");

/// Marker on the shrine_eye child entity spawned above every overworld shrine.
#[derive(Component)]
pub struct ShrineEyeMarker;

/// Present when a shrine needs repair. Eye uses the Broken tag until repaired.
/// Repair flow is wired later; this is the visual hook.
#[derive(Component)]
pub struct ShrineNeedsRepair;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ShrineEyeState {
    Idle,
    Broken,
    Done,
}

impl ShrineEyeState {
    fn tag(self) -> &'static str {
        match self {
            Self::Idle => ShrineEye::tags::IDLE,
            Self::Broken => ShrineEye::tags::BROKEN,
            Self::Done => ShrineEye::tags::DONE,
        }
    }
}

/// Overworld shrines that use standalone PNGs under `textures/shrines/` plus a shrine_eye overlay.
pub fn uses_standalone_shrine_texture(obj: &WorldObject) -> bool {
    shrine_texture_info(obj).is_some()
}

fn shrine_texture_info(obj: &WorldObject) -> Option<(&'static str, Vec2, f32)> {
    // (path, custom_size, eye local Y offset above shrine center)
    match obj {
        WorldObject::GambleShrine | WorldObject::GambleShrineDone => {
            Some(("textures/shrines/WatchtowerShrine.png", Vec2::new(37., 77.), 56.))
        }
        WorldObject::MicrowaveShrine | WorldObject::MicrowaveShrineDone => {
            Some(("textures/shrines/SwapShrine.png", Vec2::new(45., 45.), 44.))
        }
        WorldObject::ActiveSkillShrine | WorldObject::ActiveSkillShrineDone => {
            Some(("textures/shrines/SkillShrine.png", Vec2::new(35., 45.), 44.))
        }
        WorldObject::BlacksmithMerchant | WorldObject::BlacksmithMerchantDone => {
            Some(("textures/shrines/MerchantShrine.png", Vec2::new(32., 30.), 40.))
        }
        WorldObject::HeirloomShrine | WorldObject::HeirloomShrineDone => {
            Some(("textures/shrines/HeirloomShrine.png", Vec2::new(64., 41.), 42.))
        }
        WorldObject::CombatShrine | WorldObject::CombatShrineDone => {
            Some(("textures/shrines/CombatShrine.png", Vec2::new(35., 60.), 50.))
        }
        WorldObject::ChaosTotem | WorldObject::ChaosTotemDone => {
            Some(("textures/shrines/ChaosShrine.png", Vec2::new(31., 35.), 40.))
        }
        WorldObject::CauldronShrine | WorldObject::CauldronShrineDone => {
            Some(("textures/shrines/CauldronShrine.png", Vec2::new(28., 29.), 38.))
        }
        WorldObject::WellShrine | WorldObject::WellShrineDone => {
            Some(("textures/shrines/WellShrine.png", Vec2::new(38., 56.), 48.))
        }
        _ => None,
    }
}

fn is_consumed_shrine(obj: &WorldObject) -> bool {
    matches!(
        obj,
        WorldObject::GambleShrineDone
            | WorldObject::MicrowaveShrineDone
            | WorldObject::ActiveSkillShrineDone
            | WorldObject::BlacksmithMerchantDone
            | WorldObject::HeirloomShrineDone
            | WorldObject::CombatShrineDone
            | WorldObject::ChaosTotemDone
            | WorldObject::CauldronShrineDone
            | WorldObject::WellShrineDone
    )
}

/// True when this shrine has been used up (`*Done` world object).
pub fn shrine_is_consumed(obj: &WorldObject) -> bool {
    is_consumed_shrine(obj)
}

/// Present on the shrine_eye child while it plays the Done tag (including the brief
/// post-repair Done pose before the world object flips to `*Done`).
#[derive(Component)]
pub struct ShrineEyeDoneVisual;

fn eye_state_for(obj: &WorldObject, needs_repair: bool) -> ShrineEyeState {
    if is_consumed_shrine(obj) {
        ShrineEyeState::Done
    } else if needs_repair {
        ShrineEyeState::Broken
    } else {
        ShrineEyeState::Idle
    }
}

fn apply_eye_state(
    commands: &mut Commands,
    shrine_entity: Entity,
    children: Option<&Children>,
    eye_markers: &Query<(), With<ShrineEyeMarker>>,
    eye_handle: &Handle<bevy_aseprite::Aseprite>,
    eye_y: f32,
    eye_state: ShrineEyeState,
) {
    let mut existing_eye = None;
    if let Some(children) = children {
        for child in children.iter() {
            if eye_markers.get(*child).is_ok() {
                existing_eye = Some(*child);
                break;
            }
        }
    }

    if let Some(eye_entity) = existing_eye {
        if let Some(mut eye_commands) = commands.get_entity(eye_entity) {
            eye_commands.insert(AsepriteAnimation::from(eye_state.tag()));
            if eye_state == ShrineEyeState::Done {
                eye_commands.insert(ShrineEyeDoneVisual);
            } else {
                eye_commands.remove::<ShrineEyeDoneVisual>();
            }
        }
    } else if let Some(mut entity_commands) = commands.get_entity(shrine_entity) {
        entity_commands.with_children(|parent| {
            let mut eye = parent.spawn((
                ShrineEyeMarker,
                AsepriteBundle {
                    aseprite: eye_handle.clone(),
                    animation: AsepriteAnimation::from(eye_state.tag()),
                    transform: Transform::from_translation(Vec3::new(0., eye_y, 1.)),
                    ..default()
                },
                Name::new("ShrineEye"),
            ));
            if eye_state == ShrineEyeState::Done {
                eye.insert(ShrineEyeDoneVisual);
            }
        });
    }
}

/// Apply standalone PNG body + shrine_eye overlay for overworld shrines.
pub fn apply_shrine_visuals_on_spawn(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    graphics: Res<Graphics>,
    shrines: Query<
        (
            Entity,
            &WorldObject,
            Option<&Children>,
            Option<&ShrineNeedsRepair>,
            Option<&crate::item::shrine_repair::ShrineRepairChannel>,
            Option<&crate::item::shrine_repair::PendingShrineRepairFinish>,
        ),
        Or<(
            Added<WorldObject>,
            Changed<WorldObject>,
            Added<ShrineNeedsRepair>,
        )>,
    >,
    eye_markers: Query<(), With<ShrineEyeMarker>>,
    eye_one_shots: Query<(), With<crate::item::shrine_repair::ShrineEyeOneShot>>,
) {
    let Some(eye_handle) = graphics.shrine_eye.as_ref() else {
        return;
    };

    for (entity, obj, children, needs_repair, repair_channel, pending_finish) in shrines.iter() {
        let Some((path, size, eye_y)) = shrine_texture_info(obj) else {
            continue;
        };
        let Some(mut entity_commands) = commands.get_entity(entity) else {
            continue;
        };

        let texture: Handle<Image> = asset_server.load(path);
        entity_commands
            .insert((
                texture,
                Sprite {
                    custom_size: Some(size),
                    ..default()
                },
                VisibilityBundle {
                    visibility: Visibility::Inherited,
                    ..default()
                },
            ))
            .remove::<TextureAtlasSprite>()
            .remove::<Handle<TextureAtlas>>();

        // Don't clobber Startup / FlashGreen / FlashRed / Done-wait while repair runs.
        let eye_busy = repair_channel.is_some()
            || pending_finish.is_some()
            || children.map_or(false, |c| c.iter().any(|child| eye_one_shots.get(*child).is_ok()));
        if eye_busy {
            continue;
        }

        let eye_state = eye_state_for(obj, needs_repair.is_some());
        apply_eye_state(
            &mut commands,
            entity,
            children,
            &eye_markers,
            eye_handle,
            eye_y,
            eye_state,
        );
    }
}

/// When `ShrineNeedsRepair` is removed, refresh the eye back to Idle (unless consumed).
/// Skips when FlashGreen/FlashRed is playing or Done-wait is pending.
pub fn sync_shrine_eye_after_repair(
    mut commands: Commands,
    mut removed: RemovedComponents<ShrineNeedsRepair>,
    shrines: Query<(
        &WorldObject,
        Option<&Children>,
        Option<&crate::item::shrine_repair::PendingShrineRepairFinish>,
    )>,
    eye_markers: Query<(), With<ShrineEyeMarker>>,
    eye_one_shots: Query<(), With<crate::item::shrine_repair::ShrineEyeOneShot>>,
    graphics: Res<Graphics>,
) {
    let Some(eye_handle) = graphics.shrine_eye.as_ref() else {
        return;
    };

    for entity in removed.iter() {
        let Ok((obj, children, pending_finish)) = shrines.get(entity) else {
            continue;
        };
        // Channelled repair already set FlashGreen → Done; don't overwrite mid-sequence.
        if pending_finish.is_some() {
            continue;
        }
        if let Some(children) = children {
            if children.iter().any(|c| eye_one_shots.get(*c).is_ok()) {
                continue;
            }
        }
        let Some((_, _, eye_y)) = shrine_texture_info(obj) else {
            continue;
        };
        let eye_state = eye_state_for(obj, false);
        apply_eye_state(
            &mut commands,
            entity,
            children,
            &eye_markers,
            eye_handle,
            eye_y,
            eye_state,
        );
    }
}
