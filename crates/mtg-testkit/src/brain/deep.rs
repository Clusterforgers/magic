//! Information-set Monte Carlo search over a seat's candidate answers: each world is one possible deal of the hidden
//! cards ([`Engine::redeal`]: drawn from what the seat knows, never from what they are), every candidate still in the
//! running is played on in it to a horizon with every seat answering by the quick heuristics Normal is built from,
//! and the position reached is scored. Sequential halving keeps the better half of the candidates each round.

use std::time::{Duration, Instant};

use mtg_core::ids::PlayerId;
use mtg_engine::Engine;
use mtg_engine::decision::Answer;

use super::eval::{DECIDED, Weights, evaluate_with};
use super::seen::Seen;
use super::sim::{Horizon, Search, Settings};

/// How a deep search spends its budget on one decision.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Deep {
    /// Iterations (one world, one candidate, one rollout each) per decision.
    pub iterations: u32,
    /// Turns each rollout plays past the current one, through that turn's combat.
    pub turns: u32,
    /// The evaluation gain that counts as most of a won game: rewards are a logistic of the gain over this.
    pub scale: f64,
    /// A wall-clock cap per decision; `None` searches by count alone, so the answers replay with the seed.
    pub time: Option<Duration>,
}

impl Deep {
    /// A full budget: up to 1200 iterations through the end of the turn after next, about 2.5 s at most.
    pub const FULL: Deep = Deep {
        iterations: 1200,
        turns: 2,
        scale: 25.0,
        time: Some(Duration::from_millis(2500)),
    };

    /// This budget without a clock: the answers depend on the seed alone.
    #[must_use]
    pub const fn untimed(mut self) -> Deep {
        self.time = None;
        self
    }
}

/// The reward of a rollout that scored `score` from a position scored `base`: 1 for a won game, 0 for a lost one, else
/// a logistic of the gain over `scale`.
fn reward(score: f64, base: f64, scale: f64) -> f64 {
    if score >= DECIDED {
        return 1.0;
    }
    if score <= -DECIDED {
        return 0.0;
    }
    1.0 / (1.0 + (-(score - base) / scale).exp())
}

/// The settings of one rollout: no searching inside it, every seat playing its lands and spells.
fn rollout_settings(weights: Weights) -> Settings {
    Settings {
        sims: 1,
        forecasts: 0,
        targets: 1,
        depth: 0,
        combat_search: false,
        lookahead: true,
        sequences: false,
        fair: true,
        weights,
        deep: None,
        votes: 1,
        time: None,
    }
}

/// The rollouts one sequential-halving round gives each of `alive` candidates out of `iterations` over `rounds`
/// rounds: at least one.
fn per_round(iterations: u32, alive: usize, rounds: u32) -> u32 {
    let alive = u32::try_from(alive).unwrap_or(u32::MAX).max(1);
    (iterations / (alive * rounds.max(1))).max(1)
}

/// The rounds sequential halving takes to bring `candidates` down to one.
fn rounds_for(candidates: usize) -> u32 {
    usize::BITS - candidates.saturating_sub(1).leading_zeros()
}

/// The candidate of `candidates` for the decision `seat` faces in `engine` with the best mean reward, found by
/// sequential halving: each round plays every surviving candidate on the same freshly dealt worlds (so they are
/// compared on equal luck) and keeps the better half. `budget` scales the iterations and the clock (a busy server
/// thinks less). `None` when no world could be played.
pub fn deep_choice(
    engine: &Engine,
    seat: PlayerId,
    candidates: &[Answer],
    deep: Deep,
    weights: Weights,
    seed: u64,
    budget: f64,
) -> Option<Answer> {
    if candidates.len() < 2 {
        return candidates.first().cloned();
    }
    let share = budget.clamp(0.0, 1.0);
    let deadline = deep.time.map(|time| Instant::now() + time.mul_f64(share));
    let late = || deadline.is_some_and(|deadline| Instant::now() >= deadline);
    // In range: at most deep.iterations.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let iterations = (f64::from(deep.iterations) * share).round() as u32;
    let base = evaluate_with(&Seen::new(engine.state(), seat), weights);
    let settings = rollout_settings(weights);
    let rounds = rounds_for(candidates.len());
    let mut alive: Vec<usize> = (0..candidates.len()).collect();
    let mut worlds = 0u64;
    let mut best: Option<usize> = None;
    'rounds: for _ in 0..rounds {
        let mut totals = vec![0.0; candidates.len()];
        let mut played = vec![0u32; candidates.len()];
        let mut complete = 0;
        for _ in 0..per_round(iterations, alive.len(), rounds) {
            if late() {
                break;
            }
            worlds += 1;
            let key = seed ^ worlds.wrapping_mul(0x9E37_79B9_7F4A_7C15);
            let mut world = engine.clone();
            world.redeal(seat, key);
            for at in &alive {
                let mut rollout = Search::new(seat, settings, key);
                if let Some((outcome, _)) = rollout.play_out(
                    world.clone(),
                    candidates[*at].clone(),
                    Horizon::Turns(deep.turns),
                ) {
                    totals[*at] += reward(outcome.score, base, deep.scale);
                    played[*at] += 1;
                }
            }
            complete += 1;
        }
        if complete == 0 {
            break 'rounds;
        }
        alive.retain(|at| played[*at] > 0);
        alive.sort_by(|a, b| {
            let mean = |at: usize| totals[at] / f64::from(played[at]);
            mean(*b).total_cmp(&mean(*a)).then(a.cmp(b))
        });
        best = alive.first().copied();
        if alive.len() <= 1 || late() {
            break;
        }
        alive.truncate(alive.len().div_ceil(2));
    }
    best.map(|at| candidates[at].clone())
}

#[cfg(test)]
mod tests {
    //! Goal: rewards order gains and decided games, and sequential halving splits its budget into enough rounds.
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case(0.0, 10.0)]
    #[case(-30.0, -5.0)]
    #[case(40.0, 41.0)]
    fn a_bigger_gain_is_a_bigger_reward(#[case] worse: f64, #[case] better: f64) {
        assert!(reward(better, 0.0, 25.0) > reward(worse, 0.0, 25.0));
        assert!(reward(DECIDED, 0.0, 25.0) > reward(better, 0.0, 25.0));
        assert!(reward(-DECIDED, 0.0, 25.0) < reward(worse, 0.0, 25.0));
    }

    #[rstest]
    #[case(2, 1)]
    #[case(3, 2)]
    #[case(4, 2)]
    #[case(5, 3)]
    #[case(16, 4)]
    fn halving_takes_enough_rounds(#[case] candidates: usize, #[case] rounds: u32) {
        assert_eq!(rounds_for(candidates), rounds);
        assert!(per_round(0, candidates, rounds) >= 1);
        assert!(
            per_round(1200, candidates, rounds) * u32::try_from(candidates).unwrap() * rounds
                <= 1200
        );
    }
}
