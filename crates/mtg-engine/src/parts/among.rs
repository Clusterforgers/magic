//! Removing counters in all from among the permanents the payer controls ("Remove three counters from among artifacts
//! you control"). The engine takes them from the permanents in play order, each as far as it holds the kind.

use mtg_cards::refs::Selector;
use mtg_cards::value::CounterKind;
use mtg_core::ids::{ObjectId, PlayerId};

use super::Payer;
use crate::characteristics::controller_of;
use crate::choose::Target;
use crate::counters::{CountersChanged, kinds_held};
use crate::events::Event;
use crate::state::GameState;
use crate::zones::Zone;

/// The permanents the payer controls that match.
fn holders(payer: &Payer<'_>, filter: &Selector) -> Vec<ObjectId> {
    payer.matching(
        payer.state.zone(Zone::Battlefield).iter().copied(),
        filter,
        |id| controller_of(payer.state, id) == Some(payer.player),
    )
}

/// The counters of the kind, or of any kind, that the permanent holds, kind by kind.
fn held(state: &GameState, on: Target, counter: Option<&CounterKind>) -> Vec<(CounterKind, u32)> {
    kinds_held(state, on)
        .into_iter()
        .filter(|(kind, _)| counter.is_none_or(|wanted| wanted == kind))
        .collect()
}

/// The permanent as a target of a counter change.
fn on(state: &GameState, id: ObjectId) -> Option<Target> {
    state
        .object(id)
        .map(|found| Target::Object(found.reference()))
}

/// How many counters the permanents hold that the cost may remove.
pub(super) fn most(payer: &Payer<'_>, filter: &Selector, counter: Option<&CounterKind>) -> u32 {
    holders(payer, filter)
        .into_iter()
        .filter_map(|id| on(payer.state, id))
        .flat_map(|target| held(payer.state, target, counter))
        .fold(0u32, |sum, (_, number)| sum.saturating_add(number))
}

/// Whether the permanents hold at least that many counters.
pub(super) fn payable(
    payer: &Payer<'_>,
    filter: &Selector,
    counter: Option<&CounterKind>,
    count: u32,
) -> bool {
    count == 0 || most(payer, filter, counter) >= count
}

/// The counters removed, permanent by permanent until `count` are gone.
pub(super) fn events(
    state: &GameState,
    (player, source): (PlayerId, ObjectId),
    filter: &Selector,
    counter: Option<&CounterKind>,
    count: u32,
) -> Vec<Event> {
    let owner = Payer {
        state,
        player,
        source,
        barred: &[],
        granter: None,
    };
    let mut left = count;
    let mut events = Vec::new();
    for id in holders(&owner, filter) {
        let Some(target) = on(state, id) else {
            continue;
        };
        for (kind, number) in held(state, target, counter) {
            let take = number.min(left);
            if let (true, Ok(delta)) = (take > 0, i32::try_from(take)) {
                events.push(Event::CountersChanged(CountersChanged {
                    on: target,
                    counter: kind,
                    delta: -delta,
                }));
                left -= take;
            }
        }
    }
    events
}
