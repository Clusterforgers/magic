//! State triggers (CR 603.8): triggered abilities that wait for a game state instead of an event.
//!
//! A state trigger triggers as soon as its state is true, and not again until its ability has left the stack
//! (resolved, been countered or been removed); then, if its source is still where it works from and the state is still
//! true, it triggers again. They are looked for where a player would receive priority, after state-based actions
//! (`flow`), and are logged as `AbilityTriggered` like any other, so a replay folds them and detects nothing.

use std::sync::Arc;

use mtg_cards::ability::{Ability, Limit};
use mtg_cards::refs::ZoneKind;
use mtg_cards::trigger::TriggerPattern;

use super::Triggered;
use crate::abilities::{AbilityRef, Cause};
use crate::ability_reader::{abilities_of, working_here};
use crate::characteristics::controller_of_object;
use crate::object::{GameObject, ObjectKind, ObjectRef};
use crate::resolve::{Scope, zone_kind};
use crate::snapshot::snapshot;
use crate::state::GameState;
use crate::zones::Zone;

/// Whether an instance of the ability `address` of `source` waits for the stack or is on it.
fn waiting(state: &GameState, source: ObjectRef, address: AbilityRef) -> bool {
    state.pending_triggers().iter().any(|entry| {
        let t = entry.trigger();
        t.source == source
            && t.origin == address.source
            && t.ability == address.ability
            && t.delayed.is_none()
    }) || state
        .stack()
        .iter()
        .filter_map(|id| state.object(*id))
        .filter_map(GameObject::ability)
        .any(|ability| {
            ability.source() == source
                && ability.index() == address.ability
                && ability.origin() == address.source
                && ability.delayed().is_none()
        })
}

/// The state triggers of `object`, which is in the zone `zone`, that trigger now.
fn triggers_of(
    state: &GameState,
    object: &GameObject,
    zone: ZoneKind,
    abilities: impl IntoIterator<Item = (AbilityRef, Arc<Ability>)>,
    found: &mut Vec<Triggered>,
) {
    let (source, controller) = (object.reference(), controller_of_object(state, object));
    if !state.is_alive(controller) {
        return;
    }
    for (address, rules) in abilities {
        let Ability::Triggered {
            when: TriggerPattern::State { condition },
            intervening_if,
            limit,
            from,
            ..
        } = &*rules
        else {
            continue;
        };
        if *from != zone || waiting(state, source, address) {
            continue;
        }
        if !Scope::announcing(state, controller, source)
            .holds(condition)
            .unwrap_or(false)
        {
            continue;
        }
        // No event is behind a state, so the cause names nothing.
        let cause = Cause::default();
        // CR 603.4: an "if" in the trigger is checked as the state is found.
        if let Some(condition) = intervening_if
            && !Scope::triggered(state, controller, source, &cause)
                .holds(condition)
                .unwrap_or(false)
        {
            continue;
        }
        if *limit == Some(Limit::OncePerTurn)
            && state.limit_spent(source, address.source, address.ability)
        {
            continue;
        }
        let Some(shot) = snapshot(state, object.id()) else {
            continue;
        };
        found.push(Triggered {
            source,
            ability: address.ability,
            origin: address.source,
            controller,
            cause,
            snapshot: shot,
            delayed: None,
        });
    }
}

/// The state triggers that trigger in `state`, the permanents' first and then those of the cards elsewhere, in id order.
pub(crate) fn detect(state: &GameState) -> Vec<Triggered> {
    let mut found = Vec::new();
    if !state.possible.states {
        return found;
    }
    for &id in state.zone(Zone::Battlefield) {
        if let Some(object) = state.object(id).filter(|o| o.kind() != ObjectKind::Ability) {
            triggers_of(
                state,
                object,
                ZoneKind::Battlefield,
                abilities_of(state, id),
                &mut found,
            );
        }
    }
    if state.possible.elsewhere {
        for object in state.objects() {
            if object.zone() == Zone::Battlefield || object.kind() == ObjectKind::Ability {
                continue;
            }
            let here = working_here(object).map(|(address, rules)| (address, Arc::clone(rules)));
            triggers_of(state, object, zone_kind(object.zone()), here, &mut found);
        }
    }
    found
}
