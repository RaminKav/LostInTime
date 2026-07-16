//! Helpers used by generated definition code to deserialize nested RON snippets
//! for types that already implement `serde::Deserialize`.

use bevy::prelude::Vec2;

use crate::{
    attributes::{RawItemBaseAttributes, RawItemBonusAttributes},
    enemy::{
        scorpion::{ScorpionClawAttack, ScorpionTailAttack, ScorpionTornadoAttack},
        BullChargeAttack, CircleAttack, CombatAlignment, LaserAttack, Mob, MultiLeapAttack,
        ProjectileAttack,
    },
    inventory::ItemStack,
    item::{
        item_actions::ItemActions,
        object_actions::{ObjectAction, ObjectActionCost, TouchTriggerObjectAction},
        projectile::{ArcProjectileData, Projectile, ProjectileState},
        Foliage, LootTable, Wall, WorldObject,
    },
    ui::scrapper_ui::ScrapsInto,
    world::{WallTextureData, WorldGeneration},
};

use serde::Deserialize;

fn from_ron<'a, T: Deserialize<'a>>(s: &'a str, label: &str) -> T {
    ron::from_str(s).unwrap_or_else(|e| panic!("defs parse failed for {label}: {e}\n{s}"))
}

pub fn item_stack(s: &str) -> ItemStack {
    from_ron(s, "ItemStack")
}

pub fn loot_table(s: &str) -> LootTable {
    from_ron(s, "LootTable")
}

pub fn projectile_state(s: &str) -> ProjectileState {
    from_ron(&normalize_named_vec2s(s), "ProjectileState")
}

pub fn world_object(s: &str) -> WorldObject {
    from_ron(s, "WorldObject")
}

pub fn mob(s: &str) -> Mob {
    from_ron(s, "Mob")
}

pub fn projectile(s: &str) -> Projectile {
    from_ron(s, "Projectile")
}

pub fn combat_alignment(s: &str) -> CombatAlignment {
    from_ron(s, "CombatAlignment")
}

pub fn foliage(s: &str) -> Foliage {
    from_ron(s, "Foliage")
}

pub fn wall(s: &str) -> Wall {
    from_ron(s, "Wall")
}

pub fn item_actions(s: &str) -> ItemActions {
    from_ron(s, "ItemActions")
}

pub fn object_action(s: &str) -> ObjectAction {
    from_ron(s, "ObjectAction")
}

pub fn object_action_cost(s: &str) -> ObjectActionCost {
    from_ron(s, "ObjectActionCost")
}

pub fn scraps_into(s: &str) -> ScrapsInto {
    from_ron(s, "ScrapsInto")
}

/// Proto RON stores ranges as `Some((start: a, end: b))`. Serde's `RangeInclusive`
/// expects a 2-seq; we deserialize via this helper shape instead.
#[derive(Deserialize)]
struct RangeInclusiveRon {
    start: i32,
    end: i32,
}

impl From<RangeInclusiveRon> for std::ops::RangeInclusive<i32> {
    fn from(r: RangeInclusiveRon) -> Self {
        r.start..=r.end
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawItemRangesRon {
    attack: Option<RangeInclusiveRon>,
    health: Option<RangeInclusiveRon>,
    defence: Option<RangeInclusiveRon>,
    durability: Option<RangeInclusiveRon>,
    max_durability: Option<RangeInclusiveRon>,
    crit_chance: Option<RangeInclusiveRon>,
    crit_damage: Option<RangeInclusiveRon>,
    bonus_damage: Option<RangeInclusiveRon>,
    health_regen: Option<RangeInclusiveRon>,
    healing: Option<RangeInclusiveRon>,
    thorns: Option<RangeInclusiveRon>,
    dodge: Option<RangeInclusiveRon>,
    speed: Option<RangeInclusiveRon>,
    lifesteal: Option<RangeInclusiveRon>,
    xp_rate: Option<RangeInclusiveRon>,
    loot_rate: Option<RangeInclusiveRon>,
    mana: Option<RangeInclusiveRon>,
    mana_regen: Option<RangeInclusiveRon>,
    size: Option<RangeInclusiveRon>,
    attack_speed: Option<RangeInclusiveRon>,
    pickup_range: Option<RangeInclusiveRon>,
    skill_power: Option<RangeInclusiveRon>,
}

macro_rules! map_range_fields {
    ($src:expr, $dst:expr, $($field:ident),* $(,)?) => {
        $(
            $dst.$field = $src.$field.map(Into::into);
        )*
    };
}

pub fn raw_item_base(s: &str) -> RawItemBaseAttributes {
    let ron: RawItemRangesRon = from_ron(s, "RawItemBaseAttributes");
    let mut out = RawItemBaseAttributes::default();
    map_range_fields!(
        ron,
        out,
        attack,
        health,
        defence,
        durability,
        max_durability,
        crit_chance,
        crit_damage,
        bonus_damage,
        health_regen,
        healing,
        thorns,
        dodge,
        speed,
        lifesteal,
        xp_rate,
        loot_rate,
        mana,
        mana_regen,
        size,
        attack_speed,
        pickup_range,
        skill_power,
    );
    out
}

pub fn raw_item_bonus(s: &str) -> RawItemBonusAttributes {
    let ron: RawItemRangesRon = from_ron(s, "RawItemBonusAttributes");
    let mut out = RawItemBonusAttributes::default();
    map_range_fields!(
        ron,
        out,
        attack,
        health,
        defence,
        durability,
        max_durability,
        crit_chance,
        crit_damage,
        bonus_damage,
        health_regen,
        healing,
        thorns,
        dodge,
        speed,
        lifesteal,
        xp_rate,
        loot_rate,
        mana,
        mana_regen,
        size,
        attack_speed,
        pickup_range,
        skill_power,
    );
    out
}

/// Rewrite every `(x: a, y: b)` / `((x: a, y: b))` to glam's `(a, b)` form.
fn normalize_named_vec2s(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    let bytes = s.as_bytes();
    while i < bytes.len() {
        if let Some((x, y, consumed)) = try_parse_named_vec2(&s[i..]) {
            out.push_str(&format!("({x}, {y})"));
            i += consumed;
            continue;
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

/// Parse `((x: a, y: b))` or `(x: a, y: b)` at start of `s`.
fn try_parse_named_vec2(s: &str) -> Option<(f32, f32, usize)> {
    if !s.starts_with('(') {
        return None;
    }
    let mut open = 0;
    let mut outer_extra = false;
    // Double-wrapped form used by SpriteAnchor: `((x: a, y: b))`
    if s.starts_with("((") {
        let inner_close = find_matching_paren(&s[1..])?;
        let inner = s[2..1 + inner_close].trim();
        if inner.starts_with("x:") {
            open = 1;
            outer_extra = true;
        }
    }
    let close = open + find_matching_paren(&s[open..])?;
    let body = s[open + 1..close].trim();
    if !body.starts_with("x:") {
        return None;
    }
    let after_x = body["x:".len()..].trim_start();
    let (x, after_x_num) = parse_f32_prefix(after_x)?;
    let after_comma = after_x_num.trim_start().strip_prefix(',')?.trim_start();
    let after_y_key = after_comma.strip_prefix("y:")?.trim_start();
    let (y, _) = parse_f32_prefix(after_y_key)?;
    let mut consumed = close + 1;
    if outer_extra {
        let rest = s[consumed..].trim_start();
        if rest.starts_with(')') {
            consumed += s[consumed..].len() - rest.len() + 1;
        }
    }
    Some((x, y, consumed))
}

fn find_matching_paren(s: &str) -> Option<usize> {
    if !s.starts_with('(') {
        return None;
    }
    let mut depth = 0;
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Proto often uses `((x: a, y: b))` / `(x: a, y: b)`; glam Vec2 wants `(a, b)`.
fn normalize_vec2_ron(s: &str) -> String {
    normalize_named_vec2s(s)
}

fn parse_f32_prefix(s: &str) -> Option<(f32, &str)> {
    let mut end = 0;
    let bytes = s.as_bytes();
    if end < bytes.len() && (bytes[end] == b'-' || bytes[end] == b'+') {
        end += 1;
    }
    let start_digits = end;
    while end < bytes.len() && (bytes[end].is_ascii_digit() || bytes[end] == b'.') {
        end += 1;
    }
    if end == start_digits {
        return None;
    }
    let n: f32 = s[..end].parse().ok()?;
    Some((n, &s[end..]))
}

pub fn vec2(s: &str) -> Vec2 {
    from_ron(&normalize_vec2_ron(s), "Vec2")
}

pub fn world_generation(s: &str) -> WorldGeneration {
    from_ron(s, "WorldGeneration")
}

pub fn wall_texture_data(s: &str) -> WallTextureData {
    from_ron(s, "WallTextureData")
}

pub fn touch_trigger(s: &str) -> TouchTriggerObjectAction {
    from_ron(s, "TouchTriggerObjectAction")
}

pub fn projectile_attack(s: &str) -> ProjectileAttack {
    from_ron(&normalize_named_vec2s(s), "ProjectileAttack")
}

pub fn multi_leap_attack(s: &str) -> MultiLeapAttack {
    from_ron(s, "MultiLeapAttack")
}

pub fn bull_charge_attack(s: &str) -> BullChargeAttack {
    from_ron(s, "BullChargeAttack")
}

pub fn arc_projectile_data(s: &str) -> ArcProjectileData {
    from_ron(&normalize_named_vec2s(s), "ArcProjectileData")
}

pub fn scorpion_claw_attack(s: &str) -> ScorpionClawAttack {
    from_ron(s, "ScorpionClawAttack")
}

pub fn scorpion_tail_attack(s: &str) -> ScorpionTailAttack {
    from_ron(s, "ScorpionTailAttack")
}

pub fn scorpion_tornado_attack(s: &str) -> ScorpionTornadoAttack {
    from_ron(s, "ScorpionTornadoAttack")
}

pub fn circle_attack(s: &str) -> CircleAttack {
    from_ron(s, "CircleAttack")
}

pub fn laser_attack(s: &str) -> LaserAttack {
    from_ron(s, "LaserAttack")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_action_crafting() {
        let a = object_action("Crafting(Anvil)");
        assert!(matches!(
            a,
            ObjectAction::Crafting(crate::ui::crafting_ui::CraftingContainerType::Anvil)
        ));
    }

    #[test]
    fn item_actions_modify_health() {
        let a = item_actions("(actions: [ModifyHealth(10)])");
        assert_eq!(a.actions.len(), 1);
    }

    #[test]
    fn scraps_into_list() {
        let s = scraps_into(
            "([(obj: StoneChunk, chance: 1.), (obj: StoneChunk, chance: 0.5)])",
        );
        assert_eq!(s.0.len(), 2);
    }

    #[test]
    fn raw_item_base_ranges() {
        let r = raw_item_base(
            "(
        attack: Some((start: 10, end: 16)),
  )",
        );
        assert_eq!(r.attack, Some(10..=16));
    }

    #[test]
    fn register_all_entities_parses() {
        let mut defs = crate::defs::registry::GameDefs::default();
        crate::defs::generated::register_all(&mut defs);
        assert!(defs.get("Anvil").is_some());
        assert!(defs.get("FireStaff").is_some());
        assert!(defs.get("WoodAxe").is_some());
        assert!(defs.get("Scorpion").is_some());
        assert!(defs.get_era("Era1WorldGenerationParams").is_some());
        assert!(defs.get_era("Era4WorldGenerationParams").is_some());
    }
}
