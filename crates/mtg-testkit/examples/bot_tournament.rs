//! Seeded tournaments between bot levels on the duel decks: `cargo run --release -p mtg-testkit --example
//! bot_tournament -- <strong> <weak> [games] [mode] [first seed] [threads]`, a level being `easy`, `normal`, `hard` or
//! `veryhard`, a mode `1v1`, `2v2`, `3ffa` or `4ffa`. Games are untimed, so each result replays with its seed.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::print_stdout,
    clippy::pedantic
)]

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use mtg_core::house::HouseRules;
use mtg_core::ids::PlayerId;
use mtg_core::teams::{TeamMode, Teams};
use mtg_engine::decision::Answer;
use mtg_engine::{Decision, Engine, GameResult};
use mtg_testkit::brain::{HeuristicBot, Level, SeatBrain, Settings, brain};
use mtg_testkit::brain_decks::{Duel, duel_setup};
use mtg_testkit::harness::{PlayOptions, play_brains};

/// A brain that times its answers.
struct Timed {
    inner: Box<dyn SeatBrain>,
    stats: Arc<Mutex<(Duration, Duration, u64)>>,
}

/// A brain that prints its interesting answers with the board, when `TRACE_GAME` is set.
struct Traced {
    inner: Box<dyn SeatBrain>,
    name: &'static str,
}

impl SeatBrain for Traced {
    fn choose(&mut self, decision: &Decision, table: Option<&Engine>) -> Answer {
        let answer = self.inner.choose(decision, table);
        let boring = matches!(
            (&decision.kind, &answer),
            (
                mtg_engine::decision::DecisionKind::Priority(_),
                Answer::Priority(mtg_engine::decision::PriorityAction::Pass)
            ) | (_, Answer::PayMana(_))
        );
        if let Some(engine) = table
            && !boring
        {
            let state = engine.state();
            let name = |id: mtg_core::ids::ObjectId| {
                state
                    .object(id)
                    .map_or_else(|| format!("{id:?}"), |o| o.card().name.clone())
            };
            let lives: Vec<i32> = state.players().iter().map(|p| p.life()).collect();
            let board: Vec<String> = state
                .zone(mtg_engine::zones::Zone::Battlefield)
                .iter()
                .filter_map(|id| {
                    let o = state.object(*id)?;
                    let chars = mtg_engine::characteristics::characteristics(state, *id)?;
                    chars.power?;
                    Some(format!(
                        "{}:{}{}/{}{}",
                        o.controller().index(),
                        o.card().name.replace("Duel ", ""),
                        chars.power.unwrap_or(0),
                        chars.toughness.unwrap_or(0),
                        if o.permanent().is_some_and(|p| p.tapped()) {
                            "T"
                        } else {
                            ""
                        }
                    ))
                })
                .collect();
            let shown = format!("{answer:?}");
            let shown = state
                .zone(mtg_engine::zones::Zone::Battlefield)
                .iter()
                .chain(
                    state
                        .players()
                        .iter()
                        .flat_map(|p| state.zone(mtg_engine::zones::Zone::Hand(p.id())).iter()),
                )
                .fold(shown, |text, id| {
                    text.replace(&format!("ObjectId({})", id.0), &name(*id))
                });
            println!(
                "t{} {:?} {} seat{} life{lives:?} hands{:?} | {} | {shown}",
                state.turn().number(),
                state.turn().step(),
                self.name,
                decision.player.index(),
                state
                    .players()
                    .iter()
                    .map(|p| state.zone(mtg_engine::zones::Zone::Hand(p.id())).len())
                    .collect::<Vec<_>>(),
                board.join(" "),
            );
        }
        answer
    }

    fn set_budget(&mut self, share: f64) {
        self.inner.set_budget(share);
    }
}

impl SeatBrain for Timed {
    fn choose(&mut self, decision: &Decision, table: Option<&Engine>) -> Answer {
        let start = Instant::now();
        let answer = self.inner.choose(decision, table);
        let took = start.elapsed();
        let mut stats = self.stats.lock().unwrap();
        stats.0 += took;
        stats.1 = stats.1.max(took);
        stats.2 += 1;
        answer
    }
}

/// Who plays a seat: a level, the first Normal kept as a yardstick, or Hard with the settings the `EXP_*` variables
/// change, for tuning.
#[derive(Debug, Clone, Copy)]
enum Who {
    Level(Level),
    Baseline,
    Exp,
}

fn level(name: &str) -> Who {
    match name {
        "easy" => Who::Level(Level::Easy),
        "normal" => Who::Level(Level::Normal),
        "hard" => Who::Level(Level::Hard),
        "veryhard" => Who::Level(Level::VeryHard),
        "baseline" => Who::Baseline,
        "exp" => Who::Exp,
        other => panic!("unknown level {other}"),
    }
}

fn flag(name: &str, default: bool) -> bool {
    std::env::var(name).map_or(default, |v| v == "1")
}

fn number<T: std::str::FromStr>(name: &str, default: T) -> T {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// The brain of `seat` played by `who`.
fn make(who: Who, seed: u64, seat: PlayerId, timed: bool) -> Box<dyn SeatBrain> {
    match who {
        Who::Level(level) => brain(level, seed, seat, timed),
        Who::Baseline => Box::new(HeuristicBot::new(seed, seat, Settings::BASELINE.untimed())),
        Who::Exp => {
            let base = Settings::HARD.untimed();
            let mut settings = base;
            settings.sims = number("EXP_SIMS", base.sims);
            settings.sequences = flag("EXP_SEQ", base.sequences);
            settings.targets = number("EXP_TARGETS", base.targets);
            settings.depth = number("EXP_DEPTH", base.depth);
            settings.forecasts = number("EXP_FORECASTS", base.forecasts);
            if let Ok(iterations) = std::env::var("EXP_DEEP") {
                let mut deep = mtg_testkit::brain::Deep::FULL;
                deep.time = None;
                deep.iterations = iterations.parse().unwrap();
                deep.turns = number("EXP_TURNS", deep.turns);
                deep.scale = number("EXP_SCALE", deep.scale);
                deep.explore = number("EXP_EXPLORE", deep.explore);
                settings.deep = Some(deep);
            }
            settings.votes = number("EXP_VOTES", base.votes);
            settings.fair = flag("EXP_FAIR", base.fair);
            settings.careful = flag("EXP_CAREFUL", base.careful);
            settings.lookahead = flag("EXP_LOOKAHEAD", base.lookahead);
            settings.combat_search = flag("EXP_COMBAT", base.combat_search);
            settings.weights.defend = number("EXP_DEFEND", base.weights.defend);
            settings.weights.press = number("EXP_PRESS", base.weights.press);
            settings.weights.card = number("EXP_CARD", base.weights.card);
            settings.weights.life = number("EXP_LIFE", base.weights.life);
            settings.weights.board = number("EXP_BOARD", base.weights.board);
            Box::new(HeuristicBot::new(seed, seat, settings))
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let strong = level(args.first().map_or("hard", String::as_str));
    let weak = level(args.get(1).map_or("normal", String::as_str));
    let games: u64 = args.get(2).map_or(20, |a| a.parse().unwrap());
    let mode = args.get(3).cloned().unwrap_or_else(|| "1v1".into());
    let first: u64 = args.get(4).map_or(1_000, |a| a.parse().unwrap());
    let threads: u64 = args.get(5).map_or(4, |a| a.parse().unwrap());
    let timed = std::env::var("TIMED").is_ok();
    let (seats, teams) = match mode.as_str() {
        "1v1" => (2, None),
        "2v2" => (4, Some(Teams::even(TeamMode::Teams, 2, 2))),
        "3ffa" => (3, None),
        "4ffa" => (4, None),
        other => panic!("unknown mode {other}"),
    };
    let next = AtomicU64::new(0);
    let wins = AtomicU64::new(0);
    let draws = AtomicU64::new(0);
    let stats = Arc::new(Mutex::new((Duration::ZERO, Duration::ZERO, 0u64)));
    let started = Instant::now();
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                loop {
                    let game = next.fetch_add(1, Ordering::SeqCst);
                    if game >= games {
                        break;
                    }
                    // Each deal is played once with the strong side in each place, so both sides get the same luck.
                    let seed = first + game / seats as u64;
                    let duel = if (seed / 2).is_multiple_of(2) {
                        Duel::Gruul
                    } else {
                        Duel::Orzhov
                    };
                    let house = HouseRules {
                        teams: teams.clone(),
                        ..HouseRules::default()
                    };
                    let setup = duel_setup(&vec![duel; seats], seed, house).unwrap();
                    // The strong side moves round the table: a seat in free-for-all, a team in 2v2.
                    let at = usize::try_from(game).unwrap() % seats;
                    let strong_seat = |seat: usize| match &teams {
                        Some(teams) => {
                            teams.team_of(PlayerId::from_index(seat))
                                == teams.team_of(PlayerId::from_index(at % 2))
                        }
                        None => seat == at,
                    };
                    let mut brains: Vec<Box<dyn SeatBrain>> = (0..seats)
                        .map(|seat| {
                            let player = PlayerId::from_index(seat);
                            if strong_seat(seat) {
                                Box::new(Timed {
                                    inner: make(strong, seed, player, timed),
                                    stats: Arc::clone(&stats),
                                }) as Box<dyn SeatBrain>
                            } else {
                                make(weak, seed, player, timed)
                            }
                        })
                        .collect();
                    if std::env::var("TRACE_GAME").is_ok() {
                        brains = brains
                            .into_iter()
                            .enumerate()
                            .map(|(seat, inner)| {
                                let name = if strong_seat(seat) { "STRONG" } else { "weak" };
                                Box::new(Traced { inner, name }) as Box<dyn SeatBrain>
                            })
                            .collect();
                    }
                    let options = PlayOptions {
                        max_answers: 30_000,
                        ..PlayOptions::default()
                    };
                    let outcome = play_brains(setup, seed, options, &mut brains);
                    let verdict = match outcome.map(|o| o.result) {
                        Ok(GameResult::Winner(player)) => {
                            if strong_seat(player.index()) {
                                "win"
                            } else {
                                "loss"
                            }
                        }
                        Ok(GameResult::Team(team)) => {
                            let ours = teams
                                .as_ref()
                                .map(|t| t.team_of(PlayerId::from_index(at % 2)));
                            if Some(team) == ours { "win" } else { "loss" }
                        }
                        Ok(_) => "draw",
                        Err(failure) => {
                            println!("seed {seed}: FAILED {:?}", failure.kind);
                            "fail"
                        }
                    };
                    match verdict {
                        "win" => {
                            wins.fetch_add(1, Ordering::SeqCst);
                        }
                        "draw" | "fail" => {
                            draws.fetch_add(1, Ordering::SeqCst);
                        }
                        _ => {}
                    }
                    println!("seed {seed} {duel:?}: {verdict}");
                }
            });
        }
    });
    let wins = wins.load(Ordering::SeqCst);
    let draws = draws.load(Ordering::SeqCst);
    let (total, max, count) = *stats.lock().unwrap();
    let mean = total.as_secs_f64() * 1000.0 / count.max(1) as f64;
    println!(
        "{strong:?} vs {weak:?} {mode}: {wins}/{games} won ({:.0}%), {draws} drawn/failed; strong thinks {mean:.1} ms \
         mean, {} ms max over {count} answers; {:.0} s",
        100.0 * wins as f64 / games as f64,
        max.as_millis(),
        started.elapsed().as_secs_f64()
    );
}
