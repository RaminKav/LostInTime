use bevy::prelude::*;
use bevy_proto::prelude::ProtoCommands;
use rand::seq::SliceRandom;
use rand::Rng;
use strum::IntoEnumIterator;

use crate::{
    blessings::{Blessing, BlessingSelectEvent, BlessingTransitionState},
    colors::LIGHT_GREEN,
    custom_commands::CommandsExt,
    item::WorldObject,
    player::skills::{
        ActiveSkill, ActiveSkillChoiceState, HeirloomChoiceQueue, HeirloomRarity,
        HeirloomWithRarity, PlayerSkills,
    },
    proto::proto_param::ProtoParam,
    ui::{
        damage_numbers::spawn_floating_text_with_shadow,
        game_fonts::FLOATING_TEXT,
    },
};

#[derive(Component, Default)]
pub struct OwnedBlessings {
    pub blessings: Vec<Blessing>,
}

// ============================================================================
// HeirloomStats - Tracks bonus stats from heirloom shrines (similar to HallucinationStats)
// ============================================================================

/// Tracks accumulated stat bonuses from HeirloomStats blessing
/// Wraps ItemAttributes so it can be combined with player attributes
#[derive(Component, Default, Debug, Clone)]
pub struct HeirloomStatsBonuses(pub crate::attributes::ItemAttributes);

impl HeirloomStatsBonuses {
    pub fn add_stat(
        &mut self,
        stat_type: crate::player::combat_heirlooms::HallucinationStatType,
        amount: i32,
    ) {
        use crate::player::combat_heirlooms::HallucinationStatType;
        match stat_type {
            HallucinationStatType::Attack => self.0.attack.value += amount,
            HallucinationStatType::Health => self.0.health.value += amount,
            HallucinationStatType::Defence => self.0.defence.value += amount,
            HallucinationStatType::CritChance => self.0.crit_chance.value += amount,
            HallucinationStatType::CritDamage => self.0.crit_damage.value += amount,
            HallucinationStatType::Speed => self.0.speed.value += amount,
            HallucinationStatType::Lifesteal => self.0.lifesteal.value += amount,
            HallucinationStatType::Dodge => self.0.dodge.value += amount,
            HallucinationStatType::HealthRegen => self.0.health_regen.value += amount,
            HallucinationStatType::Healing => self.0.healing.value += amount,
            HallucinationStatType::Thorns => self.0.thorns.value += amount,
            HallucinationStatType::XPRate => self.0.xp_rate.value += amount,
            HallucinationStatType::Luck => self.0.loot_rate.value += amount,
            HallucinationStatType::Mana => self.0.mana.value += amount,
            HallucinationStatType::Size => self.0.size.value += amount,
            HallucinationStatType::ManaRegen => self.0.mana_regen.value += amount,
            HallucinationStatType::AttackSpeed => self.0.attack_speed.value += amount,
        }
    }

    /// Get the inner ItemAttributes for combining with player stats
    pub fn as_item_attributes(&self) -> &crate::attributes::ItemAttributes {
        &self.0
    }
}

impl OwnedBlessings {
    pub fn has_blessing(&self, blessing: Blessing) -> bool {
        self.blessings.contains(&blessing)
    }
    pub fn add_blessing(&mut self, blessing: Blessing) {
        if !self.has_blessing(blessing) {
            info!("Player acquired blessing: {:?}", blessing);
            self.blessings.push(blessing);
        }
    }
    pub fn get_skill_power_bonus(&self) -> f32 {
        let mut bonus = 1.0;
        if self.has_blessing(Blessing::SkillCooldownPower) {
            bonus = 2.0;
        }
        bonus
    }
    pub fn get_skill_cooldown_increase(&self) -> f32 {
        let mut increase = 1.0;
        if self.has_blessing(Blessing::SkillCooldownPower) {
            increase = 2.0;
        }
        increase
    }
    /// Get bonus poison chance from PoisonStacks blessing (50%)
    pub fn get_poison_bonus_chance(&self) -> f64 {
        if self.has_blessing(Blessing::PoisonStacks) {
            0.5
        } else {
            0.0
        }
    }
    /// Get mana guard percentage (80% damage from mana instead of health)
    pub fn get_mana_guard_percentage(&self) -> f32 {
        if self.has_blessing(Blessing::ManaGuard) {
            0.8
        } else {
            0.0
        }
    }
    /// Get self-damage chance from Kevin blessing (25%)
    pub fn get_kevin_self_damage_chance(&self) -> f32 {
        if self.has_blessing(Blessing::Kevin) {
            0.25
        } else {
            0.0
        }
    }
    /// Get double XP chance from DoubleXp blessing (10%)
    pub fn get_double_xp_chance(&self) -> f32 {
        if self.has_blessing(Blessing::DoubleXp) {
            0.1
        } else {
            0.0
        }
    }
    /// Check if DoubleGoldDrops blessing is active
    pub fn has_double_gold_drops(&self) -> bool {
        self.has_blessing(Blessing::DoubleGoldDrops)
    }
    /// Get the mana cost per attack from AttackManaCost blessing (5 mana)
    pub fn get_attack_mana_cost(&self) -> i32 {
        if self.has_blessing(Blessing::AttackManaCost) {
            5
        } else {
            0
        }
    }
    /// Get the damage bonus multiplier from AttackManaCost blessing (10%)
    pub fn get_attack_mana_cost_damage_bonus(&self) -> f32 {
        if self.has_blessing(Blessing::AttackManaCost) {
            0.10
        } else {
            0.0
        }
    }
}
#[derive(Resource, Default)]
pub struct BlessingItemRewards {
    pub items_to_drop_on_next_era: Vec<WorldObject>,
}

pub fn handle_blessing_selected(
    mut blessing_event: EventReader<BlessingSelectEvent>,
    mut blessings: Query<&mut OwnedBlessings>,
    mut blessing_item_rewards: ResMut<BlessingItemRewards>,
    heirloom_queue: Res<HeirloomChoiceQueue>,
    mut player: Query<(
        Entity,
        &mut PlayerSkills,
        &GlobalTransform,
        &crate::player::levels::PlayerLevel,
    )>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut blessing_transition_state: ResMut<BlessingTransitionState>,
) {
    for event in blessing_event.iter() {
        let mut blessings = blessings.get_single_mut().unwrap();
        blessings.add_blessing(event.blessing);
        let mut rng = rand::thread_rng();
        let (player_entity, mut player_skills, player_transform, player_level) =
            player.get_single_mut().unwrap();
        let player_level = player_level.level;
        match event.blessing {
            Blessing::OrbsAndTomes => {
                for _ in 0..5 {
                    blessing_item_rewards
                        .items_to_drop_on_next_era
                        .push(WorldObject::UpgradeTome);
                    blessing_item_rewards
                        .items_to_drop_on_next_era
                        .push(WorldObject::OrbOfTransformation);
                }
                // Implement the effect logic here
            }
            Blessing::LootChests => {
                for _ in 0..3 {
                    blessing_item_rewards
                        .items_to_drop_on_next_era
                        .push(WorldObject::ChestBlock);
                }
                // Implement the effect logic here
            }
            Blessing::GainRareHeirloom
            | Blessing::GainCommonHeirlooms
            | Blessing::GainLegendaryHeirloom => {
                let (rarity, count) = match event.blessing {
                    Blessing::GainCommonHeirlooms => (HeirloomRarity::Common, 3),
                    Blessing::GainRareHeirloom => (HeirloomRarity::Rare, 1),
                    Blessing::GainLegendaryHeirloom => (HeirloomRarity::Legendary, 1),
                    _ => unreachable!(),
                };
                for _ in 0..count {
                    if let Some(picked_rare_heirloom) =
                        heirloom_queue.get_skill_of_rarity(
                            rarity.clone(),
                            &mut rng,
                            player_level,
                            &|_| true,
                        )
                    {
                        let heirloom_with_rarity = HeirloomWithRarity {
                            heirloom: picked_rare_heirloom.heirloom.clone(),
                            rarity: picked_rare_heirloom.rarity.clone(),
                        };

                        player_skills.heirlooms.push(heirloom_with_rarity.clone());

                        // Add skill components to the player entity
                        heirloom_with_rarity.heirloom.add_heirloom_components(
                            player_entity,
                            &mut commands,
                            player_skills.clone(),
                        );
                        // handle drops
                        if let Some((drop, _)) = heirloom_with_rarity.heirloom.get_instant_drop() {
                            blessing_item_rewards.items_to_drop_on_next_era.push(drop);
                        }

                        blessing_transition_state
                            .heirlooms
                            .get_or_insert(Vec::new())
                            .push(heirloom_with_rarity);
                        // Trigger attribute recalculation
                    }
                }
            }
            Blessing::PlasmaWeapon => {
                blessing_item_rewards
                    .items_to_drop_on_next_era
                    .push(WorldObject::PlasmaStaff);
            }
            Blessing::LaserBeam => {
                let laser_beam_skill =
                    ActiveSkillChoiceState::new(ActiveSkill::LaserBeam, HeirloomRarity::Legendary);

                player_skills.insert_active_skill(laser_beam_skill, 4);

                ActiveSkill::LaserBeam.add_skill_components(player_entity, &mut commands);
            }
            Blessing::RandomActiveSkill => {
                let available_skills: Vec<ActiveSkill> = ActiveSkill::iter()
                    .filter(|skill| !matches!(skill, ActiveSkill::Parry | ActiveSkill::LaserBeam))
                    .collect();

                if let Some(random_skill) = available_skills.choose(&mut rng) {
                    let skill_name = random_skill.get_title();

                    let skill_choice =
                        ActiveSkillChoiceState::new(random_skill.clone(), HeirloomRarity::Rare);

                    player_skills.insert_active_skill(skill_choice, 4);

                    random_skill.add_skill_components(player_entity, &mut commands);

                    let player_pos = player_transform.translation();
                    spawn_floating_text_with_shadow(
                        &mut commands,
                        &asset_server,
                        player_pos + Vec3::new(0., 20., 2.),
                        LIGHT_GREEN,
                        format!("+{}", skill_name),
                        FLOATING_TEXT,
                    );
                }
            }
            _ => {}
        }
    }
}

pub fn spawn_blessing_item_drops(
    mut blessing_item_rewards: ResMut<BlessingItemRewards>,
    mut proto_commands: ProtoCommands,
    proto: ProtoParam,
) {
    let mut rng = rand::thread_rng();
    for item in blessing_item_rewards.items_to_drop_on_next_era.iter() {
        let x = rng.gen_range(-26.0..26.0);
        let y = rng.gen_range(-26.0..26.0);
        let level = if item.is_weapon() || item.is_armor() || item.is_accessory() {
            Some(5 + rng.gen_range(0..3))
        } else {
            None
        };
        proto_commands.spawn_item_from_proto(item.clone(), &proto, Vec2::new(x, y), 1, level);
    }
    blessing_item_rewards.items_to_drop_on_next_era.clear();
}
