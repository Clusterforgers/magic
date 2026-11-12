//! The damage one object dealt this turn to the player of the event (`Amount::DamageDealtByTo`), after an intervening
//! "if": "if it has dealt 10 or more damage to that player this turn".

use super::facts::quantity_texts;
use crate::refs::Ref;
use crate::value::{Amount, Cmp};

/// The wordings of the amount compared with the number, or nothing when no row has it.
pub(super) fn texts(left: &Amount, op: Cmp, number: u32) -> Vec<String> {
    let Amount::DamageDealtByTo {
        by: Ref::This,
        to: Ref::Triggering,
    } = left
    else {
        return Vec::new();
    };
    if op != Cmp::Ge || number == 0 {
        return Vec::new();
    }
    quantity_texts(number)
        .into_iter()
        .map(|n| format!("it has dealt {n} or more damage to that player this turn"))
        .collect()
}
