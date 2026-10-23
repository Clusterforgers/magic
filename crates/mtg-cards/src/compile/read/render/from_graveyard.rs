//! A trigger of a card in its graveyard that returns it for a payment: "At the beginning of your upkeep, if this card is
//! in your graveyard, you may pay 1 life. If you do, return it to your hand."

use super::{alternatives, closed, trigger_head};
use crate::effect::Effect;
use crate::refs::{Ref, TargetSpec};
use crate::trigger::TriggerPattern;

/// Whether the effects are one payment after which the card returns to its owner's hand.
pub(super) fn pays_to_return(effects: &[Effect]) -> bool {
    matches!(
        effects,
        [Effect::Pay { who: Ref::You, paid, unpaid, .. }]
            if unpaid.is_empty() && matches!(paid.as_slice(), [Effect::Move { what: Ref::This, .. }])
    )
}

/// The wordings of the trigger: the card says it is in the graveyard after the event, and "it" for itself.
pub(super) fn texts(
    when: &TriggerPattern,
    targets: &[TargetSpec],
    effects: &[Effect],
) -> Vec<String> {
    let Some(head) = trigger_head(when, None) else {
        return Vec::new();
    };
    let [event] = head.as_slice() else {
        return Vec::new();
    };
    let head = format!(
        "{}, if this card is in your graveyard,",
        event.trim_end_matches(',')
    );
    alternatives(targets, effects)
        .iter()
        .flat_map(|sentences| {
            let tail = closed(sentences, false).join(" ");
            [
                format!("{head} {tail}"),
                format!(
                    "{head} {}",
                    tail.replace("return ~ to its owner's hand", "return it to your hand")
                ),
            ]
        })
        .collect()
}
