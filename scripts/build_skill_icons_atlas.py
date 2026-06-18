#!/usr/bin/env python3
"""
Build a PNG atlas of icons used for skills on the class selection page and in play.

Sections (in order):
  1. Movement active skills (ActiveSkill::is_movement_skill)
  2. Active skill pool (shrine / dev picker filter)
  3. Class passive icons (ClassData.skill_icon from class_pet_data.class.ron)
  4. Pet skill icons (PetData.skill_icon from class_pet_data.class.ron)

From project root:
  python3 scripts/build_skill_icons_atlas.py
  python3 scripts/build_skill_icons_atlas.py --columns 6 --padding 2
"""

from __future__ import annotations

import argparse
import json
import math
import re
import sys
from pathlib import Path

try:
    from PIL import Image
except ImportError:
    print("Install Pillow: pip install Pillow", file=sys.stderr)
    sys.exit(1)

# Mirrors get_disabled_skills() in src/player/skills.rs
DISABLED = {
    "Parry",
    "Stealth",
    "DaggerThrow",
    "TripleThrow",
    "PossessedBlade",
    "Sprint",
    "Lightning",
}

# Mirrors ActiveSkill::is_movement_skill()
MOVEMENT = {"SpinAttack", "Teleport", "Roll", "SprintLunge", "Buckshot"}

DEFAULT_COLS = 6
DEFAULT_PADDING = 2
OUTPUT_NAME = "skill_icons_atlas.png"
ATLAS_BG = (0, 0, 0, 0)
CLASS_PET_RON = "class_pet_data.class.ron"


def load_export(assets: Path) -> list[dict]:
    export_path = assets / "skill_hovers_export.json"
    if not export_path.exists():
        print(
            "Run once: EXPORT_SKILL_HOVERS=1 cargo run (from project root)",
            file=sys.stderr,
        )
        sys.exit(1)
    return json.loads(export_path.read_text())


def included_active_skills(export: list[dict]) -> list[str]:
    """Movement skills + active skill pool, movement first then title A–Z."""
    movement: list[tuple[str, str]] = []
    active_pool: list[tuple[str, str]] = []

    for entry in export:
        skill_id = entry["id"]
        if skill_id in DISABLED or skill_id == "LaserBeam":
            continue
        title = (entry.get("title") or skill_id).strip().lower()
        if skill_id in MOVEMENT:
            movement.append((title, skill_id))
        else:
            active_pool.append((title, skill_id))

    movement.sort(key=lambda t: t[0])
    active_pool.sort(key=lambda t: t[0])
    return [sid for _, sid in movement] + [sid for _, sid in active_pool]


def parse_class_pet_icons(ron_path: Path) -> tuple[list[str], list[str]]:
    """Return (class passive icons, pet skill icons) from class_pet_data.class.ron."""
    text = ron_path.read_text()
    parts = text.split("pets:", 1)
    classes_part = parts[0]
    pets_part = parts[1] if len(parts) > 1 else ""

    def unique_icons(section: str) -> list[str]:
        return list(dict.fromkeys(re.findall(r"skill_icon:\s*(\w+)", section)))

    class_passives = unique_icons(classes_part)
    pet_skills = unique_icons(pets_part)
    return class_passives, pet_skills


def load_effect_icon(effects_dir: Path, skill_id: str) -> Image.Image:
    path = effects_dir / f"{skill_id}Icon.png"
    if not path.exists():
        raise FileNotFoundError(f"missing icon: {path}")
    return Image.open(path).convert("RGBA")


def load_ui_icon(ui_dir: Path, icon_name: str) -> Image.Image:
    path = ui_dir / f"{icon_name}.png"
    if not path.exists():
        raise FileNotFoundError(f"missing icon: {path}")
    return Image.open(path).convert("RGBA")


def build_atlas(
    icons: list[tuple[str, Image.Image]],
    cols: int,
    padding: int,
    bg: tuple[int, int, int, int],
) -> Image.Image:
    if not icons:
        raise ValueError("no icons")

    icon_w, icon_h = icons[0][1].size
    for label, icon in icons[1:]:
        if icon.size != (icon_w, icon_h):
            raise ValueError(
                f"{label} icon size {icon.size} != {(icon_w, icon_h)}"
            )

    cell_w = icon_w + padding * 2
    cell_h = icon_h + padding * 2
    rows = math.ceil(len(icons) / cols)
    out_w = cols * cell_w
    out_h = rows * cell_h

    atlas = Image.new("RGBA", (out_w, out_h), bg)
    for i, (_, icon) in enumerate(icons):
        r, c = divmod(i, cols)
        x = c * cell_w + padding
        y = r * cell_h + padding
        atlas.paste(icon, (x, y), icon)
    return atlas


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Build PNG atlas of class-selection skill / passive / pet icons."
    )
    parser.add_argument(
        "--columns",
        type=int,
        default=DEFAULT_COLS,
        help=f"Icons per row (default {DEFAULT_COLS})",
    )
    parser.add_argument(
        "--padding",
        type=int,
        default=DEFAULT_PADDING,
        help=f"Padding in px around each icon (default {DEFAULT_PADDING})",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=None,
        help=f"Output path (default assets/ui/skill_hovers/{OUTPUT_NAME})",
    )
    args = parser.parse_args()

    project_root = Path(__file__).resolve().parent.parent
    assets = project_root / "assets"
    effects_dir = assets / "effects"
    ui_dir = assets / "ui"
    ron_path = assets / CLASS_PET_RON
    out_path = args.output or (assets / "ui" / "skill_hovers" / OUTPUT_NAME)

    if not ron_path.exists():
        print(f"Missing: {ron_path}", file=sys.stderr)
        sys.exit(1)

    active_skill_ids = included_active_skills(load_export(assets))
    class_passives, pet_skills = parse_class_pet_icons(ron_path)

    icons: list[tuple[str, Image.Image]] = []
    for skill_id in active_skill_ids:
        icons.append((skill_id, load_effect_icon(effects_dir, skill_id)))
    for icon_name in class_passives:
        icons.append((f"class:{icon_name}", load_ui_icon(ui_dir, icon_name)))
    for icon_name in pet_skills:
        icons.append((f"pet:{icon_name}", load_ui_icon(ui_dir, icon_name)))

    cols = max(1, args.columns)
    atlas = build_atlas(icons, cols=cols, padding=args.padding, bg=ATLAS_BG)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    atlas.save(out_path)

    rows = math.ceil(len(icons) / cols)
    movement_count = sum(1 for sid in active_skill_ids if sid in MOVEMENT)
    pool_count = len(active_skill_ids) - movement_count
    print(f"Wrote {out_path.relative_to(project_root)}")
    print(
        f"  {len(icons)} icons "
        f"({movement_count} movement + {pool_count} active pool + "
        f"{len(class_passives)} class passive + {len(pet_skills)} pet skill), "
        f"{cols} columns × {rows} rows, {args.padding}px padding"
    )
    print("  Active skills:", ", ".join(active_skill_ids))
    print("  Class passives:", ", ".join(class_passives))
    print("  Pet skills:", ", ".join(pet_skills))


if __name__ == "__main__":
    main()
