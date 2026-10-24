//! Stops a game that runs away: caps the answers of a game and of a turn and the permanents on the table, and names the seed, the turn, the last answers, the table and the decks when a cap is passed.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt::Write;

use mtg_core::ids::PlayerId;
use mtg_engine::decision::Answer;
use mtg_engine::zones::Zone;
use mtg_engine::{GameSetup, GameState};

/// Most answers one game may take: 20 times the longest game of the suites (about 39,000).
pub const MAX_GAME_ANSWERS: usize = 800_000;

/// Most answers one turn may take: 20 times the longest turn of the suites (about 350).
pub const MAX_TURN_ANSWERS: usize = 8_000;

/// Most permanents a bot game may hold: 29 times the fullest table of the suites without a token engine (about 175);
/// the bots stop adding to the table far below it ([`crate::harness::CROWDED_TABLE`]).
pub const MAX_PERMANENTS: usize = 5_000;

/// How many of the latest answers a report names.
const RECENT: usize = 20;

/// The longest text of one decision or answer in a report.
const SHOWN: usize = 160;

/// How many kinds of permanent the table of a report names.
const KINDS: usize = 12;

/// Counts the answers of one game and panics with a report once a cap is passed, so a game that loops, or grows
/// without end, fails within seconds instead of burning CPU for hours.
#[derive(Debug, Clone)]
pub struct Watchdog {
    seed: u64,
    answers: usize,
    turn: u32,
    turn_answers: usize,
    recent: VecDeque<String>,
    decks: Vec<String>,
    table_cap: bool,
}

impl Watchdog {
    /// A watchdog for the bot game of `seed` that starts from `setup`: the report names the decks of the seats.
    pub fn for_game(seed: u64, setup: &GameSetup) -> Self {
        Watchdog {
            decks: setup
                .players
                .iter()
                .enumerate()
                .map(|(seat, player)| {
                    let names: BTreeSet<&str> = player
                        .commanders
                        .iter()
                        .chain(&player.library)
                        .chain(&player.hand)
                        .map(|card| card.name.as_str())
                        .collect();
                    let names: Vec<&str> = names.into_iter().collect();
                    format!("seat {seat}: {}", names.join(", "))
                })
                .collect(),
            table_cap: true,
            ..Watchdog::scripted(seed)
        }
    }

    /// A watchdog for a game a test scripts, whose table may hold as many permanents as the test puts there: only the
    /// answers are capped.
    pub fn scripted(seed: u64) -> Self {
        Watchdog {
            seed,
            answers: 0,
            turn: 0,
            turn_answers: 0,
            recent: VecDeque::with_capacity(RECENT),
            decks: Vec::new(),
            table_cap: false,
        }
    }

    /// Counts the answer `player` gave to a decision named `kind`; `state` is the state after it.
    ///
    /// # Panics
    /// When the game, the turn or the table passes its cap.
    pub fn answered(&mut self, state: &GameState, player: PlayerId, kind: &str, answer: &Answer) {
        let turn = state.turn().number();
        if turn != self.turn {
            self.turn = turn;
            self.turn_answers = 0;
        }
        self.answers += 1;
        self.turn_answers += 1;
        if self.recent.len() == RECENT {
            self.recent.pop_front();
        }
        let mut line = format!("#{} p{} {kind} => {answer:?}", self.answers, player.index());
        line.truncate(line.floor_char_boundary(SHOWN));
        self.recent.push_back(line);
        let permanents = state.zone(Zone::Battlefield).len();
        let over = if self.answers > MAX_GAME_ANSWERS {
            Some(format!("{} answers in the game", self.answers))
        } else if self.turn_answers > MAX_TURN_ANSWERS {
            Some(format!("{} answers in this turn", self.turn_answers))
        } else if self.table_cap && permanents > MAX_PERMANENTS {
            Some(format!("{permanents} permanents on the table"))
        } else {
            None
        };
        if let Some(over) = over {
            self.trip(state, &over);
        }
    }

    /// The report of a passed cap, as the panic message.
    fn trip(&self, state: &GameState, over: &str) -> ! {
        let mut report = format!(
            "watchdog: game of seed {} runs away in turn {}: {over}\nthe table: {}\nthe last {} answers:\n",
            self.seed,
            self.turn,
            table_of(state),
            self.recent.len()
        );
        for line in &self.recent {
            // Writing to a String does not fail.
            let _ = writeln!(report, "  {line}");
        }
        for deck in &self.decks {
            let _ = writeln!(report, "{deck}");
        }
        panic!("{report}");
    }
}

/// The permanents of `state` by name, the most numerous kinds first.
fn table_of(state: &GameState) -> String {
    let mut kinds: BTreeMap<&str, usize> = BTreeMap::new();
    for id in state.zone(Zone::Battlefield) {
        if let Some(object) = state.object(*id) {
            *kinds.entry(object.card().name.as_str()).or_default() += 1;
        }
    }
    let mut kinds: Vec<(&str, usize)> = kinds.into_iter().collect();
    kinds.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    let shown: Vec<String> = kinds
        .iter()
        .take(KINDS)
        .map(|(name, count)| format!("{name} x{count}"))
        .collect();
    shown.join(", ")
}
