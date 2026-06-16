#!/usr/bin/env python3
"""
Asset pipeline: render active skill hover PNGs (background + icon + title + description).

Layout matches src/ui/player_hud.rs::spawn_skill_tooltip_content and
handle_active_skill_hud_tooltip (SkillTooltip background at 246×71).

From project root:
  EXPORT_SKILL_HOVERS=1 cargo run   # writes assets/skill_hovers_export.json
  pip install Pillow                # if needed
  python3 scripts/build_skill_hovers.py
  python3 scripts/build_skill_hovers.py --atlas-only
  python3 scripts/build_skill_hovers.py --columns 4 --no-individual

Outputs:
  - assets/ui/skill_hovers/<ActiveSkillId>.png
  - assets/ui/skill_hovers/skill_hovers_all.png — all hovers in one image (2 columns by default)
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path

try:
    from PIL import Image, ImageDraw, ImageFont
except ImportError:
    print("Install Pillow: pip install Pillow", file=sys.stderr)
    sys.exit(1)

# Matches player_hud.rs SkillTooltip custom_size
TOOLTIP_W = 246
TOOLTIP_H = 71

# spawn_skill_tooltip_content offsets (container origin = tooltip content root)
ICONS_X_OFFSET = -24.0
TEXT_Y_OFFSET = 12.0
DESC_TEXT_X = ICONS_X_OFFSET + 12.0
TITLE_Y = TEXT_Y_OFFSET + 6.0
DESC_LINE_STEP = 9.0
ICON_DISPLAY_SIZE = 18

# Background center in container space (handle_active_skill_hud_tooltip)
BG_CENTER_X = 72.0
BG_CENTER_Y = -3.0
BG_LEFT = BG_CENTER_X - TOOLTIP_W / 2.0
BG_TOP_BEVY = BG_CENTER_Y + TOOLTIP_H / 2.0

# game_fonts.rs + colors.rs
TITLE_FONT_SIZE = 8
BODY_FONT_SIZE = 8
DARK_WOOD_BROWN = (59, 47, 28)

DEFAULT_ATLAS_COLS = 2
ALL_HOVERS_FILENAME = "skill_hovers_all.png"
ATLAS_GAP = 4
ATLAS_BG = (32, 32, 36, 255)


def bevy_to_canvas(x: float, y: float) -> tuple[float, float]:
    """Map Bevy container coords (Y-up) to canvas pixels (Y-down, origin = bg top-left)."""
    return (x - BG_LEFT, BG_TOP_BEVY - y)


def load_export(assets: Path) -> list[dict]:
    export_path = assets / "skill_hovers_export.json"
    if not export_path.exists():
        print(
            "Run once: EXPORT_SKILL_HOVERS=1 cargo run (from project root)",
            file=sys.stderr,
        )
        print(f"Expected: {export_path}", file=sys.stderr)
        sys.exit(1)
    return json.loads(export_path.read_text())


def sort_export(entries: list[dict]) -> list[dict]:
    """Movement skills first, then title A–Z."""

    def key(e: dict) -> tuple[int, str]:
        movement = 0 if e.get("is_movement_skill") else 1
        title = (e.get("title") or e.get("id") or "").strip().lower()
        return (movement, title)

    return sorted(entries, key=key)


def load_icon(effects_dir: Path, skill_id: str) -> Image.Image | None:
    path = effects_dir / f"{skill_id}Icon.png"
    if not path.exists():
        print(f"Warning: missing icon {path}", file=sys.stderr)
        return None
    icon = Image.open(path).convert("RGBA")
    if icon.size != (ICON_DISPLAY_SIZE, ICON_DISPLAY_SIZE):
        icon = icon.resize(
            (ICON_DISPLAY_SIZE, ICON_DISPLAY_SIZE), Image.Resampling.NEAREST
        )
    return icon


def render_hover(
    entry: dict,
    background: Image.Image,
    effects_dir: Path,
    title_font: ImageFont.FreeTypeFont | ImageFont.ImageFont,
    body_font: ImageFont.FreeTypeFont | ImageFont.ImageFont,
) -> Image.Image | None:
    skill_id = entry["id"]
    title = entry["title"]
    description_lines = entry["description_lines"]

    hover = background.copy()
    draw = ImageDraw.Draw(hover)

    icon = load_icon(effects_dir, skill_id)
    if icon is not None:
        icx, icy = bevy_to_canvas(ICONS_X_OFFSET, 0.0)
        paste_x = int(icx - ICON_DISPLAY_SIZE / 2)
        paste_y = int(icy - ICON_DISPLAY_SIZE / 2)
        hover.paste(icon, (paste_x, paste_y), icon)

    title_x, title_y = bevy_to_canvas(DESC_TEXT_X, TITLE_Y)
    draw.text((title_x, title_y), title, font=title_font, fill=DARK_WOOD_BROWN)

    for j, line in enumerate(description_lines):
        line_y_bevy = (TEXT_Y_OFFSET - 2.0) - j * DESC_LINE_STEP
        line_x, line_y = bevy_to_canvas(DESC_TEXT_X, line_y_bevy)
        draw.text((line_x, line_y), line, font=body_font, fill=DARK_WOOD_BROWN)

    return hover


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
    cell_w = TOOLTIP_W + gap
    cell_h = TOOLTIP_H + gap
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
    parser = argparse.ArgumentParser(
        description="Build active skill hover PNGs and optional atlas grid."
    )
    parser.add_argument(
        "--columns",
        type=int,
        default=DEFAULT_ATLAS_COLS,
        help=f"Hovers per row in atlas (default {DEFAULT_ATLAS_COLS})",
    )
    parser.add_argument(
        "--no-all",
        action="store_true",
        help=f"Do not write {ALL_HOVERS_FILENAME}",
    )
    parser.add_argument(
        "--no-atlas",
        action="store_true",
        help=f"Same as --no-all (do not write {ALL_HOVERS_FILENAME})",
    )
    parser.add_argument(
        "--atlas-only",
        action="store_true",
        help="Only write the atlas (skip per-skill files)",
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
    ui_dir = assets / "ui"
    effects_dir = assets / "effects"
    title_font_path = assets / "fonts" / "slkscrbold.ttf"
    body_font_path = assets / "fonts" / "slkscr.ttf"
    bg_path = ui_dir / "SkillTooltip.png"

    if not bg_path.exists():
        print(f"Missing: {bg_path}", file=sys.stderr)
        sys.exit(1)
    if not title_font_path.exists() or not body_font_path.exists():
        print("Missing slkscr fonts in assets/fonts/", file=sys.stderr)
        sys.exit(1)

    export = sort_export(load_export(assets))

    background = Image.open(bg_path).convert("RGBA")
    if background.size != (TOOLTIP_W, TOOLTIP_H):
        background = background.resize(
            (TOOLTIP_W, TOOLTIP_H), Image.Resampling.NEAREST
        )

    try:
        title_font = ImageFont.truetype(str(title_font_path), TITLE_FONT_SIZE)
        body_font = ImageFont.truetype(str(body_font_path), BODY_FONT_SIZE)
    except OSError:
        title_font = body_font = ImageFont.load_default()

    out_dir = ui_dir / "skill_hovers"
    if not atlas_only:
        out_dir.mkdir(parents=True, exist_ok=True)

    rendered: list[Image.Image] = []

    for entry in export:
        skill_id = entry["id"]
        hover = render_hover(entry, background, effects_dir, title_font, body_font)
        if hover is None:
            continue
        rendered.append(hover)

        if not atlas_only:
            out_path = out_dir / f"{skill_id}.png"
            hover.save(out_path)
            print(f"Wrote {out_path.relative_to(project_root)}")

    if not args.no_all and not args.no_atlas and rendered:
        cols = max(1, args.columns)
        sheet = build_atlas(rendered, cols=cols, gap=ATLAS_GAP, bg=ATLAS_BG)
        sheet_path = out_dir / ALL_HOVERS_FILENAME
        sheet_path.parent.mkdir(parents=True, exist_ok=True)
        sheet.save(sheet_path)
        rows = math.ceil(len(rendered) / cols)
        print(
            f"Wrote {sheet_path.relative_to(project_root)} "
            f"({len(rendered)} hovers, {cols} columns × {rows} rows)"
        )

    if not atlas_only:
        print(f"Done. {len(rendered)} hovers in {out_dir.relative_to(project_root)}")


if __name__ == "__main__":
    main()
