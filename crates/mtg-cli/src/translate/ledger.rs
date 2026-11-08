//! The ledger of a face: one row for each script line with the abilities it gave or the construct that stopped it, so a later proof can tell which printed clause a gap belongs to.

use mtg_cards::proposal::partial::LedgerLine;

use super::card::FaceOut;
use crate::forge::script::Params;

/// The state of a face before a line is translated.
pub(super) struct Before {
    abilities: usize,
    gaps: usize,
}

impl Before {
    /// Notes how many abilities and gaps the face holds.
    pub(super) fn of(out: &FaceOut) -> Before {
        Before {
            abilities: out.abilities.len(),
            gaps: out.gaps.len(),
        }
    }
}

/// The line's own words: its description, or for a keyword line the body itself.
fn text_of(tag: &str, body: &str) -> String {
    if tag == "K" {
        return body.to_owned();
    }
    let params = Params::parse(body);
    ["SpellDescription", "TriggerDescription", "Description"]
        .iter()
        .find_map(|key| params.get(key))
        .unwrap_or_default()
        .to_owned()
}

/// Records the lines `bodies` (one printed ability) as they translated since `before`; a line that gave nothing and
/// stopped at nothing leaves no row.
pub(super) fn record(out: &mut FaceOut, tag: &str, bodies: &[&String], before: &Before) {
    let abilities: Vec<usize> = (before.abilities..out.abilities.len()).collect();
    let gap = out.gaps.get(before.gaps).map(|gap| gap.0.clone());
    if abilities.is_empty() && gap.is_none() {
        return;
    }
    for body in bodies {
        out.ledger.push(LedgerLine {
            face: 0,
            tag: tag.to_owned(),
            text: text_of(tag, body),
            abilities: abilities.clone(),
            gap: gap.clone(),
        });
    }
}
