use crate::combat::EnemyDeathEvent;
use crate::enemy::{spawner::MobSpawningPaused, Mob};
use crate::night::EraTimer;
use crate::player::Player;
use crate::ui::tips::{SeenTips, Tip, TipEvent};
use crate::world::dimension::{Era, EraManager};
use bevy::prelude::*;
use bevy_aseprite::anim::AsepriteAnimation;
use bevy_aseprite::aseprite;
use std::collections::HashSet;

aseprite!(pub Portal, "textures/portal/portal.ase");
aseprite!(pub UIPortal, "textures/portal/portal_large.aseprite");

pub fn handle_player_near_portal(
    player_query: Query<&Transform, With<Player>>,
    mut portal_query: Query<(&GlobalTransform, &mut AsepriteAnimation), With<TimePortal>>,
) {
    for player_transform in player_query.iter() {
        for (portal_transform, mut anim) in portal_query.iter_mut() {
            let distance = player_transform
                .translation
                .distance(portal_transform.translation());
            if distance <= 32. && anim.current_frame() <= 8 {
                *anim = AsepriteAnimation::from(Portal::tags::ERA2);
            }
        }
    }
    for (_portal_transform, mut anim) in portal_query.iter_mut() {
        if anim.current_frame() == 35 {
            *anim = AsepriteAnimation::from(Portal::tags::IDLE);
        }
    }
}

/// Component marker for the time portal entity
#[derive(Component, Default, Debug)]
pub struct TimePortal;

/// Resource to track which eras have had their bosses killed
#[derive(Resource, Default, Debug, Clone)]
pub struct BossKillTracker {
    pub killed_eras: HashSet<Era>,
}

impl BossKillTracker {
    pub fn mark_boss_killed(&mut self, era: Era) {
        self.killed_eras.insert(era);
    }

    pub fn is_boss_killed(&self, era: &Era) -> bool {
        self.killed_eras.contains(era)
    }
}

/// System to track boss kills and update the tracker
pub fn track_boss_kills(
    mut death_events: EventReader<EnemyDeathEvent>,
    mob_query: Query<&Mob>,
    era_manager: Res<EraManager>,
    mut boss_kill_tracker: ResMut<BossKillTracker>,
    era_timer: Res<EraTimer>,
    mut mob_spawning_paused: ResMut<MobSpawningPaused>,
    mut tip_event: EventWriter<TipEvent>,
    seen_tips: Res<SeenTips>,
) {
    for death_event in death_events.iter() {
        if let Ok(mob) = mob_query.get(death_event.entity) {
            // Only RedMushking counts for Era::Main (Act1 achievement)
            // StoneGolem is a boss but doesn't count for era completion
            if mob == &Mob::RedMushking {
                // Mark the current era's boss as killed
                boss_kill_tracker.mark_boss_killed(era_manager.current_era.clone());
                info!("Boss killed in era {:?}", era_manager.current_era);

                if (era_manager.current_era == Era::Main || era_manager.current_era == Era::Second)
                    && era_timer.remaining_seconds > 0.0
                {
                    mob_spawning_paused.paused = true;

                    if !seen_tips.has_seen(&Tip::PeacefulPeriod) {
                        tip_event.send(TipEvent {
                            tip: Tip::PeacefulPeriod,
                            pos: Vec3::new(-184., -116., 95.),
                        });
                    }
                }
            } else if mob.is_boss() && mob != &Mob::StoneGolem {
                // Other bosses (for future eras) still count
                boss_kill_tracker.mark_boss_killed(era_manager.current_era.clone());
                info!("Boss killed in era {:?}", era_manager.current_era);

                if (era_manager.current_era == Era::Main || era_manager.current_era == Era::Second)
                    && era_timer.remaining_seconds > 0.0
                {
                    mob_spawning_paused.paused = true;

                    if !seen_tips.has_seen(&Tip::PeacefulPeriod) {
                        tip_event.send(TipEvent {
                            tip: Tip::PeacefulPeriod,
                            pos: Vec3::new(-184., -116., 90.),
                        });
                    }
                }
            }
        }
    }
}
