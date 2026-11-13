//! `Effect::PhaseOut`: permanents phase out (CR 702.26).

use mtg_cards::effect::Effect;
use mtg_cards::modify::Duration;

use mtg_cards::modify::Prohibited;
use mtg_core::ids::EffectId;

use super::wrong_effect;
use crate::choose::Target;
use crate::events::Event;
use crate::object::GameObject;
use crate::resolve::{Cx, ResolveError, Run};
use crate::rules::{Rule, RuleEffect};
use crate::zones::Zone;

/// Each permanent the effect names that is on the battlefield phases out, all together in the order the reference lists
/// them, and the Auras and Equipment attached to it phase out with it. A permanent that is phased out already, an object
/// that is not a permanent or has changed zones since it was named (CR 400.7) and a name that repeats are passed over,
/// and naming nothing phases nothing out.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::PhaseOut {
        what,
        duration,
        tapped,
        held,
    } = effect
    else {
        return Err(wrong_effect("PhaseOut"));
    };
    let until = match duration {
        None => None,
        // A source that has left already phases nothing out (CR 610.3c by analogy).
        Some(Duration::WhileSourceOnBattlefield) => {
            if !crate::exile_return::holds(cx.state(), cx.scope.source) {
                return Ok(Run::events(Vec::new()));
            }
            Some(cx.scope.source)
        }
        Some(_) => return Err(ResolveError::Unsupported("PhaseOut.Duration")),
    };
    let hold = crate::phasing::W11bHold {
        until,
        tapped: *tapped,
    };
    let named: Vec<_> = cx
        .scope
        .live_objects(what)?
        .into_iter()
        .filter(|object| object.zone() == Zone::Battlefield)
        .map(GameObject::id)
        .collect();
    let mut events = crate::phasing::w11b_out_events(cx.state(), &named, hold);
    if let Some(held) = held {
        events.extend(held_rule(cx, &events, held));
    }
    Ok(Run::events(events))
}

/// The rule that the permanents the effect itself phased out (not what went with them) can't phase in while `held`
/// lasts; none when nothing phased out.
fn held_rule(cx: &Cx<'_>, events: &[Event], held: &Duration) -> Option<Event> {
    let state = cx.state();
    let who: Vec<Target> = events
        .iter()
        .filter_map(|event| match event {
            Event::PermanentPhasedOut {
                object,
                indirect: false,
                ..
            } => state.object(*object).map(|o| Target::Object(o.reference())),
            _ => None,
        })
        .collect();
    if who.is_empty() {
        return None;
    }
    Some(Event::RuleCreated {
        effect: Box::new(RuleEffect {
            id: EffectId(state.next_effect),
            rule: Rule::Cant {
                who,
                action: Box::new(Prohibited::PhaseIn),
            },
            duration: held.clone(),
            source: Some(cx.scope.source),
            controller: cx.controller(),
            turn: crate::effects::began_turn(state, held, cx.controller()),
        }),
    })
}
