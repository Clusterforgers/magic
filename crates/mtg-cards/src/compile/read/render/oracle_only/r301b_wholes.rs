//! Whole abilities of the r30-1b card pack whose steps the renderers do not join: each is matched whole as data and worded
//! as the card prints it, so another count, kind, cost or restriction in the same ability never reads back.

use crate::ability::Ability;
use std::sync::LazyLock;

/// The abilities, as JSON, each with its wording (the card's name as `~`).
const R301B_WHOLES: [(&str, &str); 4] = [
    // rosheen_roaring_prophet
    (
        r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Mill":{"count":6,"as":"milled"}},{"Choose":{"from":{"Each":[{"Is":{"Bound":"milled"}},{"In":"Graveyard"},"XInCost"]},"count":{"min":0,"max":1},"as":"kept"}},{"Move":{"what":{"Bound":"kept"},"to":"Hand"}}]}}"#,
        r"When ~ enters, mill six cards. You may put a card with {X} in its mana cost from among them into your hand.",
    ),
    (
        r#"{"Activated":{"cost":["Tap"],"effects":[{"Choose":{"from":{"Each":[{"In":"Hand"},{"OwnedBy":"You"},"XInCost"]},"count":{"min":0,"max":null},"as":"revealed"}},{"Reveal":{"what":{"Bound":"revealed"}}},{"AddMana":{"mana":{"Fixed":["Colorless","Colorless"]},"times":{"CountOf":{"of":{"Bound":"revealed"},"filter":[]}},"only":["CostWithX"]}}]}}"#,
        r"{T}: Reveal any number of cards with {X} in their mana cost in your hand. Add {C}{C} for each card revealed this way. Spend this mana only on costs that contain {X}.",
    ),
    // rasputin_the_oneiromancer
    (
        r#"{"Activated":{"cost":["Tap",{"RemoveCounters":{"counter":{"Other":"dream"},"count":"X"}},{"XAtLeast":1}],"effects":[{"AddMana":{"mana":{"Fixed":["Colorless"]},"times":{"RemovedThisWay":{"Other":"dream"}}}}]}}"#,
        r"{T}, Remove one or more dream counters from ~: Add that much {C}.",
    ),
    // vazi_keen_negotiator
    (
        r#"{"Activated":{"cost":["Tap"],"targets":[{"what":"Opponent"}],"effects":[{"CreateToken":{"who":{"Target":0},"token":{"Named":"treasure"},"count":{"Tally":{"event":"TokenCreated","who":"You","filter":[{"Subtype":"Treasure"}]}}}}]}}"#,
        r"{T}: Target opponent creates X Treasure tokens, where X is the number of Treasure tokens you created this turn.",
    ),
];

/// The entries parsed once.
static R301B_SHAPES: LazyLock<Vec<(Ability, &str)>> = LazyLock::new(|| {
    R301B_WHOLES
        .iter()
        .filter_map(|(shape, text)| Some((serde_json::from_str(shape).ok()?, *text)))
        .collect()
});

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn r301b_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R301B_SHAPES.iter() {
        if shape == ability {
            texts.push((*text).to_owned());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the r30-1b table parses as an ability, so none is silently dropped.
    use super::{R301B_SHAPES, R301B_WHOLES};

    #[test]
    fn r301b_every_whole_parses() {
        assert_eq!(R301B_SHAPES.len(), R301B_WHOLES.len());
    }
}
