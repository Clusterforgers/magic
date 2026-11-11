//! A player's discards or sacrifices of one effect as one event: "Whenever you sacrifice one or more Foods", "Whenever a
//! player discards one or more cards", "Whenever you discard one or more land cards" (`TriggerPattern::Batch`).

use super::selectors::{Form, phrase};
use super::tokens::Reader;
use crate::refs::{Pred, Ref, Selector};
use crate::trigger::TriggerPattern;

/// After "sacrifice(s) one or more": the permanents, which the batch of that player's sacrifices holds.
pub(super) fn dk27_sacrificed_batch(r: &mut Reader<'_>, by: Ref) -> Option<TriggerPattern> {
    let what = phrase(r, Form::All)?;
    (!what.is_empty())
        .then(|| TriggerPattern::Batch(Box::new(TriggerPattern::Sacrificed { what, by })))
}

/// After "discard(s) one or more": "cards" or "<card type> cards", which the batch of that player's discards holds.
pub(super) fn dk27_discarded_batch(r: &mut Reader<'_>, by: Ref) -> Option<TriggerPattern> {
    let what = if r.word("cards") {
        Selector::default()
    } else {
        let what = phrase(r, Form::Subject)?;
        (matches!(what.0.as_slice(), [Pred::Type(_)]) && r.word("cards")).then_some(what)?
    };
    Some(TriggerPattern::Batch(Box::new(TriggerPattern::Discarded {
        what,
        by,
    })))
}
