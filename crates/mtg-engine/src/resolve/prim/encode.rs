//! `Effect::Encode`: the card of the resolving spell is exiled encoded on a creature (cipher, CR 702.99a).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::characteristics::seen_as;
use crate::events::{Event, ZoneCause};
use crate::object::ObjectKind;
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::zones::Zone;

/// The spell card the effect names moves from the stack to exile with `ZoneCause::Encoded` naming the first creature of
/// `on` that its controller controls on the battlefield, face up, and the fold links them. It happens as the spell
/// resolves: the last step leaves the stack (`resolve::finish` then gives priority and does not move the card again).
/// A spell that is not the one resolving, a copy of a spell (no card), a creature that left the battlefield or is no
/// creature any more, and one the controller no longer controls pass the step with nothing done, so the card goes to the
/// graveyard as an instant or sorcery does.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Encode { what, on } = effect else {
        return Err(wrong_effect("Encode"));
    };
    let state = cx.state();
    let spell = cx.scope.live_objects(what)?.into_iter().find(|card| {
        card.id() == cx.resolving.object()
            && card.zone() == Zone::Stack
            && card.kind() == ObjectKind::Card
    });
    let creature = cx.scope.live_objects(on)?.into_iter().find(|creature| {
        creature.zone() == Zone::Battlefield
            && seen_as(state, creature.id()).is_some_and(|(chars, controller)| {
                chars.is_creature() && controller == cx.controller()
            })
    });
    let events = match (spell, creature) {
        (Some(spell), Some(creature)) => vec![Event::ZoneChanged {
            object: spell.id(),
            from: Zone::Stack,
            to: Zone::Exile,
            cause: ZoneCause::Encoded {
                by: creature.reference(),
            },
        }],
        _ => Vec::new(),
    };
    Ok(Run::Done {
        events,
        bound: Vec::new(),
        snapshots: Vec::new(),
        next: Next::Sibling,
    })
}
