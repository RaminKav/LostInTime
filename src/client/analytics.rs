use std::fs::{create_dir_all, File};
/*
    Analytics System:
    track various data points through event listeners.

    Mobs Killed: kills per mob
    Items Collected: num per item
    Recipes Crafted: num per recipe
    Total Recipes Crafted: num
    Damage Taken: damage per mob
    Total Damage Taken: num
    Damage Dealt: damage per mob
    Total Damage Dealt: num
    Objects Broken: num per object
    Total Objects Broken: num
    Objects Placed: num per object
    Total Objects Placed: num
    Items Consumed: num per item
    Total Items Consumed: num



*/
use bevy::{prelude::*, utils::HashMap};
use serde::{Deserialize, Serialize};
use tungstenite::{connect, Message};

use crate::{
    datafiles::{self},
    enemy::Mob,
    item::WorldObject,
    player::skills::Skill,
    world::dimension::GenerationSeed,
    GameState, DEBUG,
};

#[derive(Debug, Clone, Resource, Default, Serialize, Deserialize)]
pub struct AnalyticsData {
    pub user_id: String,

    pub mobs_killed: HashMap<Mob, u32>,
    pub items_collected: HashMap<WorldObject, u32>,
    pub recipes_crafted: HashMap<WorldObject, u32>,
    pub total_recipes_crafted: u32,
    pub damage_taken: HashMap<Mob, u32>,
    pub total_damage_taken: u32,
    pub damage_dealt: HashMap<Mob, u32>,
    pub total_damage_dealt: u32,
    pub objects_broken: HashMap<WorldObject, u32>,
    pub total_objects_broken: u32,
    pub objects_placed: HashMap<WorldObject, u32>,
    pub total_objects_placed: u32,
    pub items_consumed: HashMap<WorldObject, u32>,
    pub total_items_consumed: u32,
    pub skills: Vec<Skill>,
    pub nights_survived: u32,
    pub timestamp: String,
}

pub struct AnalyticsPlugin;

impl Plugin for AnalyticsPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<AnalyticsUpdateEvent>()
            .add_event::<SendAnalyticsDataToServerEvent>()
            .add_system(handle_analytics_update.run_if(resource_exists::<AnalyticsData>()))
            .add_system(add_analytics_resource_on_start.in_schedule(OnExit(GameState::MainMenu)))
            .add_system(
                save_analytics_data_to_file_on_game_over.run_if(resource_exists::<AnalyticsData>()),
            );
    }
}

pub struct AnalyticsUpdateEvent {
    pub update_type: AnalyticsTrigger,
}

#[derive(Default)]
pub struct SendAnalyticsDataToServerEvent;

#[derive(Debug, Clone)]
pub enum AnalyticsTrigger {
    MobKilled(Mob),
    ItemCollected(WorldObject),
    RecipeCrafted(WorldObject),
    DamageTaken(Mob, u32),
    DamageDealt(Mob, u32),
    ObjectBroken(WorldObject),
    ObjectPlaced(WorldObject),
    ItemConsumed(WorldObject),
}

pub fn handle_analytics_update(
    mut analytics_data: ResMut<AnalyticsData>,
    mut events: EventReader<AnalyticsUpdateEvent>,
) {
    for event in events.iter() {
        match event.update_type.clone() {
            AnalyticsTrigger::MobKilled(mob) => {
                *analytics_data.mobs_killed.entry(mob).or_insert(0) += 1;
            }
            AnalyticsTrigger::ItemCollected(item) => {
                *analytics_data.items_collected.entry(item).or_insert(0) += 1;
            }
            AnalyticsTrigger::RecipeCrafted(recipe) => {
                *analytics_data.recipes_crafted.entry(recipe).or_insert(0) += 1;
                analytics_data.total_recipes_crafted += 1;
            }
            AnalyticsTrigger::DamageTaken(mob, damage) => {
                *analytics_data.damage_taken.entry(mob).or_insert(0) += damage;
                analytics_data.total_damage_taken += damage;
            }
            AnalyticsTrigger::DamageDealt(mob, damage) => {
                *analytics_data.damage_dealt.entry(mob).or_insert(0) += damage;
                analytics_data.total_damage_dealt += damage;
            }
            AnalyticsTrigger::ObjectBroken(object) => {
                *analytics_data.objects_broken.entry(object).or_insert(0) += 1;
                analytics_data.total_objects_broken += 1;
            }
            AnalyticsTrigger::ObjectPlaced(object) => {
                *analytics_data.objects_placed.entry(object).or_insert(0) += 1;
                analytics_data.total_objects_placed += 1;
            }
            AnalyticsTrigger::ItemConsumed(item) => {
                *analytics_data.items_consumed.entry(item).or_insert(0) += 1;
                analytics_data.total_items_consumed += 1;
            }
        }
    }
}

pub fn add_analytics_resource_on_start(mut commands: Commands) {
    commands.insert_resource(AnalyticsData::default());
}
pub fn save_analytics_data_to_file_on_game_over(
    analytics_data: Res<AnalyticsData>,
    seed: Res<GenerationSeed>,
    mut events: EventReader<SendAnalyticsDataToServerEvent>,
    commands: Commands,
) {
    if events.iter().count() == 0 {
        return;
    }
    let analytics_dir = datafiles::analytics_dir();
    if let Ok(()) = create_dir_all(analytics_dir) {
        let analytics_file = {
            let mut file = datafiles::analytics_dir();
            file.push(format!("analytics_{}.json", seed.seed));
            file
        };
        let file = File::create(analytics_file).expect("Could not open file for serialization");

        if let Err(result) = serde_json::to_writer(file, &analytics_data.clone()) {
            error!("Failed to save game state: {result:?}");
        } else {
            info!("SAVED ANALYTICS!");
        }
    }
    info!("Sending analytics data to server...");
    if !*DEBUG {
        connect_server(analytics_data.clone());
    }
}

fn connect_server(data: AnalyticsData) {
    info!("Connecting to server...");
    let json = serde_json::to_string(&data).expect("data serializes");
    info!("analytics data {json}");

    match connect("wss://bevy-analytics.shuttleapp.rs/ws") {
        Ok((mut socket, response)) => {
            if let Err(e) = socket.send(Message::Text(json)) {
                error!("failed to send txt {e}");
            }
        }
        Err(e) => error!("failed to connect {e}"),
    }
}
