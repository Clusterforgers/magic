//! Plays Delves fights and runs with bot parties to tune the bosses: `cargo run --release -p mtg-campaign --example
//! delve_sim -- <mode> [games] [heroes] [hero level] [first seed] [threads]`, a mode `fights` (each boss against a
//! fresh party) or `runs` (whole runs, life carried and loot taken), a level `easy`, `normal`, `hard` or `veryhard`.
//! The heroes play the three fixture precons in turn; every game is untimed, so a result replays with its seed.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::print_stdout,
    clippy::pedantic
)]

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use mtg_campaign::{Brain, FightPlan, HeroDeck, Run, RunEvent, Status, content};
use mtg_cards::CardDefinition;
use mtg_core::ids::PlayerId;
use mtg_engine::format::Commander;
use mtg_engine::{GameResult, GameSetup};
use mtg_testkit::brain::{Level, SeatBrain, brain};
use mtg_testkit::corpus;
use mtg_testkit::fixture_decks::{FixtureDeck, fixture_deck};
use mtg_testkit::harness::{PlayOptions, play_brains};

/// A deck card by name: a basic land from the testkit, anything else from the corpus.
fn card(name: &str) -> Arc<CardDefinition> {
    use mtg_core::color::Color;
    let basic = [
        ("Plains", Color::White),
        ("Island", Color::Blue),
        ("Swamp", Color::Black),
        ("Mountain", Color::Red),
        ("Forest", Color::Green),
    ];
    match basic.iter().find(|(basic, _)| *basic == name) {
        Some((_, color)) => mtg_testkit::cards::land(*color),
        None => corpus::card(name).unwrap(),
    }
}

const PRECONS: [(&str, &str); 3] = [
    ("precon_tramplesaurus_rex", "RGW"),
    ("precon_calling_all_angels", "WB"),
    ("precon_wretched_ranks", "B"),
];

fn level(name: &str) -> Level {
    match name {
        "easy" => Level::Easy,
        "normal" => Level::Normal,
        "hard" => Level::Hard,
        "veryhard" => Level::VeryHard,
        other => panic!("unknown level {other}"),
    }
}

fn boss_level(brain: Brain) -> Level {
    match brain {
        Brain::Easy => Level::Easy,
        Brain::Normal => Level::Normal,
        Brain::Hard => Level::Hard,
        Brain::VeryHard => Level::VeryHard,
    }
}

/// What a fight ended with: the party won, each hero's life, the Spirits, the turns.
struct Fought {
    won: bool,
    lives: Vec<i32>,
    spirits: Vec<u8>,
    turns: u32,
    failed: bool,
}

fn hero_deck(at: usize) -> (FixtureDeck, &'static str) {
    let (name, colors) = PRECONS[at % PRECONS.len()];
    (fixture_deck(name).unwrap(), colors)
}

/// The setup of a planned fight; the hero decks get their run cards in place of basic lands.
fn setup_of(plan: &FightPlan, cards: &[Vec<String>], seed: u64) -> GameSetup {
    let content = content().unwrap();
    let mut decks: Vec<Vec<Arc<CardDefinition>>> = Vec::new();
    let mut commanders = Vec::new();
    for (at, added) in plan.heroes.iter().map(|h| usize::from(h.hero)).zip(cards) {
        let (deck, _) = hero_deck(at);
        let mut library = deck.cards.clone();
        for name in added {
            let basic = library.iter().position(|c| {
                ["Plains", "Island", "Swamp", "Mountain", "Forest"].contains(&c.name.as_str())
            });
            if let Some(basic) = basic {
                library[basic] = card(name);
            }
        }
        decks.push(library);
        commanders.push(deck.commanders.clone());
    }
    decks.push(
        plan.boss
            .deck
            .iter()
            .flat_map(|(name, n)| (0..*n).map(move |_| card(name)))
            .collect(),
    );
    commanders.push(Vec::new());
    let setup = GameSetup::new(Arc::new(Commander), seed, decks).with_commanders(commanders);
    plan.apply(content, setup).unwrap()
}

/// Plays one planned fight.
fn fight(plan: &FightPlan, cards: &[Vec<String>], seed: u64, heroes: Level) -> Fought {
    let setup = setup_of(plan, cards, seed);
    let boss = plan.boss_seat();
    let mut brains: Vec<Box<dyn SeatBrain>> = (0..plan.seats())
        .map(|seat| {
            let player = PlayerId::from_index(seat);
            let level = if player == boss {
                boss_level(plan.boss.brain)
            } else {
                heroes
            };
            brain(level, seed, player, false)
        })
        .collect();
    let options = PlayOptions {
        max_answers: 60_000,
        ..PlayOptions::default()
    };
    match play_brains(setup, seed, options, &mut brains) {
        Ok(outcome) => {
            let won = matches!(outcome.result, GameResult::Team(0));
            let mut lives = Vec::new();
            let mut spirits = Vec::new();
            // The outcome carries the events; the end state comes from replaying them.
            let state = mtg_engine::replay(&setup_of(plan, cards, seed), &outcome.events);
            if let Ok(engine) = state {
                for at in 0..plan.heroes.len() {
                    let player = engine.player(PlayerId::from_index(at)).unwrap();
                    lives.push(player.life());
                    if player.is_spirit() {
                        spirits.push(u8::try_from(at).unwrap());
                    }
                }
            }
            Fought {
                won,
                lives,
                spirits,
                turns: outcome.turns,
                failed: false,
            }
        }
        Err(failure) => {
            println!("seed {seed}: FAILED {:?}", failure.kind);
            Fought {
                won: false,
                lives: vec![0; plan.heroes.len()],
                spirits: Vec::new(),
                turns: 0,
                failed: true,
            }
        }
    }
}

/// A run in its lobby with `heroes` bot heroes on the precons; the first hero counts as the player.
fn lobby(seed: u64, heroes: usize) -> (Run, Vec<RunEvent>) {
    let content = content().unwrap();
    let mut events = Vec::new();
    for at in 0..heroes {
        events.push(RunEvent::Joined {
            name: format!("Hero {at}"),
            user: if at == 0 { Some("sim".into()) } else { None },
            bot: if at == 0 { None } else { Some(Brain::Normal) },
        });
        let (deck, colors) = hero_deck(at);
        events.push(RunEvent::DeckChosen {
            hero: u8::try_from(at).unwrap(),
            deck: HeroDeck {
                id: deck.name.clone(),
                name: deck.name,
                colors: colors.into(),
            },
        });
    }
    events.push(RunEvent::Started);
    (
        Run::replay(content, "warrens", seed, 0, &events).unwrap(),
        events,
    )
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mode = args.first().cloned().unwrap_or_else(|| "fights".into());
    let games: u64 = args.get(1).map_or(8, |a| a.parse().unwrap());
    let heroes: usize = args.get(2).map_or(2, |a| a.parse().unwrap());
    let hero_level = level(args.get(3).map_or("normal", String::as_str));
    let first: u64 = args.get(4).map_or(1, |a| a.parse().unwrap());
    let threads: u64 = args.get(5).map_or(4, |a| a.parse().unwrap());
    let content = content().unwrap();
    let theme = &content.themes[0];
    let bosses: Vec<String> = theme.bosses.iter().map(|b| b.id.clone()).collect();
    // (boss, won, turns) per fight, and per run the cleared bosses.
    let fights: Mutex<BTreeMap<String, (u64, u64, u64)>> = Mutex::new(BTreeMap::new());
    let runs: Mutex<Vec<(usize, bool)>> = Mutex::new(Vec::new());
    let next = AtomicU64::new(0);
    let started = Instant::now();
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                loop {
                    let game = next.fetch_add(1, Ordering::SeqCst);
                    if game >= games {
                        break;
                    }
                    let seed = first + game;
                    if mode == "fights" {
                        // Each boss in turn against a fresh party.
                        let boss = &bosses[usize::try_from(game).unwrap() % bosses.len()];
                        let (mut run, _) = lobby(seed, heroes);
                        let node = run
                            .map
                            .nodes
                            .iter()
                            .find(|n| n.boss.as_deref() == Some(boss))
                            .unwrap()
                            .id;
                        run.at = Some(node);
                        let plan = run.fight_plan(content).unwrap().unwrap();
                        let t = Instant::now();
                        let fought = fight(&plan, &vec![Vec::new(); heroes], seed, hero_level);
                        println!(
                            "seed {seed} {boss}: {} in {} turns ({:.0}s){}",
                            if fought.won { "win" } else { "loss" },
                            fought.turns,
                            t.elapsed().as_secs_f64(),
                            if fought.failed { " FAILED" } else { "" }
                        );
                        let mut table = fights.lock().unwrap();
                        let entry = table.entry(boss.clone()).or_default();
                        entry.0 += 1;
                        entry.1 += u64::from(fought.won);
                        entry.2 += u64::from(fought.turns);
                    } else {
                        let (mut run, _) = lobby(seed, heroes);
                        let mut fights_played = 0u64;
                        while !run.is_over() && fights_played < 12 {
                            match run.status.clone() {
                                Status::Fight { game: None } => {
                                    run.apply(
                                        content,
                                        &RunEvent::FightCreated { game: "sim".into() },
                                    )
                                    .unwrap();
                                }
                                Status::Fight { game: Some(_) } => {
                                    let plan = run.fight_plan(content).unwrap().unwrap();
                                    let cards: Vec<Vec<String>> =
                                        run.heroes.iter().map(|h| h.cards.clone()).collect();
                                    fights_played += 1;
                                    let fought = fight(
                                        &plan,
                                        &cards,
                                        seed * 100 + fights_played,
                                        hero_level,
                                    );
                                    {
                                        let mut table = fights.lock().unwrap();
                                        let entry =
                                            table.entry(plan.boss.boss.clone()).or_default();
                                        entry.0 += 1;
                                        entry.1 += u64::from(fought.won);
                                        entry.2 += u64::from(fought.turns);
                                    }
                                    let ended = RunEvent::FightEnded {
                                        won: fought.won,
                                        lives: fought.lives,
                                        spirits: fought.spirits,
                                    };
                                    run.apply(content, &ended).unwrap();
                                }
                                _ => {
                                    // The player hero answers like a bot.
                                    let mut moves = run.bot_moves(content);
                                    if let Some(h) = run.heroes.first()
                                        && h.bot.is_none()
                                    {
                                        let mut as_bot = run.clone();
                                        as_bot.heroes[0].bot = Some(Brain::Normal);
                                        as_bot.heroes[0].user = None;
                                        moves = as_bot.bot_moves(content);
                                    }
                                    if moves.is_empty() {
                                        break;
                                    }
                                    for event in moves {
                                        if run.apply(content, &event).is_err() {
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        let cleared = run
                            .cleared
                            .iter()
                            .filter(|id| run.map.node(**id).is_some_and(|n| n.boss.is_some()))
                            .count();
                        println!(
                            "run {seed}: {:?}, {cleared} bosses, {} hearts",
                            run.status, run.hearts
                        );
                        runs.lock()
                            .unwrap()
                            .push((cleared, run.status == Status::Victory));
                    }
                }
            });
        }
    });
    println!(
        "\n{heroes} heroes ({hero_level:?}), {:.0}s",
        started.elapsed().as_secs_f64()
    );
    for (boss, (n, won, turns)) in fights.lock().unwrap().iter() {
        println!(
            "{boss:>16}: {won}/{n} won ({:.0}%), {:.1} turns",
            100.0 * *won as f64 / (*n).max(1) as f64,
            *turns as f64 / (*n).max(1) as f64
        );
    }
    let runs = runs.lock().unwrap();
    if !runs.is_empty() {
        let victories = runs.iter().filter(|(_, v)| *v).count();
        let cleared: usize = runs.iter().map(|(c, _)| c).sum();
        println!(
            "runs: {victories}/{} victories, {:.1} bosses cleared on average",
            runs.len(),
            cleared as f64 / runs.len() as f64
        );
    }
}
