//! `Effect::Enlist`: the source creature enlists creatures (CR 702.154a).

use mtg_cards::effect::Effect;
use mtg_cards::refs::Ref;

use super::explore::actors;
use super::wrong_effect;
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::zones::Zone;

/// Each creature `what` names that is on the battlefield and untapped becomes tapped, and the source enlists it: one
/// `Enlisted` follows each tap. A creature already tapped or gone is not enlisted.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Enlist { what } = effect else {
        return Err(wrong_effect("Enlist"));
    };
    let sources = actors(cx, &Ref::This)?;
    let mut events = Vec::new();
    for object in cx.scope.live_objects(what)? {
        let untapped = object.zone() == Zone::Battlefield
            && object
                .permanent()
                .is_some_and(|permanent| !permanent.tapped());
        if !untapped {
            continue;
        }
        events.push(Event::PermanentTapped {
            object: object.id(),
        });
        events.extend(sources.iter().map(|source| Event::Enlisted {
            object: source.object,
            enlisted: object.id(),
            player: source.player,
        }));
    }
    Ok(Run::events(events))
}
