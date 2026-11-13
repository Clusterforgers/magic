//! The endgame search, as chess engines probe for mate before searching: when an opponent is within reach of the
//! seat's creatures and burn, each answer worth trying is played to the end of the turn in several redealt worlds, and
//! one that wins the game in every world is played at once.

use std::time::Instant;

use mtg_core::ids::PlayerId;
use mtg_engine::{Decision, Engine};

use super::deep::{candidates, rollout_settings};
use super::eval::Weights;
use super::seen::Seen;
use super::sim::{Horizon, SIM_STEPS, Search, Start, minimal, reached};
use super::threat;
use crate::bot::apply_answer;
use mtg_engine::decision::Answer;

/// Redealt worlds an answer must win in to count as a forced win.
const WORLDS: u64 = 3;

/// Answers tried at most.
const TRIED: usize = 10;

/// Whether an opponent of the seat is within reach: life at most the seat's team's creature power (doubled for double
/// strike) and three for each burn spell in the seat's hand.
pub fn in_reach(seen: &Seen<'_>) -> bool {
    let bodies = seen.bodies();
    let power: i32 = bodies
        .iter()
        .filter(|body| body.is_creature() && seen.is_teammate(body.controller))
        .map(|body| body.power().max(0) * if body.keys.double_strike { 2 } else { 1 })
        .sum();
    let burn = seen
        .own_hand()
        .iter()
        .filter_map(|id| seen.card(*id))
        .filter(|card| threat::traits(card).burn)
        .count();
    let reach = power + 3 * i32::try_from(burn).unwrap_or(i32::MAX / 4);
    seen.opponents()
        .into_iter()
        .any(|player| seen.is_alive(player) && seen.life(player) <= reach)
}

/// An answer to `decision`, the seat's in its own turn, that wins the game by the end of the turn in every one of
/// [`WORLDS`] redealt worlds (keyed by `key`); `None` when no opponent is within reach ([`in_reach`]) or no answer
/// wins in all of them, or the clock reaches `until` first.
pub fn forced_win(
    engine: &Engine,
    seat: PlayerId,
    decision: &Decision,
    key: u64,
    until: Option<Instant>,
) -> Option<Answer> {
    let seen = Seen::new(engine.state(), seat);
    if decision.player != seat || !seen.is_active(seat) || !in_reach(&seen) {
        return None;
    }
    let tries = candidates(engine, seat, decision, &[], 30, 4);
    tries
        .into_iter()
        .take(TRIED)
        .take_while(|_| until.is_none_or(|until| Instant::now() < until))
        .find(|answer| (0..WORLDS).all(|world| wins(engine, seat, answer, key ^ (world + 1))))
}

/// Whether giving `answer` in a world redealt with `key` and playing on with the quick policies wins the game for the
/// seat's team by the end of the turn.
fn wins(engine: &Engine, seat: PlayerId, answer: &Answer, key: u64) -> bool {
    let mut world = engine.clone();
    world.redeal(seat, key);
    let Some(decision) = world.decision().cloned() else {
        return false;
    };
    let start = Start {
        turn: world.state().turn().number(),
    };
    if apply_answer(&mut world, &decision, answer.clone()).is_err() {
        return false;
    }
    let mut rollout = Search::new(seat, rollout_settings(Weights::NORMAL), key);
    rollout.prime(&world);
    for step in 1..SIM_STEPS {
        let Some(decision) = world.decision().cloned() else {
            break;
        };
        if world.result().is_some() || reached(&world, &decision, start, Horizon::EndOfTurn, step) {
            break;
        }
        let quick = rollout.quick(&world, &decision);
        if apply_answer(&mut world, &decision, quick).is_err() {
            let Some(fallback) = minimal(&decision) else {
                break;
            };
            if apply_answer(&mut world, &decision, fallback).is_err() {
                break;
            }
        }
    }
    Seen::new(world.state(), seat).over() == Some(true)
}
