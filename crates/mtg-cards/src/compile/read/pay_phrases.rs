//! The payments of a "you may pay" or an "unless" that have more than one part or return a permanent: "pay {2} and
//! sacrifice a token", "return an untapped Forest you control to its owner's hand".

use super::costs;
use crate::cost::{Cost, CostItem};
use crate::value::Amount;

/// The verb phrase of the payment, or `None` for a payment it does not word.
pub(super) fn phrase(cost: &Cost) -> Option<String> {
    match cost.0.as_slice() {
        [
            CostItem::Mana(mana),
            sacrifice @ CostItem::Sacrifice {
                count: Amount::Fixed(1),
                ..
            },
        ] if !mana.has_x() => {
            let sacrificed = costs::text(&Cost(vec![sacrifice.clone()]))?;
            Some(format!("pay {mana} and {}", lower(&sacrificed)?))
        }
        [CostItem::ReturnToHand { .. }] => lower(&costs::text(cost)?),
        // The counters come off the card itself: "remove a +1/+1 counter from it".
        [CostItem::RemoveCounters { .. }] => {
            Some(lower(&costs::text(cost)?)?.replace(" from ~", " from it"))
        }
        _ => None,
    }
}

fn lower(text: &str) -> Option<String> {
    let mut chars = text.chars();
    let first = chars.next()?;
    Some(format!("{}{}", first.to_lowercase(), chars.as_str()))
}
