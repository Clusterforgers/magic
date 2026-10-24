//! The commander creatures a player owns: `Commander creatures you own have "{2}, Sacrifice an artifact: Put a +1/+1
//! counter on this creature and draw a card."` The renderers say "Each your commander creature has" and join the
//! steps of the quoted ability with "Then".

use super::text::json;
use crate::ability::Ability;

/// The subject as the renderers write it and as the card writes it.
const SUBJECT: (&str, &str) = (
    "Each your commander creature has ",
    "Commander creatures you own have ",
);

/// Adds the wordings of commander creatures given an ability.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let rules = json(ability);
    if !rules.contains(r#""GrantAbility""#) || !rules.contains(r#""Commander""#) {
        return;
    }
    let said: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(SUBJECT.0))
        .flat_map(|text| {
            let owned = text.replacen(SUBJECT.0, SUBJECT.1, 1);
            [
                owned.replacen(". Then draw a card.\"", " and draw a card.\"", 1),
                owned,
            ]
        })
        .collect();
    texts.extend(said);
}
