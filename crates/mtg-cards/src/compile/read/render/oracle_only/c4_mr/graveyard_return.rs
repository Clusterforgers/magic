//! A card that returns itself from its graveyard at a step: "At the beginning of your first main phase, you may discard two
//! cards. If you do, return this card from your graveyard to your hand." The renderers add "if this card is in your
//! graveyard" after the step and "Then" before "if you do"; the card says where it returns from instead.

use crate::ability::Ability;
use crate::refs::ZoneKind;

/// The words that return the card the renderers write, and the words that return it from the graveyard.
const RETURNS: [(&str, &str); 3] = [
    (
        ", return it to your hand.",
        ", return this card from your graveyard to your hand.",
    ),
    (
        ", return ~ to its owner's hand.",
        ", return this card from your graveyard to your hand.",
    ),
    (
        ", return it to the battlefield.",
        ", return this card from your graveyard to the battlefield.",
    ),
];

/// Adds the wordings of a triggered ability of a card in its graveyard that name the graveyard where it returns from.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        from: ZoneKind::Graveyard,
        ..
    } = ability
    else {
        return;
    };
    let said: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(", if this card is in your graveyard, "))
        .flat_map(|text| {
            let bare = text
                .replacen(", if this card is in your graveyard, ", ", ", 1)
                .replacen(". Then if you do, ", ". If you do, ", 1);
            RETURNS
                .iter()
                .filter(|(from, _)| bare.ends_with(from))
                .map(|(from, to)| format!("{}{to}", bare.trim_end_matches(from)))
                .collect::<Vec<_>>()
        })
        .collect();
    texts.extend(said);
}
