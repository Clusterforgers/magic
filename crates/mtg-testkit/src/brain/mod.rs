//! Bot brains: what answers a seat's decisions for a server bot or a test harness, from random play to a heuristic
//! player that searches clones of the engine.
//!
//! Every brain implements [`SeatBrain`]. [`Level::Easy`] is [`EasyBrain`] (a land when it can, else a random legal
//! answer); [`Level::Normal`], [`Level::Hard`] and [`Level::VeryHard`] are a [`HeuristicBot`] with
//! [`Settings::NORMAL`], [`Settings::HARD`] or [`Settings::VERY_HARD`]. A brain reads the table only through [`Seen`]
//! (the shared zones, its own hand, every count), never another seat's hand or a library; Hard and Very Hard search
//! copies whose hidden cards are redealt ([`Engine::redeal`]), so no simulation draws a real hidden card. A brain plays
//! for its team: teammates are its own side in every evaluation. [`Deep`] is a determinized Monte Carlo search over a
//! seat's candidate answers that a level's settings may add.

pub mod choices;
pub mod clock;
pub mod combat;
pub mod deep;
pub mod endgame;
pub mod eval;
pub mod knobs;
pub mod priority;
pub mod seen;
pub mod sim;
pub mod suite;
pub mod targets;
pub mod threat;

use std::time::Instant;

use mtg_core::ids::PlayerId;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::{Decision, Engine};

pub use deep::Deep;
pub use seen::Seen;
pub use sim::{Horizon, Search, Settings};

use crate::bot::RandomBot;

/// How well a bot plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Level {
    /// Random legal answers, a land whenever it can play one.
    Easy,
    /// Heuristics with a shallow search ([`Settings::NORMAL`]).
    #[default]
    Normal,
    /// Heuristics with a deeper search that looks through the next turn's combat ([`Settings::HARD`]).
    Hard,
    /// Hard's search with a bigger budget in several redealt worlds, the answer most of them give winning
    /// ([`Settings::VERY_HARD`]).
    VeryHard,
}

/// What answers one seat's decisions.
pub trait SeatBrain: Send {
    /// The answer to `decision`; `table` is the game it is pending in, when the caller has it. A brain must read the
    /// table only as the seat may ([`Seen`]).
    fn choose(&mut self, decision: &Decision, table: Option<&Engine>) -> Answer;

    /// Sets the share of its clock and budget the brain may spend on its next answers, from 0 to 1 (a busy server
    /// lowers it); a brain that does not search ignores it.
    fn set_budget(&mut self, _share: f64) {}
}

/// The brain of `seat` at `level` in a game seeded with `seed`; `timed` caps each decision by the level's clock (the
/// answers then depend on the machine's speed too).
pub fn brain(level: Level, seed: u64, seat: PlayerId, timed: bool) -> Box<dyn SeatBrain> {
    let settings = |settings: Settings| if timed { settings } else { settings.untimed() };
    match level {
        Level::Easy => Box::new(EasyBrain::new(seed, seat)),
        Level::Normal => Box::new(HeuristicBot::new(seed, seat, settings(Settings::NORMAL))),
        Level::Hard => Box::new(HeuristicBot::new(seed, seat, settings(Settings::HARD))),
        Level::VeryHard => Box::new(HeuristicBot::new(seed, seat, settings(Settings::VERY_HARD))),
    }
}

/// The stream of a seat's random choices in a game seeded with `seed`.
fn stream(seat: PlayerId) -> u64 {
    0xB07 + seat.index() as u64
}

/// Easy: a land whenever it can play one, otherwise a random legal answer.
#[derive(Debug, Clone)]
pub struct EasyBrain {
    random: RandomBot,
}

impl EasyBrain {
    /// The brain of `seat` in a game seeded with `seed`: its random choices replay with the seed.
    pub fn new(seed: u64, seat: PlayerId) -> EasyBrain {
        EasyBrain {
            random: RandomBot::new(seed, stream(seat)),
        }
    }
}

impl SeatBrain for EasyBrain {
    fn choose(&mut self, decision: &Decision, _table: Option<&Engine>) -> Answer {
        if let DecisionKind::Priority(options) = &decision.kind
            && let Some(land) = options.playable_lands.first()
        {
            return Answer::Priority(PriorityAction::PlayLand(*land));
        }
        self.random.choose(decision)
    }
}

/// Plays a seat with heuristics and a budgeted search on engine clones ([`Search`]).
#[derive(Debug, Clone)]
pub struct HeuristicBot {
    seat: PlayerId,
    settings: Settings,
    seed: u64,
    asked: u64,
    random: RandomBot,
    /// The turn the plays below were made in.
    turn: u32,
    /// The plays made this turn.
    played: Vec<PriorityAction>,
    /// The share of the deep search's budget it may spend ([`SeatBrain::set_budget`]).
    budget: f64,
    /// What the tree search learnt about the seat's answers this game ([`deep::History`]).
    history: deep::History,
    /// When the decision being answered was asked.
    started: Instant,
}

/// The longest a timed bot looks for a forced win ([`endgame::forced_win`]).
const ENDGAME_TIME: std::time::Duration = std::time::Duration::from_millis(400);

/// How often one play (an ability, say) may be made in a turn.
const REPEATS: usize = 3;

/// How many plays a heuristic bot makes in one turn before it only passes.
const TURN_PLAYS: usize = 40;

impl HeuristicBot {
    /// The bot of `seat` in a game seeded with `seed`, searching with `settings`.
    pub fn new(seed: u64, seat: PlayerId, settings: Settings) -> HeuristicBot {
        HeuristicBot {
            seat,
            settings,
            seed,
            asked: 0,
            random: RandomBot::new(seed, stream(seat)),
            turn: 0,
            played: Vec::new(),
            budget: 1.0,
            history: deep::History::default(),
            started: Instant::now(),
        }
    }

    /// The plays this turn the bot will not make again.
    fn held(&self) -> Vec<PriorityAction> {
        let mut held = Vec::new();
        for play in &self.played {
            if self.played.iter().filter(|p| *p == play).count() >= REPEATS && !held.contains(play)
            {
                held.push(play.clone());
            }
        }
        held
    }

    /// The answer to `decision`, pending in `engine`.
    fn answer(&mut self, decision: &Decision, engine: &Engine) -> Answer {
        let turn = engine.state().turn().number();
        if turn != self.turn {
            self.turn = turn;
            self.played.clear();
        }
        self.asked += 1;
        let key = self.seed ^ self.asked.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        let pending = engine.decision() == Some(decision);
        let matters = matches!(
            decision.kind,
            DecisionKind::Priority(_)
                | DecisionKind::DeclareAttackers(_)
                | DecisionKind::DeclareBlockers(_)
                | DecisionKind::ChooseTargets(_)
        );
        if pending && matters && self.settings.votes > 1 {
            return self.voted(decision, engine);
        }
        if pending
            && matters
            && self.settings.endgame
            && let Some(win) = endgame::forced_win(
                engine,
                self.seat,
                decision,
                key,
                self.settings.time.map(|_| self.started + ENDGAME_TIME),
            )
        {
            if let Answer::Priority(play) = &win
                && *play != PriorityAction::Pass
            {
                self.played.push(play.clone());
            }
            return win;
        }
        if pending
            && matters
            && let Some(deep) = self.settings.deep
        {
            return self.deep_answer(decision, engine, deep, key);
        }
        self.heuristic(decision, engine, key, pending)
    }

    /// The heuristics' answer to `decision`, searched as the settings allow.
    fn heuristic(
        &mut self,
        decision: &Decision,
        engine: &Engine,
        key: u64,
        pending: bool,
    ) -> Answer {
        let mut search = Search::new(self.seat, self.settings, key);
        let world;
        let engine = if self.settings.fair && pending {
            let mut redealt = engine.clone();
            redealt.redeal(self.seat, key);
            world = redealt;
            &world
        } else {
            engine
        };
        match &decision.kind {
            DecisionKind::Priority(options) if pending => {
                if self.played.len() >= TURN_PLAYS {
                    return Answer::Priority(PriorityAction::Pass);
                }
                let play = priority::plan_priority(&mut search, engine, options, &self.held());
                if play != PriorityAction::Pass {
                    self.played.push(play.clone());
                }
                Answer::Priority(play)
            }
            DecisionKind::DeclareAttackers(options) if pending => {
                Answer::DeclareAttackers(combat::plan_attack(&mut search, engine, options))
            }
            DecisionKind::DeclareBlockers(options) if pending => {
                Answer::DeclareBlockers(combat::plan_blocks(&mut search, engine, options))
            }
            DecisionKind::AssignDamage(options) => {
                Answer::AssignDamage(combat::assign_lethal(options))
            }
            DecisionKind::PayMana(options) if options.auto_available => {
                Answer::PayMana(PayChoice::Auto)
            }
            DecisionKind::ChooseTargets(options) if pending => {
                Self::searched_targets(&mut search, engine, options, self.settings.targets)
            }
            DecisionKind::Mulligan(_)
            | DecisionKind::BottomCards(_)
            | DecisionKind::Discard(_)
            | DecisionKind::Scry(_) => choices::quick_choice(engine, decision, &mut self.random),
            DecisionKind::YesNo(_)
            | DecisionKind::ChooseMode(_)
            | DecisionKind::ChooseNumber(_)
            | DecisionKind::ChooseOption(_)
            | DecisionKind::ChooseCards(_)
            | DecisionKind::ChooseCosts(_)
            | DecisionKind::DivideAmount(_)
                if pending =>
            {
                choices::searched_choice(&mut search, engine, decision, &mut self.random)
            }
            _ => search.quick(engine, decision),
        }
    }
}

impl HeuristicBot {
    /// The seat's targets: the likeliest `most` single targets simulated through resolution, or the likeliest set when
    /// more than one is wanted.
    fn searched_targets(
        search: &mut Search,
        engine: &Engine,
        options: &mtg_engine::choose::TargetOptions,
        most: usize,
    ) -> Answer {
        let seen = Seen::new(engine.state(), search.seat());
        let ranked = targets::rank_targets(&seen, options);
        let wanted = options.min.max(1) as usize;
        let tries: Vec<Answer> = if wanted == 1 {
            ranked
                .iter()
                .take(most.max(2))
                .map(|target| targets::target_answer(options, vec![*target]))
                .collect()
        } else {
            vec![targets::target_answer(
                options,
                ranked.iter().copied().take(wanted).collect(),
            )]
        };
        let fallback = tries
            .first()
            .cloned()
            .unwrap_or_else(|| targets::target_answer(options, Vec::new()));
        if tries.len() < 2 {
            return fallback;
        }
        search
            .best(engine, tries, Horizon::Settle)
            .map_or(fallback, |(answer, _)| answer)
    }

    /// The answer most of [`Settings::votes`] redealt worlds give to `decision`, each searched as a single world; a
    /// tie goes to the answer found first. The budget share scales the worlds.
    fn voted(&mut self, decision: &Decision, engine: &Engine) -> Answer {
        // In range: at most the votes.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let worlds = ((f64::from(self.settings.votes) * self.budget).round() as u32).max(1);
        let mut tally: Vec<(Answer, u32)> = Vec::new();
        for world in 0..worlds {
            let mut voter = self.clone();
            voter.settings.votes = 1;
            voter.settings.fair = true;
            voter.asked = self.asked.wrapping_mul(64).wrapping_add(u64::from(world));
            let answer = voter.answer(decision, engine);
            match tally.iter_mut().find(|(held, _)| *held == answer) {
                Some((_, count)) => *count += 1,
                None => tally.push((answer, 1)),
            }
        }
        let mut best: Option<(Answer, u32)> = None;
        for (answer, count) in tally {
            if best.as_ref().is_none_or(|(_, held)| count > *held) {
                best = Some((answer, count));
            }
        }
        let answer = best.map_or(Answer::Priority(PriorityAction::Pass), |(answer, _)| answer);
        if let Answer::Priority(play) = &answer
            && *play != PriorityAction::Pass
        {
            self.played.push(play.clone());
        }
        answer
    }

    /// The tree search's answer to a decision that matters ([`Deep`]): the heuristic answer first among the
    /// candidates ([`deep::candidates`]), then [`deep::ismcts`]; the heuristic answer alone when there is nothing to
    /// choose between.
    fn deep_answer(
        &mut self,
        decision: &Decision,
        engine: &Engine,
        deep: Deep,
        key: u64,
    ) -> Answer {
        let before = self.played.clone();
        let held = self.held();
        let prior = self.heuristic(decision, engine, key, true);
        if before.len() >= TURN_PLAYS {
            return prior;
        }
        let mut answers = deep::candidates(
            engine,
            self.seat,
            decision,
            &held,
            self.settings.forecasts,
            self.settings.targets,
        );
        if answers.is_empty() {
            return prior;
        }
        answers.retain(|answer| *answer != prior);
        answers.insert(0, prior.clone());
        let answers = deep::distinct(engine, answers);
        let seen = Seen::new(engine.state(), self.seat);
        let budget = clock::Budget::new(
            self.started,
            deep.iterations,
            deep.time,
            clock::importance(&seen, decision),
            self.budget,
        );
        if budget.iterations == 0 {
            self.played = before;
            if let Answer::Priority(play) = &prior
                && *play != PriorityAction::Pass
            {
                self.played.push(play.clone());
            }
            return prior;
        }
        let window = priority::window(&seen);
        let costs: Vec<f64> = answers
            .iter()
            .map(|answer| match answer {
                Answer::Priority(play) => {
                    priority::hold_cost(self.settings.careful, &seen, window, play)
                }
                _ => 0.0,
            })
            .collect();
        let chosen = deep::ismcts(
            engine,
            self.seat,
            &answers,
            &costs,
            deep,
            self.settings.weights,
            key,
            budget,
            &mut self.history,
        )
        .unwrap_or(prior);
        self.played = before;
        if let Answer::Priority(play) = &chosen
            && *play != PriorityAction::Pass
        {
            self.played.push(play.clone());
        }
        chosen
    }
}

impl SeatBrain for HeuristicBot {
    fn set_budget(&mut self, share: f64) {
        self.budget = share.clamp(0.0, 1.0);
    }

    fn choose(&mut self, decision: &Decision, table: Option<&Engine>) -> Answer {
        self.started = Instant::now();
        match table {
            Some(engine) => self.answer(decision, engine),
            None => self.random.choose(decision),
        }
    }
}
