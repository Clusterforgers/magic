//! Abilities that trigger on another triggered ability triggering (CR 603.2): "whenever a permanent entering causes a
//! triggered ability to trigger". A pending trigger keeps the event it triggered on (`Cause::event`); its
//! `AbilityTriggered` event is read as `Happened::R111AbilityTriggered`, and the event is judged again by the watcher's
//! `cause` pattern.

use mtg_cards::trigger::TriggerPattern;

use super::{Happened, matching};
use crate::abilities::{Cause, R111CauseEvent};
use crate::choose::Target;
use crate::events::Event;
use crate::resolve::Scope;

/// The event a pending trigger keeps from what it triggered on, for the kinds that "causes a triggered ability to
/// trigger" names; none for the rest.
pub(super) const fn r111_event(happened: &Happened) -> Option<R111CauseEvent> {
    match *happened {
        Happened::Moved {
            object, from, to, ..
        } => Some(R111CauseEvent::Moved { object, from, to }),
        Happened::Created { object, .. } => Some(R111CauseEvent::Created { object }),
        Happened::Attacked { attacker, at, .. } => Some(R111CauseEvent::Attacked { attacker, at }),
        _ => None,
    }
}

/// What a pending trigger's `AbilityTriggered` event is, as the patterns see it.
pub(super) fn r111_happened(event: &Event) -> Option<Happened> {
    let Event::AbilityTriggered(triggered) = event else {
        return None;
    };
    // A delayed trigger is set up by an ability, not by an object's own ability (CR 603.7).
    // An ability that triggered on another one triggering keeps that one's event, but the event did not make it trigger.
    if triggered.delayed.is_some() || triggered.cause.ability.is_some() {
        return None;
    }
    Some(Happened::R111AbilityTriggered {
        ability: crate::abilities::AbilityRef {
            object: triggered.source.id,
            ability: triggered.ability,
            source: triggered.origin,
        },
        event: triggered.cause.event?,
    })
}

/// The event a kept one stands for, to be judged again by a pattern.
const fn r111_replayed(event: R111CauseEvent) -> Happened {
    match event {
        R111CauseEvent::Moved { object, from, to } => Happened::Moved {
            object,
            from,
            to,
            cause: None,
        },
        R111CauseEvent::Created { object } => Happened::Created { object, nth: 0 },
        R111CauseEvent::Attacked { attacker, at } => Happened::Attacked {
            attacker,
            at,
            nth: 0,
        },
    }
}

/// Whether `pattern` waits for the triggering `happened` is: the event that made the ability trigger matches `cause`,
/// and with `of_it` the ability is one of the object the event is about.
pub(super) fn r111_matching(
    pattern: &TriggerPattern,
    happened: &Happened,
    scope: &Scope<'_>,
) -> Option<Cause> {
    let (
        TriggerPattern::R111AbilityTriggered { cause, of_it },
        Happened::R111AbilityTriggered { ability, event },
    ) = (pattern, happened)
    else {
        return None;
    };
    let found = matching(cause, &r111_replayed(*event), scope)?;
    let about = |id| {
        found
            .subjects
            .iter()
            .any(|subject| matches!(subject, Target::Object(object) if object.id == id))
    };
    if *of_it && !about(ability.object) {
        return None;
    }
    Some(Cause {
        event: Some(*event),
        ability: Some(*ability),
        ..found
    })
}
