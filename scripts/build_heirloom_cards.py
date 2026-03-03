#!/usr/bin/env python3
"""
Asset pipeline script: generate PNG images for each heirloom card.

Composes:
  1. Card frame by rarity (SkillChoice = Common, SkillChoiceRogue = Uncommon, etc.)
  2. Heirloom icon from the main sprite sheet (sprites.desc.ron heirlooms section)
  3. Title and description text (matching in-game layout and 4x5 font size 5.0)

Run from project root. Requires:
  - EXPORT_HEIRLOOMS=1 cargo run  (once) to generate assets/heirloom_cards_export.json
  - Python 3 with Pillow (pip install Pillow)

Output:
  - assets/ui/heirloom_cards/<HeirloomId>.png (96x120 full cards)
  - assets/ui/heirloom_cards/icons/<rarity>_<HeirloomId>.png (16x16 icon only)
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

try:
    from PIL import Image, ImageDraw, ImageFont
except ImportError:
    print("Install Pillow: pip install Pillow", file=sys.stderr)
    sys.exit(1)

# Layout (matches src/ui/skill_choice_ui.rs and SKILLS_CHOICE_UI_SIZE 96x120)
CARD_W = 96
CARD_H = 120
CENTER_X = CARD_W / 2
CENTER_Y = CARD_H / 2

# Bevy coords: icon at (0, 25), title at (0.5, 50.5), desc at (0.5, 0.5 - j*9)
# Y is up in Bevy, so pixel y = CENTER_Y - bevy_y
ICON_CENTER_BEVY_Y = 25
ICON_PIXEL_X = int(CENTER_X - 8)   # 16x16 icon, center at CENTER_X
ICON_PIXEL_Y = int(CENTER_Y - ICON_CENTER_BEVY_Y - 8)

TITLE_BEVY_Y = 50.5
TITLE_PIXEL_X = int(CENTER_X + 0.5)
TITLE_PIXEL_Y = int(CENTER_Y - TITLE_BEVY_Y)

DESC_FIRST_BEVY_Y = 0.5
DESC_LINE_HEIGHT = 9

FONT_SIZE = 5   # 4x5 font at size 5.0 (per .cursor/rules/font-size.mdc)

RARITY_TO_FRAME = {
    "Common": "SkillChoice",
    "Uncommon": "SkillChoiceRogue",
    "Rare": "SkillChoiceMagic",
    "Legendary": "SkillChoiceMelee",
}

# Sprite sheet: texture_pos in 16px grid units
TILE = 16


def parse_heirloom_icons_from_ron(ron_path: Path) -> dict[str, tuple[int, int]]:
    """Extract heirloom name -> (pixel_x, pixel_y) from sprites.desc.ron heirlooms section."""
    text = ron_path.read_text()
    # Find heirlooms: { ... }
    start = text.find("heirlooms: {")
    if start == -1:
        return {}
    start += len("heirlooms: {")
    depth = 1
    i = start
    while i < len(text) and depth > 0:
        if text[i] == "{":
            depth += 1
        elif text[i] == "}":
            depth -= 1
        i += 1
    block = text[start : i - 1]

    # Match: Name: WorldObjectData( texture_pos: (x., y.), ...
    pattern = re.compile(
        r"(\w+):\s*WorldObjectData\s*\(\s*texture_pos:\s*\(\s*([\d.]+)\s*,\s*([\d.]+)\s*\)"
    )
    out = {}
    for m in pattern.finditer(block):
        name, x, y = m.group(1), float(m.group(2)), float(m.group(3))
        out[name] = (int(x * TILE), int(y * TILE))
    return out


def load_export(assets: Path) -> list[dict]:
    export_path = assets / "heirloom_cards_export.json"
    if not export_path.exists():
        print(
            "Run once: EXPORT_HEIRLOOMS=1 cargo run (from project root)",
            file=sys.stderr,
        )
        print(f"Expected: {export_path}", file=sys.stderr)
        sys.exit(1)
    return json.loads(export_path.read_text())


def main() -> None:
    project_root = Path(__file__).resolve().parent.parent
    assets = project_root / "assets"
    sprites_ron = assets / "textures" / "sprites.desc.ron"
    ui_dir = assets / "ui"
    font_path = assets / "fonts" / "4x5.ttf"
    spritesheet_path = assets / "bevy_survival_sprites.png"

    if not sprites_ron.exists():
        print(f"Missing: {sprites_ron}", file=sys.stderr)
        sys.exit(1)
    if not spritesheet_path.exists():
        print(f"Missing: {spritesheet_path}", file=sys.stderr)
        sys.exit(1)
    if not font_path.exists():
        print(f"Missing font: {font_path}", file=sys.stderr)
        sys.exit(1)

    heirloom_icons = parse_heirloom_icons_from_ron(sprites_ron)
    export = load_export(assets)

    # Load frame images by rarity
    frames = {}
    for rarity, name in RARITY_TO_FRAME.items():
        path = ui_dir / f"{name}.png"
        if not path.exists():
            print(f"Warning: missing frame {path}", file=sys.stderr)
            continue
        img = Image.open(path).convert("RGBA")
        if img.size != (CARD_W, CARD_H):
            img = img.resize((CARD_W, CARD_H), Image.Resampling.LANCZOS)
        frames[rarity] = img

    if not frames:
        print("No card frame PNGs found (ui/SkillChoice.png, etc.)", file=sys.stderr)
        sys.exit(1)

    spritesheet = Image.open(spritesheet_path).convert("RGBA")
    try:
        font = ImageFont.truetype(str(font_path), FONT_SIZE)
    except OSError:
        font = ImageFont.load_default()

    out_dir = ui_dir / "heirloom_cards"
    icons_dir = out_dir / "icons"
    out_dir.mkdir(parents=True, exist_ok=True)
    icons_dir.mkdir(parents=True, exist_ok=True)

    for entry in export:
        heirloom_id = entry["id"]
        title = entry["title"]
        description_lines = entry["description_lines"]
        rarity = entry["rarity"]

        frame = frames.get(rarity)
        if not frame:
            print(f"Skip {heirloom_id}: no frame for rarity {rarity}", file=sys.stderr)
            continue

        card = frame.copy()

        # Blit heirloom icon from sprite sheet
        icon_pos = heirloom_icons.get(heirloom_id)
        if icon_pos:
            sx, sy = icon_pos
            icon = spritesheet.crop((sx, sy, sx + TILE, sy + TILE))
            card.paste(icon, (ICON_PIXEL_X, ICON_PIXEL_Y), icon)
            # Export icon-only file: {rarity}_{name}.png
            icon_path = icons_dir / f"{rarity}_{heirloom_id}.png"
            icon.copy().save(icon_path)
            print(f"Wrote {icon_path.relative_to(project_root)}")
        else:
            print(f"Warning: no icon for {heirloom_id} in sprites.desc.ron", file=sys.stderr)

        draw = ImageDraw.Draw(card)

        # Title: centered at (TITLE_PIXEL_X, TITLE_PIXEL_Y)
        # PIL doesn't have easy center; get bbox and draw centered
        try:
            bbox = draw.textbbox((0, 0), title, font=font)
            tw = bbox[2] - bbox[0]
            draw.text(
                (CENTER_X - tw / 2, TITLE_PIXEL_Y),
                title,
                fill=(255, 255, 255),
                font=font,
            )
        except TypeError:
            # older PIL: textbbox might not exist, use textsize
            tw = draw.textlength(title, font=font) if hasattr(draw, "textlength") else len(title) * 3
            draw.text(
                (CENTER_X - tw / 2, TITLE_PIXEL_Y),
                title,
                fill=(255, 255, 255),
                font=font,
            )

        # Description lines: centered, first at DESC_FIRST_BEVY_Y in Bevy = CENTER_Y - 0.5
        first_desc_y = int(CENTER_Y - DESC_FIRST_BEVY_Y)
        for j, line in enumerate(description_lines):
            y = first_desc_y + j * DESC_LINE_HEIGHT
            try:
                bbox = draw.textbbox((0, 0), line, font=font)
                tw = bbox[2] - bbox[0]
                draw.text(
                    (CENTER_X - tw / 2, y),
                    line,
                    fill=(255, 255, 255),
                    font=font,
                )
            except TypeError:
                tw = draw.textlength(line, font=font) if hasattr(draw, "textlength") else len(line) * 3
                draw.text(
                    (CENTER_X - tw / 2, y),
                    line,
                    fill=(255, 255, 255),
                    font=font,
                )

        out_path = out_dir / f"{heirloom_id}.png"
        card.save(out_path)
        print(f"Wrote {out_path.relative_to(project_root)}")

    print(f"Done. {len(export)} cards in {out_dir.relative_to(project_root)}, icons in {icons_dir.relative_to(project_root)}")


if __name__ == "__main__":
    main()
