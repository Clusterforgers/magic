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
use crate::object::ObjectRef;
use crate::resolve::{Cx, ResolveError, Run};
use crate::rules::Rule;
use mtg_core::ids::EffectId;

/// A source on the battlefield that holds no +1/+1 counter gets the counters, as a proposed `CountersChanged` that a
/// replacement effect sees, and adapts: one `Adapted` follows the counters, also when there are none. A source that
/// holds a +1/+1 counter, or has left the battlefield, does nothing. A rule that the source's next adapt treats it as
/// though it had no +1/+1 counters (CR 701.46a, Biomancer's Familiar) lets it adapt anyway, and that adapt uses it up.
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
        let fresh = r6600_fresh_rules(cx, holder);
        events.extend(fresh.iter().map(|id| Event::RuleExpired { id: *id }));
        let holds = fresh.is_empty()
            && cx
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
/// The rules in force that `holder`'s next adapt treats it as though it had no +1/+1 counters.
fn r6600_fresh_rules(cx: &Cx<'_>, holder: ObjectRef) -> Vec<EffectId> {
    cx.state()
        .rule_effects()
        .iter()
        .filter(|effect| matches!(&effect.rule, Rule::R6600AdaptsFresh(named) if named.contains(&holder)))
        .map(|effect| effect.id)
        .collect()
}
