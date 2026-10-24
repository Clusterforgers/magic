//! The line of a loyalty ability: its cost ("+1", "−2", "0", "−X"), a colon and the sentences of its effects.

use super::{alternatives, closed};
use crate::ability::LoyaltyCost;
use crate::effect::Effect;
use crate::refs::TargetSpec;

/// The cost as the card prints it, with the minus sign U+2212.
fn cost_text(cost: LoyaltyCost) -> String {
    match cost {
        LoyaltyCost::Plus(number) => format!("+{number}"),
        LoyaltyCost::Minus(number) => format!("\u{2212}{number}"),
        LoyaltyCost::Zero => "0".to_owned(),
        LoyaltyCost::MinusX => "\u{2212}X".to_owned(),
    }
}

/// Every wording of a loyalty ability.
pub(super) fn texts(cost: LoyaltyCost, targets: &[TargetSpec], effects: &[Effect]) -> Vec<String> {
    let head = cost_text(cost);
    alternatives(targets, effects)
        .iter()
        .map(|sentences| format!("{head}: {}", closed(sentences, true).join(" ")))
        .collect()
}
