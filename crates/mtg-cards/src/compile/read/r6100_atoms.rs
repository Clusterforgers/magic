//! Suffixes of the shared noun-phrase reader and writer for pack r61-00, consulted after the others: "that fought this
//! turn" (a creature that fought, by anyone).

use super::selectors::Form;
use super::tokens::Reader;
use crate::refs::Pred;
use crate::value::TallyEvent;

/// The words of the suffix, the same in either number.
const R6100_FOUGHT: &str = "that fought this turn";

/// A creature that fought this turn.
fn r6100_fought() -> Pred {
    Pred::DidThisTurn {
        event: TallyEvent::Fought,
        by: None,
        to: None,
        combat: false,
    }
}

/// A suffix of this module after the noun.
pub(super) fn r6100_suffix(r: &mut Reader<'_>, _form: Form) -> Option<Pred> {
    r.words(R6100_FOUGHT).then(r6100_fought)
}

/// The suffix of this module for a predicate.
pub(super) fn r6100_suffix_text(pred: &Pred, _form: Form) -> Option<String> {
    (*pred == r6100_fought()).then(|| R6100_FOUGHT.to_owned())
}
