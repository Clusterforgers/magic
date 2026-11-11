//! Counter replacements that know who puts the counters or that only effects do: "If you would put one or more counters
//! on a permanent or player, put twice that many of each of those kinds of counters on that permanent or player
//! instead.", "If an opponent would put ..., they put half that many ... instead, rounded down.", "If an effect would put
//! one or more counters on a permanent you control, it puts twice that many of those counters on that permanent instead."

use super::replacements::anaphors;
use super::triggers::subject_text;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::trigger::{ReplaceBy, ReplaceWhen};

/// The sentences of a counter replacement with a putter or for effects only, without the capital and the period.
pub(super) fn ep24_counter_texts(when: &ReplaceWhen, by: &ReplaceBy) -> Vec<String> {
    let ReplaceWhen::CountersPut {
        on,
        counter: None,
        by: putter,
        effect,
    } = when
    else {
        return Vec::new();
    };
    let (place, again) = match on {
        None => (
            "a permanent or player".to_owned(),
            vec!["that permanent or player".to_owned()],
        ),
        // A permanent is on the battlefield: the words leave the zone out.
        Some(Ref::Each(what)) => match subject_text(&Selector(
            what.0
                .iter()
                .filter(|pred| **pred != Pred::In(ZoneKind::Battlefield))
                .cloned()
                .collect(),
        )) {
            Some(place) => {
                let again = anaphors(&place);
                (place, again)
            }
            None => return Vec::new(),
        },
        _ => return Vec::new(),
    };
    let mut found = Vec::new();
    for that in &again {
        let text = match (putter, *effect, by) {
            (Some(Ref::You), false, ReplaceBy::Multiply(2)) => format!(
                "if you would put one or more counters on {place}, put twice that many of each of those kinds of counters on {that} instead"
            ),
            (Some(Ref::You), false, ReplaceBy::Add(1)) => format!(
                "if you would put one or more counters on {place}, put that many plus one of each of those kinds of counters on {that} instead"
            ),
            (Some(Ref::Opponents), false, ReplaceBy::Half) => format!(
                "if an opponent would put one or more counters on {place}, they put half that many of each of those kinds of counters on {that} instead, rounded down"
            ),
            (None, true, ReplaceBy::Multiply(2)) => format!(
                "if an effect would put one or more counters on {place}, it puts twice that many of those counters on {that} instead"
            ),
            _ => return Vec::new(),
        };
        found.push(text);
    }
    found
}
