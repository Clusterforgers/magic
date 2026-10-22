//! `Effect::Monstrosity`: the source becomes monstrous (CR 701.37).

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

/// A source that is on the battlefield and not monstrous gets the counters, as a proposed `CountersChanged` that a
/// replacement effect sees, and becomes monstrous: one `Monstrous` follows the counters, also when there are none
/// (a count of 0 or less, or counters a replacement effect took away). A source that has left the battlefield or is
/// monstrous already does nothing.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Monstrosity { count } = effect else {
        return Err(wrong_effect("Monstrosity"));
    };
    let delta = whole(cx.scope.amount(count)?);
    let mut events = Vec::new();
    for source in actors(cx, &Ref::This)? {
        let Some(holder) = source.permanent else {
            continue;
        };
        let monstrous = cx
            .state()
            .object(holder.id)
            .and_then(GameObject::permanent)
            .is_none_or(PermanentState::monstrous);
        if monstrous {
            continue;
        }
        if delta > 0 {
            events.push(Event::CountersChanged(CountersChanged {
                on: Target::Object(holder),
                counter: CounterKind::PlusOnePlusOne,
                delta,
            }));
        }
        events.push(Event::Monstrous {
            object: source.object,
            player: source.player,
        });
    }
    Ok(Run::events(events))
}
