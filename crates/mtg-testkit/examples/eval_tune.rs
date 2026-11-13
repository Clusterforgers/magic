//! Texel tuning of the evaluation: plays seeded 1v1 games between bots on every pairing of the duel decks, records each
//! seat's evaluation terms at the start of every main phase, labels them with the game's result, and fits the weights
//! by coordinate descent on the squared error of a logistic of the evaluation. `cargo run --release -p mtg-testkit
//! --example eval_tune -- [games] [level] [threads]`; prints the fitted [`Weights`] and the loss on held-out games.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::print_stdout,
    clippy::pedantic
)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use mtg_core::house::HouseRules;
use mtg_core::ids::PlayerId;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::{Decision, Engine, GameResult};
use mtg_testkit::brain::eval::{Terms, Weights, terms};
use mtg_testkit::brain::{Level, SeatBrain, Seen, brain};
use mtg_testkit::brain_decks::{Duel, duel_setup};
use mtg_testkit::harness::{PlayOptions, play_brains};

/// One recorded position: its terms from a seat's side, the game and the seat.
type Record = (Terms, u64, usize);

/// A brain that records every seat's terms once a turn, at the active seat's first main phase.
struct Recorder {
    inner: Box<dyn SeatBrain>,
    game: u64,
    last_turn: u32,
    out: Arc<Mutex<Vec<Record>>>,
}

impl SeatBrain for Recorder {
    fn choose(&mut self, decision: &Decision, table: Option<&Engine>) -> Answer {
        if let Some(engine) = table
            && matches!(decision.kind, DecisionKind::Priority(_))
        {
            let state = engine.state();
            let turn = state.turn().number();
            if turn != self.last_turn
                && state.turn().step() == Step::Main1
                && state.stack().is_empty()
                && state.is_active(decision.player)
            {
                self.last_turn = turn;
                let mut out = self.out.lock().unwrap();
                for player in state.players() {
                    if player.is_in_game() {
                        let seen = Seen::new(state, player.id());
                        out.push((terms(&seen), self.game, player.id().index()));
                    }
                }
            }
        }
        self.inner.choose(decision, table)
    }
}

const NAMES: [&str; 13] = [
    "defend",
    "press",
    "card",
    "life",
    "board",
    "early_source",
    "late_source",
    "held",
    "strongest",
    "lethal_against",
    "lethal_for",
    "power",
    "evasion",
];

fn to_vec(w: &Weights) -> [f64; 13] {
    [
        w.defend,
        w.press,
        w.card,
        w.life,
        w.board,
        w.early_source,
        w.late_source,
        w.held,
        w.strongest,
        w.lethal_against,
        w.lethal_for,
        w.power,
        w.evasion,
    ]
}

fn from_vec(v: &[f64; 13]) -> Weights {
    Weights {
        defend: v[0],
        press: v[1],
        card: v[2],
        life: v[3],
        board: v[4],
        early_source: v[5],
        late_source: v[6],
        held: v[7],
        strongest: v[8],
        lethal_against: v[9],
        lethal_for: v[10],
        power: v[11],
        evasion: v[12],
    }
}

fn loss(data: &[(Terms, f64)], w: &Weights, k: f64) -> f64 {
    let total: f64 = data
        .iter()
        .map(|(terms, label)| {
            let p = 1.0 / (1.0 + (-terms.score(w) / k).exp());
            (label - p).powi(2)
        })
        .sum();
    total / data.len().max(1) as f64
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let games: u64 = args.first().map_or(1_000, |a| a.parse().unwrap());
    let level = match args.get(1).map_or("hard", String::as_str) {
        "normal" => Level::Normal,
        "veryhard" => Level::VeryHard,
        _ => Level::Hard,
    };
    let threads: u64 = args.get(2).map_or(4, |a| a.parse().unwrap());
    let mut pairs: Vec<(Duel, Duel)> = Vec::new();
    for a in Duel::ALL {
        for b in Duel::ALL {
            if !pairs.contains(&(b, a)) {
                pairs.push((a, b));
            }
        }
    }
    let records: Arc<Mutex<Vec<Record>>> = Arc::new(Mutex::new(Vec::new()));
    let results: Mutex<Vec<(u64, Option<usize>)>> = Mutex::new(Vec::new());
    let next = AtomicU64::new(0);
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                loop {
                    let game = next.fetch_add(1, Ordering::SeqCst);
                    if game >= games {
                        break;
                    }
                    let seed = 50_000 + game;
                    let (a, b) = pairs[usize::try_from(game).unwrap() % pairs.len()];
                    let setup = duel_setup(&[a, b], seed, HouseRules::default()).unwrap();
                    let mut brains: Vec<Box<dyn SeatBrain>> = (0..2)
                        .map(|seat| {
                            Box::new(Recorder {
                                inner: brain(level, seed, PlayerId::from_index(seat), false),
                                game,
                                last_turn: 0,
                                out: Arc::clone(&records),
                            }) as Box<dyn SeatBrain>
                        })
                        .collect();
                    let options = PlayOptions {
                        max_answers: 30_000,
                        ..PlayOptions::default()
                    };
                    let winner =
                        match play_brains(setup, seed, options, &mut brains).map(|o| o.result) {
                            Ok(GameResult::Winner(player)) => Some(player.index()),
                            _ => None,
                        };
                    results.lock().unwrap().push((game, winner));
                }
            });
        }
    });
    let results = results.into_inner().unwrap();
    let records = Arc::try_unwrap(records).unwrap().into_inner().unwrap();
    let label = |game: u64, seat: usize| -> f64 {
        match results
            .iter()
            .find(|(g, _)| *g == game)
            .and_then(|(_, w)| *w)
        {
            Some(winner) if winner == seat => 1.0,
            Some(_) => 0.0,
            None => 0.5,
        }
    };
    let mut train = Vec::new();
    let mut test = Vec::new();
    for (terms, game, seat) in records {
        if terms.decided.is_some() {
            continue;
        }
        let labelled = (terms, label(game, seat));
        if game % 5 == 0 {
            test.push(labelled);
        } else {
            train.push(labelled);
        }
    }
    println!(
        "{} training positions, {} held out",
        train.len(),
        test.len()
    );
    let start = Weights::NORMAL;
    let mut k = 10.0;
    let mut best_k = loss(&train, &start, k);
    for step in 1..200 {
        let candidate = f64::from(step);
        let l = loss(&train, &start, candidate);
        if l < best_k {
            best_k = l;
            k = candidate;
        }
    }
    println!(
        "K = {k}: start loss train {best_k:.5}, held out {:.5}",
        loss(&test, &start, k)
    );
    let mut v = to_vec(&start);
    let mut current = best_k;
    let mut step = 0.5;
    while step > 0.01 {
        let mut improved = false;
        for i in 0..v.len() {
            let delta = step * v[i].abs().max(0.2);
            for sign in [1.0, -1.0] {
                let mut tried = v;
                tried[i] = (tried[i] + sign * delta).max(0.0);
                let l = loss(&train, &from_vec(&tried), k);
                if l < current - 1e-7 {
                    current = l;
                    v = tried;
                    improved = true;
                    break;
                }
            }
        }
        if !improved {
            step /= 2.0;
        }
    }
    let tuned = from_vec(&v);
    println!(
        "tuned loss train {current:.5}, held out {:.5}",
        loss(&test, &tuned, k)
    );
    for (name, value) in NAMES.iter().zip(v) {
        println!("    {name}: {value:.3},");
    }
}
