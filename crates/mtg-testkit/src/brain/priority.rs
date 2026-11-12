//! What the seat does with priority: the land that makes the most of its hand, then the play whose simulated outcome
//! beats passing, in the windows where acting pays.

use mtg_core::ids::ObjectId;
use mtg_engine::Engine;
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction, PriorityOptions};
use mtg_engine::turn::Step;

use super::seen::Seen;
use super::sim::{Horizon, Search};
use crate::bot::apply_answer;

/// How much a play must beat passing by.
const MARGIN: f64 = 0.25;

/// Mana sources below which a play that adds one counts [`RAMP`] more in the seat's main phase: ramp comes first.
const RAMP_UNTIL: usize = 5;
/// What each mana source a play adds counts besides its evaluation while the seat has fewer than [`RAMP_UNTIL`].
const RAMP: f64 = 4.0;

/// When the seat acts at priority, if at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Window {
    /// Its own main phase on an empty stack: lands, spells and abilities.
    Main,
    /// An opposing spell or ability is on top of the stack: responses.
    Response,
    /// Combat once creatures attack the seat's team or blockers are declared: tricks and removal, judged through the
    /// combat damage.
    Combat,
    /// An opponent's end step: what lasts beyond the turn.
    EndStep,
    /// Any other time: the seat passes.
    Idle,
}

/// The window the seat stands in when it has priority in `engine`.
pub fn window(seen: &Seen<'_>) -> Window {
    let turn = seen.turn();
    let ours = seen.is_active(seen.seat());
    let stack = seen.stack();
    if let Some(top) = stack.last() {
        return if seen.controller(*top).is_some_and(|p| !seen.is_teammate(p)) {
            Window::Response
        } else {
            Window::Idle
        };
    }
    match turn.step() {
        Step::Main1 | Step::Main2 if ours => Window::Main,
        Step::DeclareBlockers => Window::Combat,
        Step::DeclareAttackers if !ours => Window::Combat,
        Step::End if !ours => Window::EndStep,
        _ => Window::Idle,
    }
}

/// The plays the seat may make now, likeliest worth first (the costliest spells).
pub fn plays(seen: &Seen<'_>, options: &PriorityOptions) -> Vec<PriorityAction> {
    let mut casts: Vec<ObjectId> = options.castable.clone();
    casts.sort_by_key(|id| std::cmp::Reverse(seen.mana_value(*id)));
    let mut plays: Vec<PriorityAction> = casts.into_iter().map(PriorityAction::CastSpell).collect();
    plays.extend(
        options
            .abilities
            .iter()
            .copied()
            .map(PriorityAction::Activate),
    );
    plays.extend(
        options
            .turnable
            .iter()
            .copied()
            .map(PriorityAction::TurnFaceUp),
    );
    plays.extend(
        options
            .unlockable
            .iter()
            .map(|door| PriorityAction::UnlockDoor(door.card, door.face)),
    );
    if options.companion {
        plays.push(PriorityAction::BringCompanion);
    }
    plays
}

/// The land to play: the one after which the seat can cast the most of its hand (by mana value), the first on a tie.
pub fn choose_land(engine: &Engine, lands: &[ObjectId]) -> Option<PriorityAction> {
    let decision = engine.decision()?;
    let seat = decision.player;
    let mut best: Option<(u32, PriorityAction)> = None;
    let mut names: Vec<String> = Vec::new();
    for land in lands {
        let name = engine
            .state()
            .object(*land)
            .map(|object| object.card().name.clone())
            .unwrap_or_default();
        if names.contains(&name) {
            continue;
        }
        names.push(name);
        let action = PriorityAction::PlayLand(*land);
        let mut sim = engine.clone();
        if apply_answer(&mut sim, decision, Answer::Priority(action.clone())).is_err() {
            continue;
        }
        let seen = Seen::new(sim.state(), seat);
        let reach = match sim.decision().map(|d| &d.kind) {
            Some(DecisionKind::Priority(after)) => after
                .castable
                .iter()
                .map(|id| 1 + 2 * seen.mana_value(*id))
                .sum(),
            _ => 0,
        };
        if best.as_ref().is_none_or(|(held, _)| reach > *held) {
            best = Some((reach, action));
        }
    }
    best.map(|(_, action)| action)
}

/// The seat's play at priority: a land in its main phase, else the play whose outcome beats passing by [`MARGIN`]
/// (and, outside combat, changes something that lasts beyond the turn), else a pass. `held` lists plays the seat will
/// not repeat this turn.
pub fn plan_priority(
    search: &mut Search,
    engine: &Engine,
    options: &PriorityOptions,
    held: &[PriorityAction],
) -> PriorityAction {
    let seen = Seen::new(engine.state(), search.seat());
    let window = window(&seen);
    if window == Window::Main
        && !options.playable_lands.is_empty()
        && let Some(land) = choose_land(engine, &options.playable_lands)
    {
        return land;
    }
    if window == Window::Idle {
        return PriorityAction::Pass;
    }
    let plays: Vec<PriorityAction> = plays(&seen, options)
        .into_iter()
        .filter(|play| !held.contains(play))
        .collect();
    if plays.is_empty() {
        return PriorityAction::Pass;
    }
    let combat = window == Window::Combat;
    let horizon = if combat {
        Horizon::EndOfCombat
    } else {
        Horizon::Settle
    };
    let follow_ups = window == Window::Main && search.settings().sequences;
    let pass = Answer::Priority(PriorityAction::Pass);
    let Some(baseline) = search.outcome(engine, pass, horizon) else {
        return PriorityAction::Pass;
    };
    let mut best: Option<(f64, PriorityAction)> = None;
    let mut scored: Vec<(f64, PriorityAction, Engine)> = Vec::new();
    for play in plays {
        if !search.can_simulate() {
            break;
        }
        let Some((outcome, after)) = search.played(engine, Answer::Priority(play.clone()), horizon)
        else {
            continue;
        };
        if !combat && outcome.lasting == baseline.lasting {
            continue;
        }
        let ramp = if window == Window::Main && baseline.sources < RAMP_UNTIL {
            RAMP * count(outcome.sources.saturating_sub(baseline.sources))
        } else {
            0.0
        };
        let score = outcome.score + ramp;
        if follow_ups {
            scored.push((score, play.clone(), after));
        }
        if score > baseline.score + MARGIN && best.as_ref().is_none_or(|(held, _)| score > *held) {
            best = Some((score, play));
        }
    }
    if follow_ups {
        best = best_sequence(search, scored, baseline.score + MARGIN).or(best);
    }
    best.map_or(PriorityAction::Pass, |(_, play)| play)
}

/// How many of the best plays a lookahead tries a second play after.
const SEQUENCES: usize = 3;

/// The first play of the best two-play sequence among the [`SEQUENCES`] best of `scored` (each with its score and the
/// game after it): each is worth the best of itself and itself followed by any play the seat can make next, so mana is
/// spent on the pair that uses it best. `None` when no sequence beats `bar`.
fn best_sequence(
    search: &mut Search,
    mut scored: Vec<(f64, PriorityAction, Engine)>,
    bar: f64,
) -> Option<(f64, PriorityAction)> {
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut best: Option<(f64, PriorityAction)> = None;
    for (score, play, after) in scored.into_iter().take(SEQUENCES) {
        let mut worth = score;
        if let Some(DecisionKind::Priority(next)) = after.decision().map(|d| &d.kind)
            && after.decision().is_some_and(|d| d.player == search.seat())
        {
            let seen = Seen::new(after.state(), search.seat());
            for second in plays(&seen, next) {
                if !search.can_simulate() {
                    break;
                }
                if let Some(outcome) =
                    search.outcome(&after, Answer::Priority(second), Horizon::Settle)
                {
                    worth = worth.max(outcome.score);
                }
            }
        }
        if worth > bar && best.as_ref().is_none_or(|(held, _)| worth > *held) {
            best = Some((worth, play));
        }
    }
    best
}

/// A small count as a float.
fn count(n: usize) -> f64 {
    f64::from(u32::try_from(n).unwrap_or(u32::MAX))
}
