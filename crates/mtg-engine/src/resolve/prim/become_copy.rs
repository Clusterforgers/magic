//! `Effect::BecomeCopy`: permanents become copies of an object, for a while (CR 707.2).

use std::sync::Arc;

use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::modify::{Duration, Modification};

use super::wrong_effect;
use crate::abilities::{AbilityObject, AbilityRef, AbilitySource};
use crate::ability_reader;
use crate::copies::{BecameCopy, copiable_values};
use crate::events::Event;
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::state::GameState;
use crate::zones::Zone;

/// The ability "this ability" names in a copy exception: the resolving ability, or for a reflexive or delayed trigger the
/// printed ability that set it up (its own rules are a trigger that never triggers again). The printed ability is taken
/// only when it holds the trigger's steps; else the resolving rules stand.
fn this_ability(state: &GameState, ability: &AbilityObject) -> Arc<Ability> {
    let own = ability.shared_rules();
    if ability.delayed().is_none() {
        return own;
    }
    let address = AbilityRef {
        object: ability.source().id,
        ability: ability.index(),
        source: AbilitySource::Printed,
    };
    let Some(printed) = ability_reader::ability_as_it_was(state, address, ability.snapshot())
    else {
        return own;
    };
    let steps = serde_json::to_value(own.effects()).ok();
    let holds = steps
        .zip(serde_json::to_value(&*printed).ok())
        .is_some_and(|(steps, printed)| holds(&printed, &steps));
    if holds { printed } else { own }
}

/// Whether `value` or a value nested in it is `part`.
fn holds(value: &serde_json::Value, part: &serde_json::Value) -> bool {
    value == part
        || match value {
            serde_json::Value::Array(items) => items.iter().any(|item| holds(item, part)),
            serde_json::Value::Object(fields) => fields.values().any(|item| holds(item, part)),
            _ => false,
        }
}

/// Each permanent the effect names that is still on the battlefield becomes a copy of the first object `of` names: a
/// `BecameCopy` with the copiable values of that object (read by last known information once it has left) and the
/// exceptions of the effect. An effect until end of turn lasts until the cleanup step; one without end is a copy for as
/// long as the permanent stays (CR 400.7). An object that cannot be copied, or no object, changes nothing.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::BecomeCopy {
        what,
        of,
        except,
        duration,
    } = effect
    else {
        return Err(wrong_effect("BecomeCopy"));
    };
    let state = cx.state();
    let (until_end_of_turn, until_next_turn_of) = match duration {
        Duration::UntilEndOfTurn => (true, None),
        Duration::Forever => (false, None),
        Duration::UntilYourNextTurn => (false, Some(cx.controller())),
        _ => return Err(ResolveError::Unsupported("Copy.Duration")),
    };
    let mut events = Vec::new();
    if let Some(original) = cx.scope.objects(of)?.into_iter().next()
        && let Some(values) = copiable_values(state, original, cx.scope.snapshot_of(original))
    {
        let mut definition = values;
        for modification in except {
            let applied = match modification {
                Modification::ThisAbility => cx
                    .state()
                    .object(cx.resolving.object())
                    .and_then(|object| object.ability())
                    .is_some_and(|ability| {
                        definition.except(&Modification::GrantAbility(this_ability(state, ability)))
                    }),
                other => definition.except(other),
            };
            if !applied {
                return Err(ResolveError::Unsupported("Effect.BecomeCopy.Except"));
            }
        }
        let definition = Arc::new(definition);
        for permanent in cx.scope.live_objects(what)? {
            if permanent.zone() == Zone::Battlefield {
                events.push(Event::BecameCopy(BecameCopy {
                    object: permanent.id(),
                    of: original,
                    definition: Arc::clone(&definition),
                    until_end_of_turn,
                    until_next_turn_of,
                }));
            }
        }
    }
    Ok(Run::Done {
        events,
        bound: Vec::new(),
        snapshots: Vec::new(),
        next: Next::Sibling,
    })
}
