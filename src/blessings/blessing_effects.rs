use bevy::prelude::*;
use rand::seq::{IteratorRandom, SliceRandom};
use rand::Rng;
use strum::IntoEnumIterator;

use crate::{
    attributes::{AttributeChangeEvent, FoodAttributeBonuses, ItemRarity},
    blessings::{
        ancestors::{stat_food_pool, AncestorBlessing},
        AncestorBlessingSelectEvent, BlessingMaxHpPenalty, BlessingTransitionState,
        DeferredEraSwap, EffectPoolStatusChance, EffectPoolStatusChances, HeirloomManaOverclock,
        MajorBlessing, MajorBlessingOffer, MajorBlessingSelectEvent, MajorBlessingStatBonuses,
        OwnedMajorBlessings, PendingRunStartChaos, ResolvedAncestorBlessing, ResolvedMajorBlessing,
        StartingWeaponOverride, StatConversion,
    },
    chaos::ChaosTracker,
    colors::LIGHT_GREEN,
    custom_commands::CommandsExt,
    item::{active_skill_shrine::roll_active_skill_shrine_offer_skills, WorldObject},
    player::{
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
    world::dimension::DimensionSpawnEvent,
};

#[derive(Component, Default)]
pub struct OwnedBlessings {
    pub blessings: Vec<crate::blessings::Blessing>,
}

/// Display info for a picked blessing shown on the HUD (B1 / B2 / B3).
#[derive(Clone, Debug)]
pub struct OwnedBlessingCard {
    pub title: String,
    pub description: Vec<String>,
    /// Extra chaos tradeoff lines (minor blessings), shown in red like the choice card.
    pub chaos_lines: Vec<String>,
    /// Card frame rarity; `None` uses the base SkillChoice frame.
    pub card_rarity: Option<crate::player::skills::HeirloomRarity>,
    /// Major blessing identity for trigger tracking (HUD tooltips).
    pub major: Option<MajorBlessing>,
}

/// HUD blessing slots: B1 = run-start minor, B2/B3 = majors in pick order.
#[derive(Component, Default, Debug, Clone)]
pub struct OwnedBlessingHudSlots {
    pub minor: Option<OwnedBlessingCard>,
    pub majors: Vec<OwnedBlessingCard>,
}

impl OwnedBlessingHudSlots {
    pub const MAX_MAJORS: usize = 2;

    pub fn set_minor(&mut self, card: OwnedBlessingCard) {
        self.minor = Some(card);
    }

    pub fn add_major(&mut self, card: OwnedBlessingCard) {
        if self.majors.len() < Self::MAX_MAJORS {
            self.majors.push(card);
        }
    }

    /// Slot 0 = minor (B1), 1 = first major (B2), 2 = second major (B3).
    pub fn slot(&self, index: usize) -> Option<&OwnedBlessingCard> {
        match index {
            0 => self.minor.as_ref(),
            1 => self.majors.get(0),
            2 => self.majors.get(1),
            _ => None,
        }
    }
}

impl OwnedBlessingCard {
    pub fn from_minor(choice: &ResolvedAncestorBlessing) -> Self {
        let mut chaos_lines = Vec::new();
        let penalty = choice.blessing.max_hp_penalty_pct();
        if penalty > 0.0 {
            chaos_lines.push(format!("-{}% Max HP", (penalty * 100.0) as i32));
            chaos_lines.push(format!(
                "+{} Chaos",
                choice.blessing.starting_chaos() as i32
            ));
        }
        let card_rarity = choice.blessing.display_card_rarity().or_else(|| {
            choice
                .resolved_heirloom
                .as_ref()
                .map(|h| h.rarity)
        });
        Self {
            title: choice.title.clone(),
            description: choice.description.clone(),
            chaos_lines,
            card_rarity,
            major: None,
        }
    }

    pub fn from_major(choice: &ResolvedMajorBlessing) -> Self {
        Self {
            title: choice.title.clone(),
            description: choice.description.clone(),
            chaos_lines: Vec::new(),
            card_rarity: choice.display_card_rarity(),
            major: Some(choice.blessing),
        }
    }
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
        if self.has_blessing(crate::blessings::Blessing::SkillCooldownPower) {
            2.0
        } else {
            1.0
        }
    }

    /// Legacy helper; major skill-power mult is applied on the `SkillPower` component
    /// in attribute updates (`OwnedMajorBlessings::skill_damage_multiplier`).
    pub fn get_skill_power_bonus_with_majors(&self, _majors: &OwnedMajorBlessings) -> f32 {
        self.get_skill_power_bonus()
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
    mut blessing_event: MessageReader<AncestorBlessingSelectEvent>,
    mut blessing_item_rewards: ResMut<BlessingItemRewards>,
    heirloom_queue: Res<HeirloomChoiceQueue>,
    mut player: Query<(
        Entity,
        &mut PlayerSkills,
        &GlobalTransform,
        &crate::player::levels::PlayerLevel,
        &mut OwnedBlessingHudSlots,
    )>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut blessing_transition_state: ResMut<BlessingTransitionState>,
    mut run_unlock_state: ResMut<RunUnlockState>,
    mut currency_event: MessageWriter<ModifyCurencyEvent>,
    mut attribute_event: MessageWriter<AttributeChangeEvent>,
    player_class: Option<Res<PlayerClass>>,
) {
    for event in blessing_event.read() {
        let mut rng = rand::thread_rng();
        let (player_entity, mut player_skills, player_transform, player_level, mut hud_slots) =
            player.single_mut().unwrap();
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
        );
        hud_slots.set_minor(OwnedBlessingCard::from_minor(&event.choice));

        attribute_event.write(AttributeChangeEvent);
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
    currency_event: &mut MessageWriter<ModifyCurencyEvent>,
    commands: &mut Commands,
    asset_server: &AssetServer,
    player_class: Option<&PlayerClass>,
) {
    let selected_class = player_class
        .map(|pc| pc.class.clone())
        .unwrap_or(SkillClass::None);
    let base_weapon_rarity = ItemRarity::Common;

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
            apply_chaos_tradeoff(
                choice.blessing,
                player_entity,
                selected_class.clone(),
                commands,
            );
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
            apply_chaos_tradeoff(
                choice.blessing,
                player_entity,
                selected_class.clone(),
                commands,
            );
        }
        AncestorBlessing::LaserBeam => {
            let laser_beam_skill =
                ActiveSkillChoiceState::new(ActiveSkill::LaserBeam, HeirloomRarity::Legendary);
            player_skills.insert_active_skill(
                laser_beam_skill,
                first_open_combat_skill_slot(player_skills),
            );
            ActiveSkill::LaserBeam.add_skill_components(player_entity, commands);
            apply_chaos_tradeoff(
                choice.blessing,
                player_entity,
                selected_class.clone(),
                commands,
            );
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
            apply_chaos_tradeoff(
                choice.blessing,
                player_entity,
                selected_class.clone(),
                commands,
            );
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
            apply_chaos_tradeoff(
                choice.blessing,
                player_entity,
                selected_class.clone(),
                commands,
            );
        }
        AncestorBlessing::RandomRareEquipment => {
            if let Some(equipment) = choice.resolved_item {
                blessing_item_rewards.queue_item_with_rarity(equipment, ItemRarity::Rare);
            }
            apply_chaos_tradeoff(
                choice.blessing,
                player_entity,
                selected_class.clone(),
                commands,
            );
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
            apply_chaos_tradeoff(
                choice.blessing,
                player_entity,
                selected_class.clone(),
                commands,
            );
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
    if count == 0 {
        return;
    }
    for _ in 0..count {
        player_skills.heirlooms.push(heirloom.clone());
        if let Some((drop, _)) = heirloom.heirloom.get_instant_drop() {
            blessing_item_rewards.queue_item(drop);
        }
        blessing_transition_state
            .heirlooms
            .get_or_insert_with(Vec::new)
            .push(heirloom.clone());
    }
    // Attach components once after all copies are pushed so stack-based state
    // (timers, etc.) is initialized against the final count — not reset per copy.
    heirloom
        .heirloom
        .add_heirloom_components(player_entity, commands, player_skills.clone());
}

pub fn handle_major_blessing_selected(
    mut blessing_event: MessageReader<MajorBlessingSelectEvent>,
    mut blessing_item_rewards: ResMut<BlessingItemRewards>,
    mut player: Query<(
        Entity,
        &mut OwnedMajorBlessings,
        &mut FoodAttributeBonuses,
        &PlayerSkills,
        &mut OwnedBlessingHudSlots,
        Option<&mut EffectPoolStatusChances>,
    )>,
    mut commands: Commands,
    mut run_unlock_state: ResMut<RunUnlockState>,
    mut attribute_event: MessageWriter<AttributeChangeEvent>,
    mut chaos_tracker: ResMut<ChaosTracker>,
    mut pet_weapon_event: MessageWriter<crate::pets::state::UpdatePetWeaponEvent>,
    player_class: Option<Res<PlayerClass>>,
) {
    for event in blessing_event.read() {
        let mut rng = rand::thread_rng();
        let Ok((
            player_entity,
            mut owned,
            mut food_bonuses,
            _skills,
            mut hud_slots,
            mut effect_pool,
        )) = player.single_mut()
        else {
            continue;
        };
        apply_major_blessing(
            &event.choice,
            &mut rng,
            player_entity,
            &mut owned,
            &mut food_bonuses,
            &mut blessing_item_rewards,
            &mut run_unlock_state,
            &mut chaos_tracker,
            &mut commands,
            player_class.as_deref(),
            effect_pool.as_deref_mut(),
        );
        hud_slots.add_major(OwnedBlessingCard::from_major(&event.choice));
        if event.choice.blessing == MajorBlessing::PetSizeAndAttackSpeed {
            pet_weapon_event.write(crate::pets::state::UpdatePetWeaponEvent);
        }
        attribute_event.write(AttributeChangeEvent);
    }
}

fn apply_major_blessing(
    choice: &ResolvedMajorBlessing,
    rng: &mut rand::rngs::ThreadRng,
    player_entity: Entity,
    owned: &mut OwnedMajorBlessings,
    food_bonuses: &mut FoodAttributeBonuses,
    blessing_item_rewards: &mut BlessingItemRewards,
    run_unlock_state: &mut RunUnlockState,
    chaos_tracker: &mut ChaosTracker,
    commands: &mut Commands,
    player_class: Option<&PlayerClass>,
    effect_pool: Option<&mut EffectPoolStatusChances>,
) {
    let blessing = choice.blessing;
    owned.add(blessing);

    match blessing {
        MajorBlessing::RandomLegendaryWeapon => {
            if let Some(weapon) = WorldObject::iter()
                .filter(|o| o.is_weapon() && *o != WorldObject::PlasmaStaff)
                .choose(rng)
            {
                blessing_item_rewards.queue_item_with_rarity(weapon, ItemRarity::Legendary);
            }
        }
        MajorBlessing::RandomLegendaryArmor => {
            if let Some(armor) = WorldObject::iter()
                .filter(|o| o.is_armor())
                .choose(rng)
            {
                blessing_item_rewards.queue_item_with_rarity(armor, ItemRarity::Legendary);
            }
        }
        MajorBlessing::RandomLegendaryAccessory => {
            if let Some(acc) = WorldObject::iter()
                .filter(|o| o.is_accessory())
                .choose(rng)
            {
                blessing_item_rewards.queue_item_with_rarity(acc, ItemRarity::Legendary);
            }
        }
        MajorBlessing::TomesAndOrbs => {
            for _ in 0..10 {
                blessing_item_rewards.queue_item(WorldObject::UpgradeTome);
                blessing_item_rewards.queue_item(WorldObject::OrbOfTransformation);
            }
        }
        MajorBlessing::RerollsAndBanishes => {
            run_unlock_state.rerolls_remaining =
                run_unlock_state.rerolls_remaining.saturating_add(7);
            run_unlock_state.rerolls_total = run_unlock_state.rerolls_total.saturating_add(7);
            run_unlock_state.banishes_remaining =
                run_unlock_state.banishes_remaining.saturating_add(3);
            run_unlock_state.banishes_total = run_unlock_state.banishes_total.saturating_add(3);
        }
        MajorBlessing::MerchantSlotReplenish => {
            run_unlock_state.rerolls_remaining =
                run_unlock_state.rerolls_remaining.saturating_add(3);
            run_unlock_state.rerolls_total = run_unlock_state.rerolls_total.saturating_add(3);
        }
        MajorBlessing::LuckChaosTradeoff => {
            food_bonuses.add("loot_rate", 50);
            let selected_class = player_class
                .map(|pc| pc.class.clone())
                .unwrap_or(SkillClass::None);
            let starting_hp = crate::player::get_max_health_for_class(selected_class);
            let flat_penalty = (starting_hp as f32 * blessing.max_hp_penalty_pct()) as i32;
            commands
                .entity(player_entity)
                .insert(BlessingMaxHpPenalty(flat_penalty));
            chaos_tracker.add_chaos(blessing.starting_chaos());
        }
        MajorBlessing::TripleUncommonHeirloom => {
            commands.insert_resource(PendingMajorHeirloomPick {
                mode: MajorHeirloomPickMode::TripleCopyLoseOne,
            });
        }
        MajorBlessing::ConvertUncommons => {
            commands.insert_resource(PendingMajorHeirloomPick {
                mode: MajorHeirloomPickMode::ConvertAllToChosen,
            });
        }
        MajorBlessing::OverhealToShield => {
            let selected_class = player_class
                .map(|pc| pc.class.clone())
                .unwrap_or(SkillClass::None);
            let starting_hp = crate::player::get_max_health_for_class(selected_class);
            let flat_penalty = (starting_hp as f32 * blessing.max_hp_penalty_pct()) as i32;
            commands
                .entity(player_entity)
                .insert(BlessingMaxHpPenalty(flat_penalty));
        }
        MajorBlessing::HeirloomOverclock => {
            commands
                .entity(player_entity)
                .insert(HeirloomManaOverclock::default());
        }
        MajorBlessing::EffectPoolApplyFreeze
        | MajorBlessing::EffectPoolApplyFrail
        | MajorBlessing::EffectPoolApplyPoison => {
            if let (Some(family), Some(status)) = (
                choice.resolved_effect_family,
                blessing.rolls_effect_family(),
            ) {
                let entry = EffectPoolStatusChance { family, status };
                if let Some(pool) = effect_pool {
                    pool.push(entry);
                } else {
                    commands.entity(player_entity).insert(EffectPoolStatusChances {
                        entries: vec![entry],
                    });
                }
            }
        }
        MajorBlessing::StatConversion => {
            if let Some(rolled) = choice.resolved_stat_conversion {
                info!(
                    "StatConversion applied from card: {}",
                    crate::blessings::format_stat_conversion(&rolled)
                );
                commands
                    .entity(player_entity)
                    .insert(StatConversion {
                        source: rolled.source,
                        target: rolled.target,
                        gain: rolled.gain,
                        per: rolled.per,
                        last_granted: 0,
                        last_chaos: 0.0,
                    })
                    .insert(MajorBlessingStatBonuses::default());
            } else {
                // Offer path always pre-rolls; keep fallback for safety.
                commands
                    .entity(player_entity)
                    .insert(PendingStatConversionRoll);
            }
        }
        // Passive effects are checked via OwnedMajorBlessings::has at their call sites.
        MajorBlessing::LightningCoinChance
        | MajorBlessing::EchoSizeBoost
        | MajorBlessing::PoisonTickFaster
        | MajorBlessing::HeirloomDamageBoost
        | MajorBlessing::ExtraManaRegen
        | MajorBlessing::SummonRetrigger
        | MajorBlessing::TouchThorns
        | MajorBlessing::EchoAftershock
        | MajorBlessing::ViralConductor
        | MajorBlessing::SharedAffliction
        | MajorBlessing::WeaponHeirloomDoubleTrigger
        | MajorBlessing::AttackSpeedBoost
        | MajorBlessing::SkillDamageBoost
        | MajorBlessing::SkillCooldownCut
        | MajorBlessing::SkillPoisonStacks
        | MajorBlessing::MovementSkillSummons
        | MajorBlessing::IceExplosionChain
        | MajorBlessing::ManaDrainShield
        | MajorBlessing::PetSizeAndAttackSpeed
        | MajorBlessing::CoinDropRate
        | MajorBlessing::StatusApplyShield
        | MajorBlessing::ManaRegenHeal
        | MajorBlessing::SkillsApplyAllStatuses
        | MajorBlessing::StatusApplyExtra
        | MajorBlessing::LargeObjectEcho
        | MajorBlessing::ObjectBreakLoot => {}
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MajorHeirloomPickMode {
    #[default]
    TripleCopyLoseOne,
    ConvertAllToChosen,
}

/// Set when a major blessing needs the uncommon heirloom pick UI.
#[derive(Resource, Debug, Clone)]
pub struct PendingMajorHeirloomPick {
    pub mode: MajorHeirloomPickMode,
}

/// Set when StatConversion major is picked; consumed by the conversion roller.
#[derive(Component, Default)]
pub struct PendingStatConversionRoll;

/// After the major blessing UI closes, fire any deferred era swap from the Time Portal.
/// Skips temporary leaves (inventory/map/options) while a choice is still pending —
/// [`MajorBlessingOffer`] is only removed after a completed pick.
/// Also waits for [`PendingMajorHeirloomPick`] (Singular Focus / Collector's Bargain)
/// so era swap does not race the secondary pick UI into `GameState::Initializing`.
pub fn apply_deferred_era_swap_after_major_blessing(
    mut deferred: ResMut<DeferredEraSwap>,
    mut dim_event: MessageWriter<DimensionSpawnEvent>,
    pending_offer: Option<Res<MajorBlessingOffer>>,
    pending_heirloom_pick: Option<Res<PendingMajorHeirloomPick>>,
) {
    if pending_offer.is_some() || pending_heirloom_pick.is_some() {
        return;
    }
    let Some(era) = deferred.era.take() else {
        return;
    };
    dim_event.write(DimensionSpawnEvent {
        swap_to_dim_now: true,
        new_era: Some(era),
    });
}

pub fn spawn_blessing_item_drops(
    mut blessing_item_rewards: ResMut<BlessingItemRewards>,
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
            commands.spawn_item_from_proto(item, &proto, Vec2::new(x, y), 1, level)
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
