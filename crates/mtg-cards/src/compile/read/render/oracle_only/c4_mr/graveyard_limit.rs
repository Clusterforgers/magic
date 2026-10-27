//! An activation limited by the cards in the graveyards of the opponents: "Activate only if an opponent has eight or more
//! cards in their graveyard." The renderers know the limit "Activate only during your turn".

use super::twin_variants;
use crate::ability::Ability;

/// The numbers of cards a card asks for, as the JSON of the check and as words.
const COUNTS: [(u32, &str); 6] = [
    (4, "four"),
    (5, "five"),
    (6, "six"),
    (7, "seven"),
    (8, "eight"),
    (10, "ten"),
];

/// Adds the wordings of an activated ability limited to an opponent with some cards in their graveyard.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    for (number, word) in COUNTS {
        let check = format!(
            r#""condition":{{"Compare":{{"left":{{"Largest":{{"who":"Opponents","of":[{{"In":"Graveyard"}}]}}}},"op":"Ge","right":{number}}}}}"#
        );
        let said: Vec<String> = twin_variants(ability, &check, r#""condition":"YourTurn""#)
            .iter()
            .filter(|text| text.contains("Activate only during your turn."))
            .map(|text| {
                text.replacen(
                    "Activate only during your turn.",
                    &format!(
                        "Activate only if an opponent has {word} or more cards in their graveyard."
                    ),
                    1,
                )
            })
            .collect();
        texts.extend(said);
    }
}
