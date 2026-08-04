//! Side info boxes (`TooltipInfoBox.png` + slkscr 8.5) for tooltip glossary entries and trigger counts.

use bevy::text::Justify;
use bevy::{camera::visibility::RenderLayers, prelude::*, sprite::Anchor};

use crate::{
    assets::Graphics,
    colors::{WHITE, YELLOW_2},
    item::item_drop_outline::UiShadow,
    player::skills::Heirloom,
};

use super::{game_fonts as gf, ui_helpers, ScreenResolution, UIElement, TOOLTIP_INFO_BOX_SIZE};

const INFO_BOX_GAP_FROM_TOOLTIP: f32 = 0.;
const INFO_BOX_STACK_GAP: f32 = 4.;
const INFO_BOX_SCREEN_EDGE_PAD: f32 = 6.;
/// Distance from the card's top edge to the first info box center.
const INFO_BOX_TOP_INSET: f32 = 16.;
const INFO_BOX_LINE1_Y: f32 = 5.;
const INFO_BOX_LINE2_Y: f32 = -6.;

/// Glossary term shown in a side info box.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TooltipDefinition {
    Echo,
    Summon,
    Lightning,
    IceExplosion,
    Poison,
    FreezeChance,
    CritChance,
    CritDamage,
    Attack,
    AttackSpeed,
    Defence,
    Speed,
    Health,
    Mana,
    ManaRegen,
    Thorns,
    Lifesteal,
    Frail,
    Dodge,
    SkillPower,
    PickupRange,
    Size,
    Luck,
    Chaos,
    Weapons,
    Skills,
}

impl TooltipDefinition {
    pub fn lines(self) -> [&'static str; 2] {
        match self {
            TooltipDefinition::Echo => ["Echo:", "AoE damage around you"],
            TooltipDefinition::Summon => ["Summon:", "Auto summons on a timer"],
            TooltipDefinition::Lightning => ["Lightning Strike:", "Strikes a random enemy"],
            TooltipDefinition::IceExplosion => ["Ice Explosion:", "AoE damage at target"],
            TooltipDefinition::Poison => ["Poisoned enemies take", "damage over time"],
            TooltipDefinition::FreezeChance => ["Frozen enemies are slowed", "by 15% per stack"],
            TooltipDefinition::CritChance => ["Crit Chance", "Chance to crit on hit."],
            TooltipDefinition::CritDamage => ["Crit Damage", "Bonus crit hit damage."],
            TooltipDefinition::Attack => ["Damage & Attack scale", "all sources of damage."],
            TooltipDefinition::AttackSpeed => ["Attack Speed scales your", "weapon attack rate"],
            TooltipDefinition::Defence => ["Defence Reduces incoming", "damage."],
            TooltipDefinition::Speed => ["Speed", "Movement speed."],
            TooltipDefinition::Frail => ["Frail enemies take", "+10% Damage per stack"],
            TooltipDefinition::Health => ["Health", "Maximum hit points."],
            TooltipDefinition::Mana => ["Mana is used to trigger", "Heirloom effects"],
            TooltipDefinition::ManaRegen => ["Mana Regen:", "Restores mana over time"],
            TooltipDefinition::Thorns => ["Thorns returns dmg when", "hit. Scales with attack"],
            TooltipDefinition::Lifesteal => ["Lifesteal triggers heal", "for 1 HP"],
            TooltipDefinition::Dodge => ["Dodge is the chance to", "avoid hits"],
            TooltipDefinition::SkillPower => ["Skill Power Boosts", "skill damage or effects"],
            TooltipDefinition::PickupRange => ["Pickup Range", "Item pickup radius."],
            TooltipDefinition::Size => {
                ["Size makes attacks, skills", "and heirloom effects larger"]
            }
            TooltipDefinition::Luck => ["Luck increases the chance", "of rare heirlooms and gear"],
            TooltipDefinition::Chaos => ["Chaos gives enemies more", "hp, attack, and score."],
            TooltipDefinition::Weapons => ["Weapons are equipped and", "attack on their own."],
            TooltipDefinition::Skills => ["Skills are powerful", "abilities on a cooldown."],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeirloomDescLineKind {
    Effect,
    Mana,
    Stat,
    Blank,
}

#[derive(Clone, Debug)]
pub struct HeirloomDescLine {
    pub text: String,
    pub kind: HeirloomDescLineKind,
    /// Rich spans for keyword highlighting. When `None`, spawn uses plain [`Self::text`].
    pub spans: Option<Vec<crate::ui::desc_spans::DescSpan>>,
}

impl HeirloomDescLine {
    fn flatten_spans(spans: &[crate::ui::desc_spans::DescSpan]) -> String {
        use crate::ui::desc_spans::DescSpan;
        spans
            .iter()
            .map(|s| match s {
                DescSpan::Plain(t) | DescSpan::Number(t) => t.as_str(),
                DescSpan::Keyword { text, .. } | DescSpan::Named { text, .. } => text.as_str(),
            })
            .collect()
    }

    pub fn effect(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: HeirloomDescLineKind::Effect,
            spans: None,
        }
    }

    pub fn effect_spans(spans: impl IntoIterator<Item = crate::ui::desc_spans::DescSpan>) -> Self {
        let spans: Vec<_> = spans.into_iter().collect();
        Self {
            text: Self::flatten_spans(&spans),
            kind: HeirloomDescLineKind::Effect,
            spans: Some(spans),
        }
    }

    pub fn mana(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: HeirloomDescLineKind::Mana,
            spans: None,
        }
    }

    pub fn stat(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: HeirloomDescLineKind::Stat,
            spans: None,
        }
    }

    pub fn stat_spans(spans: impl IntoIterator<Item = crate::ui::desc_spans::DescSpan>) -> Self {
        let spans: Vec<_> = spans.into_iter().collect();
        Self {
            text: Self::flatten_spans(&spans),
            kind: HeirloomDescLineKind::Stat,
            spans: Some(spans),
        }
    }

    pub fn blank() -> Self {
        Self {
            text: String::new(),
            kind: HeirloomDescLineKind::Blank,
            spans: None,
        }
    }

    pub fn as_desc_line(&self) -> crate::ui::desc_spans::DescLine {
        use crate::ui::desc_spans::{DescLine, DescSpan};
        if let Some(spans) = &self.spans {
            DescLine::spans(spans.clone())
        } else {
            DescLine::plain(self.text.clone())
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TooltipInfoBoxKind {
    TriggerCount { count: u32 },
    Definition(TooltipDefinition),
}

#[derive(Clone, Debug)]
pub struct TooltipInfoBoxSpec {
    pub kind: TooltipInfoBoxKind,
}

#[derive(Component)]
pub struct TooltipInfoBox;

#[derive(Clone, Copy, Debug)]
pub struct TooltipInfoBoxAnchor {
    pub center: Vec3,
    pub half_width: f32,
    pub half_height: f32,
    pub game_width: f32,
}

fn info_box_text_lines(kind: &TooltipInfoBoxKind) -> [String; 2] {
    match kind {
        TooltipInfoBoxKind::TriggerCount { count } => {
            let line1 = format!(
                "\n\nTriggered {} times",
                ui_helpers::format_number(*count as i64)
            );
            [line1, String::new()]
        }
        TooltipInfoBoxKind::Definition(def) => {
            let [title, body] = def.lines();
            [title.to_string(), body.to_string()]
        }
    }
}

pub fn build_tooltip_info_boxes(heirloom: Heirloom, trigger_count: u32) -> Vec<TooltipInfoBoxSpec> {
    let mut specs = Vec::new();
    if trigger_count > 0 {
        specs.push(TooltipInfoBoxSpec {
            kind: TooltipInfoBoxKind::TriggerCount {
                count: trigger_count,
            },
        });
    }

    let mut seen = std::collections::HashSet::new();
    for def in heirloom.tooltip_definitions() {
        if seen.insert(*def) {
            specs.push(TooltipInfoBoxSpec {
                kind: TooltipInfoBoxKind::Definition(*def),
            });
        }
    }
    specs
}

/// Card-local X for the info-box column: prefer right of the card, flip left if it would
/// not fit (avoid edge-clamping on the right, which overlaps the card).
fn info_boxes_local_x(anchor: &TooltipInfoBoxAnchor) -> f32 {
    let box_half_w = TOOLTIP_INFO_BOX_SIZE.x * 0.5;
    let screen_half_w = anchor.game_width * 0.5;
    let screen_right = screen_half_w - INFO_BOX_SCREEN_EDGE_PAD;
    let screen_left = -screen_half_w + INFO_BOX_SCREEN_EDGE_PAD;

    let right_center_x =
        anchor.center.x + anchor.half_width + INFO_BOX_GAP_FROM_TOOLTIP + box_half_w;
    let left_center_x =
        anchor.center.x - anchor.half_width - INFO_BOX_GAP_FROM_TOOLTIP - box_half_w;

    let world_x = if right_center_x + box_half_w <= screen_right {
        right_center_x
    } else if left_center_x - box_half_w >= screen_left {
        left_center_x
    } else {
        left_center_x
    };

    world_x - anchor.center.x
}

pub fn spawn_tooltip_info_boxes(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    anchor: TooltipInfoBoxAnchor,
    specs: &[TooltipInfoBoxSpec],
) -> Option<Entity> {
    if specs.is_empty() {
        return None;
    }

    let box_h = TOOLTIP_INFO_BOX_SIZE.y;
    let local_x = info_boxes_local_x(&anchor);
    let start_local_y = anchor.half_height - box_h * 0.5 - INFO_BOX_TOP_INSET;

    let root = commands
        .spawn((
            (Transform::IDENTITY, Visibility::default()),
            RenderLayers::from_layers(&[3]),
            TooltipInfoBox,
            Name::new("Tooltip Info Boxes"),
        ))
        .id();

    for (i, spec) in specs.iter().enumerate() {
        let local_y = start_local_y - i as f32 * (box_h + INFO_BOX_STACK_GAP);
        let lines = info_box_text_lines(&spec.kind);

        let box_e = commands
            .spawn((
                Sprite {
                    image: graphics.get_ui_element_texture(UIElement::TooltipInfoBox),
                    custom_size: Some(TOOLTIP_INFO_BOX_SIZE),
                    ..default()
                },
                Transform::from_translation(Vec3::new(local_x, local_y, 10.)),
            ))
            .insert(RenderLayers::from_layers(&[3]))
            .insert(UiShadow::container())
            .insert(ChildOf(root))
            .id();

        if !lines[0].is_empty() {
            let is_trigger = lines[0].contains("Triggered");
            let y_bonus = if lines[1].is_empty() { -4. } else { 0. };
            commands
                .spawn(
                    gf::TOOLTIP_INFO_BOX
                        .text(
                            &asset_server,
                            lines[0].clone(),
                            if is_trigger { YELLOW_2 } else { WHITE },
                        )
                        .justify(Justify::Center)
                        .anchor(Anchor::CENTER)
                        .with_transform(Transform {
                            translation: Vec3::new(0., INFO_BOX_LINE1_Y + y_bonus, 2.),
                            scale: gf::TOOLTIP_INFO_BOX.transform_scale(),
                            ..default()
                        }),
                )
                .insert(RenderLayers::from_layers(&[3]))
                .insert(ChildOf(box_e));
        }
        if !lines[1].is_empty() {
            commands
                .spawn(
                    gf::TOOLTIP_INFO_BOX
                        .text(&asset_server, lines[1].clone(), WHITE)
                        .justify(Justify::Center)
                        .anchor(Anchor::CENTER)
                        .with_transform(Transform {
                            translation: Vec3::new(0., INFO_BOX_LINE2_Y, 2.),
                            scale: gf::TOOLTIP_INFO_BOX.transform_scale(),
                            ..default()
                        }),
                )
                .insert(RenderLayers::from_layers(&[3]))
                .insert(ChildOf(box_e));
        }
    }

    Some(root)
}

pub fn spawn_tooltip_info_boxes_with_resolution(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    resolution: &ScreenResolution,
    anchor: TooltipInfoBoxAnchor,
    specs: &[TooltipInfoBoxSpec],
) -> Option<Entity> {
    spawn_tooltip_info_boxes(
        commands,
        graphics,
        asset_server,
        TooltipInfoBoxAnchor {
            game_width: resolution.game_width,
            ..anchor
        },
        specs,
    )
}
