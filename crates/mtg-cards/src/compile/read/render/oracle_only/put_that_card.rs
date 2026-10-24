//! The card of a creature that died, put onto the battlefield under your control: "Whenever a creature an opponent
//! controls with a -1/-1 counter on it dies, you may put that card onto the battlefield under your control. Do this
//! only once each turn." The renderers have no wording for the move of the card of the event.

use super::super::trigger_head;
use crate::ability::{Ability, Limit};
use crate::effect::{Destination, Effect};
use crate::refs::Ref;

/// Adds the wordings of a trigger whose only step lets the controller put the card of the event onto the battlefield.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when,
        intervening_if,
        limit,
        effects,
        ..
    } = ability
    else {
        return;
    };
    let [
        Effect::May {
            who: Ref::You,
            effects: inner,
        },
    ] = effects.as_slice()
    else {
        return;
    };
    let [
        Effect::Move {
            what: Ref::Triggering,
            to:
                Destination::Battlefield {
                    tapped: false,
                    under: Some(Ref::You),
                    transformed: false,
                    counters,
                },
        },
    ] = inner.as_slice()
    else {
        return;
    };
    let Some(head) = trigger_head(when, intervening_if.as_ref()).filter(|_| counters.is_empty())
    else {
        return;
    };
    let once = match limit {
        Some(Limit::OncePerTurn) => " Do this only once each turn.",
        None => "",
    };
    texts.push(format!(
        "{} you may put that card onto the battlefield under your control.{once}",
        head.join(" ")
    ));
}
