//! Bot brains: what answers a seat's decisions for a server bot or a test harness, from random play to a heuristic
//! player that searches clones of the engine.
//!
//! Every brain implements [`SeatBrain`]. [`Level::Easy`] is [`EasyBrain`] (a land when it can, else a random legal
//! answer); [`Level::Normal`] and [`Level::Hard`] are a [`HeuristicBot`] with [`Settings::NORMAL`] or
//! [`Settings::HARD`]. A brain reads the table only through [`Seen`] (the shared zones, its own hand, every count),
//! never another seat's hand or a library, and plays for its team: teammates are its own side in every evaluation.

pub mod choices;
pub mod combat;
pub mod eval;
pub mod priority;
pub mod seen;
pub mod sim;
pub mod targets;

use mtg_core::ids::PlayerId;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::{Decision, Engine};

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
}

/// What answers one seat's decisions. A searching brain (Monte Carlo over determinized hidden cards, say) plugs in by
/// implementing it.
pub trait SeatBrain: Send {
    /// The answer to `decision`; `table` is the game it is pending in, when the caller has it. A brain must read the
    /// table only as the seat may ([`Seen`]).
    fn choose(&mut self, decision: &Decision, table: Option<&Engine>) -> Answer;
}

/// The brain of `seat` at `level` in a game seeded with `seed`; `timed` caps each decision by the level's clock (the
/// answers then depend on the machine's speed too).
pub fn brain(level: Level, seed: u64, seat: PlayerId, timed: bool) -> Box<dyn SeatBrain> {
    let settings = |settings: Settings| if timed { settings } else { settings.untimed() };
    match level {
        Level::Easy => Box::new(EasyBrain::new(seed, seat)),
        Level::Normal => Box::new(HeuristicBot::new(seed, seat, settings(Settings::NORMAL))),
        Level::Hard => Box::new(HeuristicBot::new(seed, seat, settings(Settings::HARD))),
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
}

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
        let mut search = Search::new(self.seat, self.settings, key);
        let pending = engine.decision() == Some(decision);
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
                let seen = Seen::new(engine.state(), self.seat);
                let ranked = targets::rank_targets(&seen, options);
                let wanted = options.min.max(1) as usize;
                let tries: Vec<Answer> = if wanted == 1 {
                    ranked
                        .iter()
                        .take(self.settings.targets.max(2))
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

impl SeatBrain for HeuristicBot {
    fn choose(&mut self, decision: &Decision, table: Option<&Engine>) -> Answer {
        match table {
            Some(engine) => self.answer(decision, engine),
            None => self.random.choose(decision),
        }
    }
}
