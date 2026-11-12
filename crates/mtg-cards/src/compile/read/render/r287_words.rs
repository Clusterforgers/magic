//! Sentences of the r28-7 card pack that no other writer gives: "your life total becomes N".

use crate::effect::Effect;
use crate::refs::Ref;
use crate::value::Amount;

/// "your life total becomes 4": the controller's life total set to a fixed number.
pub(super) fn r287_text(effect: &Effect) -> Option<String> {
    match effect {
        Effect::SetLife {
            who: Ref::You,
            amount: Amount::Fixed(n),
        } => Some(format!("your life total becomes {n}")),
        _ => None,
    }
}
