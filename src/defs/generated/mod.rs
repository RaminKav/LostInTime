//! Auto-generated entity definitions from assets/proto/*.prototype.ron
//! Regenerate with: `python3 tools/gen_defs.py`

mod entities;

use super::registry::GameDefs;

pub fn register_all(defs: &mut GameDefs) {
    entities::register_entities(defs);
    entities::register_eras(defs);
}
