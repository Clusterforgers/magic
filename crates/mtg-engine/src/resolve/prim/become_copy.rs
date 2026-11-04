//! `Effect::BecomeCopy`: permanents become copies of an object, for a while (CR 707.2).

use std::sync::Arc;

use mtg_cards::effect::Effect;
use mtg_cards::modify::{Duration, Modification};

use super::wrong_effect;
use crate::copies::{BecameCopy, copiable_values};
use crate::events::Event;
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::zones::Zone;

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
                        definition.except(&Modification::GrantAbility(ability.shared_rules()))
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
