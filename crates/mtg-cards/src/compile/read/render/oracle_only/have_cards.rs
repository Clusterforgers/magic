//! A count of the cards in a graveyard that the player has: "if you have four or more creature cards in your graveyard".
//! The renderers write "if there are 4 or more creature cards in your graveyard".

use super::super::super::amounts::count_word;
use crate::ability::Ability;

/// Adds the wordings that say the player "has" the cards, with the number in words.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    if !matches!(
        ability,
        Ability::Triggered {
            intervening_if: Some(_),
            ..
        }
    ) {
        return;
    }
    let found: Vec<String> = texts.iter().filter_map(|text| have(text)).collect();
    texts.extend(found);
}

/// The text with "if there are 4 or more X cards in your graveyard" worded "if you have four or more X cards in your
/// graveyard".
fn have(text: &str) -> Option<String> {
    let from = "if there are ";
    let at = text.find(from)?;
    let rest = &text[at + from.len()..];
    let (digits, tail) = rest.split_once(" or more ")?;
    let word = count_word(digits.parse().ok()?)?;
    tail.split(", ")
        .next()?
        .ends_with(" in your graveyard")
        .then(|| format!("{}if you have {word} or more {tail}", &text[..at]))
}
