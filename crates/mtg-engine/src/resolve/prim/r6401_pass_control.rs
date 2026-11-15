//! `Effect::R6401PassControl`: each player gains control of permanents the next player on a side of them controls
//! (CR 101.4: turns pass to the left), all of them, one the controller picks for each player, or one each player picks;
//! every change is made at once and lasts for good.

use mtg_cards::effect::{Effect, R6401Pick};
use mtg_cards::modify::Duration;
use mtg_cards::refs::{Ref, Selector as Filter, Side};
use mtg_core::ids::{EffectId, ObjectId, PlayerId};

use super::wrong_effect;
use crate::characteristics::controller_of_object;
use crate::choose::{Chooser, Picked, Target, pick_cards};
use crate::effects::{ContinuousEffect, Modification, Selector};
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::zones::Zone;

/// The side the controller chose in the `ChooseWords` before: the word bound to them.
fn chosen_side(cx: &Cx<'_>) -> Result<Side, ResolveError> {
    let me = Target::Player(cx.controller());
    let chose = |word: &str| {
        cx.scope
            .refs(&Ref::Bound(word.to_owned()))
            .is_ok_and(|named| named.contains(&me))
    };
    if chose("left") {
        Ok(Side::Left)
    } else if chose("right") {
        Ok(Side::Right)
    } else {
        Err(ResolveError::Invalid(
            "no direction was chosen before the permanents pass".into(),
        ))
    }
}

/// The permanents `what` picks that `giver` controls, in battlefield order.
fn given(cx: &Cx<'_>, what: &Filter, giver: PlayerId) -> Result<Vec<ObjectId>, ResolveError> {
    let state = cx.state();
    let mut found = Vec::new();
    for id in state.zone(Zone::Battlefield) {
        let Some(object) = state.object(*id) else {
            continue;
        };
        if controller_of_object(state, object) == giver
            && cx.scope.selects(what, object.reference())?
        {
            found.push(*id);
        }
    }
    Ok(found)
}

/// The players who take, each with the player whose permanents they take: every player still in the game, from the
/// controller round in the direction, while someone else is left.
fn takers(cx: &Cx<'_>, side: Side) -> Vec<(PlayerId, PlayerId)> {
    let state = cx.state();
    let me = cx.controller();
    let mut order = vec![me];
    order.extend(state.r376_seats_from(me, side));
    order
        .into_iter()
        .filter(|player| state.is_alive(*player))
        .filter_map(|player| {
            let next = state.r376_seats_from(player, side).into_iter().next()?;
            (next != player).then_some((player, next))
        })
        .collect()
}

/// Asks the choices the pick needs, then gives each permanent to its taker in one batch of control effects.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::R6401PassControl { side, what, pick } = effect else {
        return Err(wrong_effect("R6401PassControl"));
    };
    let side = match side {
        Some(side) => *side,
        None => chosen_side(cx)?,
    };
    let takers = takers(cx, side);
    let mut passes: Vec<(PlayerId, ObjectId)> = Vec::new();
    match pick {
        R6401Pick::All => {
            for (taker, giver) in &takers {
                passes.extend(given(cx, what, *giver)?.into_iter().map(|id| (*taker, id)));
            }
        }
        R6401Pick::ByYou | R6401Pick::ByEach => {
            let mut choosers = Vec::new();
            for (taker, giver) in &takers {
                choosers.push(Chooser {
                    player: if *pick == R6401Pick::ByYou {
                        cx.controller()
                    } else {
                        *taker
                    },
                    from: given(cx, what, *giver)?,
                    take: 1,
                    text: format!(
                        "Choose a permanent of seat {} that seat {} gains control of.",
                        giver.0 + 1,
                        taker.0 + 1
                    ),
                });
            }
            let taken = match pick_cards(cx, &choosers)? {
                Picked::Ask(decision) => return Ok(Run::Ask(decision)),
                Picked::Done(taken) => taken,
            };
            for ((taker, _), (_, picked)) in takers.iter().zip(taken) {
                passes.extend(picked.into_iter().map(|id| (*taker, id)));
            }
        }
    }
    let state = cx.state();
    let mut events = Vec::new();
    for (taker, id) in passes {
        let Some(object) = state.object(id) else {
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
                    Selector::Objects(vec![object.reference()]),
                    Modification::SetController(taker),
                    Duration::Forever,
                    state.next_timestamp.saturating_add(made),
                )
                .from_source(cx.scope.source)
                .begun(cx.controller(), state.turn().number()),
            ),
        });
    }
    Ok(Run::events(events))
}
