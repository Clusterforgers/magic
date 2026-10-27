//! Cards drawn by the size of a hand: "Draw cards equal to the number of cards in target opponent's hand."

use crate::ability::Ability;

/// The targets and the draw by the cards in the hand of the target, for an opponent and for a player.
const BY_HAND: [(&str, &str); 2] = [
    (
        r#""targets":[{"what":"Opponent"}],"effects":[{"Draw":{"count":{"Count":[{"In":"Hand"},{"OwnedBy":{"Target":0}}]}}}]"#,
        "Draw cards equal to the number of cards in target opponent's hand.",
    ),
    (
        r#""targets":[{"what":"Player"}],"effects":[{"Draw":{"count":{"Count":[{"In":"Hand"},{"OwnedBy":{"Target":0}}]}}}]"#,
        "Draw cards equal to the number of cards in target player's hand.",
    ),
];

/// Adds the wording of a spell that draws as many cards as a target player has in their hand.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let json = serde_json::to_string(ability).unwrap_or_default();
    if !matches!(ability, Ability::Spell { .. }) {
        return;
    }
    for (rules, text) in BY_HAND {
        if json.contains(rules) {
            texts.push(text.to_owned());
        }
    }
}
