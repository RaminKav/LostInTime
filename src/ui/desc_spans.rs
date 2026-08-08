//! Rich description lines for heirlooms, skills, and blessings.
//!
//! Plain lines stay one [`Text2d`]; keyword / number runs are [`TextSpan`] children
//! with their own [`TextColor`].

use bevy::color::Alpha;
use bevy::text::Justify;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::colors::{LIGHT_RED, SHIELD_BLUE, YELLOW};
use crate::player::skills::HeirloomRarity;

use super::game_fonts::FontStyle;
use super::tooltip_info_boxes::TooltipDefinition;

/// Soft lilac for numeric values in skill / blessing descriptions.
pub const SKILL_NUMBER_COLOR: Color = Color::srgba(0.72, 0.55, 0.98, 1.0);

/// Named reward / currency / rarity highlights in blessing card text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NamedSpanKind {
    /// Skill / specific item name — same blue as skill numbers.
    Reward,
    Gold,
    /// Rarity word, or an heirloom name tinted by its rarity.
    Rarity(HeirloomRarity),
}

/// One styled run inside a description line.
#[derive(Clone, Debug)]
pub enum DescSpan {
    Plain(String),
    /// Glossary mechanic keyword — display text is title-cased on spawn.
    Keyword {
        text: String,
        def: TooltipDefinition,
    },
    /// Numeric / percent token (skills).
    Number(String),
    /// Specific named reward (skill/heirloom) or "gold".
    Named {
        text: String,
        kind: NamedSpanKind,
    },
}

impl DescSpan {
    pub fn plain(text: impl Into<String>) -> Self {
        Self::Plain(text.into())
    }

    pub fn keyword(def: TooltipDefinition, text: impl Into<String>) -> Self {
        Self::Keyword {
            text: text.into(),
            def,
        }
    }

    pub fn number(text: impl Into<String>) -> Self {
        Self::Number(text.into())
    }

    pub fn named(kind: NamedSpanKind, text: impl Into<String>) -> Self {
        Self::Named {
            text: text.into(),
            kind,
        }
    }
}

/// A single visual description row (may contain multiple spans).
#[derive(Clone, Debug)]
pub struct DescLine {
    pub spans: Vec<DescSpan>,
}

impl DescLine {
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            spans: vec![DescSpan::plain(text)],
        }
    }

    pub fn spans(spans: impl IntoIterator<Item = DescSpan>) -> Self {
        Self {
            spans: spans.into_iter().collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
            || self
                .spans
                .iter()
                .all(|s| matches!(s, DescSpan::Plain(t) if t.is_empty()))
    }
}

/// Best-guess keyword palette — refine later.
pub fn keyword_color(def: TooltipDefinition) -> Color {
    match def {
        // Mechanics
        TooltipDefinition::Echo => Color::srgba(1.0, 0.55, 0.35, 1.0),
        TooltipDefinition::Summon => Color::srgba(0.75, 0.55, 1.0, 1.0),
        TooltipDefinition::Lightning => Color::srgba(0.95, 0.9, 0.35, 1.0),
        TooltipDefinition::IceExplosion => Color::srgba(0.55, 0.85, 1.0, 1.0),
        TooltipDefinition::Poison => Color::srgba(0.55, 0.95, 0.55, 1.0),
        // Same bright cyan as Mana Regen / mana-cost lines.
        TooltipDefinition::FreezeChance => SHIELD_BLUE.with_alpha(1.0),
        TooltipDefinition::Frail => Color::srgba(0.9, 0.3, 0.3, 1.0),
        TooltipDefinition::Thorns => Color::srgba(0.28, 0.65, 0.35, 1.0),
        // Same RGB as mana-cost lines (`SHIELD_BLUE`), full opacity for body text.
        TooltipDefinition::ManaRegen => SHIELD_BLUE.with_alpha(1.0),
        TooltipDefinition::Chaos => LIGHT_RED,
        TooltipDefinition::Skills => Color::srgba(0.7, 0.75, 1.0, 1.0),
        TooltipDefinition::Weapons => Color::srgba(0.95, 0.7, 0.45, 1.0),
        // Stats — shared muted gold (usually omitted from body keywords)
        TooltipDefinition::Attack
        | TooltipDefinition::AttackSpeed
        | TooltipDefinition::Defence
        | TooltipDefinition::Speed
        | TooltipDefinition::Health
        | TooltipDefinition::Mana
        | TooltipDefinition::Lifesteal
        | TooltipDefinition::CritChance
        | TooltipDefinition::CritDamage
        | TooltipDefinition::Dodge
        | TooltipDefinition::SkillPower
        | TooltipDefinition::PickupRange
        | TooltipDefinition::Size
        | TooltipDefinition::Luck => Color::srgba(0.9, 0.78, 0.45, 1.0),
    }
}

/// Whether this glossary term should be highlighted inside description body text.
/// Stat defs are usually already colored by line kind — skip them for now.
#[allow(dead_code)]
pub fn is_body_keyword(def: TooltipDefinition) -> bool {
    matches!(
        def,
        TooltipDefinition::Echo
            | TooltipDefinition::Summon
            | TooltipDefinition::Lightning
            | TooltipDefinition::IceExplosion
            | TooltipDefinition::Poison
            | TooltipDefinition::FreezeChance
            | TooltipDefinition::Frail
            | TooltipDefinition::Thorns
            | TooltipDefinition::ManaRegen
            | TooltipDefinition::Chaos
            | TooltipDefinition::Skills
            | TooltipDefinition::Weapons
    )
}

fn title_case_keyword(text: &str) -> String {
    let sep = if text.contains('-') { "-" } else { " " };
    text.split(|c: char| c == ' ' || c == '-')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first
                    .to_uppercase()
                    .chain(chars.flat_map(|c| c.to_lowercase()))
                    .collect(),
            }
        })
        .collect::<Vec<_>>()
        .join(sep)
}

fn named_span_color(kind: NamedSpanKind) -> Color {
    match kind {
        NamedSpanKind::Reward => SKILL_NUMBER_COLOR,
        NamedSpanKind::Gold => YELLOW,
        NamedSpanKind::Rarity(rarity) => rarity.get_color(),
    }
}

fn span_color(span: &DescSpan, default: Color) -> Color {
    match span {
        DescSpan::Plain(_) => default,
        DescSpan::Keyword { def, .. } => keyword_color(*def),
        DescSpan::Number(_) => SKILL_NUMBER_COLOR,
        DescSpan::Named { kind, .. } => named_span_color(*kind),
    }
}

fn span_display_text(span: &DescSpan) -> String {
    match span {
        DescSpan::Plain(t) | DescSpan::Number(t) => t.clone(),
        DescSpan::Keyword { text, .. } => title_case_keyword(text),
        DescSpan::Named {
            text,
            kind: NamedSpanKind::Rarity(_),
        } => title_case_keyword(text),
        DescSpan::Named { text, .. } => text.clone(),
    }
}

/// Spawn a description line. Single plain span → one `Text2d`; mixed spans → root + children.
pub fn spawn_desc_line(
    commands: &mut Commands,
    asset_server: &AssetServer,
    style: FontStyle,
    line: &DescLine,
    default_color: Color,
    translation: Vec3,
    anchor: Anchor,
    justify: Justify,
    render_layer: u8,
    parent: Entity,
) -> Entity {
    let spans = &line.spans;
    if spans.is_empty() {
        return Entity::PLACEHOLDER;
    }

    let first = &spans[0];
    let root_text = span_display_text(first);
    let root_color = span_color(first, default_color);

    let root = commands
        .spawn((
            style
                .text(asset_server, root_text, root_color)
                .at(translation)
                .anchor(anchor)
                .justify(justify),
            RenderLayers::from_layers(&[render_layer as usize]),
            Name::new("Desc Line"),
            ChildOf(parent),
        ))
        .id();

    if spans.len() == 1 {
        return root;
    }

    let font = style.text_font(asset_server);
    for span in &spans[1..] {
        commands.spawn((
            TextSpan::new(span_display_text(span)),
            font.clone(),
            TextColor(span_color(span, default_color)),
            ChildOf(root),
        ));
    }
    root
}

/// True if `chars[i]` begins a numeric token (`+5%`, `-1.5`, `.5`, `2.5s`, …).
fn starts_number_token(chars: &[char], i: usize) -> bool {
    if chars[i].is_ascii_digit() {
        return true;
    }
    if chars[i] == '.'
        && i + 1 < chars.len()
        && chars[i + 1].is_ascii_digit()
        && (i == 0 || !chars[i - 1].is_ascii_alphabetic())
    {
        return true;
    }
    if matches!(chars[i], '+' | '-') && i + 1 < chars.len() {
        if chars[i + 1].is_ascii_digit() {
            return true;
        }
        if chars[i + 1] == '.'
            && i + 2 < chars.len()
            && chars[i + 2].is_ascii_digit()
        {
            return true;
        }
    }
    false
}

/// Length of an ordinal suffix at `chars[i]` (`st`/`nd`/`rd`/`th`), if present
/// and not the start of a longer word (`5the`, `2ndary`).
fn ordinal_suffix_len(chars: &[char], i: usize) -> Option<usize> {
    if i + 1 >= chars.len() {
        return None;
    }
    let a = chars[i].to_ascii_lowercase();
    let b = chars[i + 1].to_ascii_lowercase();
    if !matches!((a, b), ('s', 't') | ('n', 'd') | ('r', 'd') | ('t', 'h')) {
        return None;
    }
    if i + 2 < chars.len() && chars[i + 2].is_ascii_alphabetic() {
        return None;
    }
    Some(2)
}

/// Split skill description text into plain + number spans.
///
/// Matches signed integers/decimals and common glued suffixes (`%`, `x`, `s`,
/// ordinals like `5th` / `1st`).
pub fn spans_highlighting_numbers(text: &str) -> Vec<DescSpan> {
    let mut spans = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut plain = String::new();

    while i < chars.len() {
        if starts_number_token(&chars, i) {
            if !plain.is_empty() {
                spans.push(DescSpan::plain(std::mem::take(&mut plain)));
            }
            let start = i;
            if matches!(chars[i], '+' | '-') {
                i += 1;
            }
            if i < chars.len() && chars[i] == '.' {
                i += 1;
            }
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            if i < chars.len() && chars[i] == '.' {
                let j = i + 1;
                if j < chars.len() && chars[j].is_ascii_digit() {
                    i = j;
                    while i < chars.len() && chars[i].is_ascii_digit() {
                        i += 1;
                    }
                }
            }
            // Trailing unit markers glued to the number (`5%`, `2x`, `2.5s`, `5th`).
            // Only absorb `s` when it is not the start of a longer word / ordinal.
            if i < chars.len() {
                match chars[i] {
                    '%' | 'x' => i += 1,
                    's' if i + 1 >= chars.len() || !chars[i + 1].is_ascii_alphabetic() => {
                        i += 1;
                    }
                    _ => {
                        if let Some(n) = ordinal_suffix_len(&chars, i) {
                            i += n;
                        }
                    }
                }
            }
            spans.push(DescSpan::number(chars[start..i].iter().collect::<String>()));
        } else {
            plain.push(chars[i]);
            i += 1;
        }
    }
    if !plain.is_empty() {
        spans.push(DescSpan::plain(plain));
    }
    if spans.is_empty() {
        spans.push(DescSpan::plain(text.to_string()));
    }
    spans
}

/// Body keywords highlighted after number tokenization (word-boundary, case-insensitive).
/// Longer phrases first so "Lightning Strike" wins over "Lightning".
/// Shared by skill + blessing description text (same palette as heirloom keywords).
const BODY_KEYWORDS: &[(&str, TooltipDefinition)] = &[
    ("Lightning Strikes", TooltipDefinition::Lightning),
    ("Lightning Strike", TooltipDefinition::Lightning),
    ("Ice Explosions", TooltipDefinition::IceExplosion),
    ("Ice Explosion", TooltipDefinition::IceExplosion),
    ("Mana Regen", TooltipDefinition::ManaRegen),
    ("Echoes", TooltipDefinition::Echo),
    ("Echo", TooltipDefinition::Echo),
    ("Summons", TooltipDefinition::Summon),
    ("Summon", TooltipDefinition::Summon),
    ("Poison", TooltipDefinition::Poison),
    ("Thorns", TooltipDefinition::Thorns),
    ("Lightning", TooltipDefinition::Lightning),
    ("Frozen", TooltipDefinition::FreezeChance),
    ("Freeze", TooltipDefinition::FreezeChance),
    ("Frail", TooltipDefinition::Frail),
    ("Chaos", TooltipDefinition::Chaos),
    ("Skills", TooltipDefinition::Skills),
    ("Weapons", TooltipDefinition::Weapons),
];

/// Highlight body keywords in plain text (no number tokenization).
pub fn spans_highlighting_body_keywords(text: &str) -> Vec<DescSpan> {
    expand_plain_with_body_keywords(text)
}

fn expand_plain_with_body_keywords(plain: &str) -> Vec<DescSpan> {
    let lower = plain.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut cursor = 0;
    let bytes = plain.as_bytes();

    while cursor < plain.len() {
        let mut best: Option<(usize, usize, TooltipDefinition)> = None;
        for &(needle, def) in BODY_KEYWORDS {
            let needle_lower = needle.to_ascii_lowercase();
            let mut search_from = cursor;
            while let Some(rel) = lower[search_from..].find(&needle_lower) {
                let start = search_from + rel;
                let end = start + needle_lower.len();
                let before_ok = start == 0
                    || !bytes
                        .get(start - 1)
                        .copied()
                        .is_some_and(|b| (b as char).is_ascii_alphabetic());
                let after_ok = end >= plain.len()
                    || !bytes
                        .get(end)
                        .copied()
                        .is_some_and(|b| (b as char).is_ascii_alphabetic());
                if before_ok && after_ok {
                    let take = match best {
                        None => true,
                        Some((b_start, b_end, _)) => {
                            start < b_start || (start == b_start && end - start > b_end - b_start)
                        }
                    };
                    if take {
                        best = Some((start, end, def));
                    }
                    break;
                }
                search_from = start + 1;
            }
        }

        let Some((start, end, def)) = best else {
            out.push(DescSpan::plain(plain[cursor..].to_string()));
            break;
        };
        if start > cursor {
            out.push(DescSpan::plain(plain[cursor..start].to_string()));
        }
        out.push(DescSpan::keyword(def, plain[start..end].to_string()));
        cursor = end;
    }
    out
}

/// Numbers first, then body keywords (Poison, Echo, Lightning Strike, Frail, …).
pub fn spans_highlighting_skill_text(text: &str) -> Vec<DescSpan> {
    let mut out = Vec::new();
    for span in spans_highlighting_numbers(text) {
        match span {
            DescSpan::Plain(plain) => out.extend(expand_plain_with_body_keywords(&plain)),
            other => out.push(other),
        }
    }
    out
}

pub fn skill_desc_line(text: impl AsRef<str>) -> DescLine {
    DescLine::spans(spans_highlighting_skill_text(text.as_ref()))
}

/// Word-boundary, case-insensitive phrase match; longer needles win.
fn expand_plain_with_named_phrases(
    plain: &str,
    phrases: &[(String, NamedSpanKind)],
) -> Vec<DescSpan> {
    if phrases.is_empty() {
        return vec![DescSpan::plain(plain)];
    }

    let lower = plain.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut cursor = 0;
    let bytes = plain.as_bytes();

    while cursor < plain.len() {
        let mut best: Option<(usize, usize, NamedSpanKind)> = None;
        for (needle, kind) in phrases {
            let needle_lower = needle.to_ascii_lowercase();
            if needle_lower.is_empty() {
                continue;
            }
            let mut search_from = cursor;
            while let Some(rel) = lower[search_from..].find(&needle_lower) {
                let start = search_from + rel;
                let end = start + needle_lower.len();
                let before_ok = start == 0
                    || !bytes
                        .get(start - 1)
                        .copied()
                        .is_some_and(|b| (b as char).is_ascii_alphabetic());
                let after_ok = end >= plain.len()
                    || !bytes
                        .get(end)
                        .copied()
                        .is_some_and(|b| (b as char).is_ascii_alphabetic());
                if before_ok && after_ok {
                    let take = match best {
                        None => true,
                        Some((b_start, b_end, _)) => {
                            start < b_start || (start == b_start && end - start > b_end - b_start)
                        }
                    };
                    if take {
                        best = Some((start, end, *kind));
                    }
                    break;
                }
                search_from = start + 1;
            }
        }

        let Some((start, end, kind)) = best else {
            out.push(DescSpan::plain(plain[cursor..].to_string()));
            break;
        };
        if start > cursor {
            out.push(DescSpan::plain(plain[cursor..start].to_string()));
        }
        out.push(DescSpan::named(kind, plain[start..end].to_string()));
        cursor = end;
    }
    out
}

/// Blessing card body: numbers, named rewards, rarity/"gold", then effect keywords.
pub fn spans_highlighting_blessing_text(
    text: &str,
    highlight_phrases: &[(String, NamedSpanKind)],
) -> Vec<DescSpan> {
    let mut phrases: Vec<(String, NamedSpanKind)> = highlight_phrases
        .iter()
        .filter(|(n, _)| !n.is_empty())
        .cloned()
        .collect();
    phrases.push(("gold".to_string(), NamedSpanKind::Gold));
    // Longer rarity names first so "uncommon" wins over "common".
    phrases.push((
        "legendary".to_string(),
        NamedSpanKind::Rarity(HeirloomRarity::Legendary),
    ));
    phrases.push((
        "uncommon".to_string(),
        NamedSpanKind::Rarity(HeirloomRarity::Uncommon),
    ));
    phrases.push((
        "common".to_string(),
        NamedSpanKind::Rarity(HeirloomRarity::Common),
    ));
    phrases.push((
        "rare".to_string(),
        NamedSpanKind::Rarity(HeirloomRarity::Rare),
    ));
    phrases.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

    let mut out = Vec::new();
    for span in spans_highlighting_numbers(text) {
        match span {
            DescSpan::Plain(plain) => {
                for named in expand_plain_with_named_phrases(&plain, &phrases) {
                    match named {
                        DescSpan::Plain(p) => out.extend(expand_plain_with_body_keywords(&p)),
                        other => out.push(other),
                    }
                }
            }
            other => out.push(other),
        }
    }
    out
}

pub fn blessing_desc_line(
    text: impl AsRef<str>,
    highlight_phrases: &[(String, NamedSpanKind)],
) -> DescLine {
    DescLine::spans(spans_highlighting_blessing_text(
        text.as_ref(),
        highlight_phrases,
    ))
}
