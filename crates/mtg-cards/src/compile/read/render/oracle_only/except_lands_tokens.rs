//! Every permanent but lands and tokens: "Destroy all other permanents except for lands and tokens." The renderers
//! name the group by what it is not: "all other nonland, nontoken permanents".

use crate::ability::Ability;

/// The group as the renderers word it and as the oracle text does, with and without "other".
const GROUPS: [(&str, &str); 2] = [
    (
        "all other nonland, nontoken permanents",
        "all other permanents except for lands and tokens",
    ),
    (
        "all nonland, nontoken permanents",
        "all permanents except for lands and tokens",
    ),
];

/// Adds the wordings that name the group by the exception.
pub(super) fn add(_ability: &Ability, texts: &mut Vec<String>) {
    let found: Vec<String> = texts
        .iter()
        .flat_map(|text| {
            GROUPS
                .iter()
                .filter(|(said, _)| text.contains(said))
                .map(|(said, wanted)| text.replacen(said, wanted, 1))
        })
        .collect();
    texts.extend(found);
}
