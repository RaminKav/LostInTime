#!/usr/bin/env python3
"""
Codegen: assets/proto/*.prototype.ron -> src/defs/generated/*.rs

Usage (from repo root):
    python3 tools/gen_defs.py

Merges template prototypes, then emits EntityDef / EraDef registration Rust.
Complex Deserialize-backed values are passed through parse::* helpers as RON
string literals; simple enums/newtypes are emitted as Rust literals.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
PROTO_DIR = ROOT / "assets" / "proto"
OUT_DIR = ROOT / "src" / "defs" / "generated"

# Short type key -> last path segment used in RON
TYPE_ALIASES = {
    "survival_rogue_like::item::WorldObject": "WorldObject",
    "survival_rogue_like::enemy::Mob": "Mob",
    "survival_rogue_like::item::projectile::Projectile": "Projectile",
    "survival_rogue_like::enemy::CombatAlignment": "CombatAlignment",
    "survival_rogue_like::enemy::FollowSpeed": "FollowSpeed",
    "survival_rogue_like::enemy::LeapAttack": "LeapAttack",
    "survival_rogue_like::proto::ColliderProto": "ColliderProto",
    "survival_rogue_like::proto::ColliderCapsulProto": "ColliderCapsulProto",
    "survival_rogue_like::proto::SensorProto": "SensorProto",
    "survival_rogue_like::proto::KCC": "KCC",
    "survival_rogue_like::proto::SpriteSheetProto": "SpriteSheetProto",
    "survival_rogue_like::proto::AnimationTimerProto": "AnimationTimerProto",
    "survival_rogue_like::proto::IdleStateProto": "IdleStateProto",
    "survival_rogue_like::proto::SaplingProto": "SaplingProto",
    "survival_rogue_like::assets::SpriteSize": "SpriteSize",
    "survival_rogue_like::assets::SpriteAnchor": "SpriteAnchor",
    "survival_rogue_like::world::y_sort::YSort": "YSort",
    "survival_rogue_like::attributes::MaxHealth": "MaxHealth",
    "survival_rogue_like::attributes::Attack": "Attack",
    "survival_rogue_like::player::levels::ExperienceReward": "ExperienceReward",
    "survival_rogue_like::item::loot_table::LootTable": "LootTable",
    "survival_rogue_like::inventory::ItemStack": "ItemStack",
    "survival_rogue_like::item::EquipmentType": "EquipmentType",
    "survival_rogue_like::item::RequiredEquipmentType": "RequiredEquipmentType",
    "survival_rogue_like::item::projectile::RangedAttack": "RangedAttack",
    "survival_rogue_like::item::melee::MeleeAttack": "MeleeAttack",
    "survival_rogue_like::item::projectile::ProjectileState": "ProjectileState",
    "survival_rogue_like::item::item_actions::ItemActions": "ItemActions",
    "survival_rogue_like::item::item_actions::ConsumableItem": "ConsumableItem",
    "survival_rogue_like::item::item_actions::ManaCost": "ManaCost",
    "survival_rogue_like::item::object_actions::ObjectAction": "ObjectAction",
    "survival_rogue_like::item::object_actions::ObjectActionCost": "ObjectActionCost",
    "survival_rogue_like::attributes::RawItemBaseAttributes": "RawItemBaseAttributes",
    "survival_rogue_like::attributes::RawItemBonusAttributes": "RawItemBonusAttributes",
    "survival_rogue_like::ui::scrapper_ui::ScrapsInto": "ScrapsInto",
    "survival_rogue_like::sapling::GrowsInto": "GrowsInto",
    "survival_rogue_like::item::Wall": "Wall",
    "survival_rogue_like::item::Foliage": "Foliage",
    "survival_rogue_like::item::FoliageSize": "FoliageSize",
    "survival_rogue_like::item::PlacesInto": "PlacesInto",
    "survival_rogue_like::item::BreaksWith": "BreaksWith",
    "survival_rogue_like::item::Block": "Block",
    "survival_rogue_like::animations::DoneAnimation": "DoneAnimation",
    "survival_rogue_like::animations::FadeOpacity": "FadeOpacity",
    "survival_rogue_like::animations::AnimationPosTracker": "AnimationPosTracker",
    "survival_rogue_like::animations::enemy_sprites::EnemyAnimationState": "EnemyAnimationState",
    "survival_rogue_like::animations::enemy_sprites::CharacterAnimationSpriteSheetData": "CharacterAnimationSpriteSheetData",
    "survival_rogue_like::combat::status_effects::StatusEffectTracker": "StatusEffectTracker",
    "survival_rogue_like::world::WorldGeneration": "WorldGeneration",
    "survival_rogue_like::enemy::MobLevel": "MobLevel",
    "survival_rogue_like::world::WallTextureData": "WallTextureData",
    "survival_rogue_like::item::object_actions::TouchTriggerObjectAction": "TouchTriggerObjectAction",
    "survival_rogue_like::enemy::ProjectileAttack": "ProjectileAttack",
    "survival_rogue_like::pets::state::Pet": "Pet",
    "survival_rogue_like::animations::enemy_sprites::LeftFacingSideProfile": "LeftFacingSideProfile",
    "survival_rogue_like::animations::AnimationFrameTracker": "AnimationFrameTracker",
    "survival_rogue_like::enemy::MultiLeapAttack": "MultiLeapAttack",
    "survival_rogue_like::enemy::BullChargeAttack": "BullChargeAttack",
    "survival_rogue_like::item::projectile::ArcProjectileData": "ArcProjectileData",
    "survival_rogue_like::enemy::scorpion::ScorpionClawAttack": "ScorpionClawAttack",
    "survival_rogue_like::enemy::scorpion::ScorpionTailAttack": "ScorpionTailAttack",
    "survival_rogue_like::enemy::scorpion::ScorpionTornadoAttack": "ScorpionTornadoAttack",
    "survival_rogue_like::enemy::CircleAttack": "CircleAttack",
    "survival_rogue_like::enemy::LaserAttack": "LaserAttack",
    # Ignored bevy_proto bundles (graphics swapped at spawn time)
    "bevy_proto::custom::VisibilityBundle": "_Skip",
    "bevy_proto::custom::SpriteSheetBundle": "_Skip",
    # Standalone PNG textures (trees, large cactuses, dirt path, etc.)
    "bevy_proto::custom::SpriteBundle": "SpriteBundle",
}


def strip_comments(text: str) -> str:
    out = []
    i = 0
    n = len(text)
    while i < n:
        if text[i] == "/" and i + 1 < n and text[i + 1] == "/":
            while i < n and text[i] != "\n":
                i += 1
            continue
        if text[i] == "/" and i + 1 < n and text[i + 1] == "*":
            i += 2
            while i + 1 < n and not (text[i] == "*" and text[i + 1] == "/"):
                i += 1
            i += 2
            continue
        out.append(text[i])
        i += 1
    return "".join(out)


def skip_ws(s: str, i: int) -> int:
    while i < len(s) and s[i].isspace():
        i += 1
    return i


def parse_string(s: str, i: int) -> tuple[str, int]:
    assert s[i] == '"'
    i += 1
    out = []
    while i < len(s) and s[i] != '"':
        if s[i] == "\\":
            out.append(s[i : i + 2])
            i += 2
            continue
        out.append(s[i])
        i += 1
    return "".join(out), i + 1


def parse_value(s: str, i: int) -> tuple[str, int]:
    """Return the raw RON substring for one value and the index after it."""
    i = skip_ws(s, i)
    if i >= len(s):
        raise ValueError("unexpected EOF")
    start = i
    if s[i] == '"':
        _, i = parse_string(s, i)
        return s[start:i], i
    if s[i] in "([{":
        pairs = {"(": ")", "[": "]", "{": "}"}
        stack = [s[i]]
        i += 1
        in_str = False
        while i < len(s):
            c = s[i]
            if in_str:
                if c == "\\" and i + 1 < len(s):
                    i += 2
                    continue
                if c == '"':
                    in_str = False
                i += 1
                continue
            if c == '"':
                in_str = True
                i += 1
                continue
            if c in pairs:
                stack.append(c)
            elif stack and c == pairs[stack[-1]]:
                stack.pop()
                if not stack:
                    i += 1
                    return s[start:i], i
            i += 1
        raise ValueError(f"unbalanced {s[start]}")
    # atom: ident / number / bool — stop before whitespace, comma, close, or '('
    while i < len(s) and s[i] not in ",)]}" and not s[i].isspace() and s[i] != "(":
        i += 1
    # `Crafting(Anvil)` / `Some((...))` — ident followed by paren group
    j = skip_ws(s, i)
    if j < len(s) and s[j] == "(" and re.match(r"^[A-Za-z_][A-Za-z0-9_]*$", s[start:i]):
        nested, j2 = parse_value(s, j)
        return s[start:j2], j2
    return s[start:i].strip(), i


def parse_map_entries(s: str, i: int) -> tuple[dict[str, str], int]:
    """Parse `{ "key": value, ... }` starting at `{`."""
    i = skip_ws(s, i)
    assert s[i] == "{"
    i += 1
    entries: dict[str, str] = {}
    while True:
        i = skip_ws(s, i)
        if i < len(s) and s[i] == "}":
            return entries, i + 1
        if i < len(s) and s[i] == ",":
            i += 1
            continue
        key, i = parse_string(s, i)
        i = skip_ws(s, i)
        assert s[i] == ":", f"expected : after key {key}"
        i += 1
        val, i = parse_value(s, i)
        entries[key] = val.strip()
        i = skip_ws(s, i)
        if i < len(s) and s[i] == ",":
            i += 1


def parse_prototype(text: str) -> dict[str, Any]:
    text = strip_comments(text)
    text = text.strip()
    if not text.startswith("("):
        raise ValueError("prototype must start with (")
    # Find name
    m_name = re.search(r"name:\s*\"([^\"]+)\"", text)
    if not m_name:
        raise ValueError("missing name")
    name = m_name.group(1)
    entity = True
    if re.search(r"entity:\s*false", text):
        entity = False
    templates: list[str] = []
    m_tpl = re.search(r"templates:\s*\[(.*?)\]", text, re.S)
    if m_tpl:
        templates = re.findall(r"\"([^\"]+)\"", m_tpl.group(1))
    # schematics map
    idx = text.find("schematics:")
    if idx < 0:
        raise ValueError(f"{name}: missing schematics")
    i = skip_ws(text, idx + len("schematics:"))
    schematics_raw, _ = parse_map_entries(text, i)
    schematics = {}
    for k, v in schematics_raw.items():
        short = TYPE_ALIASES.get(k)
        if short == "_Skip":
            continue
        if short is None:
            # keep unknown under full path for warning
            schematics[k] = v
        else:
            schematics[short] = v
    return {
        "name": name,
        "entity": entity,
        "templates": templates,
        "schematics": schematics,
        "file": None,
    }


def load_all() -> dict[str, dict[str, Any]]:
    protos: dict[str, dict[str, Any]] = {}
    for path in sorted(PROTO_DIR.glob("*.prototype.ron")):
        try:
            p = parse_prototype(path.read_text())
            p["file"] = path.name
            # Index by file name for template lookup AND by name
            protos[path.name] = p
            protos[p["name"]] = p
        except Exception as e:
            print(f"WARN: failed to parse {path.name}: {e}", file=sys.stderr)
    return protos


def resolve_templates(proto: dict[str, Any], all_protos: dict[str, Any]) -> dict[str, str]:
    merged: dict[str, str] = {}
    for tpl in proto["templates"]:
        # templates listed as "mob_basic.prototype.ron"
        base = all_protos.get(tpl) or all_protos.get(Path(tpl).name)
        if not base:
            print(f"WARN: {proto['name']}: missing template {tpl}", file=sys.stderr)
            continue
        merged.update(resolve_templates(base, all_protos))
    merged.update(proto["schematics"])
    return merged


def rust_string(s: str) -> str:
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'


def rust_raw_string(s: str) -> str:
    # Use raw string with # count high enough
    n = 1
    while ("#" * n + '"') in s:
        n += 1
    hashes = "#" * n
    return f'r{hashes}"{s}"{hashes}'


def emit_enum(ty: str, val: str) -> str:
    val = val.strip()
    if val.startswith("(") and val.endswith(")"):
        # (Pickaxe) or (SwordProjectile)
        inner = val[1:-1].strip()
        if re.match(r"^[A-Za-z_][A-Za-z0-9_]*$", inner):
            return f"{ty}::{inner}"
        return f"parse::{ty[0].lower() + ty[1:]}({rust_raw_string(val)})"  # unlikely
    if re.match(r"^[A-Za-z_][A-Za-z0-9_]*$", val):
        return f"{ty}::{val}"
    return f"parse::{camel_to_snake(ty)}({rust_raw_string(val)})"


def camel_to_snake(name: str) -> str:
    s1 = re.sub("(.)([A-Z][a-z]+)", r"\1_\2", name)
    return re.sub("([a-z0-9])([A-Z])", r"\1_\2", s1).lower()


def parse_paren_fields(val: str) -> dict[str, str]:
    val = val.strip()
    if not (val.startswith("(") and val.endswith(")")):
        return {}
    inner = val[1:-1].strip()
    if not inner:
        return {}
    fields: dict[str, str] = {}
    i = 0
    while i < len(inner):
        i = skip_ws(inner, i)
        if i >= len(inner):
            break
        m = re.match(r"([A-Za-z_][A-Za-z0-9_]*)\s*:", inner[i:])
        if not m:
            break
        key = m.group(1)
        i += m.end()
        v, i = parse_value(inner, i)
        fields[key] = v.strip()
        i = skip_ws(inner, i)
        if i < len(inner) and inner[i] == ",":
            i += 1
    return fields


def emit_newtype_f32(ty: str, val: str) -> str:
    fields = parse_paren_fields(val)
    if len(fields) == 1:
        v = next(iter(fields.values()))
        return f"{ty}({rust_f32(v)})"
    # (0.25) style — single anonymous
    m = re.match(r"^\((.+)\)$", val.strip())
    if m and ":" not in m.group(1):
        return f"{ty}({rust_f32(m.group(1))})"
    raise ValueError(f"bad newtype {ty}: {val}")


def emit_newtype_i32(ty: str, val: str) -> str:
    fields = parse_paren_fields(val)
    if len(fields) == 1:
        v = next(iter(fields.values()))
        return f"{ty}({v.strip()})"
    m = re.match(r"^\((.+)\)$", val.strip())
    if m and ":" not in m.group(1):
        return f"{ty}({m.group(1).strip()})"
    raise ValueError(f"bad newtype {ty}: {val}")


def emit_entity_field_assigns(schematics: dict[str, str], unknown: list[str]) -> list[str]:
    lines: list[str] = []
    for key, val in schematics.items():
        if key.startswith("survival_rogue_like::") or "::" in key:
            unknown.append(key)
            continue
        try:
            line = emit_one_field(key, val)
            if line:
                lines.append("    " + line)
        except Exception as e:
            unknown.append(f"{key} ({e})")
    return lines


def emit_one_field(key: str, val: str) -> str | None:
    v = val.strip()
    if key == "WorldObject":
        return f"d.world_object = Some({emit_enum('WorldObject', v)});"
    if key == "Mob":
        return f"d.mob = Some({emit_enum('Mob', v)});"
    if key == "Projectile":
        return f"d.projectile = Some({emit_enum('Projectile', v)});"
    if key == "CombatAlignment":
        return f"d.combat_alignment = Some({emit_enum('CombatAlignment', v)});"
    if key == "FollowSpeed":
        return f"d.follow_speed = Some({emit_newtype_f32('FollowSpeed', v)});"
    if key == "MaxHealth":
        return f"d.max_health = Some({emit_newtype_i32('MaxHealth', v)});"
    if key == "Attack":
        return f"d.attack = Some({emit_newtype_i32('Attack', v)});"
    if key == "ExperienceReward":
        # ExperienceReward(u32)
        return f"d.experience_reward = Some({emit_newtype_i32('ExperienceReward', v)});"
    if key == "YSort":
        return f"d.y_sort = Some({emit_newtype_f32('YSort', v)});"
    if key == "ManaCost":
        return f"d.mana_cost = Some({emit_newtype_i32('ManaCost', v)});"
    if key == "SpriteSize":
        return f"d.sprite_size = Some({emit_enum('SpriteSize', v)});"
    if key == "SpriteAnchor":
        return f"d.sprite_anchor = Some(SpriteAnchor(parse::vec2({rust_raw_string(v)})));"
    if key == "ColliderProto":
        f = parse_paren_fields(v)
        return (
            "d.collider = Some(ColliderDef { kind: ColliderKind::Cuboid { "
            f"x: {rust_f32(f.get('x', '0.'))}, y: {rust_f32(f.get('y', '0.'))} "
            "} });"
        )
    if key == "ColliderCapsulProto":
        f = parse_paren_fields(v)
        return (
            "d.collider = Some(ColliderDef { kind: ColliderKind::Capsule { "
            f"x1: {rust_f32(f.get('x1', '0.'))}, y1: {rust_f32(f.get('y1', '0.'))}, "
            f"x2: {rust_f32(f.get('x2', '0.'))}, y2: {rust_f32(f.get('y2', '0.'))}, "
            f"r: {rust_f32(f.get('r', '0.'))} "
            "} });"
        )
    if key == "SensorProto":
        return "d.sensor = true;"
    if key == "KCC":
        return "d.kcc = true;"
    if key == "SaplingProto":
        # (60.) or 60.
        inner = v[1:-1].strip() if v.startswith("(") and v.endswith(")") else v
        return f"d.sapling_secs = Some({rust_f32(inner)});"
    if key == "DoneAnimation":
        return "d.done_animation = true;"
    if key == "FadeOpacity":
        return "d.fade_opacity = Some(FadeOpacity);"
    if key == "ConsumableItem":
        return "d.consumable = Some(ConsumableItem);"
    if key == "MeleeAttack":
        return "d.melee = Some(MeleeAttack);"
    if key == "Block":
        return "d.block = Some(Block);"
    if key == "StatusEffectTracker":
        return "d.status_effect_tracker = true;"
    if key == "SpriteSheetProto":
        f = parse_paren_fields(v)
        asset = f.get("asset", '""').strip().strip('"')
        size = f.get("size", "(x: 16., y: 16.)")
        sf = parse_paren_fields(size)
        return (
            "d.sprite_sheet = Some(SpriteSheetDef { "
            f"asset: {rust_string(asset)}.into(), "
            f"size: Vec2::new({rust_f32(sf.get('x', '16.'))}, {rust_f32(sf.get('y', '16.'))}), "
            f"cols: {f.get('cols', '1')}, "
            f"rows: {f.get('rows', '1')} "
            "});"
        )
    if key == "SpriteBundle":
        # (texture: AssetPath("foo.png")) — standalone image, not the shared atlas
        f = parse_paren_fields(v)
        tex = f.get("texture", "").strip()
        m = re.search(r'AssetPath\s*\(\s*"([^"]+)"\s*\)', tex)
        if not m:
            m = re.search(r'"([^"]+)"', tex)
        if not m:
            return None
        return f"d.sprite_texture = Some({rust_string(m.group(1))}.into());"
    if key == "AnimationTimerProto":
        f = parse_paren_fields(v)
        return f"d.animation_timer = Some(AnimationTimerDef {{ secs: {rust_f32(f.get('secs', '0.1'))} }});"
    if key == "IdleStateProto":
        f = parse_paren_fields(v)
        return (
            "d.idle_state = Some(("
            f"{rust_f32(f.get('walk_dir_change_time', '2.'))}, "
            f"{rust_f32(f.get('speed', '0.2'))}"
            "));"
        )
    if key == "RangedAttack":
        # (SwordProjectile) or SwordProjectile
        inner = v
        if inner.startswith("(") and inner.endswith(")"):
            inner = inner[1:-1].strip()
        return f"d.ranged = Some(RangedAttack({emit_enum('Projectile', inner)}));"
    if key == "GrowsInto":
        inner = v
        if inner.startswith("(") and inner.endswith(")"):
            inner = inner[1:-1].strip()
        return f"d.grows_into = Some(GrowsInto({emit_enum('WorldObject', inner)}));"
    if key == "PlacesInto":
        inner = v[1:-1].strip() if v.startswith("(") else v
        return f"d.places_into = Some(PlacesInto({emit_enum('WorldObject', inner)}));"
    if key == "BreaksWith":
        inner = v[1:-1].strip() if v.startswith("(") else v
        return f"d.breaks_with = Some(BreaksWith({emit_enum('WorldObject', inner)}));"
    if key == "LootTable":
        return f"d.loot_table = Some(parse::loot_table({rust_raw_string(v)}));"
    if key == "ItemStack":
        return f"d.item_stack = Some(parse::item_stack({rust_raw_string(v)}));"
    if key == "ProjectileState":
        return f"d.projectile_state = Some(parse::projectile_state({rust_raw_string(v)}));"
    if key == "EquipmentType":
        return f"d.equipment_type = Some({emit_enum('EquipmentType', v)});"
    if key == "RequiredEquipmentType":
        inner = v[1:-1].strip() if v.startswith("(") else v
        return f"d.required_equipment_type = Some(RequiredEquipmentType({emit_enum('EquipmentType', inner)}));"
    if key == "Foliage":
        return f"d.foliage = Some(parse::foliage({rust_raw_string(v)}));"
    if key == "Wall":
        return f"d.wall = Some(parse::wall({rust_raw_string(v)}));"
    if key == "FoliageSize":
        return f"d.foliage_size = Some(FoliageSize(parse::vec2({rust_raw_string(v)})));"
    if key == "EnemyAnimationState":
        return f"d.enemy_anim_state = Some({emit_enum('EnemyAnimationState', v)});"
    if key == "LeapAttack":
        f = parse_paren_fields(v)
        return (
            "d.leap_attack = Some(LeapAttack { "
            f"activation_distance: {rust_f32(f.get('activation_distance', '0.'))}, "
            f"startup: {rust_f32(f.get('startup', '0.'))}, "
            f"duration: {rust_f32(f.get('duration', '0.'))}, "
            f"cooldown: {rust_f32(f.get('cooldown', '0.'))}, "
            f"speed: {rust_f32(f.get('speed', '0.'))} "
            "});"
        )
    if key == "AnimationPosTracker":
        # (0.0, 0.0, 0.3)
        m = re.match(r"^\(([^,]+),\s*([^,]+),\s*([^)]+)\)$", v)
        if m:
            return (
                "d.animation_pos_tracker = Some(AnimationPosTracker("
                f"{rust_f32(m.group(1))}, {rust_f32(m.group(2))}, {rust_f32(m.group(3))}"
                "));"
            )
        return None
    if key == "CharacterAnimationSpriteSheetData":
        f = parse_paren_fields(v)
        frames = f.get("animation_frames", "[]")
        # [4,4,4,9,6] -> vec![4,4,4,9,6]
        frames_rs = "vec!" + frames if frames.startswith("[") else f"vec![{frames}]"
        return (
            "d.anim_sprite_sheet_data = Some(CharacterAnimationSpriteSheetData { "
            f"animation_frames: {frames_rs}, "
            f"anim_offset: {f.get('anim_offset', '0')} "
            "});"
        )
    if key == "ItemActions":
        return f"d.item_actions = Some(parse::item_actions({rust_raw_string(v)}));"
    if key == "ObjectAction":
        return f"d.object_action = Some(parse::object_action({rust_raw_string(v)}));"
    if key == "ObjectActionCost":
        return f"d.object_action_cost = Some(parse::object_action_cost({rust_raw_string(v)}));"
    if key == "RawItemBaseAttributes":
        return f"d.raw_item_base = Some(parse::raw_item_base({rust_raw_string(v)}));"
    if key == "RawItemBonusAttributes":
        return f"d.raw_item_bonus = Some(parse::raw_item_bonus({rust_raw_string(v)}));"
    if key == "ScrapsInto":
        return f"d.scraps_into = Some(parse::scraps_into({rust_raw_string(v)}));"
    if key == "MobLevel":
        inner = v[1:-1].strip() if v.startswith("(") and v.endswith(")") else v.strip()
        return f"d.mob_level = Some({inner});"
    if key == "WallTextureData":
        return f"d.wall_texture_data = Some(parse::wall_texture_data({rust_raw_string(v)}));"
    if key == "TouchTriggerObjectAction":
        return f"d.touch_trigger = Some(parse::touch_trigger({rust_raw_string(v)}));"
    if key == "ProjectileAttack":
        return f"d.projectile_attack = Some(parse::projectile_attack({rust_raw_string(v)}));"
    if key == "Pet":
        return f"d.pet = Some({emit_enum('Pet', v)});"
    if key == "LeftFacingSideProfile":
        return "d.left_facing_side_profile = true;"
    if key == "AnimationFrameTracker":
        m = re.match(r"^\(([^,]+),\s*([^)]+)\)$", v)
        if m:
            return f"d.animation_frame_tracker = Some(AnimationFrameTracker({m.group(1).strip()}, {m.group(2).strip()}));"
        return None
    if key == "MultiLeapAttack":
        return f"d.multi_leap_attack = Some(parse::multi_leap_attack({rust_raw_string(v)}));"
    if key == "BullChargeAttack":
        return f"d.bull_charge_attack = Some(parse::bull_charge_attack({rust_raw_string(v)}));"
    if key == "ArcProjectileData":
        return f"d.arc_projectile_data = Some(parse::arc_projectile_data({rust_raw_string(v)}));"
    if key == "ScorpionClawAttack":
        return f"d.scorpion_claw_attack = Some(parse::scorpion_claw_attack({rust_raw_string(v)}));"
    if key == "ScorpionTailAttack":
        return f"d.scorpion_tail_attack = Some(parse::scorpion_tail_attack({rust_raw_string(v)}));"
    if key == "ScorpionTornadoAttack":
        return f"d.scorpion_tornado_attack = Some(parse::scorpion_tornado_attack({rust_raw_string(v)}));"
    if key == "CircleAttack":
        return f"d.circle_attack = Some(parse::circle_attack({rust_raw_string(v)}));"
    if key == "LaserAttack":
        return f"d.laser_attack = Some(parse::laser_attack({rust_raw_string(v)}));"
    return None


HEADER = """\
//! AUTO-GENERATED by tools/gen_defs.py — do not edit by hand.
//! Regenerate: `python3 tools/gen_defs.py`

#![allow(unused_imports)]

use bevy::prelude::Vec2;

use crate::{
    animations::{
        enemy_sprites::{CharacterAnimationSpriteSheetData, EnemyAnimationState},
        AnimationFrameTracker, AnimationPosTracker, FadeOpacity,
    },
    assets::{SpriteAnchor, SpriteSize},
    attributes::{Attack, MaxHealth},
    enemy::{CombatAlignment, FollowSpeed, LeapAttack, Mob},
    item::{
        item_actions::{ConsumableItem, ManaCost},
        melee::MeleeAttack,
        projectile::{Projectile, RangedAttack},
        Block, BreaksWith, EquipmentType, FoliageSize, PlacesInto, RequiredEquipmentType,
        WorldObject,
    },
    pets::state::Pet,
    player::levels::ExperienceReward,
    sapling::GrowsInto,
    world::y_sort::YSort,
};
use crate::defs::parse;
use crate::defs::registry::GameDefs;
use crate::defs::types::{
    AnimationTimerDef, ColliderDef, ColliderKind, EntityDef, EraDef, SpriteSheetDef,
};
"""


RUST_KEYWORDS = {
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else",
    "enum", "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop",
    "match", "mod", "move", "mut", "pub", "ref", "return", "self", "Self",
    "static", "struct", "super", "trait", "true", "type", "unsafe", "use",
    "where", "while",
}


def rust_fn_name(name: str) -> str:
    s = re.sub(r"[^A-Za-z0-9]+", "_", name)
    s = s.strip("_")
    if not s:
        s = "unnamed"
    if s[0].isdigit():
        s = "n_" + s
    s = camel_to_snake(s) if any(c.isupper() for c in s) else s.lower()
    if s in RUST_KEYWORDS:
        s = s + "_"
    return s


def rust_f32(v: str) -> str:
    """Ensure a RON numeric literal is a valid Rust f32 literal."""
    v = v.strip().rstrip(",")
    if re.match(r"^-?\d+$", v):
        return v + "."
    return v


def emit_entity_fn(name: str, templates: list[str], schematics: dict[str, str], unknown: list[str]) -> str:
    fn = rust_fn_name(name)
    assigns = emit_entity_field_assigns(schematics, unknown)
    tpl_lit = ", ".join(f"{rust_string(t)}.into()" for t in templates)
    body = "\n".join(assigns)
    return f"""
fn {fn}() -> EntityDef {{
    let mut d = EntityDef {{
        name: {rust_string(name)}.into(),
        templates: vec![{tpl_lit}],
        ..Default::default()
    }};
{body}
    d
}}
"""


def emit_era_fn(name: str, schematics: dict[str, str], unknown: list[str]) -> str:
    fn = rust_fn_name(name)
    wg = schematics.get("WorldGeneration")
    if not wg:
        unknown.append("WorldGeneration missing")
        return ""
    return f"""
fn {fn}() -> EraDef {{
    EraDef {{
        name: {rust_string(name)}.into(),
        world_generation: parse::world_generation({rust_raw_string(wg)}),
    }}
}}
"""


def main() -> int:
    all_protos = load_all()
    # Unique by file
    by_file: dict[str, dict] = {}
    for p in all_protos.values():
        if p.get("file"):
            by_file[p["file"]] = p

    entity_fns: list[str] = []
    register_calls: list[str] = []
    era_fns: list[str] = []
    era_register: list[str] = []
    unknown_keys: dict[str, int] = {}

    for fname, proto in sorted(by_file.items()):
        name = proto["name"]
        # Skip base templates that are not spawned as named entities
        if name in (
            "WorldObject",
            "ItemDrop",
            "Projectile",
            "MobBasic",
            "MobPassive",
            "AsepriteMobBasic",
        ):
            continue
        merged = resolve_templates(proto, all_protos)
        unknown: list[str] = []

        if not proto["entity"] or "WorldGeneration" in merged:
            fn_src = emit_era_fn(name, merged, unknown)
            if fn_src:
                era_fns.append(fn_src)
                era_register.append(f"    defs.insert_era({rust_fn_name(name)}());")
            for u in unknown:
                unknown_keys[u] = unknown_keys.get(u, 0) + 1
            continue

        fn_src = emit_entity_fn(name, proto["templates"], merged, unknown)
        entity_fns.append(fn_src)
        register_calls.append(f"    defs.insert_entity({rust_fn_name(name)}());")
        for u in unknown:
            unknown_keys[u] = unknown_keys.get(u, 0) + 1

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    entities_rs = HEADER + "\n".join(entity_fns) + "\n".join(era_fns)
    entities_rs += "\n\npub fn register_entities(defs: &mut GameDefs) {\n"
    entities_rs += "\n".join(register_calls) + "\n}\n"
    entities_rs += "\npub fn register_eras(defs: &mut GameDefs) {\n"
    entities_rs += "\n".join(era_register) + "\n}\n"
    (OUT_DIR / "entities.rs").write_text(entities_rs)

    mod_rs = """\
//! Auto-generated entity definitions from assets/proto/*.prototype.ron
//! Regenerate with: `python3 tools/gen_defs.py`

mod entities;

use super::registry::GameDefs;

pub fn register_all(defs: &mut GameDefs) {
    entities::register_entities(defs);
    entities::register_eras(defs);
}
"""
    (OUT_DIR / "mod.rs").write_text(mod_rs)

    print(f"Generated {len(register_calls)} entity defs -> {OUT_DIR / 'entities.rs'}")
    print(f"Generated {len(era_register)} era defs")
    if unknown_keys:
        print("Unhandled / unknown schematic keys (top 20):")
        for k, c in sorted(unknown_keys.items(), key=lambda x: -x[1])[:20]:
            print(f"  {c:4d}  {k}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
