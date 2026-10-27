//! A draw up to a hand size: "At the beginning of your upkeep, if you have fewer than seven cards in hand, draw cards
//! equal to the difference." The renderers write the plain draw of a card, which is the twin without the condition.

use super::{c5_af_json, c5_af_variants};
use crate::ability::Ability;
use crate::compile::read::amounts::count_word;
use crate::effect::Effect;
use crate::value::{Amount, Cmp, Condition};

/// The cards in the controller's hand, as the rules count them.
const C5_AF_HAND: &str = r#"{"Count":[{"In":"Hand"},{"OwnedBy":"You"}]}"#;

/// Adds the wording of a draw of the cards a hand lacks to a size.
pub(super) fn c5_af_add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        intervening_if:
            Some(Condition::Compare {
                left,
                op: Cmp::Lt,
                right: Amount::Fixed(size),
            }),
        effects,
        ..
    } = ability
    else {
        return;
    };
    let [Effect::Draw { count, .. }] = effects.as_slice() else {
        return;
    };
    let lacking = format!(r#"{{"Sum":[{size},{{"Times":{{"by":-1,"of":{C5_AF_HAND}}}}}]}}"#);
    let hand = serde_json::to_string(left).unwrap_or_default();
    let count = serde_json::to_string(count).unwrap_or_default();
    if hand != C5_AF_HAND || count != lacking || !c5_af_json(ability).contains(r#""Draw":{"count""#)
    {
        return;
    }
    let Some(word) = u32::try_from(*size).ok().and_then(count_word) else {
        return;
    };
    let mut plain = ability.clone();
    if let Ability::Triggered {
        intervening_if,
        effects,
        ..
    } = &mut plain
    {
        *intervening_if = None;
        *effects = vec![Effect::Draw {
            who: crate::refs::Ref::You,
            count: Amount::Fixed(1),
        }];
    }
    let card =
        format!("if you have fewer than {word} cards in hand, draw cards equal to the difference.");
    texts.extend(c5_af_variants(&plain).iter().filter_map(|text| {
        let head = text.strip_suffix(", draw a card.")?;
        Some(format!("{head}, {card}"))
    }));
}
