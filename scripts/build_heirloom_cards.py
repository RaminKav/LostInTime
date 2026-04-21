#!/usr/bin/env python3
"""
Asset pipeline: render heirloom cards (frame + icon + text) and optionally a single atlas PNG.

Layout matches src/ui/skill_choice_ui.rs::spawn_heirloom_tooltip_card and SKILLS_CHOICE_UI_SIZE (164×191).

From project root:
  EXPORT_HEIRLOOMS=1 cargo run   # writes assets/heirloom_cards_export.json
  pip install Pillow             # if needed
  python3 scripts/build_heirloom_cards.py
  python3 scripts/build_heirloom_cards.py --atlas-only
  python3 scripts/build_heirloom_cards.py --columns 8 --no-individual

Outputs:
  - assets/ui/heirloom_cards/<HeirloomId>.png
  - assets/ui/heirloom_cards/icons/<Rarity>_<HeirloomId>.png
  - assets/ui/heirloom_cards/heirloom_cards_atlas.png  (unless --no-atlas)
"""

from __future__ import annotations

import argparse
import json
import math
import re
import sys
from pathlib import Path

try:
    from PIL import Image, ImageDraw, ImageFont
except ImportError:
    print("Install Pillow: pip install Pillow", file=sys.stderr)
    sys.exit(1)

# Matches src/ui/mod.rs SKILLS_CHOICE_UI_SIZE and skill_choice_ui icon/title/desc offsets
CARD_W = 164
CARD_H = 191
CENTER_X = CARD_W / 2
CENTER_Y = CARD_H / 2

# Icon: Vec2::new(2., 52.) from card center; 16×16 sprite, anchor center
ICON_CENTER_OFF_X = 2.0
ICON_CENTER_OFF_Y = 52.0
TILE = 16

# Title at (0, 20); desc line j at (0, -(j*9) - 4) — Bevy +Y up
FONT_SIZE = 5

RARITY_TO_FRAME = {
    "Common": "SkillChoice",
    "Uncommon": "SkillChoiceRogue",
    "Rare": "SkillChoiceMagic",
    "Legendary": "SkillChoiceMelee",
}

RARITY_ORDER = ["Common", "Uncommon", "Rare", "Legendary"]

DEFAULT_ATLAS_COLS = 8
ATLAS_GAP = 2
ATLAS_BG = (32, 32, 36, 255)


def parse_heirloom_icons_from_ron(ron_path: Path) -> dict[str, tuple[int, int]]:
    """Extract heirloom name -> (pixel_x, pixel_y) top-left from sprites.desc.ron heirlooms section."""
    text = ron_path.read_text()
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


def sort_export(entries: list[dict]) -> list[dict]:
    """By rarity (Common → Legendary), then title A–Z (case-insensitive)."""

    def key(e: dict) -> tuple[int, str]:
        r = e.get("rarity", "Common")
        try:
            ri = RARITY_ORDER.index(r)
        except ValueError:
            ri = len(RARITY_ORDER)
        title = (e.get("title") or e.get("id") or "").strip().lower()
        return (ri, title)

    return sorted(entries, key=key)


def _draw_centered_line(
    draw: ImageDraw.ImageDraw,
    cx: float,
    cy: float,
    text: str,
    font: ImageFont.FreeTypeFont | ImageFont.ImageFont,
    fill: tuple[int, int, int],
) -> None:
    """Draw text with anchor at center (cx, cy), PIL 8+ anchor='mm'."""
    try:
        draw.text((cx, cy), text, font=font, fill=fill, anchor="mm")
    except TypeError:
        bbox = draw.textbbox((0, 0), text, font=font)
        w = bbox[2] - bbox[0]
        h = bbox[3] - bbox[1]
        draw.text((cx - w / 2, cy - h / 2), text, font=font, fill=fill)


def render_card(
    entry: dict,
    frames: dict[str, Image.Image],
    spritesheet: Image.Image,
    heirloom_icons: dict[str, tuple[int, int]],
    font: ImageFont.FreeTypeFont | ImageFont.ImageFont,
) -> Image.Image | None:
    heirloom_id = entry["id"]
    title = entry["title"]
    description_lines = entry["description_lines"]
    rarity = entry["rarity"]

    frame = frames.get(rarity)
    if not frame:
        print(f"Skip {heirloom_id}: no frame for rarity {rarity}", file=sys.stderr)
        return None

    card = frame.copy()

    icon_pos = heirloom_icons.get(heirloom_id)
    if icon_pos:
        sx, sy = icon_pos
        icon = spritesheet.crop((sx, sy, sx + TILE, sy + TILE))
        icx = CENTER_X + ICON_CENTER_OFF_X
        icy = CENTER_Y - ICON_CENTER_OFF_Y
        paste_x = int(icx - TILE / 2)
        paste_y = int(icy - TILE / 2)
        card.paste(icon, (paste_x, paste_y), icon)
    else:
        print(f"Warning: no icon for {heirloom_id} in sprites.desc.ron", file=sys.stderr)

    draw = ImageDraw.Draw(card)

    _draw_centered_line(draw, CENTER_X, CENTER_Y - 20, title, font, (255, 255, 255))

    for j, line in enumerate(description_lines):
        bevy_y = -(j * 9) - 4
        cy = CENTER_Y - bevy_y
        _draw_centered_line(draw, CENTER_X, cy, line, font, (255, 255, 255))

    return card


def build_atlas(
    cards: list[Image.Image],
    cols: int,
    gap: int,
    bg: tuple[int, int, int, int],
) -> Image.Image:
    n = len(cards)
    if n == 0:
        raise ValueError("no cards")
    rows = math.ceil(n / cols)
    cell_w = CARD_W + gap
    cell_h = CARD_H + gap
    out_w = cols * cell_w - gap
    out_h = rows * cell_h - gap
    atlas = Image.new("RGBA", (out_w, out_h), bg)
    for i, card in enumerate(cards):
        r, c = divmod(i, cols)
        x = c * cell_w
        y = r * cell_h
        atlas.paste(card, (x, y), card)
    return atlas


def main() -> None:
    parser = argparse.ArgumentParser(description="Build heirloom card PNGs and optional atlas grid.")
    parser.add_argument(
        "--columns",
        type=int,
        default=DEFAULT_ATLAS_COLS,
        help=f"Cards per row in atlas (default {DEFAULT_ATLAS_COLS})",
    )
    parser.add_argument(
        "--no-atlas",
        action="store_true",
        help="Do not write heirloom_cards_atlas.png",
    )
    parser.add_argument(
        "--atlas-only",
        action="store_true",
        help="Only write the atlas (skip per-card and icon files)",
    )
    parser.add_argument(
        "--no-individual",
        action="store_true",
        help="Same as --atlas-only",
    )
    args = parser.parse_args()
    atlas_only = args.atlas_only or args.no_individual

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
    export = sort_export(load_export(assets))

    frames: dict[str, Image.Image] = {}
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
    if not atlas_only:
        out_dir.mkdir(parents=True, exist_ok=True)
        icons_dir.mkdir(parents=True, exist_ok=True)

    rendered: list[Image.Image] = []

    for entry in export:
        heirloom_id = entry["id"]
        rarity = entry["rarity"]
        card = render_card(entry, frames, spritesheet, heirloom_icons, font)
        if card is None:
            continue
        rendered.append(card)

        if not atlas_only:
            out_path = out_dir / f"{heirloom_id}.png"
            card.save(out_path)
            print(f"Wrote {out_path.relative_to(project_root)}")

            icon_pos = heirloom_icons.get(heirloom_id)
            if icon_pos:
                sx, sy = icon_pos
                icon = spritesheet.crop((sx, sy, sx + TILE, sy + TILE))
                icon_path = icons_dir / f"{rarity}_{heirloom_id}.png"
                icon.save(icon_path)
                print(f"Wrote {icon_path.relative_to(project_root)}")

    if not args.no_atlas and rendered:
        cols = max(1, args.columns)
        atlas = build_atlas(rendered, cols=cols, gap=ATLAS_GAP, bg=ATLAS_BG)
        atlas_path = out_dir / "heirloom_cards_atlas.png"
        atlas_path.parent.mkdir(parents=True, exist_ok=True)
        atlas.save(atlas_path)
        print(f"Wrote {atlas_path.relative_to(project_root)} ({len(rendered)} cards, {cols} per row)")

    if not atlas_only:
        print(
            f"Done. {len(rendered)} cards in {out_dir.relative_to(project_root)}, "
            f"icons in {icons_dir.relative_to(project_root)}"
        )


if __name__ == "__main__":
    main()
