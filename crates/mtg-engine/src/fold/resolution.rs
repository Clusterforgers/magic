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
use crate::state::{GameState, Tally};
use crate::zones::{Placement, Zone};

pub(super) fn apply(state: &mut GameState, event: &Event) -> Result<(), EngineError> {
    match event {
        Event::AbilityPutOnStack(put) => put_on_stack(state, put),
        Event::AbilityActivated {
            player,
            source,
            ability,
            origin,
        } => {
            let Some(object) = state.source_object(source.id) else {
                return Err(invalid(format!(
                    "{source:?} activated an ability and is gone"
                )));
            };
            if object.counter() != source.counter {
                // A cost moved the source (sacrificed it): the new object cannot use the ability again, and the ability
                // goes on the stack with the rules of the snapshot.
                return Ok(());
            }
            let address = AbilityRef {
                object: source.id,
                ability: *ability,
                source: *origin,
            };
            let rules = ability_reader::ability(state, address)
                .ok_or_else(|| invalid(format!("{source:?} has no ability {ability}")))?;
            if let Ability::Activated {
                limit: Some(limit), ..
            } = &*rules
                && !match crate::r4701_boast::r4701_boast_most(state, source.id, &rules) {
                    Some(most) => state.r4701_spend_boast(most, *source, *origin, *ability),
                    None => state.spend_limit(*limit, *source, *origin, *ability),
                }
                // An exhaust ability activated again under Elvish Refueler finds its use spent already.
                && crate::rules::r6007_limit_holds(state, *player, *limit)
            {
                return Err(invalid(format!(
                    "ability {ability} of {source:?} was already used"
                )));
            }
            // A planeswalker has one loyalty ability activated each turn, whichever it is, or two under a rule that
            // allows it (CR 606.3).
            if matches!(&*rules, Ability::Loyalty { .. }) {
                if state.loyalty_spent(*source) {
                    return Err(invalid(format!(
                        "a loyalty ability of {source:?} was already activated this turn"
                    )));
                }
                *state.loyalty_used.entry(*source).or_insert(0) += 1;
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
            spend_when_done(state, bound);
            step_done(resolving_mut(state)?, position, bound, snapshots).map_err(invalid)
        }
        Event::ResolutionEnded { object, outcome } => end(state, *object, *outcome),
        other => Err(invalid(format!("not a resolution event: {other:?}"))),
    }
}

/// Folds a finished step into `resolving`: the next position, the names it bound and the objects it moved away; the
/// journal of the step is cleared. The inline runner of replacement effects folds its steps the same way.
pub(crate) fn step_done(
    resolving: &mut Resolving,
    position: &[usize],
    bound: &[(String, Vec<Target>)],
    snapshots: &[crate::snapshot::Snapshot],
) -> Result<(), String> {
    resolving.position = position.to_vec();
    for (name, targets) in bound {
        // A repeat frees the names its body bound before its next pass, so they may be bound again.
        if let Some(cleared) = name.strip_prefix(crate::resolve::CLEAR) {
            resolving.bindings.remove(cleared);
            continue;
        }
        if let Some(kept) = name.strip_prefix(crate::resolve::LET) {
            let (value, kept) = kept
                .split_once(':')
                .and_then(|(value, kept)| Some((value.parse::<i64>().ok()?, kept)))
                .ok_or_else(|| format!("`{name}` names no kept number"))?;
            resolving.lets.retain(|(old, _)| old != kept);
            resolving.lets.push((kept.to_owned(), value));
            continue;
        }
        if let Some(value) = name.strip_prefix(crate::resolve::CHOSEN_VALUE) {
            let (key, json) = value
                .split_once('\t')
                .ok_or_else(|| format!("`{name}` names no chosen value"))?;
            let chosen = serde_json::from_str(json)
                .map_err(|_| format!("`{name}` names no chosen value"))?;
            resolving.chosen.insert(key.to_owned(), chosen);
            continue;
        }
        if let Some(x) = name.strip_prefix(crate::resolve::W6C_PAID_X) {
            let x = x.parse().map_err(|_| format!("`{name}` names no paid X"))?;
            resolving.w6c_paid_x = Some(x);
            continue;
        }
        if let Some(old) = resolving.bindings.insert(name.clone(), targets.clone())
            // Each payment of the resolution binds what it took anew; "if they do" reads the latest one.
            && name != crate::resolve::R5520_PAID
            && !renews(&old, targets)
            && !grows(&old, targets)
        {
            return Err(format!("`{name}` was bound twice"));
        }
    }
    resolving.snapshots.extend(snapshots.iter().cloned());
    resolving.journal.clear();
    Ok(())
}

/// Whether `new` is `old` with more after it: an adding bind (`Effect::Bind` with `adds`) grows its name.
fn grows(old: &[Target], new: &[Target]) -> bool {
    new.len() > old.len() && new.starts_with(old)
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

/// "Do this only once each turn": the limit of the resolving triggered ability is spent when its first step, a choice,
/// was done.
fn spend_when_done(state: &mut GameState, bound: &[(String, Vec<Target>)]) {
    let first = crate::resolve::first_step_done();
    if !bound
        .iter()
        .any(|(name, who)| *name == first && !who.is_empty())
    {
        return;
    }
    let Some(ability) = state
        .resolving
        .as_ref()
        .and_then(|resolving| state.objects.get(&resolving.object))
        .and_then(|object| object.ability())
    else {
        return;
    };
    let counted = matches!(
        ability.rules(),
        Ability::Triggered {
            limit: Some(Limit::OncePerTurnDone),
            ..
        }
    );
    let key = (ability.source(), ability.origin(), ability.index());
    if counted {
        state.spend_limit(Limit::OncePerTurnDone, key.0, key.1, key.2);
    }
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
    for target in put.choices.targeted_objects() {
        state.tally_object(target, Some(put.controller), Tally::Targeted);
    }
    if put.cause.is_none() {
        state.record(
            mtg_cards::value::TallyEvent::Activated,
            Some(put.controller),
            (put.source.id, None),
            1,
        );
    }
    // The modes this ability chose are kept for the turn ("choose one that hasn't been chosen this turn").
    for mode in &put.choices.modes {
        state
            .tallies
            .modes
            .insert((put.source, put.origin, put.ability, *mode));
    }
    state.rng_index += u64::from(put.choices.random_modes) + u64::from(put.choices.random_targets);
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
        let rules = ability_reader::ability_as_it_was(state, address, &put.snapshot)
            .ok_or_else(|| invalid(format!("{source:?} has no ability {}", put.ability)))?;
        if let Some(kind) =
            crate::activation_kind::specific_as_it_was(state, address, &rules, &put.snapshot)
        {
            state.record(
                mtg_cards::value::TallyEvent::KindActivated(kind),
                Some(put.controller),
                (source.id, None),
                1,
            );
        }
        (rules, Bindings::new())
    };
    c11d_keep_modes(state, put, &rules);
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

/// The modes an ability that chooses a mode not yet chosen chose are kept for the rest of the game ("choose one that
/// hasn't been chosen").
fn c11d_keep_modes(state: &mut GameState, put: &PutOnStack, rules: &Ability) {
    if crate::choose::modal_of(rules)
        .is_some_and(|modal| modal.once == crate::choose::C11dOnce::Ever)
    {
        for mode in &put.choices.modes {
            state
                .modes_ever
                .insert((put.source, put.origin, put.ability, *mode));
        }
    }
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
        // The numbers the setup kept come as `~let:<number>:<name>` names: they are kept numbers again.
        let kept: Vec<String> = resolving
            .bindings
            .keys()
            .filter(|name| name.starts_with(crate::resolve::LET))
            .cloned()
            .collect();
        for name in kept {
            resolving.bindings.remove(&name);
            let parsed = name
                .strip_prefix(crate::resolve::LET)
                .and_then(|rest| rest.split_once(':'))
                .and_then(|(value, kept)| Some((kept.to_owned(), value.parse::<i64>().ok()?)));
            resolving.lets.extend(parsed);
        }
    }
    let counted = state
        .objects
        .get(&id)
        .and_then(|o| o.ability())
        .map(|ability| (ability.source(), ability.origin(), ability.index()));
    if let Some((source, origin, index)) = counted {
        state.count_resolution(source, origin, index);
    }
    state.resolving = Some(resolving);
    Ok(())
}

/// Takes the object off the stack. An ability ceases to exist (CR 113.7a); a spell that resolved is moved by
/// `SpellResolved`, which follows, and a spell that did not goes to its owner's graveyard.
fn end(state: &mut GameState, id: ObjectId, outcome: ResolutionOutcome) -> Result<(), EngineError> {
    // A copy of a spell that a replacement effect sent elsewhere (Rest in Peace) ceased to exist with that move, so its
    // end finds it apart from the game (CR 111.7, 707.10a).
    let (kind, owner, zone) = state
        .objects
        .get(&id)
        .or_else(|| state.ceased.get(&id))
        .map(|o| (o.kind, o.owner, o.zone))
        .ok_or_else(|| invalid(format!("unknown object {id:?}")))?;
    // A spell that a replacement effect sent elsewhere (exile instead of the graveyard) is off the stack already.
    let on_stack = state.zones.get(Zone::Stack).contains(&id);
    if !on_stack && (kind == ObjectKind::Ability || zone == Zone::Battlefield) {
        return Err(invalid(format!("{id:?} is not on the stack")));
    }
    let resolving_it = state.resolving.as_ref().is_some_and(|r| r.object == id);
    if !resolving_it && outcome != ResolutionOutcome::Countered {
        return Err(invalid(format!("{id:?} is not resolving")));
    }
    if resolving_it {
        state.resolving = None;
    }
    let keeps = crate::r281_mana_at_priority::r281_keeps_priority(state, id);
    match (kind, outcome) {
        (ObjectKind::Ability, _) => {
            state.r288_resolved_chapter = state
                .objects
                .get(&id)
                .and_then(GameObject::ability)
                .filter(|_| outcome == ResolutionOutcome::Resolved && resolving_it)
                .and_then(|put| {
                    crate::saga::r288_chapter_number(put.rules()).map(|n| (id, put.source(), n))
                });
            state.objects.remove(&id);
            state.zones.remove(Zone::Stack, id);
        }
        (_, ResolutionOutcome::Resolved) => return Ok(()),
        (
            _,
            ResolutionOutcome::Fizzled | ResolutionOutcome::Countered | ResolutionOutcome::Lapsed,
        ) => {
            if on_stack {
                move_object(state, id, Zone::Graveyard(owner), Placement::Top)?;
            }
        }
    }
    match keeps {
        // The player who activated a mana ability receives priority again (CR 117.3c).
        Some(holder) if !state.turn.ending => {
            state.priority = Some(crate::priority::PriorityState {
                holder,
                passed: std::collections::BTreeSet::default(),
            });
        }
        _ => priority_to_active(state),
    }
    Ok(())
}
