//! `Effect::DelayedTrigger`: a delayed triggered ability is set up (CR 603.7).

use mtg_cards::effect::Effect;
use mtg_cards::value::Amount;

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
        this_turn,
        effects,
    } = effect
    else {
        return Err(wrong_effect("DelayedTrigger"));
    };
    let state = cx.state();
    let source = cx.scope.source;
    let gone = || ResolveError::Invalid("the source of a delayed trigger is gone".into());
    // The source as it is, else as the resolution knows it (it may have left the battlefield since it was chosen).
    let mut snapshot = snapshot(state, source.id)
        .filter(|shot| shot.counter == source.counter)
        .or_else(|| {
            cx.scope
                .snapshots
                .iter()
                .find(|shot| shot.reference() == source)
                .map(|shot| (*shot).clone())
        })
        .ok_or_else(gone)?;
    // It reads the X of the spell or ability that sets it up, whatever its source remembers (CR 107.3k).
    snapshot.x = cx.scope.announced_x();
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
            this_turn: *this_turn,
            effects: fixed_target_stats(cx, effects)?,
            bindings,
        },
    ))]))
}

/// The delayed effects with every stat of a target of this resolution read now (CR 608.2h): the delayed ability has no
/// targets of its own, and "that spell's mana value" is the last known value of a spell this resolution countered.
fn fixed_target_stats(cx: &Cx<'_>, effects: &[Effect]) -> Result<Vec<Effect>, ResolveError> {
    let bad = |error: serde_json::Error| ResolveError::Invalid(format!("delayed effects: {error}"));
    let mut tree = serde_json::to_value(effects).map_err(bad)?;
    fix_stats(cx, &mut tree)?;
    serde_json::from_value(tree).map_err(bad)
}

/// Replaces each `{"Stat":{"of":{"Target":i},..}}` under `tree` with the number it reads now.
fn fix_stats(cx: &Cx<'_>, tree: &mut serde_json::Value) -> Result<(), ResolveError> {
    let is_target_stat = tree
        .get("Stat")
        .and_then(|stat| stat.get("of"))
        .is_some_and(|of| of.get("Target").is_some());
    if is_target_stat {
        let amount: Amount = serde_json::from_value(tree.clone())
            .map_err(|error| ResolveError::Invalid(format!("delayed stat: {error}")))?;
        *tree = serde_json::Value::from(cx.scope.amount(&amount)?);
        return Ok(());
    }
    match tree {
        serde_json::Value::Array(items) => {
            items.iter_mut().try_for_each(|item| fix_stats(cx, item))
        }
        serde_json::Value::Object(fields) => fields
            .values_mut()
            .try_for_each(|field| fix_stats(cx, field)),
        _ => Ok(()),
    }
}
