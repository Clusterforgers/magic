//! `Effect::GainSuspend`: cards in exile gain suspend (CR 702.62), as "exile it with three time counters on it and it
//! gains suspend" says.

use mtg_cards::cost::Cost;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::modify::Duration;
use mtg_core::ids::EffectId;

use super::wrong_effect;
use crate::effects::{ContinuousEffect, Modification, Selector};
use crate::events::Event;
use crate::keywords::keywords_of;
use crate::resolve::{Cx, ResolveError, Run};
use crate::zones::Zone;

/// The suspend a card gains: the keyword's abilities read neither its number nor its cost, which only suspending a card
/// from a hand pays.
pub(crate) fn gained_suspend() -> Keyword {
    Keyword::Suspend {
        time: 0,
        cost: Cost(Vec::new()),
    }
}

/// Each card the reference names that is still in exile gains suspend for as long as it stays there: an effect names the
/// card, and the card that leaves exile is a new object it no longer names (CR 400.7). With `unless_has`, a card that
/// has suspend already gains nothing. Anything else is passed over.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::GainSuspend { what, unless_has } = effect else {
        return Err(wrong_effect("GainSuspend"));
    };
    let state = cx.state();
    let mut events = Vec::new();
    for object in cx.scope.live_objects(what)? {
        if object.zone() != Zone::Exile
            || (*unless_has
                && keywords_of(state, object.id())
                    .iter()
                    .any(|keyword| matches!(keyword, Keyword::Suspend { .. })))
        {
            continue;
        }
        let made = u64::try_from(events.len()).unwrap_or(u64::MAX);
        events.push(Event::EffectCreated {
            effect: Box::new(
                ContinuousEffect::new(
                    EffectId(state.next_effect + made),
                    Selector::Objects(vec![object.reference()]),
                    Modification::AddKeyword(gained_suspend()),
                    Duration::Forever,
                    state.next_timestamp + made,
                )
                .from_source(cx.scope.source)
                .begun(
                    cx.controller(),
                    crate::effects::began_turn(state, &Duration::Forever, cx.controller()),
                ),
            ),
        });
    }
    Ok(Run::events(events))
}
