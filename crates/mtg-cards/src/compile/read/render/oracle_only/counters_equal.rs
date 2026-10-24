//! Counters of an amount that a "where" clause names: "each opponent gets a number of rad counters equal to its power."
//! The renderers write "X rad counters, where X is its power".

use crate::ability::Ability;

/// The kinds of counter a player gets.
const KINDS: [&str; 3] = ["rad", "poison", "experience"];

/// Adds the wordings that count the counters as a number equal to the amount.
pub(super) fn add(_ability: &Ability, texts: &mut Vec<String>) {
    let found: Vec<String> = texts
        .iter()
        .flat_map(|text| {
            KINDS.iter().filter_map(move |kind| {
                let said = format!("X {kind} counters, where X is ");
                let wanted = format!("a number of {kind} counters equal to ");
                text.contains(&said)
                    .then(|| text.replacen(&said, &wanted, 1))
            })
        })
        .collect();
    texts.extend(found);
}
