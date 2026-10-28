//! `Effect::TimeTravel`: players time travel (CR 701.56).

use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_core::ids::{ObjectId, PlayerId};

use super::wrong_effect;
use crate::choose::{Questions, Stop, Target};
use crate::counters::CountersChanged;
use crate::events::Event;
use crate::keywords::keywords_of;
use crate::keywords::suspend::time_counter;
use crate::resolve::{Cx, ResolveError, Run};
use crate::state::GameState;
use crate::wording;
use crate::zones::Zone;

/// Each player the effect names who is still in the game time travels, in turn order: "For each suspended card you own
/// and each permanent you control with a time counter on it, you may add or remove a time counter" (CR 701.56a). They
/// choose any number of those to add a counter to, then any number of the rest to remove one from (asked whenever
/// there is one to choose, since none is an answer too). Everything is decided before any counter moves; each choice
/// is one `CountersChanged` of one time counter, in the order of the zones.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::TimeTravel { who } = effect else {
        return Err(wrong_effect("TimeTravel"));
    };
    let state = cx.state();
    let players: Vec<PlayerId> = cx
        .scope
        .players(who)?
        .into_iter()
        .filter(|player| state.is_alive(*player))
        .collect();
    Stop::settle(c8_time_travels(cx, &players).map(Run::events))
}

/// The time travel of each player, asking in turn.
fn c8_time_travels(cx: &Cx<'_>, players: &[PlayerId]) -> Result<Vec<Event>, Stop> {
    let state = cx.state();
    let mut questions = Questions::new(cx);
    let mut events = Vec::new();
    for player in players {
        let held = c8_time_holders(state, *player);
        if held.is_empty() {
            continue;
        }
        let added = questions.cards(
            *player,
            held.clone(),
            (0, held.len()),
            wording::C8_TIME_ADD.to_owned(),
        )?;
        let rest: Vec<ObjectId> = held
            .iter()
            .copied()
            .filter(|id| !added.contains(id))
            .collect();
        let removed = if rest.is_empty() {
            Vec::new()
        } else {
            questions.cards(
                *player,
                rest.clone(),
                (0, rest.len()),
                wording::C8_TIME_REMOVE.to_owned(),
            )?
        };
        for id in &held {
            let delta = if added.contains(id) {
                1
            } else if removed.contains(id) {
                -1
            } else {
                continue;
            };
            let Some(object) = state.object(*id) else {
                continue;
            };
            events.push(Event::CountersChanged(CountersChanged {
                on: Target::Object(object.reference()),
                counter: time_counter(),
                delta,
            }));
        }
    }
    questions.end()?;
    Ok(events)
}

/// The permanents `player` controls with a time counter on them, in the board's order, then the suspended cards they
/// own (in exile, with suspend and a time counter, CR 702.62b), in the order of exile.
fn c8_time_holders(state: &GameState, player: PlayerId) -> Vec<ObjectId> {
    let time = time_counter();
    let permanents = state.zone(Zone::Battlefield).iter().copied().filter(|id| {
        state.object(*id).is_some_and(|object| {
            crate::characteristics::controller_of_object(state, object) == player
                && object
                    .permanent()
                    .is_some_and(|permanent| permanent.counters_of(&time) > 0)
        })
    });
    let suspended = state.zone(Zone::Exile).iter().copied().filter(|id| {
        state.object(*id).is_some_and(|object| {
            object.owner() == player
                && object.card_counters_of(&time) > 0
                && keywords_of(state, *id)
                    .iter()
                    .any(|keyword| matches!(keyword, Keyword::Suspend { .. }))
        })
    });
    permanents.chain(suspended).collect()
}

#[cfg(test)]
mod c8_time_tests;
