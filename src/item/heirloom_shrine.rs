use bevy::prelude::*;
use bevy_aseprite::{anim::AsepriteAnimation, aseprite, AsepriteBundle};

use crate::{
    assets::Graphics, item::object_actions::ObjectAction, player::skills::HeirloomChoiceQueue,
    ui::key_input_guide::InteractionGuideTrigger,
};

use super::WorldObject;

#[derive(Component)]
pub struct HeirloomShrineState {
    pub is_used: bool,
}

// TODO: Create proper aseprite asset for heirloom shrine
// For now, using CombatShrine sprite as a placeholder
aseprite!(pub HeirloomMerchantSprite, "textures/heirloom.ase");

pub fn add_heirloom_shrine_visuals_on_spawn(
    mut commands: Commands,
    new_shrines: Query<
        (Entity, &WorldObject, &Transform),
        Or<(Added<WorldObject>, Changed<WorldObject>)>,
    >,
    graphics: Res<Graphics>,
) {
    for (e, obj, t) in new_shrines.iter() {
        if obj == &WorldObject::HeirloomShrine {
            commands
                .entity(e)
                .insert(AsepriteBundle {
                    transform: *t,
                    animation: AsepriteAnimation::from(HeirloomMerchantSprite::tags::IDLE),
                    aseprite: graphics.heirloom_shrine_anim.as_ref().unwrap().clone(),
                    ..default()
                })
                .insert(Name::new("HEIRLOOM_SHRINE"));
        } else if obj == &WorldObject::HeirloomShrineDone {
            commands
                .entity(e)
                .insert(AsepriteBundle {
                    transform: *t,
                    animation: AsepriteAnimation::from(HeirloomMerchantSprite::tags::DONE),
                    aseprite: graphics.heirloom_shrine_anim.as_ref().unwrap().clone(),
                    ..default()
                })
                .insert(Name::new("HEIRLOOM_SHRINE_DONE"));
        }
    }
}

pub fn handle_heirloom_shrine_completion(
    shrines: Query<(Entity, &HeirloomShrineState)>,
    mut commands: Commands,
) {
    for (e, shrine) in shrines.iter() {
        if shrine.is_used {
            commands
                .entity(e)
                .insert(WorldObject::HeirloomShrineDone)
                .remove::<ObjectAction>()
                .remove::<InteractionGuideTrigger>()
                .remove::<HeirloomShrineState>();
        }
    }
}

/// Populate the HeirloomChoiceQueue when the shrine UI opens
pub fn handle_heirloom_shrine_ui_setup(
    mut skills_queue: ResMut<HeirloomChoiceQueue>,
    shrine_query: Query<&HeirloomShrineState>,
    player_atts: Query<&crate::attributes::ItemAttributes, With<crate::player::Player>>,
) {
    // Check if there are any shrines that just got activated (is_used = false)
    let shrine_just_activated = shrine_query.iter().any(|shrine| !shrine.is_used);

    if shrine_just_activated {
        // Generate 3 random heirlooms (only if queue is empty per add_new_skills_after_levelup logic)
        let mut rng = rand::thread_rng();
        let loot_bonus = player_atts.get_single().map(|a| a.loot_rate.value).unwrap_or(0);
        skills_queue.add_new_skills_after_levelup(&mut rng, loot_bonus);
    }
}
