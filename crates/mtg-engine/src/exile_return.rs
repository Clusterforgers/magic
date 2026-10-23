//! Cards exiled until a permanent leaves the battlefield (CR 610.3): each returns to the battlefield under its owner's
//! control, as a new object, once that permanent has left.

use crate::attach;
use crate::events::{Event, ZoneCause};
use crate::loyalty;
use crate::object::{GameObject, ObjectRef};
use crate::saga;
use crate::state::GameState;
use crate::zones::Zone;

/// Whether `source` is still the permanent it names: on the battlefield and not a new object since (CR 400.7).
pub(crate) fn holds(state: &GameState, source: ObjectRef) -> bool {
    state.object(source.id).is_some_and(|object| {
        object.counter() == source.counter && object.zone() == Zone::Battlefield
    })
}

/// The events that return every card in exile whose permanent has left the battlefield, all together. The duration is
/// not a triggered ability (CR 610.3a): the control loop runs this before anything else is checked, so nothing happens
/// between the permanent leaving and the card returning. An Aura with nothing to enchant stays in exile (CR 303.4g).
pub(crate) fn returns(state: &GameState) -> Vec<Event> {
    if !state.possible.linked_exile {
        return Vec::new();
    }
    state
        .zone(Zone::Exile)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|card| card.exiled_until().is_some_and(|by| !holds(state, by)))
        .flat_map(|card| back(state, card))
        .collect()
}

/// The events of one card coming back: the move, what it enters with and, for an Aura, the permanent it enchants.
fn back(state: &GameState, card: &GameObject) -> Vec<Event> {
    let id = card.id();
    let host = match attach::hosts_when_entering(state, card, card.owner()) {
        None => None,
        Some(hosts) => match hosts.first() {
            Some(host) => Some(*host),
            None => return Vec::new(),
        },
    };
    let mut events = vec![Event::ZoneChanged {
        object: id,
        from: Zone::Exile,
        to: Zone::Battlefield,
        cause: ZoneCause::Moved,
    }];
    events.extend(loyalty::entering(state, id));
    events.extend(saga::entering(state, id));
    events.extend(host.map(|to| Event::Attached { object: id, to }));
    events
}
