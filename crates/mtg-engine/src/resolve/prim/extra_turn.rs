//! `Effect::ExtraTurn`: players take extra turns after this one (CR 500.7).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};

/// Each player the effect names, in turn order from the active player, gets `count` extra turns, added one at a time:
/// the latest added is taken first (CR 500.7), so the turns of the player named last are taken before the others. A
/// player who has left the game gets none.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::ExtraTurn {
        who,
        count,
        skip_untap,
        no_power_up,
        r109_unpreventable,
        at_end,
    } = effect
    else {
        return Err(wrong_effect("ExtraTurn"));
    };
    let turns = cx.scope.amount(count)?.max(0);
    let r6703_end = r6703_end_trigger(cx, at_end)?;
    let named = cx.scope.players(who)?;
    let mut events = Vec::new();
    for player in cx.state().turn_order_from(cx.state().turn().active()) {
        if named.contains(&player) {
            for _ in 0..turns {
                events.push(Event::ExtraTurnAdded {
                    player,
                    skip_untap: *skip_untap,
                    no_power_up: *no_power_up,
                    r109_unpreventable: *r109_unpreventable,
                    r6703_end: r6703_end.clone(),
                });
            }
        }
    }
    Ok(Run::events(events))
}

/// The delayed trigger "at the beginning of that turn's end step" the extra turns carry, set up as this resolution would
/// set it up now; none without effects.
fn r6703_end_trigger(
    cx: &Cx<'_>,
    at_end: &[Effect],
) -> Result<Option<Box<crate::triggers::DelayedCreated>>, ResolveError> {
    if at_end.is_empty() {
        return Ok(None);
    }
    let delayed = Effect::DelayedTrigger {
        intervening_if: None,
        when: mtg_cards::trigger::TriggerPattern::StepBegins {
            step: mtg_cards::trigger::Step::End,
            whose: mtg_cards::refs::Ref::EachPlayer,
        },
        once: true,
        this_turn: true,
        until: None,
        targets: Vec::new(),
        effects: at_end.to_vec(),
    };
    let crate::resolve::Run::Done { events, .. } = super::delayed_trigger::run(cx, &delayed)?
    else {
        return Ok(None);
    };
    Ok(events.into_iter().find_map(|event| match event {
        Event::DelayedTriggerCreated(created) => Some(created),
        _ => None,
    }))
}
