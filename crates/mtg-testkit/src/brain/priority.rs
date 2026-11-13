//! What the seat does with priority: the land that makes the most of its hand, then the play whose simulated outcome
//! beats passing, in the windows where acting pays.

use mtg_core::ids::ObjectId;
use mtg_engine::Engine;
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction, PriorityOptions};
use mtg_engine::turn::Step;

use super::seen::Seen;
use super::sim::{Horizon, Search};
use super::threat;
use crate::bot::apply_answer;

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

/// The seat's play at priority: a land in its main phase, else the play whose outcome beats passing by the knob `margin`
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
        Horizon::EndOfTurn
    } else if window == Window::Main && search.settings().lookahead {
        Horizon::NextTurn
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
        let knobs = super::knobs::knobs();
        let ramp = if window == Window::Main && count(baseline.sources) < knobs.ramp_until {
            knobs.ramp * count(outcome.sources.saturating_sub(baseline.sources))
        } else {
            0.0
        };
        let score =
            outcome.score + ramp - hold_cost(search.settings().careful, &seen, window, &play);
        if follow_ups {
            scored.push((score, play.clone(), after));
        }
        if score > baseline.score + knobs.margin
            && best.as_ref().is_none_or(|(held, _)| score > *held)
        {
            best = Some((score, play));
        }
    }
    if follow_ups {
        best = best_sequence(
            search,
            scored,
            baseline.score + super::knobs::knobs().margin,
        )
        .or(best);
    }
    best.map_or(PriorityAction::Pass, |(_, play)| play)
}

/// The share of held interaction's worth that tapping below it loses: a counterspell only works in reply, so all of
/// it; removal and tricks can still be cast on the seat's own turn, so a quarter.
fn reactive(share: f64) -> f64 {
    let knobs = super::knobs::knobs();
    if (share - knobs.counter).abs() < f64::EPSILON {
        share
    } else {
        share * knobs.reactive
    }
}

/// The cards in hand from which the seat spends its mana rather than keep it up: past the maximum hand size it would
/// discard.
const CROWDED_HAND: usize = 8;

/// What `play` must gain beyond the margin in `window` when the seat plays carefully: a main-phase play that leaves
/// too little mana for the instant-speed interaction the seat holds pays for that interaction's worth
/// ([`threat::hold_value`]); a flash permanent or other instant in the main phase waits for the opponents' turn.
/// Interaction cast at any time already pays its worth in the evaluation ([`threat::held_interaction`]).
pub fn hold_cost(careful: bool, seen: &Seen<'_>, window: Window, play: &PriorityAction) -> f64 {
    if !careful || window != Window::Main {
        return 0.0;
    }
    let PriorityAction::CastSpell(id) = play else {
        return 0.0;
    };
    let Some(card) = seen.card(*id) else {
        return 0.0;
    };
    let traits = threat::traits(card);
    let opponents = !seen.opponents().is_empty();
    let mut cost = 0.0;
    if opponents && traits.fast {
        cost += if traits.permanent {
            super::knobs::knobs().flash_wait
        } else {
            super::knobs::knobs().instant_wait
        };
    }
    let bodies = seen.bodies();
    let open = threat::open_mana(seen, &bodies);
    let after = open.saturating_sub(seen.mana_value(*id));
    let held = threat::fast_interaction(seen, Some(*id));
    let lost: f64 = held
        .iter()
        .filter(|(cost, _)| *cost <= open && *cost > after)
        .map(|(_, share)| reactive(*share))
        .fold(0.0, f64::max);
    // A hand that would go past its maximum size at the end of the turn spends rather than discards.
    let crowded = seen.own_hand().len() >= CROWDED_HAND;
    if lost > 0.0 && opponents && !crowded {
        cost += super::knobs::knobs().tap_out * lost * threat::hold_value(seen);
    }
    cost
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
