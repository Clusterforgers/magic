//! The search a brain runs on clones of the engine: give an answer, play on to a horizon with quick answers for every
//! seat, and score the position the seat reaches with [`evaluate`].
//!
//! The seat's own questions on the way are answered by its quick heuristics (and, to the depth its settings allow, its
//! targets are searched too). Every other seat passes priority, attacks and blocks by what the battlefield shows, and
//! answers anything else from the question alone ([`RandomBot::choose`]), so no simulated answer reads a hidden card.
//! Cards a simulation draws stay unread: the evaluation counts hands, it never looks into them.

use std::time::{Duration, Instant};

use mtg_core::ids::PlayerId;
use mtg_engine::choose::{Target, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind, MulliganChoice, PayChoice, PriorityAction};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Decision, Engine};

use super::combat::{assign_lethal, quick_attack, quick_blocks, safe_attack};
use super::deep::Deep;
use super::eval::{Weights, evaluate_with};
use super::seen::Seen;
use super::targets::{rank_targets, target_answer};
use crate::bot::{RandomBot, apply_answer};

/// The most answers one simulation gives before it is scored where it stands.
pub(crate) const SIM_STEPS: usize = 400;

/// Combat forecasts for each block declaration inside a simulation: fixed, so every candidate meets the same blocks.
const QUICK_FORECASTS: u32 = 30;

/// How much a brain may search for one decision, and how far ahead.
// Independent switches of one search.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Settings {
    /// Simulations per decision.
    pub sims: u32,
    /// Combat forecasts per block declaration.
    pub forecasts: u32,
    /// Targets tried for one target question.
    pub targets: usize,
    /// How many of its own target questions inside a simulation the seat searches rather than guesses.
    pub depth: u8,
    /// Whether attacks and blocks are chosen by simulating candidates (else read off the battlefield and the combat
    /// forecast).
    pub combat_search: bool,
    /// Whether attacks and main-phase plays are played on through the next turn's combat.
    pub lookahead: bool,
    /// Whether the seat and the seats it simulates play carefully: attacks keep enough creatures home to survive the
    /// next attacks ([`super::combat::safe_attack`]), and instants wait for combat, a response or a target worth them
    /// ([`super::priority::plan_priority`]).
    pub careful: bool,
    /// Whether a main-phase play is judged with the best play that can follow it.
    pub sequences: bool,
    /// Whether each decision is searched in a world whose hidden cards are redealt ([`Engine::redeal`]), where every
    /// seat plays its lands and spells in simulations as the seat itself does.
    pub fair: bool,
    /// How the evaluation weighs the coming turn's attacks.
    pub weights: Weights,
    /// The information-set Monte Carlo search the decisions that matter go to, if any ([`Deep`]).
    pub deep: Option<Deep>,
    /// Worlds each decision that matters is searched in, one redeal of the hidden cards each, the answer most of them
    /// give winning; 1 searches the table as it is.
    pub votes: u32,
    /// A wall-clock cap per decision; `None` searches by count alone, so the answers replay with the seed.
    pub time: Option<Duration>,
}

impl Settings {
    /// Normal: heuristic combat and a shallow search of plays and targets with quick answers inside it, about 300 ms
    /// at most.
    pub const NORMAL: Settings = Settings {
        sims: 40,
        forecasts: 160,
        targets: 3,
        depth: 1,
        combat_search: false,
        lookahead: false,
        careful: true,
        sequences: false,
        fair: false,
        weights: Weights::NORMAL,
        deep: None,
        votes: 1,
        time: Some(Duration::from_millis(300)),
    };

    /// The first Normal: [`Settings::NORMAL`] without careful play, kept as a fixed yardstick for measuring the levels.
    pub const BASELINE: Settings = Settings {
        careful: false,
        ..Settings::NORMAL
    };

    /// Hard: simulated attacks, blocks and main-phase plays through the next turn's combat in a world whose hidden
    /// cards are redealt, more candidates and searched targets inside simulations, about 1 s at most.
    pub const HARD: Settings = Settings {
        sims: 160,
        forecasts: 500,
        targets: 6,
        depth: 2,
        combat_search: true,
        lookahead: true,
        careful: true,
        sequences: true,
        fair: true,
        weights: Weights::NORMAL,
        deep: None,
        votes: 1,
        time: Some(Duration::from_millis(1000)),
    };

    /// Very Hard: Hard's answer to every decision, and for the decisions that matter (casts, attacks, blocks, targets)
    /// a tree search over that answer and its alternatives in worlds redealt from what the seat has seen
    /// ([`Deep::FULL`]), about 2.5 s at most.
    pub const VERY_HARD: Settings = Settings {
        deep: Some(Deep::FULL),
        ..Settings::HARD
    };

    /// These settings without a clock: the answers depend on the seed alone.
    #[must_use]
    pub const fn untimed(mut self) -> Settings {
        self.time = None;
        if let Some(deep) = &mut self.deep {
            deep.time = None;
        }
        self
    }
}

/// How far a simulation plays on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Horizon {
    /// Until the object on top of the stack has resolved (or, on an empty stack, until priority comes back).
    Settle,
    /// Until combat is over this turn.
    EndOfCombat,
    /// Until the combat of the next turn is over.
    NextTurn,
    /// Until the combat of the turn this many turns on is over (`Turns(1)` is [`Horizon::NextTurn`]).
    Turns(u32),
}

/// Where a simulation started.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Start {
    /// The turn number.
    pub(crate) turn: u32,
    /// The objects on the stack.
    pub(crate) stack: usize,
}

/// One simulated outcome: its score and a fingerprint of what lasts beyond the turn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Outcome {
    /// The evaluation of the position reached.
    pub score: f64,
    /// [`durable`] of that position.
    pub lasting: u64,
    /// The seat's mana sources there: lands and permanents whose rules add mana.
    pub sources: usize,
}

/// The searcher of one decision: its seat, settings and what is left of its budget.
#[derive(Debug)]
pub struct Search {
    seat: PlayerId,
    settings: Settings,
    sims_left: u32,
    forecasts_left: u32,
    deadline: Option<Instant>,
    others: RandomBot,
    /// Lands and spells each seat may still play by itself in the current simulation ([`ROLLOUT_PLAYS`]), by seat.
    plays_left: Vec<u32>,
    /// The turn `plays_left` was filled for: a new turn fills it again.
    plays_turn: u32,
}

/// Lands and permanent spells the seat plays by itself in one look-ahead simulation, its most expensive castable first.
const ROLLOUT_PLAYS: u32 = 6;

/// Whether `step` comes after the combat damage of its turn.
pub fn after_combat(step: Step) -> bool {
    matches!(
        step,
        Step::EndCombat | Step::Main2 | Step::End | Step::Cleanup
    )
}

impl Search {
    /// A search for `seat` with `settings`; `seed` keys the quick answers of the other seats.
    pub fn new(seat: PlayerId, settings: Settings, seed: u64) -> Search {
        Search {
            seat,
            settings,
            sims_left: settings.sims,
            forecasts_left: settings.forecasts,
            deadline: settings.time.map(|time| Instant::now() + time),
            others: RandomBot::new(seed, 0x005E_A4C4),
            plays_left: Vec::new(),
            plays_turn: 0,
        }
    }

    /// The seat searched for.
    pub const fn seat(&self) -> PlayerId {
        self.seat
    }

    /// The settings searched with.
    pub const fn settings(&self) -> Settings {
        self.settings
    }

    /// Whether the clock has run out.
    fn late(&self) -> bool {
        self.deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
    }

    /// Whether another simulation may start.
    pub fn can_simulate(&self) -> bool {
        self.sims_left > 0 && !self.late()
    }

    /// Takes one combat forecast from the budget; false once it is spent.
    pub fn take_forecast(&mut self) -> bool {
        if self.forecasts_left == 0 || self.late() {
            return false;
        }
        self.forecasts_left -= 1;
        true
    }

    /// The outcome of giving `answer` to the decision pending in `engine` and playing on to `horizon`, with the game
    /// where it stopped; `None` when the budget is spent or the answer is refused.
    pub fn played(
        &mut self,
        engine: &Engine,
        answer: Answer,
        horizon: Horizon,
    ) -> Option<(Outcome, Engine)> {
        if !self.can_simulate() {
            return None;
        }
        self.play_out(engine.clone(), answer, horizon)
    }

    /// [`Search::played`] on a game the search may use up, taking one simulation from the budget without checking it.
    pub fn play_out(
        &mut self,
        mut sim: Engine,
        answer: Answer,
        horizon: Horizon,
    ) -> Option<(Outcome, Engine)> {
        self.sims_left = self.sims_left.saturating_sub(1);
        let decision = sim.decision()?.clone();
        let start = Start {
            turn: sim.state().turn().number(),
            stack: sim.state().stack().len(),
        };
        apply_answer(&mut sim, &decision, answer).ok()?;
        self.prime(&sim);
        self.settle(sim, start, horizon, 0)
    }

    /// Starts a simulation of `engine`: every seat that plays by itself may make its [`ROLLOUT_PLAYS`] again.
    pub fn prime(&mut self, engine: &Engine) {
        self.plays_turn = engine.state().turn().number();
        self.plays_left = engine
            .state()
            .players()
            .iter()
            .map(|player| {
                let own = player.id() == self.seat;
                if (own && self.settings.lookahead) || (!own && self.settings.fair) {
                    ROLLOUT_PLAYS
                } else {
                    0
                }
            })
            .collect();
    }

    /// The outcome of `answer` as [`Search::outcome`] gives it.
    pub fn outcome(
        &mut self,
        engine: &Engine,
        answer: Answer,
        horizon: Horizon,
    ) -> Option<Outcome> {
        self.played(engine, answer, horizon)
            .map(|(outcome, _)| outcome)
    }

    /// The best of `candidates` for the decision pending in `engine` by their outcomes at `horizon`, with its outcome;
    /// the first wins a tie. `None` when none could be simulated.
    pub fn best(
        &mut self,
        engine: &Engine,
        candidates: Vec<Answer>,
        horizon: Horizon,
    ) -> Option<(Answer, Outcome)> {
        let mut best: Option<(Answer, Outcome)> = None;
        for candidate in candidates {
            let Some(outcome) = self.outcome(engine, candidate.clone(), horizon) else {
                continue;
            };
            if best
                .as_ref()
                .is_none_or(|(_, held)| outcome.score > held.score)
            {
                best = Some((candidate, outcome));
            }
        }
        best
    }

    /// Plays `engine` on from `start` to `horizon` and scores where it stops.
    fn settle(
        &mut self,
        mut engine: Engine,
        start: Start,
        horizon: Horizon,
        depth: u8,
    ) -> Option<(Outcome, Engine)> {
        for step in 0..SIM_STEPS {
            let Some(decision) = engine.decision().cloned() else {
                break;
            };
            if engine.result().is_some() || reached(&engine, &decision, start, horizon, step) {
                break;
            }
            if self.branches(&engine, &decision, depth) {
                return self.branch(&engine, &decision, start, horizon, depth);
            }
            let answer = self.quick(&engine, &decision);
            if apply_answer(&mut engine, &decision, answer).is_err() {
                let fallback = minimal(&decision)?;
                apply_answer(&mut engine, &decision, fallback).ok()?;
            }
        }
        let seen = Seen::new(engine.state(), self.seat);
        let outcome = Outcome {
            score: evaluate_with(&seen, self.settings.weights),
            lasting: durable(&engine, self.seat),
            sources: seen
                .bodies()
                .iter()
                .filter(|body| body.controller == self.seat && (body.is_land() || body.makes_mana))
                .count(),
        };
        Some((outcome, engine))
    }

    /// Whether the seat's own target question `decision` is searched here: within the depth, with the budget, and with
    /// at least two targets to try.
    fn branches(&self, engine: &Engine, decision: &Decision, depth: u8) -> bool {
        let DecisionKind::ChooseTargets(options) = &decision.kind else {
            return false;
        };
        decision.player == self.seat
            && depth < self.settings.depth
            && self.can_simulate()
            && rank_targets(&Seen::new(engine.state(), self.seat), options).len() >= 2
    }

    /// The best outcome over the seat's likeliest targets at `decision` ([`Search::branches`]), each played on to the
    /// horizon; `None` when none could be played.
    fn branch(
        &mut self,
        engine: &Engine,
        decision: &Decision,
        start: Start,
        horizon: Horizon,
        depth: u8,
    ) -> Option<(Outcome, Engine)> {
        let DecisionKind::ChooseTargets(options) = &decision.kind else {
            return None;
        };
        let ranked = rank_targets(&Seen::new(engine.state(), self.seat), options);
        let mut best: Option<(Outcome, Engine)> = None;
        for target in ranked.into_iter().take(self.settings.targets.max(2)) {
            if !self.can_simulate() {
                break;
            }
            self.sims_left -= 1;
            let mut sim = engine.clone();
            if apply_answer(&mut sim, decision, target_answer(options, vec![target])).is_err() {
                continue;
            }
            if let Some(played) = self.settle(sim, start, horizon, depth + 1)
                && best
                    .as_ref()
                    .is_none_or(|held| played.0.score > held.0.score)
            {
                best = Some(played);
            }
        }
        best
    }

    /// The quick answer of whoever is asked `decision` in a simulation.
    pub fn quick(&mut self, engine: &Engine, decision: &Decision) -> Answer {
        let player = decision.player;
        let own = player == self.seat;
        let turn = engine.state().turn().number();
        if turn != self.plays_turn {
            self.plays_turn = turn;
            for (at, left) in self.plays_left.iter_mut().enumerate() {
                let own = at == self.seat.index();
                if (own && self.settings.lookahead) || (!own && self.settings.fair) {
                    *left = ROLLOUT_PLAYS;
                }
            }
        }
        match &decision.kind {
            DecisionKind::Priority(options)
                if self.plays_left.get(player.index()).is_some_and(|n| *n > 0) =>
            {
                let play = rollout_play(engine, options);
                if play != PriorityAction::Pass
                    && let Some(left) = self.plays_left.get_mut(player.index())
                {
                    *left -= 1;
                }
                Answer::Priority(play)
            }
            DecisionKind::Priority(_) => Answer::Priority(PriorityAction::Pass),
            DecisionKind::Mulligan(_) => Answer::Mulligan(MulliganChoice::Keep),
            DecisionKind::PayMana(options) if options.auto_available => {
                Answer::PayMana(PayChoice::Auto)
            }
            DecisionKind::DeclareAttackers(options) => {
                let seen = Seen::new(engine.state(), player);
                Answer::DeclareAttackers(if self.settings.careful || self.settings.fair {
                    safe_attack(&seen, options)
                } else {
                    quick_attack(&seen, options)
                })
            }
            DecisionKind::DeclareBlockers(options) => {
                Answer::DeclareBlockers(quick_blocks(engine, player, options, QUICK_FORECASTS))
            }
            DecisionKind::AssignDamage(options) => Answer::AssignDamage(assign_lethal(options)),
            DecisionKind::ChooseTargets(options) if own || self.settings.fair => {
                let seen = Seen::new(engine.state(), player);
                let ranked = rank_targets(&seen, options);
                let wanted = options.min.max(1) as usize;
                target_answer(options, ranked.into_iter().take(wanted).collect())
            }
            _ if own || self.settings.fair => {
                super::choices::quick_choice(engine, decision, &mut self.others)
            }
            _ => self.others.choose_in(decision, engine.state()),
        }
    }
}

/// A seat's own play in a look-ahead simulation on an empty stack: in its main phase a land, else its most expensive
/// castable permanent or sorcery spell; at an opponent's end step its most expensive castable instant (removal, burn);
/// else a pass.
fn rollout_play(
    engine: &Engine,
    options: &mtg_engine::decision::PriorityOptions,
) -> PriorityAction {
    let state = engine.state();
    let Some(seat) = engine.decision().map(|d| d.player) else {
        return PriorityAction::Pass;
    };
    if !state.stack().is_empty() || engine.announcing() {
        return PriorityAction::Pass;
    }
    let seen = Seen::new(state, seat);
    let main = matches!(state.turn().step(), Step::Main1 | Step::Main2) && state.is_active(seat);
    let their_end = state.turn().step() == Step::End
        && seen
            .alive()
            .into_iter()
            .any(|player| seen.is_active(player) && !seen.is_teammate(player));
    if !main && !their_end {
        return PriorityAction::Pass;
    }
    if main && let Some(land) = options.playable_lands.first() {
        return PriorityAction::PlayLand(*land);
    }
    options
        .castable
        .iter()
        .filter(|id| {
            seen.chars(**id).is_some_and(|chars| {
                chars.types.contains(mtg_core::card::CardType::Instant) == their_end
            })
        })
        .max_by_key(|id| (seen.mana_value(**id), std::cmp::Reverse(**id)))
        .map_or(PriorityAction::Pass, |id| PriorityAction::CastSpell(*id))
}

/// Whether a simulation that started at `start` and has given `steps` answers has reached `horizon` at `decision`.
pub(crate) fn reached(
    engine: &Engine,
    decision: &Decision,
    start: Start,
    horizon: Horizon,
    steps: usize,
) -> bool {
    let state = engine.state();
    let at_priority = matches!(decision.kind, DecisionKind::Priority(_)) && !engine.announcing();
    if steps == 0 || !at_priority {
        return false;
    }
    let turn = state.turn().number();
    let stack = state.stack().len();
    match horizon {
        Horizon::Settle => turn != start.turn || stack <= start.stack.saturating_sub(1),
        Horizon::EndOfCombat => {
            turn != start.turn || (stack == 0 && after_combat(state.turn().step()))
        }
        Horizon::NextTurn => reached_turn(state, start.turn + 1, stack),
        Horizon::Turns(turns) => reached_turn(state, start.turn + turns, stack),
    }
}

/// Whether `state` (with `stack` objects on the stack) is past the combat of turn `end`.
fn reached_turn(state: &mtg_engine::state::GameState, end: u32, stack: usize) -> bool {
    let turn = state.turn().number();
    turn > end || (turn == end && stack == 0 && after_combat(state.turn().step()))
}

/// The answer that decides least, when a quick answer was refused: a pass, the fewest targets, a keep; `None` for a
/// question with no such answer.
pub(crate) fn minimal(decision: &Decision) -> Option<Answer> {
    Some(match &decision.kind {
        DecisionKind::Priority(_) => Answer::Priority(PriorityAction::Pass),
        DecisionKind::Mulligan(_) => Answer::Mulligan(MulliganChoice::Keep),
        DecisionKind::DeclareAttackers(options) => Answer::DeclareAttackers(options.legal_attack()),
        DecisionKind::DeclareBlockers(options) => Answer::DeclareBlockers(options.legal_blocks()),
        DecisionKind::PayMana(options) if options.can_cancel => Answer::PayMana(PayChoice::Cancel),
        DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
        DecisionKind::ChooseTargets(options) => {
            let picked: Vec<Target> = options
                .c10_dice_choosable()
                .iter()
                .copied()
                .take(options.min as usize)
                .collect();
            Answer::ChooseTargets(TargetChoice::Targets(picked))
        }
        _ => return None,
    })
}

/// A fingerprint of what outlasts the turn, read as `seat` sees it: life, poison, the counts of every hidden zone, the
/// seat's own hand and graveyard counted together (a card cast from hand into the graveyard changes nothing), the
/// other zones' counts, and every permanent with its controller, counters and attachment. Pumps and other effects
/// that end with the turn leave it as it was.
pub fn durable(engine: &Engine, seat: PlayerId) -> u64 {
    use std::hash::{Hash, Hasher};
    let state = engine.state();
    let mut hasher = mtg_engine::hash::StableHasher::default();
    for player in state.players() {
        let id = player.id();
        player.life().hash(&mut hasher);
        player.is_in_game().hash(&mut hasher);
        let hand = state.zone(Zone::Hand(id)).len();
        let grave = state.zone(Zone::Graveyard(id)).len();
        if id == seat {
            (hand + grave).hash(&mut hasher);
        } else {
            (hand, grave).hash(&mut hasher);
        }
        state.zone(Zone::Library(id)).len().hash(&mut hasher);
        state.zone(Zone::Command(id)).len().hash(&mut hasher);
        for (kind, count) in player.counters() {
            (format!("{kind:?}"), count).hash(&mut hasher);
        }
    }
    state.zone(Zone::Exile).len().hash(&mut hasher);
    for id in state.zone(Zone::Battlefield) {
        id.hash(&mut hasher);
        if let Some(object) = state.object(*id) {
            object.controller().hash(&mut hasher);
            if let Some(permanent) = object.permanent() {
                for (kind, count) in permanent.counters() {
                    (format!("{kind:?}"), count).hash(&mut hasher);
                }
                permanent
                    .attached_to()
                    .map(|attached| attached.id)
                    .hash(&mut hasher);
            }
        }
    }
    hasher.finish()
}
