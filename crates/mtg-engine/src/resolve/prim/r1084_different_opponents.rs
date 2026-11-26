//! `Effect::R1084ToDifferentOpponents` (CR 108.4, 613.1b): for each permanent the controller chooses a different
//! opponent, and each opponent gains control of the permanent chosen for them, for good.

use mtg_cards::effect::Effect;
use mtg_cards::modify::Duration;
use mtg_core::ids::{EffectId, PlayerId};

use super::wrong_effect;
use crate::characteristics::controller_of_object;
use crate::choose::ChoiceOption;
use crate::choose::{Questions, Stop};
use crate::effects::{ContinuousEffect, Modification, Selector};
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::zones::Zone;

/// Asks, permanent by permanent, which opponent not yet chosen takes it; one opponent left is taken without asking.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::R1084ToDifferentOpponents { what } = effect else {
        return Err(wrong_effect("R1084ToDifferentOpponents"));
    };
    Stop::settle(give(cx, what))
}

fn give(cx: &Cx<'_>, what: &mtg_cards::refs::Ref) -> Result<Run, Stop> {
    let state = cx.state();
    let me = cx.controller();
    let permanents: Vec<_> = cx
        .scope
        .live_objects(what)?
        .into_iter()
        .filter(|object| object.zone() == Zone::Battlefield)
        .map(crate::object::GameObject::reference)
        .collect();
    let mut left: Vec<PlayerId> = state.opponents_of(me);
    let mut questions = Questions::new(cx);
    let mut passes = Vec::new();
    for permanent in permanents {
        if left.is_empty() {
            break;
        }
        let options = left.iter().copied().map(ChoiceOption::Player).collect();
        let at = questions.w9b_option(
            me,
            options,
            "Choose an opponent who gains control of this permanent.".to_owned(),
        )?;
        let taker = left.remove(at.min(left.len().saturating_sub(1)));
        passes.push((taker, permanent));
    }
    questions.end()?;
    let mut events = Vec::new();
    for (taker, permanent) in passes {
        let Some(object) = state.object(permanent.id) else {
            continue;
        };
        if controller_of_object(state, object) == taker {
            continue;
        }
        let made = u64::try_from(events.len()).unwrap_or(u64::MAX);
        events.push(Event::EffectCreated {
            effect: Box::new(
                ContinuousEffect::new(
                    EffectId(state.next_effect.saturating_add(made)),
                    Selector::Objects(vec![permanent]),
                    Modification::SetController(taker),
                    Duration::Forever,
                    state.next_timestamp.saturating_add(made),
                )
                .from_source(cx.scope.source)
                .begun(me, state.turn().number()),
            ),
        });
    }
    Ok(Run::events(events))
}
