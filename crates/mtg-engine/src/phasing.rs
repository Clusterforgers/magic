//! Phasing (CR 702.26): a permanent that phases out is treated as though it does not exist until it phases in during
//! its controller's untap step, as the same object with everything it had.

use std::collections::BTreeSet;

use mtg_core::ids::{ObjectId, PlayerId};

use crate::characteristics::controller_of;
use crate::error::EngineError;
use crate::events::Event;
use crate::fold::invalid;
use crate::object::{GameObject, PhasedState, Phasing};
use crate::state::GameState;
use crate::zones::{Placement, Zone};

/// Whether `id` is a permanent that is phased out.
pub fn is_phased_out(state: &GameState, id: ObjectId) -> bool {
    state
        .object(id)
        .is_some_and(|object| object.zone() == Zone::PhasedOut)
}

/// The permanents in `zone` attached to `host`, as the object they were attached to, in id order.
fn attached_in(state: &GameState, zone: Zone, host: ObjectId) -> Vec<ObjectId> {
    let Some(host) = state.object(host).map(GameObject::reference) else {
        return Vec::new();
    };
    let mut found: Vec<ObjectId> = state
        .zone(zone)
        .iter()
        .copied()
        .filter(|id| {
            state
                .object(*id)
                .and_then(GameObject::permanent)
                .and_then(|permanent| permanent.attached_to())
                .is_some_and(|attached| attached == host)
        })
        .collect();
    found.sort();
    found
}

/// The events that phase out the permanents `named` (CR 702.26): each one on the battlefield once, in order, and after
/// it the Auras, Equipment and Fortifications attached to it, which phase out with it (CR 702.26g). An object `named`
/// that is gone, not a permanent or phased out already is passed over.
pub(crate) fn out_events(state: &GameState, named: &[ObjectId]) -> Vec<Event> {
    let direct: BTreeSet<ObjectId> = named
        .iter()
        .copied()
        .filter(|id| {
            state
                .object(*id)
                .is_some_and(|o| o.zone() == Zone::Battlefield)
        })
        .collect();
    let mut done = BTreeSet::new();
    let mut events = Vec::new();
    let mut work: Vec<(ObjectId, bool)> = Vec::new();
    for id in named {
        if direct.contains(id) && done.insert(*id) {
            work.push((*id, false));
        }
        while let Some((id, indirect)) = work.pop() {
            let Some(controller) = controller_of(state, id) else {
                continue;
            };
            events.push(Event::PermanentPhasedOut {
                object: id,
                controller,
                indirect,
            });
            for attached in attached_in(state, Zone::Battlefield, id).into_iter().rev() {
                // One named too phases out on its own and comes back on its own.
                if !direct.contains(&attached) && done.insert(attached) {
                    work.push((attached, true));
                }
            }
        }
    }
    events
}

/// The events that phase in what phases in as the untap step of `player` begins (CR 502.1, 702.26): the permanents that
/// were phased out as `player` controlled them, each followed by what phased out with it. An attachment whose host is no
/// longer phased out, because it left the game, phases in on its own.
pub(crate) fn in_events(state: &GameState, player: PlayerId) -> Vec<Event> {
    let phasing = |id: ObjectId| {
        state
            .object(id)
            .and_then(GameObject::permanent)
            .and_then(|permanent| permanent.phasing())
    };
    let phased = state.zone(Zone::PhasedOut);
    let hosts_waiting = |attachment: ObjectId| {
        state
            .object(attachment)
            .and_then(GameObject::permanent)
            .and_then(|permanent| permanent.attached_to())
            .is_some_and(|host| {
                state.object(host.id).is_some_and(|object| {
                    object.zone() == Zone::PhasedOut && object.counter() == host.counter
                })
            })
    };
    let mut events = Vec::new();
    for id in phased {
        let Some(own) = phasing(*id).filter(|own| own.controller == player) else {
            continue;
        };
        if own.indirect && hosts_waiting(*id) {
            continue;
        }
        let mut work = vec![*id];
        while let Some(id) = work.pop() {
            events.push(Event::PermanentPhasedIn { object: id });
            work.extend(
                attached_in(state, Zone::PhasedOut, id)
                    .into_iter()
                    .rev()
                    .filter(|attached| phasing(*attached).is_some_and(|own| own.indirect)),
            );
        }
    }
    events
}

/// A permanent phases out: it leaves the battlefield's list without leaving the battlefield, and leaves combat (CR 506.4).
pub(crate) fn fold_out(
    state: &mut GameState,
    id: ObjectId,
    controller: PlayerId,
    indirect: bool,
) -> Result<(), EngineError> {
    let object = state
        .objects
        .get_mut(&id)
        .filter(|object| object.zone == Zone::Battlefield)
        .ok_or_else(|| invalid(format!("{id:?} is not a permanent that is phased in")))?;
    let permanent = object
        .permanent
        .as_mut()
        .ok_or_else(|| invalid(format!("{id:?} is not a permanent")))?;
    permanent.phased = PhasedState(Some(Phasing {
        controller,
        indirect,
    }));
    object.zone = Zone::PhasedOut;
    state.zones.remove(Zone::Battlefield, id);
    state.zones.insert(Zone::PhasedOut, id, Placement::Top);
    state.combat.remove_object(id);
    state.combat.planeswalker_left(id);
    Ok(())
}

/// A permanent phases in: the same object, with the counters, damage and tapped state it had. It has been under its
/// controller's control all along, so a controller whose turn it is no longer holds it back for summoning sickness.
pub(crate) fn fold_in(state: &mut GameState, id: ObjectId) -> Result<(), EngineError> {
    let active = state.turn.active();
    let object = state
        .objects
        .get_mut(&id)
        .filter(|object| object.zone == Zone::PhasedOut)
        .ok_or_else(|| invalid(format!("{id:?} is not a permanent that is phased out")))?;
    let permanent = object
        .permanent
        .as_mut()
        .ok_or_else(|| invalid(format!("{id:?} is not a permanent")))?;
    let phasing = permanent.phased.0.take();
    if phasing.is_some_and(|phasing| phasing.controller == active) {
        permanent.new_to = permanent.sick_for(active).then_some(active);
        permanent.summoning_sick = false;
        permanent.observed = Some(active);
    }
    object.zone = Zone::Battlefield;
    state.zones.remove(Zone::PhasedOut, id);
    state.zones.insert(Zone::Battlefield, id, Placement::Top);
    Ok(())
}
