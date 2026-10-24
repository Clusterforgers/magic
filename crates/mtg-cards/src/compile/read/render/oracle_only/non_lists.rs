//! A negated type that heads a list of two types names the negation twice in the oracle text: "target noncreature
//! artifact or noncreature enchantment". The renderers write it once.

use crate::ability::Ability;

/// The card types a negation can head.
const TYPES: [&str; 7] = [
    "artifact",
    "creature",
    "enchantment",
    "land",
    "planeswalker",
    "battle",
    "permanent",
];

/// Adds the wordings that repeat the negation before the second type of a list.
pub(super) fn add(_ability: &Ability, texts: &mut Vec<String>) {
    let found: Vec<String> = texts.iter().filter_map(|text| repeated(text)).collect();
    texts.extend(found);
}

/// The text with "noncreature artifact or enchantment" as "noncreature artifact or noncreature enchantment".
fn repeated(text: &str) -> Option<String> {
    let words: Vec<&str> = text.split(' ').collect();
    let at = words.windows(4).position(|w| {
        let last = w[3].trim_end_matches([',', '.']);
        w[0].len() > 3
            && w[0].starts_with("non")
            && w[2] == "or"
            && TYPES.contains(&w[1])
            && TYPES.contains(&last)
    })?;
    let mut changed = words.clone();
    let joined = format!("{} {}", words[at], words[at + 3]);
    changed[at + 3] = &joined;
    Some(changed.join(" "))
}
