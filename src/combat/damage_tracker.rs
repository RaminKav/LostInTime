use bevy::prelude::*;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    Shout,
    LaserBeam,
    Bomb,
    DaggerSlash,
    DaggerThrow,
    FuryKunai,
    SpinAttack,
    SpearGravity,

    // Heirloom effects
    Echo,
    IceExplosion,
    IceFloor,
    TeleportShock,
    Thorns,
    Lightning,
    ManaOrb,
    Poison,
    AntFarm,
    StoneTooth,
    SummonRing,
    ReaperSoul,
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
            | DamageSource::Shout
            | DamageSource::LaserBeam
            | DamageSource::Bomb
            | DamageSource::DaggerSlash
            | DamageSource::DaggerThrow
            | DamageSource::FuryKunai
            | DamageSource::SpinAttack
            | DamageSource::TeleportShock
            | DamageSource::SpearGravity => DamageSourceCategory::Skill,

            DamageSource::Echo
            | DamageSource::IceExplosion
            | DamageSource::IceFloor
            | DamageSource::Thorns
            | DamageSource::Lightning
            | DamageSource::ManaOrb
            | DamageSource::Poison
            | DamageSource::AntFarm
            | DamageSource::SummonRing
            | DamageSource::StoneTooth
            | DamageSource::Arc
            | DamageSource::ReaperSoul => DamageSourceCategory::Heirloom,
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
            DamageSource::Shout => "Shout",
            DamageSource::LaserBeam => "Laser Beam",
            DamageSource::Bomb => "Bomb",
            DamageSource::DaggerSlash => "Dagger Slash",
            DamageSource::DaggerThrow => "Dagger Throw",
            DamageSource::FuryKunai => "Fury Kunai",
            DamageSource::SpinAttack => "Spin Attack",
            DamageSource::SpearGravity => "Grav. Spear",
            DamageSource::Echo => "Echo",
            DamageSource::IceExplosion => "Ice Explosion",
            DamageSource::IceFloor => "Ice Floor",
            DamageSource::TeleportShock => "Teleport",
            DamageSource::Thorns => "Thorns",
            DamageSource::Lightning => "Lightning",
            DamageSource::ManaOrb => "Mana Orb",
            DamageSource::Poison => "Poison",
            DamageSource::AntFarm => "Ant Farm",
            DamageSource::SummonRing => "Piercing Ring",
            DamageSource::StoneTooth => "Stone Tooth",
            DamageSource::ReaperSoul => "Reaper Soul",
        }
    }

    pub fn from_heirloom(heirloom: &Heirloom) -> Option<DamageSource> {
        match heirloom {
            Heirloom::OnHitEcho | Heirloom::ParryEcho | Heirloom::SkillEcho => {
                Some(DamageSource::Echo)
            }
            Heirloom::FrozenAoE => Some(DamageSource::IceExplosion),
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
            Projectile::ThrowingStar | Projectile::ThrowingStarLarge => Some(DamageSource::Claw),
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
            Projectile::Shout => Some(DamageSource::Shout),
            Projectile::LaserBeam => Some(DamageSource::LaserBeam),
            Projectile::Bomb | Projectile::BombExplosion => Some(DamageSource::Bomb),
            Projectile::DaggerSlash => Some(DamageSource::DaggerSlash),
            Projectile::DaggerThrow => Some(DamageSource::DaggerThrow),
            Projectile::FuryKunai => Some(DamageSource::FuryKunai),
            Projectile::SpinAttack => Some(DamageSource::SpinAttack),
            Projectile::SpearGravity => Some(DamageSource::SpearGravity),

            Projectile::Echo => Some(DamageSource::Echo),
            Projectile::IceExplosionAOE => Some(DamageSource::IceExplosion),
            Projectile::IceFloor => Some(DamageSource::IceFloor),
            Projectile::TeleportShock => Some(DamageSource::TeleportShock),
            // Both ThornsProjectile and direct thorns damage merge into Thorns
            Projectile::ThornsProjectile => Some(DamageSource::Thorns),
            Projectile::Lightning => Some(DamageSource::Lightning),
            Projectile::ManaOrbProjectile => Some(DamageSource::ManaOrb),

            _ => None,
        }
    }
}

/// Resource that accumulates all damage dealt by the player during a run
#[derive(Resource, Default, Debug)]
pub struct DamageTracker {
    pub totals: HashMap<DamageSource, i64>,
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
        self.shields_generated > 0
            || self.healing > 0
            || self.coins > 0
            || self.self_damage > 0
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

/// Helper function to spawn the damage tracker UI elements.
/// Returns a vector of spawned entities (the parent panel and all text nodes)
/// so the caller can add context-specific components (like `GameOverText`).
///
/// `width` specifies the horizontal distance between the left-aligned text and right-aligned text.
/// `base_alpha` is used for fading (0.0 for fading in on game over, 1.0 for inventory).
/// `pet_stats` when provided adds Shields/Healing/Coins/Self damage lines under the Pet category.
pub fn spawn_damage_tracker_ui(
    commands: &mut Commands,
    asset_server: &AssetServer,
    tracker: &DamageTracker,
    parent_transform: Transform,
    base_alpha: f32,
    width: f32,
    pet_stats: Option<&PetAbilityStats>,
) -> Option<Vec<Entity>> {
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
                                    format_damage(ps.shields_generated as i64),
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
                                    format_damage(ps.healing),
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
                                    format_damage(ps.coins as i64),
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
                                    format_damage(ps.self_damage),
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
            }
        }

        cursor_y -= category_gap;
    }

    Some(spawned_entities)
}
