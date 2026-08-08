use bevy::prelude::*;
use rand::Rng;

use crate::{
    attributes::{
        AttackSpeed, AttributeChangeEvent, ItemAttributes, ManaRegen, MaxMana, ProjectileSize,
    },
    blessings::{MajorBlessing, OwnedMajorBlessings, PendingStatConversionRoll},
    chaos::ChaosTracker,
    player::{combat_heirlooms::HallucinationStatType, Player},
    GameParam, GameState,
};

/// Relative value table for Twisted Exchange (StatConversion).
///
/// Higher value = more precious (more likely to roll).
/// Conversion is value-preserving and normalized so one side is always `1`
/// (amounts rounded to the nearest 0.5).
///
/// Rough value order (high → low): lifesteal, luck, xp, mana regen, health regen,
/// crit, size, speed, dodge, attack, attack speed / crit dmg, mana, health, thorns.
pub const STAT_CONVERSION_WEIGHTS: &[(HallucinationStatType, i32)] = &[
    (HallucinationStatType::Lifesteal, 40),
    (HallucinationStatType::Luck, 34),
    (HallucinationStatType::XPRate, 30),
    (HallucinationStatType::ManaRegen, 26),
    (HallucinationStatType::HealthRegen, 24),
    (HallucinationStatType::Healing, 22),
    (HallucinationStatType::CritChance, 20),
    (HallucinationStatType::Size, 17),
    (HallucinationStatType::Speed, 15),
    (HallucinationStatType::Dodge, 14),
    (HallucinationStatType::Attack, 12),
    (HallucinationStatType::AttackSpeed, 11),
    (HallucinationStatType::CritDamage, 10),
    (HallucinationStatType::Defence, 9),
    (HallucinationStatType::Mana, 8),
    (HallucinationStatType::Health, 4),
    (HallucinationStatType::Thorns, 3),
];

/// Chaos accrues at 1/`STAT_CONVERSION_CHAOS_RATIO_MULT` the target-stat grant rate.
pub const STAT_CONVERSION_CHAOS_RATIO_MULT: i32 = 5;

/// Live "+gain Target per per Source" conversion from the StatConversion major.
///
/// `gain` / `per` are stored in half-units (2 = 1.0, 5 = 2.5). One of them is always 2.
#[derive(Component, Clone, Debug)]
pub struct StatConversion {
    pub source: HallucinationStatType,
    pub target: HallucinationStatType,
    /// Target amount granted, in half-units.
    pub gain: i32,
    /// Source amount required, in half-units.
    pub per: i32,
    /// Last granted target amount (for delta updates).
    pub last_granted: i32,
    /// Chaos granted from this conversion.
    pub last_chaos: f32,
}

/// Bonuses written by [`StatConversion`]; combined in attribute recalc like food/heirloom stats.
#[derive(Component, Clone, Debug, Default)]
pub struct MajorBlessingStatBonuses(pub ItemAttributes);

impl MajorBlessingStatBonuses {
    pub fn clear_and_set(&mut self, stat: HallucinationStatType, amount: i32) {
        self.0 = ItemAttributes::default();
        if amount != 0 {
            add_stat_to_attrs(&mut self.0, stat, amount);
        }
    }

    pub fn as_item_attributes(&self) -> &ItemAttributes {
        &self.0
    }

    fn source_contribution(&self, source: HallucinationStatType) -> i32 {
        match source {
            HallucinationStatType::Attack => self.0.attack.value,
            HallucinationStatType::Health => self.0.health.value,
            HallucinationStatType::Defence => self.0.defence.value,
            HallucinationStatType::CritChance => self.0.crit_chance.value,
            HallucinationStatType::CritDamage => self.0.crit_damage.value,
            HallucinationStatType::Speed => self.0.speed.value,
            HallucinationStatType::Lifesteal => self.0.lifesteal.value,
            HallucinationStatType::Dodge => self.0.dodge.value,
            HallucinationStatType::HealthRegen => self.0.health_regen.value,
            HallucinationStatType::Healing => self.0.healing.value,
            HallucinationStatType::Thorns => self.0.thorns.value,
            HallucinationStatType::XPRate => self.0.xp_rate.value,
            HallucinationStatType::Luck => self.0.loot_rate.value,
            HallucinationStatType::Mana => self.0.mana.value,
            HallucinationStatType::Size => self.0.size.value,
            HallucinationStatType::ManaRegen => self.0.mana_regen.value,
            HallucinationStatType::AttackSpeed => self.0.attack_speed.value,
        }
    }
}

fn add_stat_to_attrs(attrs: &mut ItemAttributes, stat_type: HallucinationStatType, amount: i32) {
    match stat_type {
        HallucinationStatType::Attack => attrs.attack.value += amount,
        HallucinationStatType::Health => attrs.health.value += amount,
        HallucinationStatType::Defence => attrs.defence.value += amount,
        HallucinationStatType::CritChance => attrs.crit_chance.value += amount,
        HallucinationStatType::CritDamage => attrs.crit_damage.value += amount,
        HallucinationStatType::Speed => attrs.speed.value += amount,
        HallucinationStatType::Lifesteal => attrs.lifesteal.value += amount,
        HallucinationStatType::Dodge => attrs.dodge.value += amount,
        HallucinationStatType::HealthRegen => attrs.health_regen.value += amount,
        HallucinationStatType::Healing => attrs.healing.value += amount,
        HallucinationStatType::Thorns => attrs.thorns.value += amount,
        HallucinationStatType::XPRate => attrs.xp_rate.value += amount,
        HallucinationStatType::Luck => attrs.loot_rate.value += amount,
        HallucinationStatType::Mana => attrs.mana.value += amount,
        HallucinationStatType::Size => attrs.size.value += amount,
        HallucinationStatType::ManaRegen => attrs.mana_regen.value += amount,
        HallucinationStatType::AttackSpeed => attrs.attack_speed.value += amount,
    }
}

pub fn roll_weighted_stat(rng: &mut impl Rng) -> (HallucinationStatType, i32) {
    let total: i32 = STAT_CONVERSION_WEIGHTS
        .iter()
        .map(|(_, w)| (*w).max(1))
        .sum();
    let mut roll = rng.gen_range(0..total.max(1));
    for &(stat, weight) in STAT_CONVERSION_WEIGHTS {
        let w = weight.max(1);
        if roll < w {
            return (stat, weight.max(1));
        }
        roll -= w;
    }
    (HallucinationStatType::Attack, 12)
}

/// Round a positive amount to the nearest 0.5, returned in half-units (2 = 1.0).
fn round_to_half_units(x: f32) -> i32 {
    (x * 2.0).round().max(1.0) as i32
}

fn format_half_amount(halves: i32) -> String {
    if halves % 2 == 0 {
        format!("{}", halves / 2)
    } else {
        format!("{:.1}", halves as f32 / 2.0)
    }
}

fn format_per_clause(halves: i32, source_name: &str) -> String {
    if halves <= 2 {
        format!("per {}.", source_name)
    } else {
        format!("per {} {}.", format_half_amount(halves), source_name)
    }
}

/// Value-preserving terms in half-units; one side is always `2` (1.0).
/// Size(17)←Lifesteal(40) => (5, 2) → +2.5 Size per Lifesteal.
/// Crit(20)←Health(4) => (2, 10) → +1 Crit per 5 Health.
pub fn conversion_terms(source_value: i32, target_value: i32) -> (i32, i32) {
    let source_value = source_value.max(1) as f32;
    let target_value = target_value.max(1) as f32;
    let ratio = source_value / target_value; // target gained per 1 source
    if ratio >= 1.0 {
        (round_to_half_units(ratio), 2)
    } else {
        (2, round_to_half_units(1.0 / ratio))
    }
}

/// Pre-rolled StatA/StatB pair shown on the major blessing card and applied on pick.
/// `gain` / `per` are half-units (2 = 1.0, 5 = 2.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResolvedStatConversion {
    /// StatB — source counted for the conversion.
    pub source: HallucinationStatType,
    /// StatA — target granted (`+gain` per `per` source).
    pub target: HallucinationStatType,
    pub gain: i32,
    pub per: i32,
}

impl ResolvedStatConversion {
    /// Always `+1 Chaos per X Source`, with `X` in half-units.
    pub fn chaos_per_halves(self) -> i32 {
        let gain = self.gain.max(1);
        let per = self.per.max(1);
        // X = MULT * per / gain, rounded to nearest 0.5.
        let numer = 2 * STAT_CONVERSION_CHAOS_RATIO_MULT * per;
        ((numer + gain / 2) / gain).max(1)
    }

    /// `(chaos_gain_halves, chaos_per_halves)` — gain is always 2 (1.0).
    pub fn chaos_terms(self) -> (i32, i32) {
        (2, self.chaos_per_halves())
    }
}

pub fn roll_stat_conversion_pair(rng: &mut impl Rng) -> ResolvedStatConversion {
    let (target, target_w) = roll_weighted_stat(rng);
    let mut source = target;
    let mut source_w = target_w;
    for _ in 0..16 {
        let (rolled, w) = roll_weighted_stat(rng);
        if rolled != target {
            source = rolled;
            source_w = w;
            break;
        }
    }
    let (gain, per) = conversion_terms(source_w, target_w);
    ResolvedStatConversion {
        source,
        target,
        gain: gain.max(1),
        per: per.max(1),
    }
}

pub fn format_stat_conversion_description(rolled: &ResolvedStatConversion) -> Vec<String> {
    let chaos_per = rolled.chaos_per_halves();
    vec![
        format!(
            "Gain +{} {}",
            format_half_amount(rolled.gain.max(1)),
            rolled.target.name(),
        ),
        format_per_clause(rolled.per.max(1), rolled.source.name()),
        "Gain +1 Chaos".to_string(),
        format_per_clause(chaos_per, rolled.source.name()),
    ]
}

pub fn format_stat_conversion(rolled: &ResolvedStatConversion) -> String {
    format_stat_conversion_description(rolled).join(" ")
}

/// Fallback: consumes [`PendingStatConversionRoll`] if a pick somehow lacked a pre-roll.
pub fn roll_pending_stat_conversion(
    mut commands: Commands,
    pending: Query<Entity, (With<Player>, With<PendingStatConversionRoll>)>,
    majors: Query<&OwnedMajorBlessings, With<Player>>,
) {
    let Ok(entity) = pending.single() else {
        return;
    };
    let Ok(majors) = majors.single() else {
        return;
    };
    if !majors.has(MajorBlessing::StatConversion) {
        commands
            .entity(entity)
            .remove::<PendingStatConversionRoll>();
        return;
    }

    let mut rng = rand::thread_rng();
    let rolled = roll_stat_conversion_pair(&mut rng);
    info!(
        "StatConversion rolled (pending fallback): {}",
        format_stat_conversion(&rolled)
    );
    commands
        .entity(entity)
        .remove::<PendingStatConversionRoll>()
        .insert(StatConversion {
            source: rolled.source,
            target: rolled.target,
            gain: rolled.gain,
            per: rolled.per,
            last_granted: 0,
            last_chaos: 0.0,
        })
        .insert(MajorBlessingStatBonuses::default());
}

/// Live recompute of conversion bonuses when source stats change.
pub fn update_stat_conversion(
    mut player: Query<(&mut StatConversion, &mut MajorBlessingStatBonuses), With<Player>>,
    game: GameParam,
    extra: Query<(&MaxMana, &ProjectileSize, &ManaRegen, &AttackSpeed), With<Player>>,
    mut chaos: ResMut<ChaosTracker>,
    mut attribute_event: MessageWriter<AttributeChangeEvent>,
) {
    let Ok((mut conv, mut bonuses)) = player.single_mut() else {
        return;
    };
    let Ok(stats) = game.player_stats.single() else {
        return;
    };
    let (max_mana, size, mana_regen, attack_speed) = extra
        .single()
        .map(|(m, s, mr, a)| (m.0, s.0, mr.0, a.0))
        .unwrap_or((0, 0, 0, 0));

    let (
        attack,
        max_health,
        defence,
        crit_chance,
        crit_damage,
        _bonus_dmg,
        _combo,
        health_regen,
        healing,
        thorns,
        dodge,
        speed,
        lifesteal,
        xp_rate,
        luck,
    ) = stats;

    let raw = match conv.source {
        HallucinationStatType::Attack => attack.0,
        HallucinationStatType::Health => max_health.0,
        HallucinationStatType::Defence => defence.0,
        HallucinationStatType::CritChance => crit_chance.0,
        HallucinationStatType::CritDamage => crit_damage.0,
        HallucinationStatType::Speed => speed.0,
        HallucinationStatType::Lifesteal => lifesteal.0,
        HallucinationStatType::Dodge => dodge.0,
        HallucinationStatType::HealthRegen => health_regen.0,
        HallucinationStatType::Healing => healing.0,
        HallucinationStatType::Thorns => thorns.0,
        HallucinationStatType::XPRate => xp_rate.0,
        HallucinationStatType::Luck => luck.0,
        HallucinationStatType::Mana => max_mana,
        HallucinationStatType::Size => size,
        HallucinationStatType::ManaRegen => mana_regen,
        HallucinationStatType::AttackSpeed => attack_speed,
    };
    let source_value = raw - bonuses.source_contribution(conv.source);

    let gain = conv.gain.max(1);
    let per = conv.per.max(1);
    // Half-units cancel: granted = source * (gain/2) / (per/2) = source * gain / per.
    let granted = ((source_value as i64 * gain as i64) / per as i64).max(0) as i32;
    let chaos_granted = (source_value as f32 * gain as f32
        / (per as f32 * STAT_CONVERSION_CHAOS_RATIO_MULT as f32))
        .max(0.0);
    if granted == conv.last_granted && chaos_granted == conv.last_chaos {
        return;
    }

    let chaos_delta = chaos_granted - conv.last_chaos;
    if chaos_delta != 0.0 {
        chaos.add_chaos(chaos_delta);
    }
    conv.last_granted = granted;
    conv.last_chaos = chaos_granted;
    bonuses.clear_and_set(conv.target, granted);
    attribute_event.write(AttributeChangeEvent);
}

pub struct StatConversionPlugin;

impl Plugin for StatConversionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                roll_pending_stat_conversion,
                update_stat_conversion.after(roll_pending_stat_conversion),
            )
                .run_if(in_state(GameState::Main)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_from_lifesteal_normalizes_to_half_steps() {
        // 40/17 ≈ 2.353 → +2.5 Size per 1 Lifesteal
        assert_eq!(conversion_terms(40, 17), (5, 2));
    }

    #[test]
    fn crit_from_health_uses_unit_gain() {
        // 4/20 = 0.2 → +1 Crit per 5 Health
        assert_eq!(conversion_terms(4, 20), (2, 10));
    }

    #[test]
    fn chaos_always_one_per_x() {
        let rolled = ResolvedStatConversion {
            source: HallucinationStatType::Lifesteal,
            target: HallucinationStatType::Size,
            gain: 5,
            per: 2,
        };
        // 5 * 1 / 2.5 = 2 → 1 Chaos per 2 Lifesteal
        assert_eq!(rolled.chaos_terms(), (2, 4));
    }
}
