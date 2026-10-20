//! `Effect::DelayedTrigger`: a delayed triggered ability is set up (CR 603.7).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run, spell_ability};
use crate::snapshot::snapshot;
use crate::triggers::{DelayedCreated, DelayedId};

/// Sets the delayed trigger up with the names this resolution has bound so far, as they are now; it triggers on the
/// next event its pattern describes (`docs/cards-dsl.md`, Triggers).
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::DelayedTrigger {
        when,
        once,
        effects,
    } = effect
    else {
        return Err(wrong_effect("DelayedTrigger"));
    };
    let state = cx.state();
    let source = cx.scope.source;
    let gone = || ResolveError::Invalid("the source of a delayed trigger is gone".into());
    // The source as it is, else as the resolution knows it (it may have left the battlefield since it was chosen).
    let snapshot = snapshot(state, source.id)
        .filter(|shot| shot.counter == source.counter)
        .or_else(|| {
            cx.scope
                .snapshots
                .iter()
                .find(|shot| shot.reference() == source)
                .map(|shot| (*shot).clone())
        })
        .ok_or_else(gone)?;
    let object = state.object(cx.resolving.object()).ok_or_else(gone)?;
    // The ability whose resolution this is: the one an ability object came from, else the spell ability of the card.
    let ability = object.ability().map_or_else(
        || spell_ability(state, object.id()).map_or(0, |(address, _)| address.ability),
        crate::abilities::AbilityObject::index,
    );
    let bindings = cx
        .resolving
        .bindings()
        .iter()
        .map(|(name, targets)| (name.clone(), targets.clone()))
        .collect();
    Ok(Run::events(vec![Event::DelayedTriggerCreated(Box::new(
        DelayedCreated {
            id: DelayedId(state.next_delayed),
            source,
            snapshot,
            controller: cx.controller(),
            ability,
            when: when.clone(),
            once: *once,
            effects: effects.clone(),
            bindings,
        },
    ))]))
}
