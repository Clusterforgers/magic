//! A draw after a counter on the card keeps its subject: "put a +1/+1 counter on ~ and you draw a card". The renderers
//! write "and draw a card".

use crate::ability::Ability;

/// The step as the renderers word it and as the oracle text does.
const DRAW: (&str, &str) = (
    " counter on ~ and draw a card",
    " counter on ~ and you draw a card",
);

/// Adds the wordings that say "you draw" after a counter put on the card.
pub(super) fn add(_ability: &Ability, texts: &mut Vec<String>) {
    let (said, wanted) = DRAW;
    let found: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(said))
        .map(|text| text.replacen(said, wanted, 1))
        .collect();
    texts.extend(found);
}
