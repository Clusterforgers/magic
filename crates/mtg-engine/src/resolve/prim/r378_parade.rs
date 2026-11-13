//! `Effect::Parade`: the controller chooses the objects one at a time until each has been chosen, and each gets +1/+1
//! until end of turn for each object chosen before it ("Parade!").

use mtg_cards::effect::Effect;
use mtg_cards::modify::Duration;
use mtg_cards::refs::Ref;
use mtg_core::ids::{EffectId, ObjectId};

use super::wrong_effect;
use crate::choose::{Questions, Stop};
use crate::effects::{ContinuousEffect, Modification, Selector};
use crate::events::Event;
use crate::object::GameObject;
use crate::resolve::{Cx, ResolveError, Run};
use crate::zones::Zone;

/// Asks the order (none for one object) and gives the nth object chosen +(n-1)/+(n-1) until end of turn.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Parade { what } = effect else {
        return Err(wrong_effect("Parade"));
    };
    Stop::settle(parade(cx, what))
}

fn parade(cx: &Cx<'_>, what: &mtg_cards::refs::Selector) -> Result<Run, Stop> {
    let state = cx.state();
    let objects: Vec<_> = cx
        .scope
        .live_objects(&Ref::Each(what.clone()))?
        .into_iter()
        .filter(|object| object.zone() == Zone::Battlefield)
        .map(GameObject::reference)
        .collect();
    let line: Vec<ObjectId> = if objects.len() < 2 {
        objects.iter().map(|object| object.id).collect()
    } else {
        let mut questions = Questions::new(cx);
        let ids = objects.iter().map(|object| object.id).collect();
        let text = "Choose the creatures one at a time, the first first.".to_owned();
        let order = questions.order(cx.controller(), ids, text)?;
        questions.end()?;
        order
    };
    let duration = Duration::UntilEndOfTurn;
    let mut events = Vec::new();
    for ((made, id), before) in (0u64..).zip(line.iter().skip(1)).zip(1i32..) {
        let Some(object) = objects.iter().find(|object| object.id == *id) else {
            continue;
        };
        events.push(Event::EffectCreated {
            effect: Box::new(
                ContinuousEffect::new(
                    EffectId(state.next_effect + made),
                    Selector::Objects(vec![*object]),
                    Modification::ModifyPowerToughness {
                        power: before,
                        toughness: before,
                    },
                    duration.clone(),
                    state.next_timestamp + made,
                )
                .from_source(cx.scope.source)
                .begun(
                    cx.controller(),
                    crate::effects::began_turn(state, &duration, cx.controller()),
                ),
            ),
        });
    }
    Ok(Run::events(events))
}
