//! Energy gained by an amount: "You get an amount of {E} equal to its mana value." The renderers word the count with a
//! "where" clause: "You get X {E}, where X is its mana value."

use crate::ability::Ability;

/// The words after "get" as the renderers write them and as the oracle text does.
const AMOUNT: (&str, &str) = ("get X {E}, where X is ", "get an amount of {E} equal to ");

/// Adds the wordings that name the energy as an amount equal to a count.
pub(super) fn add(_ability: &Ability, texts: &mut Vec<String>) {
    let (said, wanted) = AMOUNT;
    let found: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(said))
        .map(|text| text.replacen(said, wanted, 1))
        .collect();
    texts.extend(found);
}
