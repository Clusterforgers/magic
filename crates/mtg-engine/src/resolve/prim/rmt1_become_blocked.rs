//! `Effect::BecomeBlocked`: attacking creatures become blocked with no creature blocking them (CR 509.1h).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::zones::Zone;

/// Each creature the effect names that is still the object named, on the battlefield and attacking but not yet
/// blocked becomes blocked; one event names them all. A creature that is not attacking or already blocked, an object
/// that changed zones since it was named (CR 400.7) and a name that repeats are passed over.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::BecomeBlocked { what } = effect else {
        return Err(wrong_effect("BecomeBlocked"));
    };
    let combat = &cx.state().combat;
    let mut attackers = Vec::new();
    for object in cx.scope.live_objects(what)? {
        let unblocked = combat
            .attackers()
            .iter()
            .any(|attacker| attacker.object() == object.id() && !attacker.blocked());
        if object.zone() == Zone::Battlefield && unblocked && !attackers.contains(&object.id()) {
            attackers.push(object.id());
        }
    }
    let events = if attackers.is_empty() {
        Vec::new()
    } else {
        vec![Event::AttackersBlocked { attackers }]
    };
    Ok(Run::events(events))
}
