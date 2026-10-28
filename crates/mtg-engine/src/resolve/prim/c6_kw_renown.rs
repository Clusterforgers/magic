//! `Effect::Renown`: the source becomes renowned (CR 702.112).

use mtg_cards::effect::Effect;
use mtg_cards::refs::Ref;
use mtg_cards::value::CounterKind;

use super::explore::actors;
use super::put_counters::whole;
use super::wrong_effect;
use crate::choose::Target;
use crate::counters::CountersChanged;
use crate::events::Event;
use crate::object::{GameObject, PermanentState};
use crate::resolve::{Cx, ResolveError, Run};

/// A source that is on the battlefield and not renowned gets the counters, as a proposed `CountersChanged` that a
/// replacement effect sees, and becomes renowned: one `Renowned` follows the counters, also when there are none. A
/// source that has left the battlefield or is renowned already does nothing (CR 702.112a).
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Renown { count } = effect else {
        return Err(wrong_effect("Renown"));
    };
    let delta = whole(cx.scope.amount(count)?);
    let mut events = Vec::new();
    for source in actors(cx, &Ref::This)? {
        let Some(holder) = source.permanent else {
            continue;
        };
        let renowned = cx
            .state()
            .object(holder.id)
            .and_then(GameObject::permanent)
            .is_none_or(PermanentState::renowned);
        if renowned {
            continue;
        }
        if delta > 0 {
            events.push(Event::CountersChanged(CountersChanged {
                on: Target::Object(holder),
                counter: CounterKind::PlusOnePlusOne,
                delta,
            }));
        }
        events.push(Event::Renowned {
            object: source.object,
            player: source.player,
        });
    }
    Ok(Run::events(events))
}
