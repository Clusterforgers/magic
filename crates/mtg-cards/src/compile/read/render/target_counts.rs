//! Cards drawn for what each of two players controls: "Draw a card for each creature target player controls, then draw a
//! card for each creature another target player controls."

use super::{Effect, Ref, TargetSpec, cards_text};
use crate::refs::{Pred, Targetable};
use crate::value::Amount;

/// The sentence for a draw counted by what the first target controls and one counted by what the second target controls.
pub(super) fn text(targets: &[TargetSpec], effects: &[Effect]) -> Option<String> {
    let [first, second] = targets else {
        return None;
    };
    let players =
        |spec: &TargetSpec| matches!(spec.what, Targetable::Player) && spec.bounds.is_one();
    (players(first) && players(second)).then_some(())?;
    let [
        Effect::Draw {
            who: Ref::You,
            count: one,
        },
        Effect::Draw {
            who: Ref::You,
            count: other,
        },
    ] = effects
    else {
        return None;
    };
    controlled_by(one, 0)?;
    controlled_by(other, 1)?;
    Some(format!(
        "draw {}, then draw {}",
        cards_text(one)?,
        cards_text(other)?
    ))
}

/// Whether the amount counts what target `number` controls, and no other player.
fn controlled_by(amount: &Amount, number: u8) -> Option<()> {
    let Amount::Count(selector) = amount else {
        return None;
    };
    let holders: Vec<&Pred> = selector
        .0
        .iter()
        .filter(|pred| matches!(pred, Pred::ControlledBy(_)))
        .collect();
    (holders == [&Pred::ControlledBy(Ref::Target(number))]).then_some(())
}
