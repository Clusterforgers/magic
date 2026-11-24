//! Suffixes of the shared noun-phrase reader and writer for pack r120-01, consulted after those of pack r61-00: "that
//! you've put a counter on this turn" (a permanent a spell or ability you controlled put counters on this turn).

use super::selectors::Form;
use super::tokens::Reader;
use crate::refs::{Pred, Ref};
use crate::value::TallyEvent;

/// The words of the suffix, the same in either number.
const R120_COUNTERED: &str = "that you've put a counter on this turn";

/// A permanent a spell or ability you controlled put one or more counters on this turn.
fn r120_countered() -> Pred {
    Pred::DidThisTurn {
        event: TallyEvent::CountersPut,
        by: Some(Ref::You),
        to: None,
        combat: false,
    }
}

/// A suffix of this module after the noun, after those of pack r61-00.
pub(super) fn r120_suffix(r: &mut Reader<'_>, form: Form) -> Option<Pred> {
    super::r6100_atoms::r6100_suffix(r, form)
        .or_else(|| r.words(R120_COUNTERED).then(r120_countered))
}

/// The suffix of this module for a predicate, after those of pack r61-00.
pub(super) fn r120_suffix_text(pred: &Pred, form: Form) -> Option<String> {
    super::r6100_atoms::r6100_suffix_text(pred, form)
        .or_else(|| (*pred == r120_countered()).then(|| R120_COUNTERED.to_owned()))
}
