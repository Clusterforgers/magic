//! A trigger of a card in its graveyard that returns it for a payment: "At the beginning of your upkeep, if this card is
//! in your graveyard, you may pay 1 life. If you do, return it to your hand."

use crate::effect::{Destination, Effect};
use crate::refs::Ref;

/// Whether the effects are one payment after which the card returns to its owner's hand.
pub(super) fn pays_to_return(effects: &[Effect]) -> bool {
    matches!(
        effects,
        [Effect::Pay { who: Ref::You, paid, unpaid, .. }]
            if unpaid.is_empty() && matches!(paid.as_slice(), [Effect::Move { what: Ref::This, to: Destination::Hand }])
    )
}

/// Whether the effects are the card returning itself to its owner's hand, which it does from its graveyard ("you may
/// return this card from your graveyard to your hand").
pub(super) fn returns_to_hand(effects: &[Effect]) -> bool {
    match effects {
        [
            Effect::May {
                who: Ref::You,
                effects,
            },
        ] => returns_to_hand(effects),
        [
            Effect::Move {
                what: Ref::This,
                to: Destination::Hand,
            },
        ] => true,
        // A payment after which the card returns: "you may pay {R}. If you do, return this card from your graveyard".
        [
            Effect::Pay {
                who: Ref::You,
                paid,
                unpaid,
                ..
            },
        ] if unpaid.is_empty() => returns_to_hand(paid),
        _ => false,
    }
}

/// The wording of the return from the graveyard to the hand: the card names the zone it comes from.
pub(super) fn from_your_graveyard(texts: &mut Vec<String>) {
    let said: Vec<String> = texts
        .iter()
        .filter(|text| text.contains("return ~ to its owner's hand"))
        .map(|text| {
            text.replace(
                "return ~ to its owner's hand",
                "return ~ from your graveyard to your hand",
            )
        })
        .collect();
    *texts = said;
}

/// The wordings of the trigger: the card says it is in the graveyard after the event, and "it" for itself.
pub(super) fn say(texts: &mut Vec<String>) {
    let said: Vec<String> = texts
        .iter()
        .filter_map(|text| {
            let (event, tail) = text.split_once(", ")?;
            Some(format!(
                "{event}, if this card is in your graveyard, {tail}"
            ))
        })
        .collect();
    *texts = said
        .iter()
        .cloned()
        .chain(
            said.iter()
                .map(|text| text.replace("return ~ to its owner's hand", "return it to your hand")),
        )
        .collect();
}
