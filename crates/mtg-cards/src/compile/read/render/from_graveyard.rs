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
