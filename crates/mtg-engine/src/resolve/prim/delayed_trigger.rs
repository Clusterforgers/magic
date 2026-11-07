//! `Effect::DelayedTrigger`: a delayed triggered ability is set up (CR 603.7).

use mtg_cards::effect::Effect;
use mtg_cards::refs::Ref;
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::Amount;

use super::wrong_effect;
use crate::choose::Target;
use crate::events::Event;
use crate::object::ObjectRef;
use crate::resolve::{Cx, ResolveError, Run, spell_ability};
use crate::snapshot::{Snapshot, snapshot};
use crate::triggers::{DelayedCreated, DelayedId};

/// What a trigger set up by a resolution remembers of it.
pub(super) struct Origin {
    pub(super) source: ObjectRef,
    pub(super) snapshot: Snapshot,
    pub(super) ability: usize,
    pub(super) bindings: Vec<(String, Vec<Target>)>,
}

/// The source as it is (else as the resolution knows it), the ability whose resolution this is and the names bound so far.
pub(super) fn origin(cx: &Cx<'_>) -> Result<Origin, ResolveError> {
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
    // What a step did belongs to the resolution that ran it: the delayed trigger reads its own steps by position.
    let mut bindings: Vec<(String, Vec<Target>)> = cx
        .resolving
        .bindings()
        .iter()
        .filter(|(name, _)| !name.starts_with("?performed"))
        .map(|(name, targets)| (name.clone(), targets.clone()))
        .collect();
    // The numbers `Let` kept travel as names too, so the trigger reads them as the setup fixed them.
    if let Some(resolving) = state.resolving.as_ref() {
        bindings.extend(
            resolving.lets.iter().map(|(name, value)| {
                (format!("{}{value}:{name}", crate::resolve::LET), Vec::new())
            }),
        );
    }
    Ok(Origin {
        source,
        snapshot,
        ability,
        bindings,
    })
}

/// Sets the delayed trigger up with the names this resolution has bound so far, as they are now; it triggers on the
/// next event its pattern describes (`docs/cards-dsl.md`, Triggers, replacement and layers).
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
    let Origin {
        source,
        snapshot,
        ability,
        mut bindings,
    } = origin(cx)?;
    let when = name_targets(cx, when, &mut bindings)?;
    let state = cx.state();
    Ok(Run::events(vec![Event::DelayedTriggerCreated(Box::new(
        DelayedCreated {
            id: DelayedId(state.next_delayed),
            source,
            snapshot,
            controller: cx.controller(),
            ability,
            when,
            once: *once,
            this_turn: *this_turn,
            effects: fixed_target_stats(cx, effects)?,
            targets: Vec::new(),
            bindings,
        },
    ))]))
}

/// The event pattern with each `Target` it names replaced by a bound name that holds the targets as chosen now: the delayed
/// ability has no targets of its own, and "when that creature dies" must keep the creature the spell chose.
fn name_targets(
    cx: &Cx<'_>,
    when: &TriggerPattern,
    bindings: &mut Vec<(String, Vec<Target>)>,
) -> Result<TriggerPattern, ResolveError> {
    let bad = |error: serde_json::Error| ResolveError::Invalid(format!("delayed event: {error}"));
    let mut tree = serde_json::to_value(when).map_err(bad)?;
    let mut named = Vec::new();
    rename_targets(&mut tree, &mut named);
    for index in named {
        let chosen = cx.scope.refs(&Ref::Target(index))?;
        bindings.push((target_name(index), chosen));
    }
    serde_json::from_value(tree).map_err(bad)
}

fn target_name(index: u8) -> String {
    format!("?target{index}")
}

fn rename_targets(tree: &mut serde_json::Value, named: &mut Vec<u8>) {
    match tree {
        serde_json::Value::Object(fields) => {
            let index = (fields.len() == 1)
                .then(|| fields.get("Target").and_then(serde_json::Value::as_u64))
                .flatten()
                .and_then(|index| u8::try_from(index).ok());
            if let Some(index) = index {
                if !named.contains(&index) {
                    named.push(index);
                }
                *tree = serde_json::json!({ "Bound": target_name(index) });
            } else {
                fields
                    .values_mut()
                    .for_each(|field| rename_targets(field, named));
            }
        }
        serde_json::Value::Array(items) => items
            .iter_mut()
            .for_each(|item| rename_targets(item, named)),
        _ => {}
    }
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
