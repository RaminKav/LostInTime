//! Runtime helpers / systems for newer major blessings.

use bevy::prelude::*;
use rand::seq::SliceRandom;
use rand::Rng;

use crate::{
    attributes::{
        modifiers::{ModifyHealthEvent, ModifyManaEvent},
        Attack,
    },
    blessings::{BlessingTriggerCounts, MajorBlessing, OwnedMajorBlessings},
    combat::{
        status_effects::{
            try_add_frail_stacks, try_add_poison_stacks, try_add_slow_stacks, MobStatusEffects,
            StatusEffectEvent,
        },
        HitEvent, ObjBreakEvent,
    },
    custom_commands::CommandsExt,
    enemy::Mob,
    item::{
        projectile::{Projectile, RangedAttackEvent},
        WorldObject,
    },
    player::{
        skills::{Heirloom, ManaGainSource},
        Player,
    },
    proto::proto_param::ProtoParam,
    world::world_helpers::tile_pos_to_world_pos,
    GameState,
};

/// Random heirloom-effect family rolled onto certain major blessing cards.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HeirloomEffectFamily {
    LightningStrikes,
    Echoes,
    IceExplosions,
    Summons,
}

impl HeirloomEffectFamily {
    pub const POOL: [Self; 4] = [
        Self::LightningStrikes,
        Self::Echoes,
        Self::IceExplosions,
        Self::Summons,
    ];

    pub fn roll(rng: &mut impl Rng) -> Self {
        *Self::POOL.choose(rng).unwrap_or(&Self::Echoes)
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::LightningStrikes => "Lightning Strikes",
            Self::Echoes => "Echoes",
            Self::IceExplosions => "Ice Explosions",
            Self::Summons => "Summons",
        }
    }

    pub fn matches_hit(self, hit: &HitEvent) -> bool {
        match self {
            Self::LightningStrikes => {
                hit.hit_with_projectile == Some(Projectile::Lightning)
                    || hit.from_heirloom_effect == Some(Heirloom::CoinLightning)
                    || hit.from_heirloom_effect == Some(Heirloom::KillLightning)
                    || hit.from_heirloom_effect == Some(Heirloom::ManaRegenLightning)
            }
            Self::Echoes => {
                hit.hit_with_projectile == Some(Projectile::Echo)
                    || hit.from_heirloom_effect == Some(Heirloom::OnHitEcho)
                    || hit.from_heirloom_effect == Some(Heirloom::HealEcho)
                    || hit.from_heirloom_effect == Some(Heirloom::SkillEcho)
                    || hit.from_heirloom_effect == Some(Heirloom::ParryEcho)
            }
            Self::IceExplosions => {
                hit.hit_with_projectile == Some(Projectile::IceExplosionAOE)
                    || hit.from_heirloom_effect == Some(Heirloom::IceStaffAoE)
                    || hit.from_heirloom_effect == Some(Heirloom::FrozenAoE)
            }
            Self::Summons => matches!(
                hit.from_heirloom_effect,
                Some(
                    Heirloom::AntFarm
                        | Heirloom::StoneTooth
                        | Heirloom::SummonRing
                        | Heirloom::HealSummons
                )
            ),
        }
    }
}

/// Status applied by the random-pool family-hit majors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectPoolStatus {
    Freeze,
    Frail,
    Poison,
}

/// One rolled "Your {family} apply {status}" major.
#[derive(Clone, Copy, Debug)]
pub struct EffectPoolStatusChance {
    pub family: HeirloomEffectFamily,
    pub status: EffectPoolStatus,
}

/// Live list of effect-pool status majors (player may own more than one).
#[derive(Component, Clone, Debug, Default)]
pub struct EffectPoolStatusChances {
    pub entries: Vec<EffectPoolStatusChance>,
}

impl EffectPoolStatusChances {
    pub fn push(&mut self, entry: EffectPoolStatusChance) {
        self.entries.push(entry);
    }
}

/// Counts mana-costing heirloom triggers for Heirloom Overclock (every 5th is free).
#[derive(Component, Default, Debug, Clone)]
pub struct HeirloomManaOverclock {
    pub triggers: u32,
}

/// Returns mana actually charged after Overclock. `base_cost` should already include DiscountMP.
/// When a cast is made free, increments `blessing_triggers` if provided.
pub fn overclock_mana_cost(
    majors: &OwnedMajorBlessings,
    overclock: Option<&mut HeirloomManaOverclock>,
    base_cost: i32,
    blessing_triggers: Option<&mut BlessingTriggerCounts>,
) -> i32 {
    if base_cost <= 0 || !majors.has(MajorBlessing::HeirloomOverclock) {
        return base_cost.max(0);
    }
    let Some(oc) = overclock else {
        return base_cost;
    };
    oc.triggers = oc.triggers.saturating_add(1);
    if oc.triggers % 5 == 0 {
        if let Some(triggers) = blessing_triggers {
            triggers.increment(MajorBlessing::HeirloomOverclock);
        }
        0
    } else {
        base_cost
    }
}

pub fn format_effect_pool_description(
    family: HeirloomEffectFamily,
    status: EffectPoolStatus,
) -> Vec<String> {
    let status_name = match status {
        EffectPoolStatus::Freeze => "freeze",
        EffectPoolStatus::Frail => "frail",
        EffectPoolStatus::Poison => "poison",
    };
    vec![
        format!("Your {} apply", family.display_name()),
        format!("{}.", status_name),
    ]
}

/// Viral Conductor: poison ticks have 1% chance to lightning a nearby enemy.
pub fn handle_viral_conductor(
    mut ticks: MessageReader<HitEvent>,
    majors: Query<&OwnedMajorBlessings, With<Player>>,
    player: Query<&Attack, With<Player>>,
    mobs: Query<(Entity, &GlobalTransform), (With<Mob>, Without<Player>)>,
    mut ranged: MessageWriter<RangedAttackEvent>,
    mut blessing_triggers: ResMut<BlessingTriggerCounts>,
) {
    let Ok(majors) = majors.single() else {
        return;
    };
    if !majors.has(MajorBlessing::ViralConductor) {
        return;
    }
    let Ok(attack) = player.single() else {
        return;
    };
    let mut rng = rand::thread_rng();
    for hit in ticks.read() {
        if hit.from_heirloom_effect != Some(Heirloom::PoisonStacks) {
            continue;
        }
        if !rng.gen_bool(0.05) {
            continue;
        }
        blessing_triggers.increment(MajorBlessing::ViralConductor);
        let Ok((_, source_t)) = mobs.get(hit.hit_entity) else {
            continue;
        };
        let source_pos = source_t.translation().truncate();
        let Some((target_e, target_t)) = mobs
            .iter()
            .filter(|(e, _)| *e != hit.hit_entity)
            .min_by(|(_, a), (_, b)| {
                let da = a.translation().truncate().distance_squared(source_pos);
                let db = b.translation().truncate().distance_squared(source_pos);
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            })
            .or_else(|| mobs.get(hit.hit_entity).ok().map(|(e, t)| (e, t)))
        else {
            continue;
        };
        let _ = target_e;
        let pos = target_t.translation().truncate() + Vec2::new(0., 48.);
        ranged.write(RangedAttackEvent {
            projectile: Projectile::Lightning,
            direction: Vec2::ZERO,
            from_enemy: false,
            from_entity: None,
            is_followup_proj: true,
            mana_cost: None,
            mana_cost_heirloom: None,
            dmg_override: Some((attack.0 as f32).round().max(1.) as i32),
            pos_override: Some(pos),
            spawn_delay: 0.0,
        });
    }
}

/// Random-pool majors: family hits always apply freeze / frail / poison.
pub fn handle_effect_pool_status_chance(
    mut hits: MessageReader<HitEvent>,
    pool: Query<&EffectPoolStatusChances, With<Player>>,
    majors: Query<&OwnedMajorBlessings, With<Player>>,
    mut statuses: Query<&mut MobStatusEffects>,
    mut status_event: MessageWriter<StatusEffectEvent>,
    mut blessing_triggers: ResMut<BlessingTriggerCounts>,
) {
    let Ok(majors) = majors.single() else {
        return;
    };
    let Ok(pool) = pool.single() else {
        return;
    };
    let poison_tick_secs = 0.5 / majors.poison_tick_multiplier();
    let apply_extra = majors.has(MajorBlessing::StatusApplyExtra);
    if pool.entries.is_empty() {
        return;
    }
    for hit in hits.read() {
        let Ok(mut status) = statuses.get_mut(hit.hit_entity) else {
            continue;
        };
        for chance in &pool.entries {
            if !chance.family.matches_hit(hit) {
                continue;
            }
            let applied = match chance.status {
                EffectPoolStatus::Freeze => {
                    blessing_triggers.increment(MajorBlessing::EffectPoolApplyFreeze);
                    try_add_slow_stacks(
                        hit.hit_entity,
                        status.as_mut(),
                        &mut status_event,
                        1,
                        None,
                        apply_extra,
                    )
                }
                EffectPoolStatus::Frail => {
                    blessing_triggers.increment(MajorBlessing::EffectPoolApplyFrail);
                    try_add_frail_stacks(
                        hit.hit_entity,
                        status.as_mut(),
                        &mut status_event,
                        1,
                        apply_extra,
                    )
                }
                EffectPoolStatus::Poison => {
                    blessing_triggers.increment(MajorBlessing::EffectPoolApplyPoison);
                    try_add_poison_stacks(
                        hit.hit_entity,
                        status.as_mut(),
                        &mut status_event,
                        1,
                        poison_tick_secs,
                        3.0,
                        apply_extra,
                    )
                }
            };
            if applied && apply_extra {
                blessing_triggers.increment(MajorBlessing::StatusApplyExtra);
            }
        }
    }
}

/// Mana Regen Heal: natural mana regen also heals 1 HP.
pub fn handle_mana_regen_heal(
    mut mana_events: MessageReader<ModifyManaEvent>,
    majors: Query<&OwnedMajorBlessings, With<Player>>,
    mut heal: MessageWriter<ModifyHealthEvent>,
    mut blessing_triggers: ResMut<BlessingTriggerCounts>,
) {
    let Ok(majors) = majors.single() else {
        return;
    };
    if !majors.has(MajorBlessing::ManaRegenHeal) {
        return;
    }
    for event in mana_events.read() {
        if event.0 > 0 && event.1 == Some(ManaGainSource::ManaRegen) {
            blessing_triggers.increment(MajorBlessing::ManaRegenHeal);
            heal.write(ModifyHealthEvent(1));
        }
    }
}

/// Shared eligibility for Resonant Ruin / Scavenger's Eye: breakables that are not
/// grass (any era), crates, or XP jugs. Small props (flowers, rocks, etc.) count.
fn is_object_blessing_eligible(obj: WorldObject) -> bool {
    !matches!(
        obj,
        WorldObject::Grass
            | WorldObject::Grass2
            | WorldObject::Grass3
            | WorldObject::GrassBlock
            | WorldObject::Era2Grass
            | WorldObject::Era2Grass2
            | WorldObject::Era2Grass3
            | WorldObject::DesertGrass1
            | WorldObject::DesertGrass2
            | WorldObject::DesertGrass3
            | WorldObject::DesertGrass4
            | WorldObject::SnowGrass1
            | WorldObject::SnowGrass2
            | WorldObject::SnowGrass3
            | WorldObject::SnowGrass4
            | WorldObject::Crate
            | WorldObject::Crate2
            | WorldObject::CrateBlock
            | WorldObject::DesertCrate
            | WorldObject::DesertCrate2
            | WorldObject::SnowCrate1
            | WorldObject::SnowCrate2
            | WorldObject::SnowCrate3
            | WorldObject::SnowCrate4
            | WorldObject::XPJug
    )
}

/// Object-break echo + loot majors (shared eligible object pool).
pub fn handle_object_break_major_effects(
    mut breaks: MessageReader<ObjBreakEvent>,
    majors: Query<&OwnedMajorBlessings, With<Player>>,
    player: Query<(Entity, &Attack, &GlobalTransform), With<Player>>,
    proto: ProtoParam,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    mut blessing_triggers: ResMut<BlessingTriggerCounts>,
) {
    let Ok(majors) = majors.single() else {
        return;
    };
    let Ok((_player_e, attack, _player_t)) = player.single() else {
        return;
    };
    let mut rng = rand::thread_rng();
    for broken in breaks.read() {
        if !broken.give_drops_and_xp {
            continue;
        }
        if !is_object_blessing_eligible(broken.obj) {
            continue;
        }
        let world_pos = tile_pos_to_world_pos(broken.pos, broken.obj.is_medium_size(&proto));

        if majors.has(MajorBlessing::LargeObjectEcho) {
            blessing_triggers.increment(MajorBlessing::LargeObjectEcho);
            crate::player::melee_skills::spawn_world_echo_hitbox(
                &mut commands,
                &asset_server,
                world_pos.extend(1.),
                attack.0,
                1.0,
                majors.echo_size_multiplier(),
            );
        }

        if majors.has(MajorBlessing::ObjectBreakLoot) {
            // Scavenger's Eye: gem 1%, orb 1%, tome 1.5%, coin 20%.
            const GEM_CHANCE: f64 = 0.01;
            const ORB_CHANCE: f64 = 0.01;
            const TOME_CHANCE: f64 = 0.015;
            const COIN_CHANCE: f64 = 0.20;
            let roll = rng.gen::<f64>();
            let drop = if roll < GEM_CHANCE {
                Some(WorldObject::MagicGem)
            } else if roll < GEM_CHANCE + ORB_CHANCE {
                Some(WorldObject::OrbOfTransformation)
            } else if roll < GEM_CHANCE + ORB_CHANCE + TOME_CHANCE {
                Some(WorldObject::UpgradeTome)
            } else if roll < GEM_CHANCE + ORB_CHANCE + TOME_CHANCE + COIN_CHANCE {
                Some(WorldObject::Coin)
            } else {
                None
            };
            if let Some(obj) = drop {
                blessing_triggers.increment(MajorBlessing::ObjectBreakLoot);
                let pos = world_pos + Vec2::new(rng.gen_range(-8.0..8.0), rng.gen_range(-8.0..8.0));
                commands.spawn_item_from_proto(obj, &proto, pos, 1, None);
            }
        }
    }
}

pub struct MajorNewEffectsPlugin;

impl Plugin for MajorNewEffectsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                handle_viral_conductor,
                handle_effect_pool_status_chance,
                handle_mana_regen_heal,
                handle_object_break_major_effects,
            )
                .run_if(in_state(GameState::Main))
                .run_if(crate::client::is_not_paused),
        );
    }
}
