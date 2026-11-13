//! Information-set Monte Carlo tree search (single observer) over the seat's own decisions: each iteration deals one
//! possible world ([`Engine::redeal`]: the hidden cards drawn from what the seat knows, never from what they are),
//! walks the tree of the seat's casts, attacks, blocks and targets in that world by UCB1 over the answers available
//! there, adds one node, plays on to the horizon with every seat answering by the quick heuristics Normal is built
//! from, and scores where it stops. The root answer tried most often wins, the first candidate (the heuristic answer)
//! on a tie. The search is anytime under a per-decision [`Budget`] (soft and hard limits), prunes root answers that are
//! clearly worse than the best (late move reductions), merges root answers that do the same thing (transpositions) and
//! orders untried answers by what earlier searches this game learnt ([`History`]).

use std::time::Duration;

use mtg_core::ids::PlayerId;
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_engine::{Decision, Engine};

use super::clock::Budget;
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
    /// The soft time limit of a decision of ordinary importance ([`super::clock::Budget`]); `None` searches by count
    /// alone, so the answers replay with the seed.
    pub time: Option<Duration>,
    /// The mean reward by which the tree's favourite must beat the heuristic answer (the first root answer) to replace
    /// it: rollouts are noisy, and the heuristic answer carries the policy's knowledge the rollouts lack.
    pub trust: f64,
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
        trust: 0.05,
    };

    /// A light budget for Hard: up to 150 iterations, about 0.6 s for a decision of ordinary importance.
    pub const LIGHT: Deep = Deep {
        iterations: 150,
        time: Some(Duration::from_millis(600)),
        ..Deep::FULL
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
pub(crate) fn rollout_settings(weights: Weights) -> Settings {
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
        endgame: false,
    }
}

/// One answer at a node of the tree and its tally.
#[derive(Debug, Clone)]
struct Child {
    answer: Answer,
    visits: u32,
    reward: f64,
    /// The sum of the squared rewards, for the spread of the mean.
    squares: f64,
    /// The iterations in which it was available.
    available: u32,
    /// The node of the seat's next decision after it, once expanded.
    next: Option<usize>,
    /// Pruned at the root: clearly worse than the best ([`prune`]).
    pruned: bool,
}

impl Child {
    fn new(answer: Answer) -> Child {
        Child {
            answer,
            visits: 0,
            reward: 0.0,
            squares: 0.0,
            available: 0,
            next: None,
            pruned: false,
        }
    }

    fn mean(&self) -> f64 {
        if self.visits == 0 {
            0.0
        } else {
            self.reward / f64::from(self.visits)
        }
    }

    /// The standard error of the mean reward.
    fn spread(&self) -> f64 {
        if self.visits < 2 {
            return 1.0;
        }
        let n = f64::from(self.visits);
        let mean = self.mean();
        ((self.squares / n - mean * mean).max(0.0) / n).sqrt()
    }
}

/// What the seat learnt about its answers in earlier searches this game, by what they do ([`answer_key`]): the mean
/// reward each beat its siblings by. Untried answers are tried in its order, the history heuristic of chess engines.
#[derive(Debug, Clone, Default)]
pub struct History {
    table: std::collections::BTreeMap<String, (f64, u32)>,
}

impl History {
    /// The mean advantage of answers like `key`, 0 when unknown.
    fn score(&self, key: &str) -> f64 {
        self.table
            .get(key)
            .map_or(0.0, |(sum, n)| sum / f64::from((*n).max(1)))
    }

    /// Records that an answer like `key` beat its siblings by `advantage`.
    fn record(&mut self, key: String, advantage: f64) {
        let entry = self.table.entry(key).or_insert((0.0, 0));
        entry.0 += advantage;
        entry.1 += 1;
    }
}

/// What `answer` does in `engine`, the same for answers that do the same thing (two copies of one card cast, say):
/// the key of the history table and of the transpositions merged at the root.
pub fn answer_key(engine: &Engine, answer: &Answer) -> String {
    let state = engine.state();
    let name = |id: mtg_core::ids::ObjectId| {
        state
            .object(id)
            .map_or_else(|| format!("{id:?}"), |object| object.card().name.clone())
    };
    match answer {
        Answer::Priority(PriorityAction::CastSpell(id)) => format!("cast {}", name(*id)),
        Answer::Priority(PriorityAction::PlayLand(id)) => format!("land {}", name(*id)),
        Answer::Priority(PriorityAction::Pass) => "pass".to_owned(),
        Answer::DeclareAttackers(attack) => format!("attack {}", attack.len()),
        Answer::DeclareBlockers(blocks) => format!("block {}", blocks.len()),
        other => format!("{other:?}"),
    }
}

/// The root answers of `root` without transpositions: an answer that does what an earlier one does (a second copy of
/// the same card) is dropped; casts and land plays are compared by card, the rest exactly.
pub fn distinct(engine: &Engine, root: Vec<Answer>) -> Vec<Answer> {
    let mut keys: Vec<String> = Vec::new();
    let mut kept = Vec::new();
    for answer in root {
        let key = match &answer {
            Answer::Priority(PriorityAction::CastSpell(_) | PriorityAction::PlayLand(_)) => {
                answer_key(engine, &answer)
            }
            other => format!("{other:?}"),
        };
        if !keys.contains(&key) {
            keys.push(key);
            kept.push(answer);
        }
    }
    kept
}

/// The child of `children` to walk among `open` (indices of the available ones, pruned ones left out): the first
/// untried one, else the best UCB1 bound over its availability, for the seat or, at an opponent's node (`minimize`),
/// against it; the earlier on a tie.
fn select(children: &[Child], open: &[usize], explore: f64, minimize: bool) -> Option<usize> {
    if let Some(untried) = open.iter().copied().find(|at| children[*at].visits == 0) {
        return Some(untried);
    }
    let bound = |at: usize| {
        let child = &children[at];
        let log = f64::from(child.available.max(1)).ln();
        let mean = if minimize {
            1.0 - child.mean()
        } else {
            child.mean()
        };
        mean + explore * (log / f64::from(child.visits)).sqrt()
    };
    open.iter()
        .copied()
        .filter(|at| !children[*at].pruned)
        .reduce(|best, at| if bound(at) > bound(best) { at } else { best })
}

/// Visits a root answer needs before it may be pruned or prune others.
const PRUNE_VISITS: u32 = 12;

/// The mean reward below the best, beyond two standard errors, at which a root answer is pruned.
const PRUNE_MARGIN: f64 = 0.03;

/// Late move reductions at the root: prunes each answer (the heuristic answer, first, aside) whose mean reward is
/// clearly below the best's: below by [`PRUNE_MARGIN`] even at two standard errors above its mean.
fn prune(root: &mut [Child]) {
    let Some(best) = root
        .iter()
        .filter(|child| child.visits >= PRUNE_VISITS && !child.pruned)
        .map(Child::mean)
        .reduce(f64::max)
    else {
        return;
    };
    for child in root.iter_mut().skip(1) {
        if child.visits >= PRUNE_VISITS && child.mean() + 2.0 * child.spread() < best - PRUNE_MARGIN
        {
            child.pruned = true;
        }
    }
}

/// The most visited root answer's index, the earlier on a tie.
fn leader(root: &[Child]) -> Option<usize> {
    let mut best: Option<usize> = None;
    for (at, child) in root.iter().enumerate() {
        if child.visits > 0 && best.is_none_or(|held| child.visits > root[held].visits) {
            best = Some(at);
        }
    }
    best
}

/// Whether the search can stop early: one root answer is left unpruned, or no other can catch the leader's visits
/// in the iterations left.
fn settled(root: &[Child], left: u32) -> bool {
    let open = root.iter().filter(|child| !child.pruned).count();
    if open <= 1 {
        return true;
    }
    let Some(lead) = leader(root) else {
        return false;
    };
    let second = root
        .iter()
        .enumerate()
        .filter(|(at, child)| *at != lead && !child.pruned)
        .map(|(_, child)| child.visits)
        .max()
        .unwrap_or(0);
    root[lead].visits > second + left
}

/// The redealt worlds one search cycles through ([`ismcts`]).
const WORLDS: u32 = 32;

/// How often (in iterations) the root is pruned and its leader checked.
const CHECK_EVERY: u32 = 16;

/// The answer to the decision `seat` faces in `engine` among `root` (the heuristic answer first) that the most
/// iterations of the tree search chose, the earlier on a tie; `costs` (by root answer) are taken off the scores of
/// the rollouts through each, for what the evaluation does not see (mana kept for held interaction); `budget` bounds
/// the iterations and the clock ([`Budget`]): past the soft limit the search stops once its leader is stable, at the
/// hard limit whatever its state, a rollout cut short then left unscored. Root answers clearly worse than the best are
/// pruned ([`prune`]), untried answers come in `history`'s order, and what the search learnt goes back into it. `None`
/// when no iteration ran.
// One search's inputs.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub fn ismcts(
    engine: &Engine,
    seat: PlayerId,
    root: &[Answer],
    costs: &[f64],
    deep: Deep,
    weights: Weights,
    seed: u64,
    budget: Budget,
    history: &mut History,
) -> Option<Answer> {
    if root.len() < 2 {
        return root.first().cloned();
    }
    let base = evaluate_with(&Seen::new(engine.state(), seat), weights);
    let settings = rollout_settings(weights);
    let root_keys: Vec<String> = root.iter().map(|a| answer_key(engine, a)).collect();
    // The heuristic answer first, then the others in the history's order.
    let mut order: Vec<usize> = (1..root.len()).collect();
    order.sort_by(|a, b| {
        history
            .score(&root_keys[*b])
            .total_cmp(&history.score(&root_keys[*a]))
    });
    order.insert(0, 0);
    // At a table of more than two, a rollout plays at least once round the table, so every opponent's next attack (the
    // crack back) is seen.
    let seats = u32::try_from(Seen::new(engine.state(), seat).alive().len()).unwrap_or(2);
    let turns = deep.turns.max(seats);
    let mut tree: Vec<Vec<Child>> = vec![root.iter().cloned().map(Child::new).collect()];
    // Whose decision each node is: the seat's, or an opponent's blocks (a minimizing node).
    let mut owners: Vec<Option<PlayerId>> = vec![Some(seat)];
    let mut last_leader = None;
    let mut stable_checks = 0u32;
    for iteration in 0..budget.iterations {
        if budget.out() {
            break;
        }
        if iteration > 0 && iteration % CHECK_EVERY == 0 {
            prune(&mut tree[0]);
            if settled(&tree[0], budget.iterations - iteration) {
                break;
            }
            let lead = leader(&tree[0]);
            if lead == last_leader {
                stable_checks += 1;
            } else {
                stable_checks = 0;
                last_leader = lead;
            }
            if budget.past_soft() && stable_checks >= 2 {
                break;
            }
        }
        // Common random numbers: the iterations cycle through a fixed set of worlds, so the answers are compared in
        // the same deals and the same quick answers of the other seats.
        let world_index = iteration % WORLDS + 1;
        let key = seed ^ u64::from(world_index).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        let mut world = engine.clone();
        world.redeal(seat, key);
        let mut rollout = Search::new(seat, settings, key);
        rollout.prime(&world);
        let start = Start {
            turn: world.state().turn().number(),
        };
        let mut path: Vec<(usize, usize)> = Vec::new();
        let mut node = Some(0);
        let mut cut = false;
        for step in 0..SIM_STEPS {
            let Some(decision) = world.decision().cloned() else {
                break;
            };
            if world.result().is_some()
                || reached(&world, &decision, start, Horizon::Turns(turns), step)
            {
                break;
            }
            if step % 16 == 15 && budget.out() {
                cut = true;
                break;
            }
            let mut answer = None;
            let own = decision.player == seat;
            let opposing_blocks = !own
                && node.is_some()
                && matches!(decision.kind, DecisionKind::DeclareBlockers(_))
                && !Seen::new(world.state(), seat).is_teammate(decision.player);
            if let Some(at) = node
                && (own || opposing_blocks)
            {
                match owners.get(at).copied().flatten() {
                    Some(owner) if owner != decision.player => node = None,
                    Some(_) => {}
                    None => {
                        if let Some(slot) = owners.get_mut(at) {
                            *slot = Some(decision.player);
                        }
                    }
                }
            }
            if let Some(at) = node
                && (own || opposing_blocks)
            {
                let offered = if step == 0 {
                    Vec::new()
                } else if own {
                    candidates(&world, seat, &decision, &[], deep.forecasts, deep.targets)
                } else if let DecisionKind::DeclareBlockers(options) = &decision.kind {
                    combat::block_candidates(&world, decision.player, options, deep.forecasts)
                        .into_iter()
                        .map(Answer::DeclareBlockers)
                        .collect()
                } else {
                    Vec::new()
                };
                let available: Vec<usize> = if step == 0 {
                    order.clone()
                } else {
                    offered
                        .into_iter()
                        .map(|offered| {
                            if let Some(index) = tree[at].iter().position(|c| c.answer == offered) {
                                index
                            } else {
                                tree[at].push(Child::new(offered));
                                tree[at].len() - 1
                            }
                        })
                        .collect()
                };
                if !available.is_empty() {
                    for index in &available {
                        tree[at][*index].available += 1;
                    }
                    if let Some(pick) = select(&tree[at], &available, deep.explore, !own) {
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
                            owners.push(None);
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
        if cut {
            break;
        }
        let score =
            evaluate_with(&Seen::new(world.state(), seat), weights) - root_cost(&path, costs);
        let gained = reward(score, base, deep.scale);
        for (at, pick) in path {
            let child = &mut tree[at][pick];
            child.visits += 1;
            child.reward += gained;
            child.squares += gained * gained;
        }
    }
    let tried: Vec<&Child> = tree[0].iter().filter(|child| child.visits > 0).collect();
    if !tried.is_empty() {
        let mean = tried.iter().map(|child| child.mean()).sum::<f64>()
            / f64::from(u32::try_from(tried.len()).unwrap_or(1));
        for (at, child) in tree[0].iter().enumerate() {
            if child.visits > 0 {
                history.record(root_keys[at].clone(), child.mean() - mean);
            }
        }
    }
    choose(&tree[0], deep.trust)
}

/// The root answer to play: the most visited child, the earlier on a tie, unless the first (the heuristic answer) was
/// tried and the favourite's mean reward does not beat it by `trust` plus one standard error of the difference.
fn choose(root: &[Child], trust: f64) -> Option<Answer> {
    let mut best: Option<&Child> = None;
    for child in root {
        if child.visits > 0 && best.is_none_or(|held| child.visits > held.visits) {
            best = Some(child);
        }
    }
    let prior = root.first().filter(|child| child.visits > 0);
    match (best, prior) {
        (Some(best), Some(prior))
            if best.mean() < prior.mean() + trust + best.spread().hypot(prior.spread()) =>
        {
            Some(prior.answer.clone())
        }
        (best, _) => best.map(|child| child.answer.clone()),
    }
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

    #[rstest]
    #[case::clearly_better(0.6, 0.5, 1)]
    #[case::barely_better(0.53, 0.5, 0)]
    #[case::worse(0.4, 0.5, 0)]
    fn the_heuristic_answer_stands_unless_clearly_beaten(
        #[case] other: f64,
        #[case] prior: f64,
        #[case] chosen: usize,
    ) {
        let mut root = vec![child(20, prior * 20.0, 30), child(40, other * 40.0, 30)];
        root[1].answer = Answer::Priority(PriorityAction::BringCompanion);
        let picked = choose(&root, 0.05).unwrap();
        assert_eq!(picked, root[chosen].answer);
    }

    #[rstest]
    #[case::clearly_worse(0.3, true)]
    #[case::close(0.48, false)]
    fn root_answers_clearly_below_the_best_are_pruned(#[case] mean: f64, #[case] pruned: bool) {
        let mut root = vec![
            child(40, 0.1 * 40.0, 60),
            child(40, 0.5 * 40.0, 60),
            child(40, mean * 40.0, 60),
        ];
        for c in &mut root {
            let mean = c.mean();
            c.squares = f64::from(c.visits) * (mean * mean + 0.04);
        }
        prune(&mut root);
        assert!(!root[0].pruned, "the heuristic answer is never pruned");
        assert!(!root[1].pruned);
        assert_eq!(root[2].pruned, pruned);
    }

    #[rstest]
    #[case::one_left(vec![(30, false), (10, true)], 100, true)]
    #[case::uncatchable(vec![(90, false), (10, false)], 50, true)]
    #[case::open(vec![(40, false), (30, false)], 50, false)]
    fn a_search_settles_when_nothing_can_change(
        #[case] tallies: Vec<(u32, bool)>,
        #[case] left: u32,
        #[case] expected: bool,
    ) {
        let root: Vec<Child> = tallies
            .into_iter()
            .map(|(visits, pruned)| Child {
                pruned,
                ..child(visits, 0.0, visits)
            })
            .collect();
        assert_eq!(settled(&root, left), expected);
    }

    #[test]
    fn the_history_orders_by_mean_advantage() {
        let mut history = History::default();
        history.record("cast Bolt".into(), 0.2);
        history.record("cast Bolt".into(), 0.0);
        history.record("pass".into(), -0.1);
        assert!((history.score("cast Bolt") - 0.1).abs() < 1e-9);
        assert!(history.score("pass") < history.score("unknown"));
    }

    #[test]
    fn untried_answers_come_first_then_the_best_bound() {
        let children = vec![child(3, 2.0, 6), child(0, 0.0, 6), child(3, 2.0, 6)];
        assert_eq!(select(&children, &[0, 1, 2], 0.4, false), Some(1));
        let poor = child(10, 1.0, 20);
        let good = child(10, 9.0, 20);
        assert_eq!(
            select(&[poor.clone(), good.clone()], &[0, 1], 0.4, false),
            Some(1)
        );
        assert_eq!(
            select(&[poor.clone(), good.clone()], &[0], 0.4, false),
            Some(0)
        );
        assert_eq!(select(&[], &[], 0.4, false), None);
        assert_eq!(
            select(&[poor, good], &[0, 1], 0.4, true),
            Some(0),
            "an opponent picks the seat's worst"
        );
    }
}
