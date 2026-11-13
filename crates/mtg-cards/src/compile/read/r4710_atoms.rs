//! Suffixes of the shared noun-phrase reader and writer for pack r47-10, consulted after the others: "that was turned
//! face up this turn" (one permanent) and "that were turned face up this turn" (several).

use super::selectors::Form;
use super::tokens::Reader;
use crate::refs::Pred;
use crate::value::TallyEvent;

/// A permanent turned face up this turn, by anyone.
fn r4710_turned_up() -> Pred {
    Pred::DidThisTurn {
        event: TallyEvent::TurnedFaceUp,
        by: None,
        to: None,
        combat: false,
    }
}

/// The words of the suffix in the form's number.
const fn r4710_words(form: Form) -> &'static str {
    if form.plural() {
        "that were turned face up this turn"
    } else {
        "that was turned face up this turn"
    }
}

/// A suffix of this module after the noun.
pub(super) fn r4710_suffix(r: &mut Reader<'_>, form: Form) -> Option<Pred> {
    r.words(r4710_words(form)).then(r4710_turned_up)
}

/// The suffix of this module for a predicate.
pub(super) fn r4710_suffix_text(pred: &Pred, form: Form) -> Option<String> {
    (*pred == r4710_turned_up()).then(|| r4710_words(form).to_owned())
}
