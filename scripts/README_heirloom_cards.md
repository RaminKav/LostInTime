# Heirloom card asset pipeline

Generates one PNG per heirloom card by composing:

1. **Card frame** (by rarity) from `assets/ui/`:
   - Common → `SkillChoice.png`
   - Uncommon → `SkillChoiceRogue.png`
   - Rare → `SkillChoiceMagic.png`
   - Legendary → `SkillChoiceMelee.png`

2. **Heirloom icon** from the main sprite sheet using `assets/textures/sprites.desc.ron` (heirlooms section). Icons are 16×16 at the defined `texture_pos` (in 16px grid units).

3. **Title and description** text using `assets/fonts/4x5.ttf` at size 5, at the same positions as in-game (`skill_choice_ui.rs` / `spawn_heirloom_tooltip_card`).

Card size is **164×191** (`SKILLS_CHOICE_UI_SIZE` in `src/ui/mod.rs`).

## Usage

From the **project root**:

1. **Export heirloom data from the game** (when you add/change heirlooms or text):

   ```bash
   EXPORT_HEIRLOOMS=1 cargo run
   ```

   This writes `assets/heirloom_cards_export.json` (id, title, description_lines, rarity for each heirloom in the choice pool) and exits.

2. **Build card images**:

   ```bash
   pip install Pillow   # if needed
   python3 scripts/build_heirloom_cards.py
   ```

   Outputs:
   - `assets/ui/heirloom_cards/<HeirloomId>.png` (e.g. `Defence.png`, `Health.png`)
   - `assets/ui/heirloom_cards/heirloom_cards_atlas.png` — all cards in one grid (**8 columns** by default), sorted by **rarity** (Common → Legendary) then **title** A–Z.

   Atlas-only (no per-card files): `python3 scripts/build_heirloom_cards.py --atlas-only`. Change column count: `--columns 8`.

## Requirements

- **Frame assets**: `assets/ui/SkillChoice.png`, `SkillChoiceRogue.png`, `SkillChoiceMagic.png`, `SkillChoiceMelee.png` (96×120 each).
- **Sprite sheet**: `assets/bevy_survival_sprites.png` (heirloom icons as in `sprites.desc.ron`).
- **Font**: `assets/fonts/4x5.ttf`.
- **Export JSON**: from step 1.

## In-engine alternative

To generate cards at runtime or in-editor instead of a script, you could add a system that:

1. For each `HeirloomChoiceState` in the pool, creates a 96×120 render target.
2. Draws the correct `UIElement` frame texture, then the heirloom atlas sprite at (0, 25) in card space, then text with the same font/size/positions.
3. Reads the render target to a buffer and saves as PNG (e.g. with `image` crate or Bevy’s image save APIs).

That would guarantee pixel-perfect match with the game UI but requires more engine code and running the game to export.
