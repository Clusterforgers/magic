//! Suffixes of the shared noun-phrase reader and writer for round 28's third pack, consulted after the others: "cast
//! from a graveyard", "that targets a player", "that dealt damage to you this turn" and "that's the second spell cast this
//! turn".

use super::selectors::Form;
use super::tokens::Reader;
use crate::refs::{Pred, Ref, ZoneKind};
use crate::value::TallyEvent;

/// The fixed suffixes of this module and what they read.
fn r283_fixed() -> [(&'static str, Pred); 4] {
    [
        (
            "that's the second spell cast this turn",
            Pred::NthCastThisTurn(2),
        ),
        ("cast from a graveyard", Pred::CastFrom(ZoneKind::Graveyard)),
        (
            "that targets a player",
            Pred::TargetsPlayer(Ref::EachPlayer),
        ),
        (
            "that dealt damage to you this turn",
            Pred::DidThisTurn {
                event: TallyEvent::Damaged,
                by: None,
                to: Some(Ref::You),
                combat: false,
            },
        ),
    ]
}

/// A suffix of this module after the noun of a single object.
pub(super) fn r283_suffix(r: &mut Reader<'_>, form: Form) -> Option<Pred> {
    if form.plural() {
        return None;
    }
    r283_fixed()
        .into_iter()
        .find_map(|(words, pred)| r.words(words).then_some(pred))
}

/// The suffix of this module for a predicate.
pub(super) fn r283_suffix_text(pred: &Pred, form: Form) -> Option<String> {
    if form.plural() {
        return None;
    }
    r283_fixed()
        .into_iter()
        .find_map(|(words, held)| (held == *pred).then(|| words.to_owned()))
}
