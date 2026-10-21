//! Folding abilities on the stack and the journal of a resolution.

use mtg_cards::ability::{Ability, Limit};
use mtg_core::ids::ObjectId;

use super::zones::move_object;
use super::{invalid, priority_to_active, reset_passes};
use crate::abilities::{AbilityObject, AbilityRef, PutOnStack};
use crate::ability_reader;
use crate::choose::Target;
use crate::error::EngineError;
use crate::events::{Event, ResolutionOutcome};
use crate::object::{GameObject, ObjectKind};
use crate::resolve::{Bindings, Resolving};
use crate::state::GameState;
use crate::zones::{Placement, Zone};

pub(super) fn apply(state: &mut GameState, event: &Event) -> Result<(), EngineError> {
    match event {
        Event::AbilityPutOnStack(put) => put_on_stack(state, put),
        Event::AbilityActivated {
            source,
            ability,
            origin,
            ..
        } => {
            if state.source_object(source.id).is_none() {
                return Err(invalid(format!(
                    "{source:?} activated an ability and is gone"
                )));
            }
            let address = AbilityRef {
                object: source.id,
                ability: *ability,
                source: *origin,
            };
            let rules = ability_reader::ability(state, address)
                .ok_or_else(|| invalid(format!("{source:?} has no ability {ability}")))?;
            let once = matches!(
                &*rules,
                Ability::Activated {
                    limit: Some(Limit::OncePerTurn),
                    ..
                }
            );
            if once && !state.limits_spent.insert((*source, *origin, *ability)) {
                return Err(invalid(format!(
                    "ability {ability} of {source:?} was already used this turn"
                )));
            }
            // A planeswalker has one loyalty ability activated each turn, whichever it is (CR 606.3).
            if matches!(&*rules, Ability::Loyalty { .. }) && !state.loyalty_used.insert(*source) {
                return Err(invalid(format!(
                    "a loyalty ability of {source:?} was already activated this turn"
                )));
            }
            Ok(())
        }
        Event::ResolutionBegan { object } => begin(state, *object),
        Event::ChoiceMade { player, choice } => {
            resolving_mut(state)?
                .journal
                .push((*player, choice.clone()));
            Ok(())
        }
        Event::StepDone {
            position,
            bound,
            snapshots,
        } => {
            let resolving = resolving_mut(state)?;
            resolving.position.clone_from(position);
            for (name, targets) in bound {
                if let Some(old) = resolving.bindings.insert(name.clone(), targets.clone())
                    && !renews(&old, targets)
                {
                    return Err(invalid(format!("`{name}` was bound twice")));
                }
            }
            resolving.snapshots.extend(snapshots.iter().cloned());
            resolving.journal.clear();
            Ok(())
        }
        Event::ResolutionEnded { object, outcome } => end(state, *object, *outcome),
        other => Err(invalid(format!("not a resolution event: {other:?}"))),
    }
}

/// Whether `new` names the same things as `old` with the objects that have moved on since: the only way a name is bound
/// again, when the step that moved its cards onto the battlefield names the new objects (CR 400.7).
fn renews(old: &[Target], new: &[Target]) -> bool {
    old.len() == new.len()
        && old != new
        && old.iter().zip(new).all(|pair| match pair {
            (Target::Object(old), Target::Object(new)) => {
                old.id == new.id && old.counter <= new.counter
            }
            (old, new) => old == new,
        })
}

fn resolving_mut(state: &mut GameState) -> Result<&mut Resolving, EngineError> {
    state
        .resolving
        .as_mut()
        .ok_or_else(|| invalid("nothing is resolving"))
}

fn put_on_stack(state: &mut GameState, put: &PutOnStack) -> Result<(), EngineError> {
    let id = put.object;
    if state.objects.contains_key(&id) {
        return Err(invalid(format!("{id:?} already exists")));
    }
    let source = put.source;
    // A triggered ability leaves the queue as it goes on the stack and takes what waited with it: the rules of a
    // delayed trigger and the names it captured.
    let (rules, bindings) = if put.cause.is_some() {
        let waiting = crate::triggers::take(state, put)?;
        (waiting.rules, waiting.bindings)
    } else {
        if state.source_object(source.id).is_none() {
            return Err(invalid(format!(
                "the source {source:?} of an ability is gone"
            )));
        }
        let address = AbilityRef {
            object: source.id,
            ability: put.ability,
            source: put.origin,
        };
        let rules = ability_reader::ability(state, address)
            .ok_or_else(|| invalid(format!("{source:?} has no ability {}", put.ability)))?;
        (rules, Bindings::new())
    };
    let card = state
        .source_object(source.id)
        .ok_or_else(|| invalid(format!("the source {source:?} of an ability is gone")))?;
    let object = GameObject::on_stack(
        id,
        put.controller,
        card,
        put.choices.clone(),
        AbilityObject {
            source,
            snapshot: put.snapshot.clone(),
            index: put.ability,
            origin: put.origin,
            rules,
            cause: put.cause.clone(),
            delayed: put.delayed,
            bindings,
        },
    );
    state.objects.insert(id, object);
    state.zones.insert(Zone::Stack, id, Placement::Top);
    state.next_object = state.next_object.max(id.get() + 1);
    reset_passes(state);
    Ok(())
}

fn begin(state: &mut GameState, id: ObjectId) -> Result<(), EngineError> {
    if state.resolving.is_some() {
        return Err(invalid("another spell or ability is already resolving"));
    }
    if state.zones.get(Zone::Stack).last() != Some(&id) {
        return Err(invalid(format!("{id:?} is not the top of the stack")));
    }
    let mut resolving = Resolving::begin(id);
    // A delayed trigger starts with the names the resolution that set it up had bound.
    if let Some(ability) = state.objects.get(&id).and_then(|o| o.ability()) {
        resolving.bindings.clone_from(ability.bindings());
    }
    state.resolving = Some(resolving);
    Ok(())
}

/// Takes the object off the stack. An ability ceases to exist (CR 113.7a); a spell that resolved is moved by
/// `SpellResolved`, which follows, and a spell that did not goes to its owner's graveyard.
fn end(state: &mut GameState, id: ObjectId, outcome: ResolutionOutcome) -> Result<(), EngineError> {
    let (kind, owner) = state
        .objects
        .get(&id)
        .map(|o| (o.kind, o.owner))
        .ok_or_else(|| invalid(format!("unknown object {id:?}")))?;
    if !state.zones.get(Zone::Stack).contains(&id) {
        return Err(invalid(format!("{id:?} is not on the stack")));
    }
    let resolving_it = state.resolving.as_ref().is_some_and(|r| r.object == id);
    if !resolving_it && outcome != ResolutionOutcome::Countered {
        return Err(invalid(format!("{id:?} is not resolving")));
    }
    if resolving_it {
        state.resolving = None;
    }
    match (kind, outcome) {
        (ObjectKind::Ability, _) => {
            state.objects.remove(&id);
            state.zones.remove(Zone::Stack, id);
        }
        (_, ResolutionOutcome::Resolved) => return Ok(()),
        (
            _,
            ResolutionOutcome::Fizzled | ResolutionOutcome::Countered | ResolutionOutcome::Lapsed,
        ) => {
            move_object(state, id, Zone::Graveyard(owner), Placement::Top)?;
        }
    }
    priority_to_active(state);
    Ok(())
}
