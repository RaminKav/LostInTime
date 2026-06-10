use bevy::prelude::*;
use serde::Serialize;
use std::collections::HashMap;

use super::{HitEvent, InvincibilityTimer};
use crate::colors::{LEVEL_BLUE, WHITE, YELLOW_2};
use crate::enemy::Mob;
use crate::item::{projectile::Projectile, WorldObject};
use crate::player::skills::Heirloom;
use crate::Player;
use bevy::render::view::RenderLayers;
use bevy::sprite::Anchor;

/// Categories for sorting damage sources in the game over display.
/// Lower value = higher priority (displayed first).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DamageSourceCategory {
    Weapon = 0,
    Pet = 1,
    Skill = 2,
    Heirloom = 3,
}

impl DamageSourceCategory {
    pub fn display_name(&self) -> &'static str {
        match self {
            DamageSourceCategory::Weapon => "Weapons",
            DamageSourceCategory::Pet => "Pets",
            DamageSourceCategory::Skill => "Skills",
            DamageSourceCategory::Heirloom => "Heirlooms",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum DamageSource {
    // Weapons (melee)
    Sword,
    Spear,
    Dagger,
    Hammer,

    // Weapons (ranged)
    Bow,
    Gun,
    Blowdart,
    Claw,

    // Weapons (magic)
    IceStaff,
    FireStaff,
    PlasmaStaff,
    BasicStaff,
    MagicWhip,

    // Pet
    Pet,

    // Skills
    Arc,
    FireRing,
    IceWall,
    Meteors,
    Shout,
    LaserBeam,
    Bomb,
    DaggerSlash,
    DaggerThrow,
    FuryKunai,
    SpinAttack,
    SpearGravity,
    ArrowVolley,
    PossessedBlade,
    PiercingStar,
    SprintLunge,
    Recall,

    // Heirloom effects
    Echo,
    IceExplosion,
    SmallExplosion,
    IceFloor,
    TeleportShock,
    Thorns,
    Lightning,
    CherryBomb,
    ManaOrb,
    Poison,
    AntFarm,
    StoneTooth,
    SummonRing,
    ReaperSoul,
    EnergyBall,
}

impl DamageSource {
    pub fn category(&self) -> DamageSourceCategory {
        match self {
            DamageSource::Sword
            | DamageSource::Spear
            | DamageSource::Dagger
            | DamageSource::Hammer
            | DamageSource::Bow
            | DamageSource::Gun
            | DamageSource::Blowdart
            | DamageSource::Claw
            | DamageSource::IceStaff
            | DamageSource::FireStaff
            | DamageSource::PlasmaStaff
            | DamageSource::BasicStaff
            | DamageSource::MagicWhip => DamageSourceCategory::Weapon,

            DamageSource::Pet => DamageSourceCategory::Pet,

            DamageSource::FireRing
            | DamageSource::IceWall
            | DamageSource::Meteors
            | DamageSource::Shout
            | DamageSource::LaserBeam
            | DamageSource::Bomb
            | DamageSource::DaggerSlash
            | DamageSource::DaggerThrow
            | DamageSource::FuryKunai
            | DamageSource::SpinAttack
            | DamageSource::TeleportShock
            | DamageSource::SpearGravity
            | DamageSource::ArrowVolley
            | DamageSource::PossessedBlade
            | DamageSource::PiercingStar
            | DamageSource::SprintLunge
            | DamageSource::Recall => DamageSourceCategory::Skill,

            DamageSource::Echo
            | DamageSource::IceExplosion
            | DamageSource::SmallExplosion
            | DamageSource::IceFloor
            | DamageSource::Thorns
            | DamageSource::Lightning
            | DamageSource::CherryBomb
            | DamageSource::ManaOrb
            | DamageSource::Poison
            | DamageSource::AntFarm
            | DamageSource::SummonRing
            | DamageSource::StoneTooth
            | DamageSource::Arc
            | DamageSource::ReaperSoul
            | DamageSource::EnergyBall => DamageSourceCategory::Heirloom,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            DamageSource::Sword => "Sword",
            DamageSource::Spear => "Spear",
            DamageSource::Dagger => "Dagger",
            DamageSource::Hammer => "Hammer",
            DamageSource::Bow => "Bow",
            DamageSource::Gun => "Gun",
            DamageSource::Blowdart => "Blowdart",
            DamageSource::Claw => "Claw",
            DamageSource::IceStaff => "Ice Staff",
            DamageSource::FireStaff => "Fire Staff",
            DamageSource::PlasmaStaff => "Plasma Staff",
            DamageSource::BasicStaff => "Lightning Staff",
            DamageSource::MagicWhip => "Magic Whip",
            DamageSource::Pet => "Pet",
            DamageSource::Arc => "Hero Sword",
            DamageSource::FireRing => "Fire Ring",
            DamageSource::IceWall => "Ice Wall",
            DamageSource::Meteors => "Meteors",
            DamageSource::Shout => "Shout",
            DamageSource::LaserBeam => "Laser Beam",
            DamageSource::Bomb => "Bomb",
            DamageSource::DaggerSlash => "Dagger Slash",
            DamageSource::DaggerThrow => "Dagger Throw",
            DamageSource::FuryKunai => "Fury",
            DamageSource::SpinAttack => "Spin Attack",
            DamageSource::SpearGravity => "Grav. Spear",
            DamageSource::ArrowVolley => "Arrow Volley",
            DamageSource::PossessedBlade => "Possessed Blade",
            DamageSource::PiercingStar => "Piercing Star",
            DamageSource::SprintLunge => "Lunge",
            DamageSource::Recall => "Shadow Step",
            DamageSource::Echo => "Echo",
            DamageSource::IceExplosion => "Ice Explosion",
            DamageSource::SmallExplosion => "Small Explosion",
            DamageSource::IceFloor => "Ice Floor",
            DamageSource::TeleportShock => "Teleport",
            DamageSource::Thorns => "Thorns",
            DamageSource::Lightning => "Lightning",
            DamageSource::CherryBomb => "Cherry Bomb",
            DamageSource::ManaOrb => "Mana Orb",
            DamageSource::Poison => "Poison",
            DamageSource::AntFarm => "Ant Farm",
            DamageSource::SummonRing => "Piercing Ring",
            DamageSource::StoneTooth => "Stone Tooth",
            DamageSource::ReaperSoul => "Reaper Soul",
            DamageSource::EnergyBall => "UndWrld's Hat",
        }
    }

    pub fn from_heirloom(heirloom: &Heirloom) -> Option<DamageSource> {
        match heirloom {
            Heirloom::OnHitEcho | Heirloom::ParryEcho | Heirloom::SkillEcho => {
                Some(DamageSource::Echo)
            }
            Heirloom::FrozenAoE => Some(DamageSource::IceExplosion),
            Heirloom::SkillExplosion => Some(DamageSource::SmallExplosion),
            Heirloom::IceStaffFloor => Some(DamageSource::IceFloor),
            Heirloom::TeleportShock => Some(DamageSource::TeleportShock),
            Heirloom::Thorns | Heirloom::ThornsSpikes | Heirloom::ThornsOnDamage => {
                Some(DamageSource::Thorns)
            }
            Heirloom::CoinLightning | Heirloom::KillLightning | Heirloom::ManaRegenLightning => {
                Some(DamageSource::Lightning)
            }
            Heirloom::ManaOrbs | Heirloom::ManaOrbAttack => Some(DamageSource::ManaOrb),
            Heirloom::PoisonStacks
            | Heirloom::PoisonDuration
            | Heirloom::PoisonStrength
            | Heirloom::ViralVenum
            | Heirloom::ManaRegenPoison => Some(DamageSource::Poison),
            Heirloom::AntFarm => Some(DamageSource::AntFarm),
            Heirloom::SummonRing => Some(DamageSource::SummonRing),
            Heirloom::StoneTooth => Some(DamageSource::StoneTooth),
            Heirloom::Reaper => Some(DamageSource::ReaperSoul),
            Heirloom::EnergyBallBarrage => Some(DamageSource::EnergyBall),
            Heirloom::CherryBomb => Some(DamageSource::CherryBomb),
            _ => None,
        }
    }

    pub fn from_melee_weapon(obj: &WorldObject) -> Option<DamageSource> {
        match obj {
            WorldObject::Sword | WorldObject::WoodSword => Some(DamageSource::Sword),
            WorldObject::Spear => Some(DamageSource::Spear),
            WorldObject::Dagger => Some(DamageSource::Dagger),
            WorldObject::Hammer => Some(DamageSource::Hammer),
            _ => None,
        }
    }

    pub fn from_projectile(proj: &Projectile) -> Option<DamageSource> {
        match proj {
            Projectile::Arrow => Some(DamageSource::Bow),
            Projectile::Bullet | Projectile::Buckshot => Some(DamageSource::Gun),
            Projectile::Dart => Some(DamageSource::Blowdart),
            Projectile::ThrowingStar => Some(DamageSource::Claw),
            Projectile::ThrowingStarLarge => Some(DamageSource::PiercingStar),
            Projectile::IceShard => Some(DamageSource::IceStaff),
            Projectile::Fireball | Projectile::FireAttack => Some(DamageSource::FireStaff),
            Projectile::PlasmaBall | Projectile::PlasmaExplosion => Some(DamageSource::PlasmaStaff),
            Projectile::Rock => Some(DamageSource::BasicStaff),
            Projectile::GreenWhip => Some(DamageSource::MagicWhip),
            Projectile::Electricity => Some(DamageSource::BasicStaff),
            Projectile::SwordProjectile => Some(DamageSource::Sword),
            Projectile::DaggerProjectile1 | Projectile::DaggerProjectile2 => {
                Some(DamageSource::Dagger)
            }
            Projectile::SpearProjectile => Some(DamageSource::Spear),
            Projectile::HammerProjectile => Some(DamageSource::Hammer),

            Projectile::Arc => Some(DamageSource::Arc),
            Projectile::FireRing => Some(DamageSource::FireRing),
            Projectile::IceWall => Some(DamageSource::IceWall),
            Projectile::Meteor => Some(DamageSource::Meteors),
            Projectile::Shout => Some(DamageSource::Shout),
            Projectile::LaserBeam => Some(DamageSource::LaserBeam),
            Projectile::Bomb | Projectile::BombExplosion => Some(DamageSource::Bomb),
            Projectile::CherryBombExplosion => Some(DamageSource::CherryBomb),
            Projectile::DaggerSlash => Some(DamageSource::DaggerSlash),
            Projectile::DaggerThrow => Some(DamageSource::DaggerThrow),
            Projectile::FuryKunai => Some(DamageSource::FuryKunai),
            Projectile::SpinAttack => Some(DamageSource::SpinAttack),
            Projectile::SpearGravity => Some(DamageSource::SpearGravity),
            Projectile::ArrowVolleyShot => Some(DamageSource::ArrowVolley),
            Projectile::PossessedBlade => Some(DamageSource::PossessedBlade),

            Projectile::Echo => Some(DamageSource::Echo),
            Projectile::IceExplosionAOE => Some(DamageSource::IceExplosion),
            Projectile::SmallExplosionAOE => Some(DamageSource::SmallExplosion),
            Projectile::IceFloor => Some(DamageSource::IceFloor),
            Projectile::TeleportShock => Some(DamageSource::TeleportShock),
            Projectile::SprintLunge => Some(DamageSource::SprintLunge),
            Projectile::Recall => Some(DamageSource::Recall),
            // Both ThornsProjectile and direct thorns damage merge into Thorns
            Projectile::ThornsProjectile => Some(DamageSource::Thorns),
            Projectile::Lightning => Some(DamageSource::Lightning),
            Projectile::ManaOrbProjectile => Some(DamageSource::ManaOrb),
            Projectile::EnergyBall => Some(DamageSource::EnergyBall),

            _ => None,
        }
    }
}

/// Resource that accumulates all damage dealt by the player during a run
#[derive(Resource, Default, Debug, Serialize)]
pub struct DamageTracker {
    pub totals: HashMap<DamageSource, i64>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct MobStatEntry {
    pub kills: u32,
    pub damage_taken: i64,
}

/// Per mob type: kills attributed to the player and damage taken from that mob type during the run.
#[derive(Resource, Default, Debug, Serialize)]
pub struct MobStatTracker {
    pub per_mob: HashMap<Mob, MobStatEntry>,
}

impl MobStatTracker {
    pub fn record_damage_taken(&mut self, mob: Mob, amount: i32) {
        if mob == Mob::None || amount <= 0 {
            return;
        }
        let e = self.per_mob.entry(mob).or_default();
        e.damage_taken += amount as i64;
    }

    pub fn record_kill(&mut self, mob: Mob) {
        if mob == Mob::None {
            return;
        }
        let e = self.per_mob.entry(mob).or_default();
        e.kills += 1;
    }

    pub fn sorted_entries(&self) -> Vec<(Mob, &MobStatEntry)> {
        let mut v: Vec<_> = self
            .per_mob
            .iter()
            .filter(|(m, s)| **m != Mob::None && (s.kills > 0 || s.damage_taken > 0))
            .map(|(m, s)| (m.clone(), s))
            .collect();
        v.sort_by(|a, b| {
            b.1.damage_taken
                .cmp(&a.1.damage_taken)
                .then_with(|| b.1.kills.cmp(&a.1.kills))
                .then_with(|| {
                    a.0.stat_tracker_display_name()
                        .cmp(&b.0.stat_tracker_display_name())
                })
        });
        v
    }

    pub fn has_any(&self) -> bool {
        !self.sorted_entries().is_empty()
    }
}

/// Stats from pet active abilities (shields, healing, coins, self-damage) for the damage tracker UI.
#[derive(Resource, Default, Debug, Clone)]
pub struct PetAbilityStats {
    pub shields_generated: u32,
    pub healing: i64,
    pub coins: u32,
    pub self_damage: i64,
}

impl PetAbilityStats {
    pub fn has_any(&self) -> bool {
        self.shields_generated > 0 || self.healing > 0 || self.coins > 0 || self.self_damage > 0
    }
}

impl DamageTracker {
    pub fn record(&mut self, source: DamageSource, amount: i32) {
        if amount > 0 {
            *self.totals.entry(source).or_insert(0) += amount as i64;
        }
    }

    /// Returns entries grouped by category. Each group is a tuple of
    /// (category, vec of (source, damage)) sorted by damage descending within each group.
    /// Only includes categories/entries with damage > 0.
    pub fn grouped_entries(&self) -> Vec<(DamageSourceCategory, Vec<(DamageSource, i64)>)> {
        let mut by_category: HashMap<DamageSourceCategory, Vec<(DamageSource, i64)>> =
            HashMap::new();

        for (&src, &dmg) in &self.totals {
            if dmg > 0 {
                by_category
                    .entry(src.category())
                    .or_default()
                    .push((src, dmg));
            }
        }

        // Sort entries within each category by damage descending
        for entries in by_category.values_mut() {
            entries.sort_by(|a, b| b.1.cmp(&a.1));
        }

        // Sort categories by priority
        let mut groups: Vec<(DamageSourceCategory, Vec<(DamageSource, i64)>)> =
            by_category.into_iter().collect();
        groups.sort_by_key(|(cat, _)| *cat);

        groups
    }
}

pub fn format_damage(damage: i64) -> String {
    crate::ui::ui_helpers::format_number(damage)
}

/// System that reads HitEvents and tracks all player-dealt damage
pub fn track_player_damage(
    mut hit_events: EventReader<HitEvent>,
    mut tracker: ResMut<DamageTracker>,
    player_query: Query<Entity, With<Player>>,
    in_i_frame: Query<&InvincibilityTimer>,
    mob_query: Query<(), With<Mob>>,
) {
    let Ok(player_entity) = player_query.get_single() else {
        return;
    };

    for hit in hit_events.iter() {
        if hit.hit_entity == player_entity {
            continue;
        }
        if hit.hit_by_mob.is_some() {
            continue;
        }
        // Only track damage dealt to enemies, not world objects
        if mob_query.get(hit.hit_entity).is_err() {
            continue;
        }
        if in_i_frame.get(hit.hit_entity).is_ok() {
            continue;
        }

        let dmg = if hit.damage == 0 { 1 } else { hit.damage };

        let source = if hit.hit_by_pet.is_some() {
            Some(DamageSource::Pet)
        } else if let Some(ref melee_obj) = hit.hit_with_melee {
            DamageSource::from_melee_weapon(melee_obj)
        } else if let Some(ref proj) = hit.hit_with_projectile {
            DamageSource::from_projectile(proj)
        } else if let Some(ref heirloom) = hit.from_heirloom_effect {
            DamageSource::from_heirloom(heirloom)
        } else {
            Some(DamageSource::Thorns)
        };

        if let Some(source) = source {
            tracker.record(source, dmg);
        }
    }
}

/// Spawn damage-dealt UI (panel entity first in returned vec; use for `GameOverText` etc.).
///
/// `width` specifies the horizontal distance between the left-aligned text and right-aligned text.
/// `base_alpha` is used for fading (0.0 for fading in on game over, 1.0 for inventory).
/// `pet_stats` when provided adds Shields/Healing/Coins/Self damage lines under the Pet category.
///
/// Returns spawned entities and final relative `cursor_y` (negative offset below the title) for stacking another panel below.
pub fn spawn_damage_tracker_ui(
    commands: &mut Commands,
    asset_server: &AssetServer,
    tracker: &DamageTracker,
    parent_transform: Transform,
    base_alpha: f32,
    width: f32,
    pet_stats: Option<&PetAbilityStats>,
) -> Option<(Vec<Entity>, f32)> {
    let mut groups = tracker.grouped_entries();
    if let Some(ps) = pet_stats {
        if ps.has_any() && !groups.iter().any(|(c, _)| *c == DamageSourceCategory::Pet) {
            groups.push((DamageSourceCategory::Pet, vec![]));
            groups.sort_by_key(|(cat, _)| *cat);
        }
    }
    if groups.is_empty() {
        return None;
    }

    let mut spawned_entities = Vec::new();

    let panel = commands
        .spawn((
            SpriteBundle {
                sprite: Sprite {
                    color: Color::rgba(0., 0., 0., 0.),
                    ..Default::default()
                },
                transform: parent_transform,
                ..default()
            },
            RenderLayers::from_layers(&[3]),
            Name::new("Damage Tracker Panel"),
        ))
        .id();
    spawned_entities.push(panel);

    let row_spacing = 8.0;
    let category_gap = 4.0;
    let mut cursor_y = 0.0;
    let hw = width / 2.0;

    let title = commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    "Damage Dealt",
                    TextStyle {
                        font: asset_server.load("fonts/4x5.ttf"),
                        font_size: 5.0,
                        color: YELLOW_2.with_a(base_alpha),
                    },
                )
                .with_alignment(TextAlignment::Left),
                text_anchor: Anchor::CenterLeft,
                transform: Transform::from_translation(Vec3::new(-hw, cursor_y, 1.)),
                ..default()
            },
            RenderLayers::from_layers(&[3]),
            Name::new("Damage Tracker Title"),
        ))
        .id();
    commands.entity(panel).add_child(title);
    spawned_entities.push(title);
    cursor_y -= 10.0;

    for (category, entries) in &groups {
        let header = commands
            .spawn((
                Text2dBundle {
                    text: Text::from_section(
                        category.display_name(),
                        TextStyle {
                            font: asset_server.load("fonts/4x5.ttf"),
                            font_size: 5.0,
                            color: LEVEL_BLUE.with_a(base_alpha),
                        },
                    )
                    .with_alignment(TextAlignment::Left),
                    text_anchor: Anchor::CenterLeft,
                    transform: Transform::from_translation(Vec3::new(-hw, cursor_y, 1.)),
                    ..default()
                },
                RenderLayers::from_layers(&[3]),
            ))
            .id();
        commands.entity(panel).add_child(header);
        spawned_entities.push(header);
        cursor_y -= row_spacing;

        for (source, amount) in entries {
            let name = commands
                .spawn((
                    Text2dBundle {
                        text: Text::from_section(
                            format!(" {}", source.display_name()),
                            TextStyle {
                                font: asset_server.load("fonts/4x5.ttf"),
                                font_size: 5.0,
                                color: WHITE.with_a(base_alpha),
                            },
                        )
                        .with_alignment(TextAlignment::Left),
                        text_anchor: Anchor::CenterLeft,
                        transform: Transform::from_translation(Vec3::new(-hw, cursor_y, 1.)),
                        ..default()
                    },
                    RenderLayers::from_layers(&[3]),
                ))
                .id();
            commands.entity(panel).add_child(name);
            spawned_entities.push(name);

            let value = commands
                .spawn((
                    Text2dBundle {
                        text: Text::from_section(
                            format_damage(*amount),
                            TextStyle {
                                font: asset_server.load("fonts/4x5.ttf"),
                                font_size: 5.0,
                                color: WHITE.with_a(base_alpha),
                            },
                        )
                        .with_alignment(TextAlignment::Right),
                        text_anchor: Anchor::CenterRight,
                        transform: Transform::from_translation(Vec3::new(hw + 10., cursor_y, 1.)),
                        ..default()
                    },
                    RenderLayers::from_layers(&[3]),
                ))
                .id();
            commands.entity(panel).add_child(value);
            spawned_entities.push(value);

            cursor_y -= row_spacing;
        }

        // Pet category: add ability stats (shields, healing, coins, self damage) when provided
        if *category == DamageSourceCategory::Pet {
            if let Some(ps) = pet_stats {
                if ps.shields_generated > 0 {
                    let name = commands
                        .spawn((
                            Text2dBundle {
                                text: Text::from_section(
                                    " Shields",
                                    TextStyle {
                                        font: asset_server.load("fonts/4x5.ttf"),
                                        font_size: 5.0,
                                        color: WHITE.with_a(base_alpha),
                                    },
                                )
                                .with_alignment(TextAlignment::Left),
                                text_anchor: Anchor::CenterLeft,
                                transform: Transform::from_translation(Vec3::new(
                                    -hw, cursor_y, 1.,
                                )),
                                ..default()
                            },
                            RenderLayers::from_layers(&[3]),
                        ))
                        .id();
                    commands.entity(panel).add_child(name);
                    spawned_entities.push(name);
                    let value = commands
                        .spawn((
                            Text2dBundle {
                                text: Text::from_section(
                                    format_damage(ps.shields_generated as i64),
                                    TextStyle {
                                        font: asset_server.load("fonts/4x5.ttf"),
                                        font_size: 5.0,
                                        color: WHITE.with_a(base_alpha),
                                    },
                                )
                                .with_alignment(TextAlignment::Right),
                                text_anchor: Anchor::CenterRight,
                                transform: Transform::from_translation(Vec3::new(
                                    hw + 10.,
                                    cursor_y,
                                    1.,
                                )),
                                ..default()
                            },
                            RenderLayers::from_layers(&[3]),
                        ))
                        .id();
                    commands.entity(panel).add_child(value);
                    spawned_entities.push(value);
                    cursor_y -= row_spacing;
                }
                if ps.healing > 0 {
                    let name = commands
                        .spawn((
                            Text2dBundle {
                                text: Text::from_section(
                                    " Healing",
                                    TextStyle {
                                        font: asset_server.load("fonts/4x5.ttf"),
                                        font_size: 5.0,
                                        color: WHITE.with_a(base_alpha),
                                    },
                                )
                                .with_alignment(TextAlignment::Left),
                                text_anchor: Anchor::CenterLeft,
                                transform: Transform::from_translation(Vec3::new(
                                    -hw, cursor_y, 1.,
                                )),
                                ..default()
                            },
                            RenderLayers::from_layers(&[3]),
                        ))
                        .id();
                    commands.entity(panel).add_child(name);
                    spawned_entities.push(name);
                    let value = commands
                        .spawn((
                            Text2dBundle {
                                text: Text::from_section(
                                    format_damage(ps.healing),
                                    TextStyle {
                                        font: asset_server.load("fonts/4x5.ttf"),
                                        font_size: 5.0,
                                        color: WHITE.with_a(base_alpha),
                                    },
                                )
                                .with_alignment(TextAlignment::Right),
                                text_anchor: Anchor::CenterRight,
                                transform: Transform::from_translation(Vec3::new(
                                    hw + 10.,
                                    cursor_y,
                                    1.,
                                )),
                                ..default()
                            },
                            RenderLayers::from_layers(&[3]),
                        ))
                        .id();
                    commands.entity(panel).add_child(value);
                    spawned_entities.push(value);
                    cursor_y -= row_spacing;
                }
                if ps.coins > 0 {
                    let name = commands
                        .spawn((
                            Text2dBundle {
                                text: Text::from_section(
                                    " Coins",
                                    TextStyle {
                                        font: asset_server.load("fonts/4x5.ttf"),
                                        font_size: 5.0,
                                        color: WHITE.with_a(base_alpha),
                                    },
                                )
                                .with_alignment(TextAlignment::Left),
                                text_anchor: Anchor::CenterLeft,
                                transform: Transform::from_translation(Vec3::new(
                                    -hw, cursor_y, 1.,
                                )),
                                ..default()
                            },
                            RenderLayers::from_layers(&[3]),
                        ))
                        .id();
                    commands.entity(panel).add_child(name);
                    spawned_entities.push(name);
                    let value = commands
                        .spawn((
                            Text2dBundle {
                                text: Text::from_section(
                                    format_damage(ps.coins as i64),
                                    TextStyle {
                                        font: asset_server.load("fonts/4x5.ttf"),
                                        font_size: 5.0,
                                        color: WHITE.with_a(base_alpha),
                                    },
                                )
                                .with_alignment(TextAlignment::Right),
                                text_anchor: Anchor::CenterRight,
                                transform: Transform::from_translation(Vec3::new(
                                    hw + 10.,
                                    cursor_y,
                                    1.,
                                )),
                                ..default()
                            },
                            RenderLayers::from_layers(&[3]),
                        ))
                        .id();
                    commands.entity(panel).add_child(value);
                    spawned_entities.push(value);
                    cursor_y -= row_spacing;
                }
                if ps.self_damage > 0 {
                    let name = commands
                        .spawn((
                            Text2dBundle {
                                text: Text::from_section(
                                    " Self damage",
                                    TextStyle {
                                        font: asset_server.load("fonts/4x5.ttf"),
                                        font_size: 5.0,
                                        color: WHITE.with_a(base_alpha),
                                    },
                                )
                                .with_alignment(TextAlignment::Left),
                                text_anchor: Anchor::CenterLeft,
                                transform: Transform::from_translation(Vec3::new(
                                    -hw, cursor_y, 1.,
                                )),
                                ..default()
                            },
                            RenderLayers::from_layers(&[3]),
                        ))
                        .id();
                    commands.entity(panel).add_child(name);
                    spawned_entities.push(name);
                    let value = commands
                        .spawn((
                            Text2dBundle {
                                text: Text::from_section(
                                    format_damage(ps.self_damage),
                                    TextStyle {
                                        font: asset_server.load("fonts/4x5.ttf"),
                                        font_size: 5.0,
                                        color: WHITE.with_a(base_alpha),
                                    },
                                )
                                .with_alignment(TextAlignment::Right),
                                text_anchor: Anchor::CenterRight,
                                transform: Transform::from_translation(Vec3::new(
                                    hw + 10.,
                                    cursor_y,
                                    1.,
                                )),
                                ..default()
                            },
                            RenderLayers::from_layers(&[3]),
                        ))
                        .id();
                    commands.entity(panel).add_child(value);
                    spawned_entities.push(value);
                    cursor_y -= row_spacing;
                }
            }
        }

        cursor_y -= category_gap;
    }

    Some((spawned_entities, cursor_y))
}

/// Mob kills and damage taken from each mob type (stack below damage-dealt via transform).
pub fn spawn_mob_stat_tracker_ui(
    commands: &mut Commands,
    asset_server: &AssetServer,
    tracker: &MobStatTracker,
    parent_transform: Transform,
    base_alpha: f32,
    width: f32,
) -> Option<(Vec<Entity>, f32)> {
    let entries = tracker.sorted_entries();
    if entries.is_empty() {
        return None;
    }

    let mut spawned_entities = Vec::new();
    let row_spacing = 8.0;
    let category_gap = 4.0;
    let mut cursor_y = 0.0;
    let hw = width / 2.0;

    let panel = commands
        .spawn((
            SpriteBundle {
                sprite: Sprite {
                    color: Color::rgba(0., 0., 0., 0.),
                    ..Default::default()
                },
                transform: parent_transform,
                ..default()
            },
            RenderLayers::from_layers(&[3]),
            Name::new("Mob Stat Tracker Panel"),
        ))
        .id();
    spawned_entities.push(panel);

    let title = commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    "Mob encounters",
                    TextStyle {
                        font: asset_server.load("fonts/4x5.ttf"),
                        font_size: 5.0,
                        color: YELLOW_2.with_a(base_alpha),
                    },
                )
                .with_alignment(TextAlignment::Left),
                text_anchor: Anchor::CenterLeft,
                transform: Transform::from_translation(Vec3::new(-hw, cursor_y, 1.)),
                ..default()
            },
            RenderLayers::from_layers(&[3]),
            Name::new("Mob Stat Tracker Title"),
        ))
        .id();
    commands.entity(panel).add_child(title);
    spawned_entities.push(title);
    cursor_y -= 10.0;

    for (mob, stats) in entries {
        let mob_header = commands
            .spawn((
                Text2dBundle {
                    text: Text::from_section(
                        mob.stat_tracker_display_name(),
                        TextStyle {
                            font: asset_server.load("fonts/4x5.ttf"),
                            font_size: 5.0,
                            color: LEVEL_BLUE.with_a(base_alpha),
                        },
                    )
                    .with_alignment(TextAlignment::Left),
                    text_anchor: Anchor::CenterLeft,
                    transform: Transform::from_translation(Vec3::new(-hw, cursor_y, 1.)),
                    ..default()
                },
                RenderLayers::from_layers(&[3]),
            ))
            .id();
        commands.entity(panel).add_child(mob_header);
        spawned_entities.push(mob_header);
        cursor_y -= row_spacing;

        let kills_label = commands
            .spawn((
                Text2dBundle {
                    text: Text::from_section(
                        "    Kills:",
                        TextStyle {
                            font: asset_server.load("fonts/4x5.ttf"),
                            font_size: 5.0,
                            color: WHITE.with_a(base_alpha),
                        },
                    )
                    .with_alignment(TextAlignment::Left),
                    text_anchor: Anchor::CenterLeft,
                    transform: Transform::from_translation(Vec3::new(-hw, cursor_y, 1.)),
                    ..default()
                },
                RenderLayers::from_layers(&[3]),
            ))
            .id();
        commands.entity(panel).add_child(kills_label);
        spawned_entities.push(kills_label);

        let kills_val = commands
            .spawn((
                Text2dBundle {
                    text: Text::from_section(
                        format_damage(stats.kills as i64),
                        TextStyle {
                            font: asset_server.load("fonts/4x5.ttf"),
                            font_size: 5.0,
                            color: WHITE.with_a(base_alpha),
                        },
                    )
                    .with_alignment(TextAlignment::Right),
                    text_anchor: Anchor::CenterRight,
                    transform: Transform::from_translation(Vec3::new(hw + 10., cursor_y, 1.)),
                    ..default()
                },
                RenderLayers::from_layers(&[3]),
            ))
            .id();
        commands.entity(panel).add_child(kills_val);
        spawned_entities.push(kills_val);
        cursor_y -= row_spacing;

        let hp_label = commands
            .spawn((
                Text2dBundle {
                    text: Text::from_section(
                        "    HP Lost:",
                        TextStyle {
                            font: asset_server.load("fonts/4x5.ttf"),
                            font_size: 5.0,
                            color: WHITE.with_a(base_alpha),
                        },
                    )
                    .with_alignment(TextAlignment::Left),
                    text_anchor: Anchor::CenterLeft,
                    transform: Transform::from_translation(Vec3::new(-hw, cursor_y, 1.)),
                    ..default()
                },
                RenderLayers::from_layers(&[3]),
            ))
            .id();
        commands.entity(panel).add_child(hp_label);
        spawned_entities.push(hp_label);

        let hp_val = commands
            .spawn((
                Text2dBundle {
                    text: Text::from_section(
                        format_damage(stats.damage_taken),
                        TextStyle {
                            font: asset_server.load("fonts/4x5.ttf"),
                            font_size: 5.0,
                            color: WHITE.with_a(base_alpha),
                        },
                    )
                    .with_alignment(TextAlignment::Right),
                    text_anchor: Anchor::CenterRight,
                    transform: Transform::from_translation(Vec3::new(hw + 10., cursor_y, 1.)),
                    ..default()
                },
                RenderLayers::from_layers(&[3]),
            ))
            .id();
        commands.entity(panel).add_child(hp_val);
        spawned_entities.push(hp_val);
        cursor_y -= row_spacing;

        cursor_y -= category_gap;
    }

    Some((spawned_entities, cursor_y))
}
