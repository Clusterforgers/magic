//! "Sacrifice all permanents you control" as a cost: every permanent the payer controls that matches goes, with nothing
//! to choose; the cost can't be paid while one of them can't be sacrificed (CR 601.2h).

use mtg_cards::cost::CostItem;
use mtg_cards::refs::{Pred, Ref, Selector};
use mtg_cards::value::Amount;
use mtg_core::ids::ObjectId;

use super::{Payer, sacrifice};
use crate::events::Event;
use crate::state::GameState;

/// The filter of a sacrifice whose number is every permanent of the filter the payer controls, `None` for any other item.
pub(super) fn r303c_reads(item: &CostItem) -> Option<Selector> {
    let CostItem::Sacrifice {
        filter,
        count: Amount::Count(counted),
    } = item
    else {
        return None;
    };
    let mut yours = filter.0.clone();
    yours.push(Pred::ControlledBy(Ref::You));
    (counted.0 == yours).then(|| filter.clone())
}

/// Every permanent the payer controls that matches, or `None` when one of them can't be sacrificed.
pub(super) fn r303c_all(payer: &Payer<'_>, filter: &Selector) -> Option<Vec<ObjectId>> {
    let found = sacrifice::candidates(payer, filter);
    found
        .iter()
        .all(|id| {
            crate::prohibit::may_sacrifice(payer.state, payer.player, *id)
                && !crate::r4707_pay_costs::r4707_sacrifice_barred(
                    payer.state,
                    payer.player,
                    *id,
                    payer.pays,
                )
        })
        .then_some(found)
}

/// The sacrifice of each of them.
pub(super) fn r303c_events(payer: &Payer<'_>, filter: &Selector) -> Vec<Event> {
    sacrifice::candidates(payer, filter)
        .into_iter()
        .filter_map(|id| sacrifice::event(payer.state, id))
        .collect()
}

/// The payer of a cost with nothing barred.
pub(super) const fn r303c_payer(
    state: &GameState,
    player: mtg_core::ids::PlayerId,
    source: ObjectId,
) -> Payer<'_> {
    Payer {
        state,
        player,
        source,
        barred: &[],
        granter: None,
        pays: crate::r4707_pay_costs::PaysFor::Cost,
    }
}
