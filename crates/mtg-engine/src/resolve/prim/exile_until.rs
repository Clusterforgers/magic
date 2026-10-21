//! `Effect::ExileUntil`: cards are exiled from the top of a library until one matches (CR 702.85a, 701.57a).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::choose::Target;
use crate::events::{Event, ZoneCause};
use crate::object::ObjectRef;
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::zones::Zone;

/// Each player the effect names who is in the game exiles cards from the top of their library, one at a time and face up
/// (exile is open to every seat), until one matches the filter or the library is empty. The exiling stops at the first
/// match, which stays in exile with the cards before it; a library with no match is exiled whole. The card found is
/// bound to `as` and every card exiled, the one found included, to `exiled_as`, each as the object it becomes in exile
/// (CR 400.7). Nothing is bound to `as` when nothing matched. With several players each exiles from their own library, in
/// turn order, and every card found is bound.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::ExileUntil {
        who,
        filter,
        bind,
        exiled_as,
    } = effect
    else {
        return Err(wrong_effect("ExileUntil"));
    };
    let state = cx.state();
    let mut events = Vec::new();
    let mut found = Vec::new();
    let mut exiled = Vec::new();
    for owner in cx.scope.players(who)? {
        if !state.is_alive(owner) {
            continue;
        }
        for &id in state.zone(Zone::Library(owner)).iter().rev() {
            let Some(card) = state.object(id) else {
                continue;
            };
            let reference = card.reference();
            events.push(Event::ZoneChanged {
                object: id,
                from: Zone::Library(owner),
                to: Zone::Exile,
                cause: ZoneCause::Exiled,
            });
            // The object the card becomes in exile is the one the later steps name.
            let arrived = Target::Object(ObjectRef {
                id,
                counter: reference.counter + 1,
            });
            exiled.push(arrived);
            if cx.scope.selects(filter, reference)? {
                found.push(arrived);
                break;
            }
        }
    }
    Ok(Run::Done {
        events,
        bound: vec![(bind.clone(), found), (exiled_as.clone(), exiled)],
        snapshots: Vec::new(),
        next: Next::Sibling,
    })
}
