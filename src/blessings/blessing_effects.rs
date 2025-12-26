use bevy::prelude::*;
use bevy_proto::prelude::ProtoCommands;
use rand::Rng;

use crate::{
    blessings::{Blessing, BlessingSelectEvent},
    custom_commands::CommandsExt,
    item::WorldObject,
    proto::proto_param::ProtoParam,
};

#[derive(Component, Default)]
pub struct OwnedBlessings {
    pub blessings: Vec<Blessing>,
}

impl OwnedBlessings {
    pub fn has_blessing(&self, blessing: Blessing) -> bool {
        self.blessings.contains(&blessing)
    }
    pub fn add_blessing(&mut self, blessing: Blessing) {
        if !self.has_blessing(blessing) {
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
}
#[derive(Resource, Default)]
pub struct BlessingItemRewards {
    pub items_to_drop_on_next_era: Vec<WorldObject>,
}

pub fn handle_blessing_selected(
    mut blessing_event: EventReader<BlessingSelectEvent>,
    mut blessings: Query<&mut OwnedBlessings>,
    mut blessing_item_rewards: ResMut<BlessingItemRewards>,
) {
    for event in blessing_event.iter() {
        let mut blessings = blessings.get_single_mut().unwrap();
        blessings.add_blessing(event.blessing);
        match event.blessing {
            Blessing::SkillAttackSpeed => {
                info!("Applying Skill Attack Speed Blessing Effect");
                // Implement the effect logic here
            }
            Blessing::OrbsAndTomes => {
                info!("Applying Orbs and Tomes Blessing Effect");
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
            Blessing::SkillCooldownPower => {
                info!("Applying Skill Cooldown Power Blessing Effect");
                // Implement the effect logic here
            }
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
        proto_commands.spawn_item_from_proto(item.clone(), &proto, Vec2::new(x, y), 1, None);
    }
    blessing_item_rewards.items_to_drop_on_next_era.clear();
}
