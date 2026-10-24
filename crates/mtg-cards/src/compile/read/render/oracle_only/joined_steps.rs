//! Steps the oracle joins or words another way than the renderers: "Put one of those cards into your hand", "loses 3
//! life and you gain 3 life", "deals 1 damage to you and you draw a card", "proliferate, then proliferate again", "draws
//! two cards, then discards two cards", "Tap an untapped legendary creature you control" and "Draw a card if you control
//! an artifact. Draw a card if you control an enchantment."

use crate::ability::Ability;

/// The effect the ability must hold, the wording the renderers write and the one the oracle uses, one line each.
const EDITS: [(&str, &str, &str); 6] = [
    (
        "\"LookAtTop\"",
        "Put one of them into your hand",
        "Put one of those cards into your hand",
    ),
    (
        "\"LoseLife\"",
        " life. Then you gain ",
        " life and you gain ",
    ),
    (
        "\"DealDamage\"",
        " damage to you. Then draw ",
        " damage to you and you draw ",
    ),
    (
        "\"Proliferate\"",
        "proliferate. Then proliferate",
        "proliferate, then proliferate again",
    ),
    (
        "\"Discard\"",
        " cards and discards ",
        " cards, then discards ",
    ),
    (
        "\"TapPermanents\"",
        "Tap a legendary untapped creature",
        "Tap an untapped legendary creature",
    ),
];

/// Adds the wordings that join or reorder the steps of the ability.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ok(json) = serde_json::to_string(ability) else {
        return;
    };
    let mut said = Vec::new();
    for (effect, written, oracle) in EDITS {
        if json.contains(effect) {
            said.extend(
                texts
                    .iter()
                    .filter(|text| text.contains(written))
                    .map(|text| text.replacen(written, oracle, 1)),
            );
        }
    }
    if json.contains("\"If\"") {
        said.extend(texts.iter().filter_map(|text| if_you_control(text)));
    }
    texts.extend(said);
}

/// "{2}, {T}, Sacrifice ~: If you control an artifact, draw a card. Then draw a card if you control an enchantment."
/// as the two sentences the card prints.
fn if_you_control(text: &str) -> Option<String> {
    let (head, rest) = text.split_once(": If you control ")?;
    let (first, tail) = rest.split_once(", draw a card. ")?;
    let second = tail
        .strip_prefix("Then d")
        .map_or_else(|| tail.to_owned(), |after| format!("D{after}"));
    second
        .starts_with("Draw a card if you control ")
        .then(|| format!("{head}: Draw a card if you control {first}. {second}"))
}
