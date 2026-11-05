//! `Effect::Reflexive`: a reflexive triggered ability ("when you do", CR 603.12) triggers and waits for the stack.

use mtg_cards::effect::Effect;
use mtg_cards::trigger::TriggerPattern;

use super::delayed_trigger::{Origin, origin};
use super::may::performed_name;
use super::wrong_effect;
use crate::abilities::{AbilitySource, Cause};
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::triggers::{DelayedCreated, DelayedId, Triggered};

/// When the step before it was carried out (a step that reports nothing counts as done), the trigger is set up and
/// triggers at once; it goes on the stack, with its targets chosen then, the next time a player would receive priority,
/// and is removed when a target it needs has no legal choice (CR 603.3d). It sees the names this resolution bound.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Reflexive { targets, effects } = effect else {
        return Err(wrong_effect("Reflexive"));
    };
    if !before_was_done(cx) {
        return Ok(Run::events(Vec::new()));
    }
    let Origin {
        source,
        snapshot,
        ability,
        bindings,
    } = origin(cx)?;
    let id = DelayedId(cx.state().next_delayed);
    let created = DelayedCreated {
        id,
        source,
        snapshot: snapshot.clone(),
        controller: cx.controller(),
        ability,
        // It triggers below, not on an event: this pattern never matches.
        when: TriggerPattern::AnyOf(Vec::new()),
        once: true,
        this_turn: true,
        effects: effects.clone(),
        targets: targets.clone(),
        bindings,
    };
    Ok(Run::events(vec![
        Event::DelayedTriggerCreated(Box::new(created)),
        Event::AbilityTriggered(Box::new(Triggered {
            source,
            ability,
            origin: AbilitySource::Printed,
            controller: cx.controller(),
            // What the earlier steps took away (a sacrificed creature) stays readable as it was (CR 608.2h).
            cause: Cause {
                snapshots: cx.resolving.snapshots().to_vec(),
                ..Cause::default()
            },
            snapshot,
            delayed: Some(id),
        })),
    ]))
}

/// Whether the step before this one did what it says: a `May` that was accepted and whose own steps took something,
/// a sacrifice or a removal of counters that took something. A step that reports nothing, or no step before, counts as
/// done.
fn before_was_done(cx: &Cx<'_>) -> bool {
    let position = cx.resolving.position();
    let Some((last, holder)) = position.split_last() else {
        return true;
    };
    let Some(before) = last.checked_sub(1) else {
        return true;
    };
    let before = [holder, &[before]].concat();
    let bindings = &cx.scope.bindings;
    let name = performed_name(&before);
    if bindings.get(&name).is_some_and(Vec::is_empty) {
        return false;
    }
    // The steps directly inside a `May` report on their own: one that did nothing leaves nothing to follow up.
    let inside = format!("{name}.0.");
    !bindings.iter().any(|(bound, who)| {
        who.is_empty()
            && bound
                .strip_prefix(&inside)
                .is_some_and(|rest| !rest.contains('.'))
    })
}
