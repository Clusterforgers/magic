//! Information-set Monte Carlo tree search (single observer) over the seat's own decisions: each iteration deals one
//! possible world ([`Engine::redeal`]: the hidden cards drawn from what the seat knows, never from what they are),
//! walks the tree of the seat's casts, attacks, blocks and targets in that world by UCB1 over the answers available
//! there, adds one node, plays on to the horizon with every seat answering by the quick heuristics Normal is built
//! from, and scores where it stops. The root answer tried most often wins, the first candidate (the heuristic answer)
//! on a tie.

use std::time::{Duration, Instant};

use mtg_core::ids::PlayerId;
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_engine::{Decision, Engine};

use super::eval::{DECIDED, Weights, evaluate_with};
use super::seen::Seen;
use super::sim::{Horizon, SIM_STEPS, Search, Settings, Start, minimal, reached};
use super::{combat, priority, targets};
use crate::bot::apply_answer;

/// How a deep search spends its budget on one decision.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Deep {
    /// Iterations (one world, one walk of the tree, one rollout each) per decision.
    pub iterations: u32,
    /// Turns each rollout plays past the current one, through that turn's combat.
    pub turns: u32,
    /// The evaluation gain that counts as most of a won game: rewards are a logistic of the gain over this.
    pub scale: f64,
    /// The UCB1 exploration constant on rewards between 0 and 1.
    pub explore: f64,
    /// Combat forecasts for building block candidates inside the tree.
    pub forecasts: u32,
    /// Targets tried for one target question inside the tree.
    pub targets: usize,
    /// A wall-clock cap per decision; `None` searches by count alone, so the answers replay with the seed.
    pub time: Option<Duration>,
}

impl Deep {
    /// A full budget: up to 600 iterations through the combat of the turn after next, about 2.5 s at most.
    pub const FULL: Deep = Deep {
        iterations: 600,
        turns: 2,
        scale: 25.0,
        explore: 0.4,
        forecasts: 40,
        targets: 4,
        time: Some(Duration::from_millis(2500)),
    };

    /// This budget without a clock: the answers depend on the seed alone.
    #[must_use]
    pub const fn untimed(mut self) -> Deep {
        self.time = None;
        self
    }
}

/// The answers worth searching for `decision`, the seat's, pending in `engine`, the likeliest first; none when the
/// decision is not one that matters (a priority window where acting does not pay, a main phase with a land to play,
/// a question of another kind) or has a single answer. `held` lists plays the seat will not repeat this turn.
pub fn candidates(
    engine: &Engine,
    seat: PlayerId,
    decision: &Decision,
    held: &[PriorityAction],
    forecasts: u32,
    most_targets: usize,
) -> Vec<Answer> {
    let seen = Seen::new(engine.state(), seat);
    let answers: Vec<Answer> = match &decision.kind {
        DecisionKind::Priority(options) => {
            let window = priority::window(&seen);
            if window == priority::Window::Idle
                || (window == priority::Window::Main && !options.playable_lands.is_empty())
            {
                return Vec::new();
            }
            let mut plays: Vec<PriorityAction> = priority::plays(&seen, options)
                .into_iter()
                .filter(|play| !held.contains(play))
                .collect();
            if plays.is_empty() {
                return Vec::new();
            }
            // Developing comes first in the main phase, keeping the card otherwise.
            if window == priority::Window::Main {
                plays.push(PriorityAction::Pass);
            } else {
                plays.insert(0, PriorityAction::Pass);
            }
            plays.into_iter().map(Answer::Priority).collect()
        }
        DecisionKind::DeclareAttackers(options) if !options.attackers.is_empty() => {
            combat::attack_candidates(&seen, options)
                .into_iter()
                .map(Answer::DeclareAttackers)
                .collect()
        }
        DecisionKind::DeclareBlockers(options) if !options.blockers.is_empty() => {
            combat::block_candidates(engine, seat, options, forecasts)
                .into_iter()
                .map(Answer::DeclareBlockers)
                .collect()
        }
        DecisionKind::ChooseTargets(options) if options.min.max(1) == 1 => {
            targets::rank_targets(&seen, options)
                .into_iter()
                .take(most_targets.max(2))
                .map(|target| targets::target_answer(options, vec![target]))
                .collect()
        }
        _ => Vec::new(),
    };
    if answers.len() < 2 {
        return Vec::new();
    }
    answers
}

/// The cost (of `costs`, by root answer) of the root answer `path` starts with.
fn root_cost(path: &[(usize, usize)], costs: &[f64]) -> f64 {
    path.first()
        .filter(|(at, _)| *at == 0)
        .and_then(|(_, pick)| costs.get(*pick))
        .copied()
        .unwrap_or(0.0)
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
        careful: true,
        sequences: false,
        fair: true,
        weights,
        deep: None,
        votes: 1,
        time: None,
    }
}

/// One answer at a node of the tree and its tally.
#[derive(Debug, Clone)]
struct Child {
    answer: Answer,
    visits: u32,
    reward: f64,
    /// The iterations in which it was available.
    available: u32,
    /// The node of the seat's next decision after it, once expanded.
    next: Option<usize>,
}

impl Child {
    fn new(answer: Answer) -> Child {
        Child {
            answer,
            visits: 0,
            reward: 0.0,
            available: 0,
            next: None,
        }
    }

    fn mean(&self) -> f64 {
        if self.visits == 0 {
            0.0
        } else {
            self.reward / f64::from(self.visits)
        }
    }
}

/// The child of `children` to walk among `open` (indices of the available ones): the first untried one, else the best
/// UCB1 bound over its availability; the earlier on a tie.
fn select(children: &[Child], open: &[usize], explore: f64) -> Option<usize> {
    if let Some(untried) = open.iter().copied().find(|at| children[*at].visits == 0) {
        return Some(untried);
    }
    let bound = |at: usize| {
        let child = &children[at];
        let log = f64::from(child.available.max(1)).ln();
        child.mean() + explore * (log / f64::from(child.visits)).sqrt()
    };
    open.iter()
        .copied()
        .reduce(|best, at| if bound(at) > bound(best) { at } else { best })
}

/// The answer to the decision `seat` faces in `engine` among `root` (the heuristic answer first) that the most
/// iterations of the tree search chose, the earlier on a tie; `costs` (by root answer) are taken off the scores of
/// the rollouts through each, for what the evaluation does not see (mana kept for held interaction); `budget` scales
/// the iterations and the clock (a busy server thinks less). `None` when no iteration ran.
// One search's inputs.
#[allow(clippy::too_many_arguments)]
pub fn ismcts(
    engine: &Engine,
    seat: PlayerId,
    root: &[Answer],
    costs: &[f64],
    deep: Deep,
    weights: Weights,
    seed: u64,
    budget: f64,
) -> Option<Answer> {
    if root.len() < 2 {
        return root.first().cloned();
    }
    let share = budget.clamp(0.0, 1.0);
    let deadline = deep.time.map(|time| Instant::now() + time.mul_f64(share));
    // In range: at most deep.iterations.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let iterations = (f64::from(deep.iterations) * share).round() as u32;
    let base = evaluate_with(&Seen::new(engine.state(), seat), weights);
    let settings = rollout_settings(weights);
    let mut tree: Vec<Vec<Child>> = vec![root.iter().cloned().map(Child::new).collect()];
    for iteration in 0..iterations {
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            break;
        }
        let key = seed ^ u64::from(iteration + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        let mut world = engine.clone();
        world.redeal(seat, key);
        let mut rollout = Search::new(seat, settings, key);
        rollout.prime(&world);
        let start = Start {
            turn: world.state().turn().number(),
        };
        let mut path: Vec<(usize, usize)> = Vec::new();
        let mut node = Some(0);
        for step in 0..SIM_STEPS {
            let Some(decision) = world.decision().cloned() else {
                break;
            };
            if world.result().is_some()
                || reached(&world, &decision, start, Horizon::Turns(deep.turns), step)
            {
                break;
            }
            let mut answer = None;
            if let Some(at) = node
                && decision.player == seat
            {
                let available = if step == 0 {
                    root.to_vec()
                } else {
                    candidates(&world, seat, &decision, &[], deep.forecasts, deep.targets)
                };
                if !available.is_empty() {
                    let mut open = Vec::with_capacity(available.len());
                    for offered in available {
                        let index = if let Some(index) =
                            tree[at].iter().position(|c| c.answer == offered)
                        {
                            index
                        } else {
                            tree[at].push(Child::new(offered));
                            tree[at].len() - 1
                        };
                        tree[at][index].available += 1;
                        open.push(index);
                    }
                    if let Some(pick) = select(&tree[at], &open, deep.explore) {
                        path.push((at, pick));
                        let child = &tree[at][pick];
                        answer = Some(child.answer.clone());
                        node = if child.visits == 0 {
                            // Expanded: the rest of this iteration is a rollout.
                            None
                        } else if let Some(next) = child.next {
                            Some(next)
                        } else {
                            tree.push(Vec::new());
                            let next = tree.len() - 1;
                            tree[at][pick].next = Some(next);
                            Some(next)
                        };
                    }
                }
            }
            let answer = answer.unwrap_or_else(|| rollout.quick(&world, &decision));
            if apply_answer(&mut world, &decision, answer).is_err() {
                let Some(fallback) = minimal(&decision) else {
                    break;
                };
                if apply_answer(&mut world, &decision, fallback).is_err() {
                    break;
                }
            }
        }
        let score =
            evaluate_with(&Seen::new(world.state(), seat), weights) - root_cost(&path, costs);
        let gained = reward(score, base, deep.scale);
        for (at, pick) in path {
            let child = &mut tree[at][pick];
            child.visits += 1;
            child.reward += gained;
        }
    }
    let mut best: Option<&Child> = None;
    for child in &tree[0] {
        if child.visits > 0 && best.is_none_or(|held| child.visits > held.visits) {
            best = Some(child);
        }
    }
    best.map(|child| child.answer.clone())
}

#[cfg(test)]
mod tests {
    //! Goal: rewards order gains and decided games, and UCB1 over the available answers tries each before it favours the
    //! best.
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

    fn child(visits: u32, reward: f64, available: u32) -> Child {
        Child {
            visits,
            reward,
            available,
            ..Child::new(Answer::Priority(PriorityAction::Pass))
        }
    }

    #[test]
    fn untried_answers_come_first_then_the_best_bound() {
        let children = vec![child(3, 2.0, 6), child(0, 0.0, 6), child(3, 2.0, 6)];
        assert_eq!(select(&children, &[0, 1, 2], 0.4), Some(1));
        let poor = child(10, 1.0, 20);
        let good = child(10, 9.0, 20);
        assert_eq!(select(&[poor.clone(), good.clone()], &[0, 1], 0.4), Some(1));
        assert_eq!(select(&[poor, good], &[0], 0.4), Some(0));
        assert_eq!(select(&[], &[], 0.4), None);
    }
}
