//! `Effect::Adapt`: the source adapts (CR 701.46).

use mtg_cards::effect::Effect;
use mtg_cards::refs::Ref;
use mtg_cards::value::CounterKind;

use super::explore::actors;
use super::put_counters::whole;
use super::wrong_effect;
use crate::choose::Target;
use crate::counters::CountersChanged;
use crate::events::Event;
use crate::object::GameObject;
use crate::resolve::{Cx, ResolveError, Run};

/// A source on the battlefield that holds no +1/+1 counter gets the counters, as a proposed `CountersChanged` that a
/// replacement effect sees, and adapts: one `Adapted` follows the counters, also when there are none. A source that
/// holds a +1/+1 counter, or has left the battlefield, does nothing.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Adapt { count } = effect else {
        return Err(wrong_effect("Adapt"));
    };
    let delta = whole(cx.scope.amount(count)?);
    let mut events = Vec::new();
    for source in actors(cx, &Ref::This)? {
        let Some(holder) = source.permanent else {
            continue;
        };
        let holds = cx
            .state()
            .object(holder.id)
            .and_then(GameObject::permanent)
            .is_none_or(|held| held.counters_of(&CounterKind::PlusOnePlusOne) > 0);
        if holds {
            continue;
        }
        if delta > 0 {
            events.push(Event::CountersChanged(CountersChanged {
                on: Target::Object(holder),
                counter: CounterKind::PlusOnePlusOne,
                delta,
            }));
        }
        events.push(Event::Adapted {
            object: source.object,
            player: source.player,
        });
    }
    Ok(Run::events(events))
}
