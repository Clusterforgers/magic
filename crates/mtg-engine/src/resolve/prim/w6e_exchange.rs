//! `Effect::ExchangeControl`: two permanents exchange controllers (CR 701.12b).

use mtg_cards::effect::Effect;
use mtg_core::ids::EffectId;

use super::wrong_effect;
use crate::characteristics::controller_of_object;
use crate::effects::{ContinuousEffect, Modification, Selector};
use crate::events::Event;
use crate::object::GameObject;
use crate::resolve::{Cx, ResolveError, Run};
use crate::zones::Zone;

/// The references name the two permanents still on the battlefield; each takes the controller the other has now, as two
/// control effects that begin together with consecutive timestamps. Nothing happens unless exactly two permanents are
/// named and two players control them (CR 701.12b), so one target that became illegal stops the whole exchange.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::ExchangeControl { what, duration } = effect else {
        return Err(wrong_effect("ExchangeControl"));
    };
    let state = cx.state();
    let mut named: Vec<&GameObject> = Vec::new();
    for reference in what {
        for object in cx.scope.live_objects(reference)? {
            if object.zone() == Zone::Battlefield
                && named.iter().all(|seen| seen.id() != object.id())
            {
                named.push(object);
            }
        }
    }
    let [first, second] = named.as_slice() else {
        return Ok(Run::events(Vec::new()));
    };
    let (one, other) = (
        controller_of_object(state, first),
        controller_of_object(state, second),
    );
    if one == other {
        return Ok(Run::events(Vec::new()));
    }
    let events = [(first, other), (second, one)]
        .into_iter()
        .zip(0u64..)
        .map(|((object, player), made)| Event::EffectCreated {
            effect: ContinuousEffect::new(
                EffectId(state.next_effect + made),
                Selector::Objects(vec![object.reference()]),
                Modification::SetController(player),
                *duration,
                state.next_timestamp + made,
            )
            .from_source(cx.scope.source)
            .begun(cx.controller(), state.turn().number()),
        })
        .collect();
    Ok(Run::events(events))
}
