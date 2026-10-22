//! `Effect::ExileTop`: cards are exiled from the top of a library, face up, and named for the steps after it (CR 406.3).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::choose::Target;
use crate::events::{Event, ZoneCause};
use crate::library::top_cards;
use crate::object::ObjectRef;
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::zones::Zone;

/// Each player the effect names who is in the game exiles `count` cards from the top of their library, the top card
/// first, face up (exile is open to every seat). A library shorter than the count is exiled whole and an empty one
/// exiles nothing. The cards are bound to the name as the objects they become in exile (CR 400.7), whether or not
/// anything was exiled.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::ExileTop { who, count, bind } = effect else {
        return Err(wrong_effect("ExileTop"));
    };
    let wanted = usize::try_from(cx.scope.amount(count)?.max(0)).unwrap_or(usize::MAX);
    let state = cx.state();
    let mut events = Vec::new();
    let mut exiled = Vec::new();
    for owner in cx.scope.players(who)? {
        if !state.is_alive(owner) {
            continue;
        }
        for id in top_cards(state, owner, wanted) {
            let Some(card) = state.object(id) else {
                continue;
            };
            events.push(Event::ZoneChanged {
                object: id,
                from: Zone::Library(owner),
                to: Zone::Exile,
                cause: ZoneCause::Exiled,
            });
            exiled.push(Target::Object(ObjectRef {
                id,
                counter: card.reference().counter + 1,
            }));
        }
    }
    Ok(Run::Done {
        events,
        bound: vec![(bind.clone(), exiled)],
        snapshots: Vec::new(),
        next: Next::Sibling,
    })
}
