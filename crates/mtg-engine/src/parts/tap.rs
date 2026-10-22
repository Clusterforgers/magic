//! Tapping untapped permanents the payer controls as a cost ("Tap an untapped creature you control"). Summoning
//! sickness does not matter: it holds back the `{T}` of the creature itself, not the tapping of another (CR 302.6).

use mtg_cards::refs::Selector;
use mtg_core::ids::ObjectId;

use super::Payer;
use crate::characteristics::{characteristics, controller_of};
use crate::events::Event;
use crate::state::GameState;
use crate::zones::Zone;

/// The untapped permanents the payer controls that match.
pub(super) fn candidates(payer: &Payer<'_>, filter: &Selector) -> Vec<ObjectId> {
    payer.matching(
        payer.state.zone(Zone::Battlefield).iter().copied(),
        filter,
        |id| {
            controller_of(payer.state, id) == Some(payer.player)
                && payer
                    .state
                    .object(id)
                    .and_then(|object| object.permanent())
                    .is_some_and(|permanent| !permanent.tapped())
        },
    )
}

/// The permanent becomes tapped.
pub(super) const fn event(object: ObjectId) -> Event {
    Event::PermanentTapped { object }
}

/// "Tap an untapped creature you control.", "Tap 2 untapped creatures you control."
pub(super) fn text(filter: &Selector, count: usize) -> String {
    crate::wording::tap_untapped(filter, count)
}

/// The power of a permanent as it stands, 0 for no power or a negative one (CR 702.122a counts what is there).
pub(super) fn power_of(state: &GameState, object: ObjectId) -> u32 {
    characteristics(state, object)
        .and_then(|found| found.power)
        .map_or(0, |power| u32::try_from(power).unwrap_or(0))
}

/// The power of all of `objects` together.
pub(super) fn total_power(state: &GameState, objects: &[ObjectId]) -> u32 {
    objects
        .iter()
        .map(|object| power_of(state, *object))
        .fold(0, u32::saturating_add)
}

/// Enough of `from` for `power` as far as it goes, the ones in `spare` last and the strongest first among the rest.
pub(super) fn enough(
    state: &GameState,
    mut from: Vec<ObjectId>,
    power: u32,
    spare: &[ObjectId],
) -> Option<Vec<ObjectId>> {
    from.sort_by_key(|id| (spare.contains(id), std::cmp::Reverse(power_of(state, *id))));
    let mut taken = Vec::new();
    let mut total = 0u32;
    for id in from {
        if total >= power {
            break;
        }
        total = total.saturating_add(power_of(state, id));
        taken.push(id);
    }
    (total >= power).then_some(taken)
}

/// "Tap any number of untapped creatures you control with total power 3 or more."
pub(super) fn power_text(filter: &Selector, power: usize) -> String {
    crate::wording::tap_power(filter, power)
}
