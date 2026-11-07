//! g8e: the damage an Aura or Equipment prevents for the creature it is attached to, written back: "Prevent all combat
//! damage that would be dealt to equipped creature.", "Prevent all damage that would be dealt by enchanted creature."

use crate::refs::{Pred, Ref, Selector};

/// The wordings, without a capital and a period, of a replacement that prevents all `kind` of damage dealt to the
/// attached creature from any source, or dealt by it to anything; none for any other recipient or source.
pub(super) fn g8e_prevented(to: Option<&Ref>, source: &Selector, kind: &str) -> Vec<String> {
    let side = match (to, source.0.as_slice()) {
        (Some(Ref::Attached), []) => "to",
        (None, [Pred::Is(Ref::Attached)]) => "by",
        _ => return Vec::new(),
    };
    ["enchanted creature", "equipped creature"]
        .iter()
        .map(|creature| format!("prevent all {kind} that would be dealt {side} {creature}"))
        .collect()
}
