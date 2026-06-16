# Skill hover asset pipeline

Generates one PNG per active skill hover by composing:

1. **Background** — `assets/ui/SkillTooltip.png` at **246×71** (matches HUD hover in `player_hud.rs`).

2. **Skill icon** — `assets/effects/<ActiveSkill>Icon.png`, drawn at **18×18** at the same offset as `spawn_skill_tooltip_content`.

3. **Title and description** — `slkscrbold.ttf` / `slkscr.ttf` at size 8, color `#3B2F1C` (`DARK_WOOD_BROWN`), same positions as in-game.

Disabled skills (`get_disabled_skills()` in `skills.rs`) are excluded from the export.

## Usage

From the **project root**:

1. **Export skill data from the game** (when you add/change skills or tooltip text):

   ```bash
   EXPORT_SKILL_HOVERS=1 cargo run
   ```

   This writes `assets/skill_hovers_export.json` (id, title, description_lines, is_movement_skill) and exits.

2. **Build hover images**:

   ```bash
   pip install Pillow   # if needed
   python3 scripts/build_skill_hovers.py
   ```

   Outputs:
   - `assets/ui/skill_hovers/<ActiveSkillId>.png` (e.g. `SpinAttack.png`, `Teleport.png`)
   - `assets/ui/skill_hovers/skill_hovers_all.png` — **all skills in one image** (2 columns × 10 rows by default), movement skills first then title A–Z.

   Combined sheet only: `python3 scripts/build_skill_hovers.py --atlas-only`. Change column count: `--columns 4`.

## Requirements

- **Background**: `assets/ui/SkillTooltip.png`
- **Icons**: `assets/effects/*Icon.png` (one per `ActiveSkill`)
- **Fonts**: `assets/fonts/slkscr.ttf`, `assets/fonts/slkscrbold.ttf`
- **Export JSON**: from step 1
