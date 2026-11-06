//! The payments of a "you may pay" or an "unless" that have more than one part or return a permanent: "pay {2} and
//! sacrifice a token", "return an untapped Forest you control to its owner's hand".

use super::costs;
use crate::cost::{Cost, CostItem};
use crate::refs::{Pred, Ref, Selector, ZoneKind};
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
        // "draw a card", "draw two cards" (r5b).
        [CostItem::Draw(count @ Amount::Fixed(1..))] => {
            Some(format!("draw {}", super::amounts::cards_text(count)?))
        }
        // "exile it": the card of the source, just put into a graveyard (r5b).
        [
            CostItem::ExileFromGraveyard {
                filter,
                any_player: true,
                count,
            },
        ] if count.is_one() && *filter == arrived_self() => Some("exile it".to_owned()),
        // "exile two cards from your graveyard", "tap two untapped creatures you control", "discard a creature card",
        // "mill two cards", "exile a red card from your hand", "reveal a Dragon card from your hand" (r5b).
        [
            CostItem::ExileFromGraveyard {
                any_player: false, ..
            }
            | CostItem::TapPermanents { .. }
            | CostItem::Discard {
                filter: Some(_), ..
            }
            | CostItem::Mill(Amount::Fixed(1..))
            | CostItem::ExileFromHand { .. }
            | CostItem::RevealFromHand { .. },
        ] => lower(&costs::text(cost)?),
        _ => None,
    }
}

/// The card of the source as it arrived in a graveyard (CR 400.7): "it" in "When ~ dies, you may exile it".
fn arrived_self() -> Selector {
    Selector(vec![Pred::Is(Ref::Arrived {
        of: Box::new(Ref::This),
        zone: ZoneKind::Graveyard,
    })])
}

fn lower(text: &str) -> Option<String> {
    let mut chars = text.chars();
    let first = chars.next()?;
    Some(format!("{}{}", first.to_lowercase(), chars.as_str()))
}
