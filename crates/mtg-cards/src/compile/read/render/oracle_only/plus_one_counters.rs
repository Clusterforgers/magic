//! One counter more of each kind: "If one or more counters would be put on an artifact or creature you control, that
//! many plus one of each of those kinds of counters are put on that permanent instead." and "If you would get one or
//! more counters, you get that many plus one of each of those kinds of counters instead."

use crate::ability::Ability;
use crate::refs::Ref;
use crate::trigger::{ReplaceBy, ReplaceWhen};

/// What the renderers write for the added counter and the card's words for it.
const PLUS_ONE: [(&str, &str); 2] = [
    (
        "that many plus one counters",
        "that many plus one of each of those kinds of counters",
    ),
    (
        "that many plus 1 counters",
        "that many plus one of each of those kinds of counters",
    ),
];

/// Adds the wordings of a replacement that adds one counter of each kind that would be put.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Replacement {
        when: ReplaceWhen::CountersPut { on, counter: None },
        by: ReplaceBy::Add(1),
        condition: None,
        ..
    } = ability
    else {
        return;
    };
    if on.as_ref() == Some(&Ref::You) {
        texts.push(
            "If you would get one or more counters, you get that many plus one of each of those kinds of counters instead."
                .to_owned(),
        );
        return;
    }
    for (from, into) in PLUS_ONE {
        let said: Vec<String> = texts
            .iter()
            .filter(|text| text.contains(from))
            .map(|text| text.replace(from, into))
            .collect();
        texts.extend(said);
    }
}
