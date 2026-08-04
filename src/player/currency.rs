use bevy::prelude::*;
use rand::Rng;

use crate::{
    attributes::{
        modifiers::{ModifyHealthEvent, ModifyManaEvent},
        Attack, AttributeChangeEvent, CurrentMana,
    },
    audio::{AudioSoundEffect, SoundSpawner},
    client::persist_time_fragments,
    enemy::Mob,
    item::{
        projectile::{Projectile, RangedAttackEvent},
        WorldObject,
    },
    player::{
        skills::{Heirloom, PlayerSkills},
        Player,
    },
    GameState,
};

#[derive(Resource, Debug)]
pub struct TimeFragmentCurrency {
    pub time_fragments: i32,
    pub total_collected_time_fragments_all_time: u128,
    pub total_collected_time_fragments_this_run: i32,
    pub bounce_timer: Timer,
}
impl TimeFragmentCurrency {
    pub fn new(amount: i32, total_this_run: i32, total_all_time: u128) -> Self {
        TimeFragmentCurrency {
            time_fragments: amount,
            total_collected_time_fragments_all_time: total_all_time,
            total_collected_time_fragments_this_run: total_this_run,
            bounce_timer: Timer::from_seconds(0.5, TimerMode::Once),
        }
    }

    pub fn can_spend(&self, amount: u32) -> bool {
        self.time_fragments as i64 >= amount as i64
    }

    pub fn spend(&mut self, amount: u32) -> bool {
        if self.can_spend(amount) {
            self.time_fragments -= amount as i32;
            true
        } else {
            false
        }
    }
}

impl Default for TimeFragmentCurrency {
    fn default() -> Self {
        TimeFragmentCurrency::new(0, 0, 0)
    }
}

#[derive(Resource, Default, Debug)]
pub struct CoinCurrency {
    pub coins: u32,
    pub bounce_timer: Timer,
}

#[derive(Message)]
pub struct ModifyCurencyEvent {
    pub delta: i32,
    pub obj: WorldObject,
}

pub fn handle_modify_currency(
    mut time_fragments: ResMut<TimeFragmentCurrency>,
    mut events: MessageReader<ModifyCurencyEvent>,
    state: Res<State<GameState>>,
    mut commands: Commands,
    mut coins: ResMut<CoinCurrency>,
    player_skills: Query<&PlayerSkills, With<Player>>,
    player_query: Query<(&GlobalTransform, &Attack, &CurrentMana), With<Player>>,
    enemies: Query<(&GlobalTransform, &Mob), (With<Mob>, Without<Player>)>,
    mut modify_health_event: MessageWriter<ModifyHealthEvent>,
    mut attribute_change_event: MessageWriter<AttributeChangeEvent>,
    mut ranged_attack_event: MessageWriter<RangedAttackEvent>,
    mut modify_mana_event: MessageWriter<ModifyManaEvent>,
    mut trigger_counts: ResMut<crate::player::skills::HeirloomTriggerCounts>,
) {
    let skills = player_skills.single().ok();
    let mut rng = rand::thread_rng();

    for event in events.read() {
        if event.obj == WorldObject::TimeFragment {
            time_fragments.time_fragments = (time_fragments.time_fragments + event.delta).max(0);

            // Persist time fragments immediately when they are spent (delta < 0)
            // This prevents save-scumming by closing and restarting the game
            if event.delta < 0 {
                persist_time_fragments(time_fragments.time_fragments);
            }
        } else if event.obj == WorldObject::Coin {
            coins.coins = (coins.coins as i32 + event.delta).max(0) as u32;

            // Trigger attribute recalculation if player has GoldIntoDamage
            if let Some(skills) = skills {
                if skills.has(Heirloom::GoldIntoDamage) {
                    attribute_change_event.write_default();
                }
            }

            // CoinHeal: Picking up coins has a chance to heal
            // CoinLightning: Picking up a coin causes a lightning strike
            if event.delta > 0 {
                if let Some(skills) = skills {
                    let stacks = skills.get_count(Heirloom::CoinHeal);
                    if stacks > 0 {
                        // Each coin pickup is a separate check
                        for _ in 0..event.delta {
                            let chance = 25 * stacks; // 25% per stack
                            let heal_amount = if chance > 100 {
                                // Past 100%, chance for 2 HP
                                let extra_chance = chance - 100;
                                if rng.gen_ratio(extra_chance.clamp(1, 100) as u32, 100) {
                                    2
                                } else {
                                    1
                                }
                            } else if rng.gen_ratio(chance.clamp(1, 100) as u32, 100) {
                                1
                            } else {
                                0
                            };
                            if heal_amount > 0 {
                                modify_health_event.write(ModifyHealthEvent(heal_amount));
                                trigger_counts.increment(Heirloom::CoinHeal);
                                trigger_counts.record_health_gain(
                                    crate::player::skills::HealthGainSource::CoinHeal,
                                    heal_amount,
                                );
                            }
                        }
                    }

                    // CoinLightning: Picking up a coin causes a lightning strike
                    if skills.has(Heirloom::CoinLightning) {
                        if let Ok((player_txfm, attack, current_mana)) = player_query.single() {
                            const MANA_COST: i32 = 5;
                            if current_mana.0 >= MANA_COST {
                                // Find nearest enemy for lightning target
                                let player_pos = player_txfm.translation().truncate();
                                let mut enemy_distances: Vec<(Vec2, f32)> = enemies
                                    .iter()
                                    .map(|(e_txfm, _)| {
                                        let enemy_pos = e_txfm.translation().truncate();
                                        let dist = player_pos.distance(enemy_pos);
                                        (enemy_pos, dist)
                                    })
                                    .collect();
                                enemy_distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

                                if let Some((target_pos, _)) = enemy_distances.first() {
                                    let lightning_damage = attack.0; // 100% damage
                                    ranged_attack_event.write(RangedAttackEvent {
                                        projectile: Projectile::Lightning,
                                        direction: Vec2::ZERO,
                                        mana_cost: Some(MANA_COST),
                                        mana_cost_heirloom: Some(Heirloom::CoinLightning),
                                        from_enemy: false,
                                        from_entity: None,
                                        is_followup_proj: false,
                                        dmg_override: Some(lightning_damage),
                                        pos_override: Some(*target_pos + Vec2::new(0., 48.)),
                                        spawn_delay: 0.0,
                                    });
                                    modify_mana_event.write(ModifyManaEvent::new(-MANA_COST));
                                    commands.spawn(SoundSpawner::new(
                                        AudioSoundEffect::LightningStaffCast,
                                        0.4,
                                    ));
                                    trigger_counts.increment(Heirloom::CoinLightning);
                                }
                            }
                        }
                    }
                }
            }
        }

        if event.delta > 0 {
            if event.obj == WorldObject::TimeFragment {
                if *state == GameState::Main {
                    time_fragments.total_collected_time_fragments_this_run += event.delta;
                } else if *state == GameState::GameOver {
                    time_fragments.total_collected_time_fragments_all_time += event.delta as u128;
                }
            }

            commands.spawn(SoundSpawner::new(AudioSoundEffect::CurrencyPickup, 0.25));
        }
    }
}

pub fn reset_time_fragment_counters(mut currency: ResMut<TimeFragmentCurrency>) {
    currency.total_collected_time_fragments_this_run = 0;
    currency.bounce_timer.reset();
}

pub fn reset_coin_counters(mut coins: ResMut<CoinCurrency>) {
    coins.coins = 0;
    coins.bounce_timer.reset();
}
