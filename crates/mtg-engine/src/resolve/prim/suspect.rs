//! `Effect::Suspect` and `Effect::Unsuspect`: creatures become suspected or stop being so (CR 701.60).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::characteristics::characteristics;
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::zones::Zone;

/// Each permanent the effect names that is still on the battlefield becomes suspected (a creature that is not yet) or
/// stops being so (one that is), in the order the reference lists them. Anything else is passed over: a player, an
/// object in another zone or that changed zones since it was named, and a name that repeats.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let (what, suspect) = match effect {
        Effect::Suspect { what } => (what, true),
        Effect::Unsuspect { what } => (what, false),
        _ => return Err(wrong_effect("Suspect")),
    };
    let state = cx.scope.state;
    let events = cx
        .scope
        .live_objects(what)?
        .into_iter()
        .filter(|object| {
            object.zone() == Zone::Battlefield
                && object
                    .permanent()
                    .is_some_and(|held| held.suspected() != suspect)
                && (!suspect
                    || characteristics(state, object.id()).is_some_and(|chars| chars.is_creature()))
        })
        .map(|object| {
            let object = object.id();
            if suspect {
                Event::Suspected { object }
            } else {
                Event::Unsuspected { object }
            }
        })
        .collect();
    Ok(Run::events(events))
}
