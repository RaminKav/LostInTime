use bevy::prelude::*;
use bevy_proto::prelude::ProtoCommands;
use rand::seq::SliceRandom;
use rand::Rng;
use strum::IntoEnumIterator;

use crate::{
    attributes::{AttributeChangeEvent, ItemRarity},
    blessings::{
        ancestors::{stat_food_pool, AncestorBlessing},
        AncestorBlessingSelectEvent, BlessingMaxHpPenalty, BlessingTransitionState,
        PendingRunStartChaos, ResolvedAncestorBlessing, StartingWeaponOverride,
    },
    colors::LIGHT_GREEN,
    custom_commands::CommandsExt,
    item::{active_skill_shrine::roll_active_skill_shrine_offer_skills, WorldObject},
    player::{
        class_rank::ClassRankSystem,
        currency::ModifyCurencyEvent,
        score::StartingWeapon,
        skills::{
            ActiveSkill, ActiveSkillChoiceState, HeirloomChoiceQueue, HeirloomRarity,
            HeirloomWithRarity, PlayerClass, PlayerSkills, SkillClass,
        },
        unlocks::RunUnlockState,
    },
    proto::proto_param::ProtoParam,
    ui::{damage_numbers::spawn_floating_text_with_shadow, game_fonts::FLOATING_TEXT},
};

#[derive(Component, Default)]
pub struct OwnedBlessings {
    pub blessings: Vec<crate::blessings::Blessing>,
}

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

    pub fn as_item_attributes(&self) -> &crate::attributes::ItemAttributes {
        &self.0
    }
}

impl OwnedBlessings {
    pub fn has_blessing(&self, blessing: crate::blessings::Blessing) -> bool {
        self.blessings.contains(&blessing)
    }
    pub fn add_blessing(&mut self, blessing: crate::blessings::Blessing) {
        if !self.has_blessing(blessing) {
            info!("Player acquired blessing: {:?}", blessing);
            self.blessings.push(blessing);
        }
    }
    pub fn get_skill_power_bonus(&self) -> f32 {
        let mut bonus = 1.0;
        if self.has_blessing(crate::blessings::Blessing::SkillCooldownPower) {
            bonus = 2.0;
        }
        bonus
    }
    pub fn get_skill_cooldown_increase(&self) -> f32 {
        let mut increase = 1.0;
        if self.has_blessing(crate::blessings::Blessing::SkillCooldownPower) {
            increase = 2.0;
        }
        increase
    }
    pub fn get_poison_bonus_chance(&self) -> f64 {
        if self.has_blessing(crate::blessings::Blessing::PoisonStacks) {
            0.5
        } else {
            0.0
        }
    }
    pub fn get_mana_guard_percentage(&self) -> f32 {
        if self.has_blessing(crate::blessings::Blessing::ManaGuard) {
            0.8
        } else {
            0.0
        }
    }
    pub fn get_kevin_self_damage_chance(&self) -> f32 {
        if self.has_blessing(crate::blessings::Blessing::Kevin) {
            0.25
        } else {
            0.0
        }
    }
    pub fn get_double_xp_chance(&self) -> f32 {
        if self.has_blessing(crate::blessings::Blessing::DoubleXp) {
            0.1
        } else {
            0.0
        }
    }
    pub fn has_double_gold_drops(&self) -> bool {
        self.has_blessing(crate::blessings::Blessing::DoubleGoldDrops)
    }
    pub fn get_attack_mana_cost(&self) -> i32 {
        if self.has_blessing(crate::blessings::Blessing::AttackManaCost) {
            5
        } else {
            0
        }
    }
    pub fn get_attack_mana_cost_damage_bonus(&self) -> f32 {
        if self.has_blessing(crate::blessings::Blessing::AttackManaCost) {
            0.10
        } else {
            0.0
        }
    }
}

/// A single item queued to drop when the next era begins.
#[derive(Clone, Debug)]
pub struct BlessingItemDrop {
    pub item: WorldObject,
    /// When set, the gear is forced to this rarity instead of rolling randomly.
    pub forced_rarity: Option<ItemRarity>,
}

#[derive(Resource, Default)]
pub struct BlessingItemRewards {
    pub items_to_drop_on_next_era: Vec<BlessingItemDrop>,
}

impl BlessingItemRewards {
    /// Queue an item to drop with its normal (randomly rolled) rarity.
    pub fn queue_item(&mut self, item: WorldObject) {
        self.items_to_drop_on_next_era.push(BlessingItemDrop {
            item,
            forced_rarity: None,
        });
    }

    /// Queue gear to drop forced to a specific rarity.
    pub fn queue_item_with_rarity(&mut self, item: WorldObject, rarity: ItemRarity) {
        self.items_to_drop_on_next_era.push(BlessingItemDrop {
            item,
            forced_rarity: Some(rarity),
        });
    }
}

pub fn handle_ancestor_blessing_selected(
    mut blessing_event: EventReader<AncestorBlessingSelectEvent>,
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
    mut run_unlock_state: ResMut<RunUnlockState>,
    mut currency_event: EventWriter<ModifyCurencyEvent>,
    mut attribute_event: EventWriter<AttributeChangeEvent>,
    player_class: Option<Res<PlayerClass>>,
    class_ranks: Option<Res<ClassRankSystem>>,
) {
    for event in blessing_event.iter() {
        let mut rng = rand::thread_rng();
        let (player_entity, mut player_skills, player_transform, player_level) =
            player.get_single_mut().unwrap();
        let player_level = player_level.level;

        apply_ancestor_blessing(
            &event.choice,
            &mut rng,
            player_entity,
            &mut player_skills,
            player_transform,
            player_level,
            &heirloom_queue,
            &mut blessing_item_rewards,
            &mut blessing_transition_state,
            &mut run_unlock_state,
            &mut currency_event,
            &mut commands,
            &asset_server,
            player_class.as_deref(),
            class_ranks.as_deref(),
        );

        attribute_event.send(AttributeChangeEvent);
    }
}

fn apply_ancestor_blessing(
    choice: &ResolvedAncestorBlessing,
    rng: &mut rand::rngs::ThreadRng,
    player_entity: Entity,
    player_skills: &mut PlayerSkills,
    player_transform: &GlobalTransform,
    player_level: u8,
    heirloom_queue: &HeirloomChoiceQueue,
    blessing_item_rewards: &mut BlessingItemRewards,
    blessing_transition_state: &mut BlessingTransitionState,
    run_unlock_state: &mut RunUnlockState,
    currency_event: &mut EventWriter<ModifyCurencyEvent>,
    commands: &mut Commands,
    asset_server: &AssetServer,
    player_class: Option<&PlayerClass>,
    class_ranks: Option<&ClassRankSystem>,
) {
    let selected_class = player_class
        .map(|pc| pc.class.clone())
        .unwrap_or(SkillClass::None);
    let base_weapon_rarity = class_ranks
        .map(|ranks| ranks.get_starting_weapon_rarity(&selected_class))
        .unwrap_or(ItemRarity::Common);

    match choice.blessing {
        AncestorBlessing::ThreeTomes => {
            for _ in 0..3 {
                blessing_item_rewards.queue_item(WorldObject::UpgradeTome);
            }
        }
        AncestorBlessing::ThreeOrbs => {
            for _ in 0..3 {
                blessing_item_rewards.queue_item(WorldObject::OrbOfTransformation);
            }
        }
        AncestorBlessing::TwoOrbsTwoTomes => {
            for _ in 0..2 {
                blessing_item_rewards.queue_item(WorldObject::UpgradeTome);
                blessing_item_rewards.queue_item(WorldObject::OrbOfTransformation);
            }
        }
        AncestorBlessing::FiftyGold => {
            for _ in 0..50 {
                blessing_item_rewards.queue_item(WorldObject::Coin);
            }
        }
        AncestorBlessing::ThreeStatFoods => {
            let pool = stat_food_pool();
            for _ in 0..3 {
                if let Some(food) = pool.choose(rng) {
                    blessing_item_rewards.queue_item(*food);
                }
            }
        }
        AncestorBlessing::ThreeRerolls => {
            run_unlock_state.rerolls_remaining =
                run_unlock_state.rerolls_remaining.saturating_add(3);
            run_unlock_state.rerolls_total = run_unlock_state.rerolls_total.saturating_add(3);
        }
        AncestorBlessing::TwoBanishes => {
            run_unlock_state.banishes_remaining =
                run_unlock_state.banishes_remaining.saturating_add(2);
            run_unlock_state.banishes_total = run_unlock_state.banishes_total.saturating_add(2);
        }
        AncestorBlessing::ThreeCommonHeirlooms => {
            grant_random_heirlooms(
                HeirloomRarity::Common,
                3,
                rng,
                player_level,
                heirloom_queue,
                player_entity,
                player_skills,
                commands,
                blessing_item_rewards,
                blessing_transition_state,
            );
        }
        AncestorBlessing::OneUncommonHeirloom => {
            grant_random_heirlooms(
                HeirloomRarity::Uncommon,
                1,
                rng,
                player_level,
                heirloom_queue,
                player_entity,
                player_skills,
                commands,
                blessing_item_rewards,
                blessing_transition_state,
            );
        }
        AncestorBlessing::SpecificUncommon => {
            if let Some(heirloom) = choice.resolved_heirloom.clone() {
                grant_specific_heirloom(
                    heirloom,
                    1,
                    player_entity,
                    player_skills,
                    commands,
                    blessing_item_rewards,
                    blessing_transition_state,
                );
            }
        }
        AncestorBlessing::SpecificRareHeirloom => {
            if let Some(heirloom) = choice.resolved_heirloom.clone() {
                grant_specific_heirloom(
                    heirloom,
                    1,
                    player_entity,
                    player_skills,
                    commands,
                    blessing_item_rewards,
                    blessing_transition_state,
                );
            }
            apply_chaos_tradeoff(choice.blessing, player_entity, selected_class.clone(), commands);
        }
        AncestorBlessing::TwoOfSpecificCommon => {
            if let Some(heirloom) = choice.resolved_heirloom.clone() {
                grant_specific_heirloom(
                    heirloom,
                    2,
                    player_entity,
                    player_skills,
                    commands,
                    blessing_item_rewards,
                    blessing_transition_state,
                );
            }
        }
        AncestorBlessing::UpgradeStartingWeapon => {
            commands.insert_resource(StartingWeaponOverride {
                weapon: selected_class.get_starting_wep(),
                rarity: base_weapon_rarity,
                replace_starting_weapon: false,
                upgrade_starting_weapon: true,
            });
        }
        AncestorBlessing::RandomWeapon => {
            if let Some(weapon) = choice.resolved_item {
                blessing_item_rewards.queue_item(weapon);
            }
        }
        AncestorBlessing::RandomEquipment => {
            if let Some(equipment) = choice.resolved_item {
                blessing_item_rewards.queue_item(equipment);
            }
        }
        AncestorBlessing::RandomAccessory => {
            if let Some(accessory) = choice.resolved_item {
                blessing_item_rewards.queue_item(accessory);
            }
        }
        AncestorBlessing::ReplaceWithSpecificWeapon => {
            if let Some(weapon) = choice.resolved_weapon {
                commands.insert_resource(StartingWeaponOverride {
                    weapon,
                    rarity: base_weapon_rarity,
                    replace_starting_weapon: true,
                    upgrade_starting_weapon: true,
                });
            }
        }
        AncestorBlessing::RandomWeaponHeirloom => {
            if let Some(heirloom) = choice.resolved_heirloom.clone() {
                grant_specific_heirloom(
                    heirloom,
                    1,
                    player_entity,
                    player_skills,
                    commands,
                    blessing_item_rewards,
                    blessing_transition_state,
                );
            }
        }
        AncestorBlessing::RandomSkill => {
            let skill = choice.resolved_skill.or_else(|| {
                roll_active_skill_shrine_offer_skills(Some(player_skills))
                    .choose(rng)
                    .copied()
            });
            if let Some(skill) = skill {
                grant_active_skill(
                    skill,
                    player_entity,
                    player_skills,
                    commands,
                    player_transform,
                    asset_server,
                );
            }
        }
        AncestorBlessing::SpecificSkill => {
            if let Some(skill) = choice.resolved_skill {
                grant_active_skill(
                    skill,
                    player_entity,
                    player_skills,
                    commands,
                    player_transform,
                    asset_server,
                );
            }
        }
        AncestorBlessing::SkillHeirloom => {
            if let Some(heirloom) = choice.resolved_heirloom.clone() {
                grant_specific_heirloom(
                    heirloom,
                    1,
                    player_entity,
                    player_skills,
                    commands,
                    blessing_item_rewards,
                    blessing_transition_state,
                );
            }
        }
        AncestorBlessing::PlasmaWeapon => {
            blessing_item_rewards.queue_item(WorldObject::PlasmaStaff);
            apply_chaos_tradeoff(choice.blessing, player_entity, selected_class.clone(), commands);
        }
        AncestorBlessing::LaserBeam => {
            let laser_beam_skill =
                ActiveSkillChoiceState::new(ActiveSkill::LaserBeam, HeirloomRarity::Legendary);
            player_skills.insert_active_skill(
                laser_beam_skill,
                first_open_combat_skill_slot(player_skills),
            );
            ActiveSkill::LaserBeam.add_skill_components(player_entity, commands);
            apply_chaos_tradeoff(choice.blessing, player_entity, selected_class.clone(), commands);
        }
        AncestorBlessing::TwoRandomRareHeirlooms => {
            grant_random_heirlooms(
                HeirloomRarity::Rare,
                2,
                rng,
                player_level,
                heirloom_queue,
                player_entity,
                player_skills,
                commands,
                blessing_item_rewards,
                blessing_transition_state,
            );
            apply_chaos_tradeoff(choice.blessing, player_entity, selected_class.clone(), commands);
        }
        AncestorBlessing::FiveOfRandomCommon => {
            if let Some(heirloom) = choice.resolved_heirloom.clone() {
                grant_specific_heirloom(
                    heirloom,
                    5,
                    player_entity,
                    player_skills,
                    commands,
                    blessing_item_rewards,
                    blessing_transition_state,
                );
            }
            apply_chaos_tradeoff(choice.blessing, player_entity, selected_class.clone(), commands);
        }
        AncestorBlessing::RandomRareEquipment => {
            if let Some(equipment) = choice.resolved_item {
                blessing_item_rewards.queue_item_with_rarity(equipment, ItemRarity::Rare);
            }
            apply_chaos_tradeoff(choice.blessing, player_entity, selected_class.clone(), commands);
        }
        AncestorBlessing::ThreeOfRandomUncommon => {
            if let Some(heirloom) = choice.resolved_heirloom.clone() {
                grant_specific_heirloom(
                    heirloom,
                    3,
                    player_entity,
                    player_skills,
                    commands,
                    blessing_item_rewards,
                    blessing_transition_state,
                );
            }
            apply_chaos_tradeoff(choice.blessing, player_entity, selected_class.clone(), commands);
        }
    }
}

fn apply_chaos_tradeoff(
    blessing: AncestorBlessing,
    player_entity: Entity,
    selected_class: SkillClass,
    commands: &mut Commands,
) {
    let penalty_pct = blessing.max_hp_penalty_pct();
    let chaos = blessing.starting_chaos();
    if penalty_pct > 0.0 {
        let starting_hp = crate::player::get_max_health_for_class(selected_class);
        let flat_penalty = (starting_hp as f32 * penalty_pct) as i32;
        commands
            .entity(player_entity)
            .insert(BlessingMaxHpPenalty(flat_penalty));
        commands.insert_resource(PendingRunStartChaos { amount: chaos });
    }
}

fn first_open_combat_skill_slot(player_skills: &PlayerSkills) -> usize {
    for slot in [1, 2] {
        if player_skills.get_active_skill_in_slot(slot).is_none() {
            return slot;
        }
    }
    1
}

fn grant_active_skill(
    skill: ActiveSkill,
    player_entity: Entity,
    player_skills: &mut PlayerSkills,
    commands: &mut Commands,
    player_transform: &GlobalTransform,
    asset_server: &AssetServer,
) {
    let slot = first_open_combat_skill_slot(player_skills);
    let skill_name = skill.get_title();
    let skill_choice = ActiveSkillChoiceState::new(skill, HeirloomRarity::Rare);
    player_skills.insert_active_skill(skill_choice, slot);
    skill.add_skill_components(player_entity, commands);

    let player_pos = player_transform.translation();
    spawn_floating_text_with_shadow(
        commands,
        asset_server,
        player_pos + Vec3::new(0., 20., 2.),
        LIGHT_GREEN,
        format!("+{skill_name}"),
        FLOATING_TEXT,
    );
}

fn grant_random_heirlooms(
    rarity: HeirloomRarity,
    count: usize,
    rng: &mut rand::rngs::ThreadRng,
    player_level: u8,
    heirloom_queue: &HeirloomChoiceQueue,
    player_entity: Entity,
    player_skills: &mut PlayerSkills,
    commands: &mut Commands,
    blessing_item_rewards: &mut BlessingItemRewards,
    blessing_transition_state: &mut BlessingTransitionState,
) {
    for _ in 0..count {
        if let Some(picked) =
            heirloom_queue.get_skill_of_rarity(rarity, rng, player_level, &|_| true)
        {
            let heirloom_with_rarity = HeirloomWithRarity {
                heirloom: picked.heirloom.clone(),
                rarity: picked.rarity.clone(),
            };
            grant_specific_heirloom(
                heirloom_with_rarity,
                1,
                player_entity,
                player_skills,
                commands,
                blessing_item_rewards,
                blessing_transition_state,
            );
        }
    }
}

fn grant_specific_heirloom(
    heirloom: HeirloomWithRarity,
    count: usize,
    player_entity: Entity,
    player_skills: &mut PlayerSkills,
    commands: &mut Commands,
    blessing_item_rewards: &mut BlessingItemRewards,
    blessing_transition_state: &mut BlessingTransitionState,
) {
    for _ in 0..count {
        player_skills.heirlooms.push(heirloom.clone());
        heirloom
            .heirloom
            .add_heirloom_components(player_entity, commands, player_skills.clone());
        if let Some((drop, _)) = heirloom.heirloom.get_instant_drop() {
            blessing_item_rewards.queue_item(drop);
        }
        blessing_transition_state
            .heirlooms
            .get_or_insert_with(Vec::new)
            .push(heirloom.clone());
    }
}

pub fn spawn_blessing_item_drops(
    mut blessing_item_rewards: ResMut<BlessingItemRewards>,
    mut proto_commands: ProtoCommands,
    proto: ProtoParam,
    mut commands: Commands,
) {
    let mut rng = rand::thread_rng();
    for drop in blessing_item_rewards.items_to_drop_on_next_era.iter() {
        let item = drop.item;
        let x = rng.gen_range(-26.0..26.0);
        let y = rng.gen_range(-26.0..26.0);
        let is_gear = item.is_weapon() || item.is_armor() || item.is_accessory();
        let level = if is_gear { Some(1) } else { None };
        if let Some(entity) =
            proto_commands.spawn_item_from_proto(item, &proto, Vec2::new(x, y), 1, level)
        {
            // Forcing rarity only applies to gear, which resolves its rarity from raw attributes.
            if let Some(rarity) = drop.forced_rarity.clone() {
                if is_gear {
                    commands.entity(entity).insert(StartingWeapon { rarity });
                }
            }
        }
    }
    blessing_item_rewards.items_to_drop_on_next_era.clear();
}
