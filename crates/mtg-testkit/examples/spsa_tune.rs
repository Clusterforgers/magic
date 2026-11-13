//! SPSA tuning of the policy constants ([`Knobs`]) and evaluation weights ([`Weights`]) by match play, as chess engines
//! tune their search: each round perturbs every parameter up or down at random, plays paired seeded 1v1 games between
//! the two perturbed brains on every pairing of the duel decks, and moves the parameters toward the side that scored.
//! `cargo run --release -p mtg-testkit --example spsa_tune -- [rounds] [pairs per round] [threads] [level]`; prints the
//! tuned values.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::print_stdout,
    clippy::pedantic
)]

use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};

use mtg_core::house::HouseRules;
use mtg_core::ids::PlayerId;
use mtg_engine::decision::Answer;
use mtg_engine::{Decision, Engine, GameResult};
use mtg_testkit::brain::eval::Weights;
use mtg_testkit::brain::knobs::{Knobs, with_knobs};
use mtg_testkit::brain::{HeuristicBot, SeatBrain, Settings};
use mtg_testkit::brain_decks::{Duel, duel_setup};
use mtg_testkit::harness::{PlayOptions, play_brains};

/// A brain that plays with its own constants.
struct Tuned {
    inner: HeuristicBot,
    knobs: Knobs,
}

impl SeatBrain for Tuned {
    fn choose(&mut self, decision: &Decision, table: Option<&Engine>) -> Answer {
        let inner = &mut self.inner;
        with_knobs(self.knobs, || inner.choose(decision, table))
    }
}

/// Every tuned parameter's name, value and least value.
fn params(knobs: &Knobs, w: &Weights) -> Vec<(&'static str, f64, f64)> {
    vec![
        ("margin", knobs.margin, 0.0),
        ("ramp", knobs.ramp, 0.0),
        ("ramp_until", knobs.ramp_until, 1.0),
        ("instant_wait", knobs.instant_wait, 0.0),
        ("flash_wait", knobs.flash_wait, 0.0),
        ("tap_out", knobs.tap_out, 0.0),
        ("reactive", knobs.reactive, 0.0),
        ("hold_base", knobs.hold_base, 0.0),
        ("hold_per_card", knobs.hold_per_card, 0.0),
        ("hold_decay", knobs.hold_decay, 0.0),
        ("wipe", knobs.wipe, 0.0),
        ("counter", knobs.counter, 0.05),
        ("removal", knobs.removal, 0.0),
        ("fog", knobs.fog, 0.0),
        ("burn", knobs.burn, 0.0),
        ("trick", knobs.trick, 0.0),
        ("trade_up", knobs.trade_up, 0.0),
        ("wall", knobs.wall, 0.0),
        ("lethal_hit", knobs.lethal_hit, 0.0),
        ("lands_wanted", knobs.lands_wanted, 2.0),
        ("defend", w.defend, 0.0),
        ("press", w.press, 0.0),
        ("card", w.card, 0.0),
        ("life", w.life, 0.05),
        ("board", w.board, 0.05),
        ("early_source", w.early_source, 0.0),
        ("late_source", w.late_source, 0.0),
        ("held", w.held, 0.0),
        ("lethal_against", w.lethal_against, 0.0),
        ("lethal_for", w.lethal_for, 0.0),
        ("power", w.power, 0.0),
        ("evasion", w.evasion, 0.0),
    ]
}

/// The constants and weights `values` (in [`params`] order) stand for.
fn unpack(values: &[f64]) -> (Knobs, Weights) {
    let mut k = Knobs::TUNED;
    let mut w = Weights::NORMAL;
    let v = values;
    k.margin = v[0];
    k.ramp = v[1];
    k.ramp_until = v[2];
    k.instant_wait = v[3];
    k.flash_wait = v[4];
    k.tap_out = v[5];
    k.reactive = v[6];
    k.hold_base = v[7];
    k.hold_per_card = v[8];
    k.hold_decay = v[9];
    k.wipe = v[10];
    k.counter = v[11];
    k.removal = v[12];
    k.fog = v[13];
    k.burn = v[14];
    k.trick = v[15];
    k.trade_up = v[16];
    k.wall = v[17];
    k.lethal_hit = v[18];
    k.lands_wanted = v[19];
    w.defend = v[20];
    w.press = v[21];
    w.card = v[22];
    w.life = v[23];
    w.board = v[24];
    w.early_source = v[25];
    w.late_source = v[26];
    w.held = v[27];
    w.lethal_against = v[28];
    w.lethal_for = v[29];
    w.power = v[30];
    w.evasion = v[31];
    (k, w)
}

/// A small deterministic generator for the perturbations.
fn split_mix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rounds: u64 = args.first().map_or(200, |a| a.parse().unwrap());
    let pairs: u64 = args.get(1).map_or(8, |a| a.parse().unwrap());
    let threads: u64 = args.get(2).map_or(4, |a| a.parse().unwrap());
    let base = match args.get(3).map_or("hard", String::as_str) {
        "normal" => Settings::NORMAL,
        _ => Settings::HARD,
    }
    .untimed();
    let start = params(&Knobs::TUNED, &Weights::NORMAL);
    let names: Vec<&str> = start.iter().map(|p| p.0).collect();
    let floors: Vec<f64> = start.iter().map(|p| p.2).collect();
    // Each parameter moves by about a fifth of its starting size (at least 0.1).
    let steps: Vec<f64> = start.iter().map(|p| (p.1.abs() * 0.2).max(0.1)).collect();
    let mut theta: Vec<f64> = start.iter().map(|p| p.1).collect();
    let mut random = 0xC0FF_EE00_u64;
    let mut decks: Vec<(Duel, Duel)> = Vec::new();
    for a in Duel::ALL {
        for b in Duel::ALL {
            if !decks.contains(&(b, a)) {
                decks.push((a, b));
            }
        }
    }
    let mut total = 0i64;
    for round in 0..rounds {
        let signs: Vec<f64> = theta
            .iter()
            .map(|_| {
                if split_mix(&mut random) & 1 == 0 {
                    1.0
                } else {
                    -1.0
                }
            })
            .collect();
        // The perturbation shrinks slowly over the run, as in SPSA's c_k.
        let c = 1.0 / (1.0 + round as f64 / rounds as f64);
        let plus: Vec<f64> = (0..theta.len())
            .map(|i| (theta[i] + c * steps[i] * signs[i]).max(floors[i]))
            .collect();
        let minus: Vec<f64> = (0..theta.len())
            .map(|i| (theta[i] - c * steps[i] * signs[i]).max(floors[i]))
            .collect();
        let (plus_knobs, plus_weights) = unpack(&plus);
        let (minus_knobs, minus_weights) = unpack(&minus);
        let next = AtomicU64::new(0);
        let balance = AtomicI64::new(0);
        std::thread::scope(|scope| {
            for _ in 0..threads {
                scope.spawn(|| {
                    loop {
                        let game = next.fetch_add(1, Ordering::SeqCst);
                        if game >= pairs * 2 {
                            break;
                        }
                        let seed = 70_000 + round * pairs + game / 2;
                        let plus_seat = usize::try_from(game % 2).unwrap();
                        let pairing = decks[usize::try_from(seed).unwrap() % decks.len()];
                        let setup =
                            duel_setup(&[pairing.0, pairing.1], seed, HouseRules::default())
                                .unwrap();
                        let mut brains: Vec<Box<dyn SeatBrain>> = (0..2)
                            .map(|seat| {
                                let (knobs, weights) = if seat == plus_seat {
                                    (plus_knobs, plus_weights)
                                } else {
                                    (minus_knobs, minus_weights)
                                };
                                let mut settings = base;
                                settings.weights = weights;
                                Box::new(Tuned {
                                    inner: HeuristicBot::new(
                                        seed,
                                        PlayerId::from_index(seat),
                                        settings,
                                    ),
                                    knobs,
                                }) as Box<dyn SeatBrain>
                            })
                            .collect();
                        let options = PlayOptions {
                            max_answers: 30_000,
                            ..PlayOptions::default()
                        };
                        let result =
                            play_brains(setup, seed, options, &mut brains).map(|o| o.result);
                        let delta = match result {
                            Ok(GameResult::Winner(player)) if player.index() == plus_seat => 1,
                            Ok(GameResult::Winner(_)) => -1,
                            _ => 0,
                        };
                        balance.fetch_add(delta, Ordering::SeqCst);
                    }
                });
            }
        });
        let balance = balance.load(Ordering::SeqCst);
        total += balance;
        let games = (pairs * 2) as f64;
        // The learning rate: a full sweep of one side moves each parameter by its step.
        let rate = 0.1 * c;
        for i in 0..theta.len() {
            theta[i] = (theta[i] + rate * steps[i] * signs[i] * balance as f64 / games * 4.0)
                .max(floors[i]);
        }
        if round % 10 == 9 || round + 1 == rounds {
            println!("round {} balance {total}", round + 1);
            for (name, value) in names.iter().zip(&theta) {
                print!("{name}:{value:.3} ");
            }
            println!();
        }
    }
}
