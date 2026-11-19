//! Plays Delves fights and runs with bot parties to tune the bosses: `cargo run --release -p mtg-campaign --example
//! delve_sim -- <mode> [games] [heroes] [hero level] [first seed] [threads] [heat]`, a mode `fights` (each boss against a
//! fresh party) or `runs` (whole runs, life carried and loot taken), a level `easy`, `normal`, `hard` or `veryhard`.
//! The heroes play the four fixture Commander decks in turn; every game is untimed, so a result replays with its seed.
//! Each fight reports its turns, the boss side's share of them and an estimate of its minutes at a human pace.
//! A run plays the newest content generation (`DELVE_SIM_CONTENT` sets another); `DELVE_SIM_BOSSES` picks the bosses.
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

const PRECONS: [(&str, &str); 4] = [
    ("precon_tramplesaurus_rex", "RGW"),
    ("precon_calling_all_angels", "WB"),
    ("owner", "WUB"),
    ("precon_wretched_ranks", "B"),
];

/// Minutes a hero's turn takes at a human pace.
const HERO_TURN_MINUTES: f64 = 0.6;
/// Minutes a turn of the boss's side takes: a bot plays, the party answers.
const BOSS_TURN_MINUTES: f64 = 0.5;

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
    /// The turns of the boss's side.
    boss_turns: u32,
    failed: bool,
    /// Heroes who became Spirits, Spirits who rose again and haunt actions taken.
    falls: u64,
    rises: u64,
    haunts: u64,
    /// The pressure each hero received from the boss's side (`mtg_engine::spotlight::received`).
    pressure: Vec<u32>,
    /// The round in which a hero first fell to half their starting life or below, if one did.
    first_half: Option<u32>,
    /// The round in which a hero first fell, if one did.
    first_fall: Option<u32>,
}

/// What the log shows about the heroes' life: the round each hero first fell to half life or below, and first fell.
fn life_marks(
    setup: &GameSetup,
    events: &[mtg_engine::events::Event],
    party: usize,
) -> (Option<u32>, Option<u32>) {
    use mtg_engine::events::Event;
    let Ok(mut state) = mtg_engine::GameState::from_setup(setup) else {
        return (None, None);
    };
    let seats = u32::try_from(state.players().len()).unwrap().max(1);
    let starts: Vec<i32> = (0..party)
        .map(|at| state.player(PlayerId::from_index(at)).unwrap().life())
        .collect();
    let (mut half, mut fall) = (None, None);
    let mut turn = 0u32;
    for event in events {
        if mtg_engine::apply_event(&mut state, event).is_err() {
            break;
        }
        if let Event::TurnBegan { number, .. } = event {
            turn = *number;
        }
        let round = turn.saturating_sub(1) / seats + 1;
        for (at, start) in starts.iter().enumerate() {
            let player = state.player(PlayerId::from_index(at)).unwrap();
            if half.is_none() && player.life() * 2 <= *start {
                half = Some(round);
            }
            if fall.is_none() && (player.is_spirit() || !player.is_in_game()) {
                fall = Some(round);
            }
        }
        if half.is_some() && fall.is_some() {
            break;
        }
    }
    (half, fall)
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
    let mut satchels = Vec::new();
    for (at, added) in plan.heroes.iter().map(|h| usize::from(h.hero)).zip(cards) {
        let (deck, _) = hero_deck(at);
        decks.push(deck.cards.clone());
        commanders.push(deck.commanders.clone());
        // The earned cards wait in the Satchel; the deck stays as it is.
        satchels.push(added.iter().map(|name| card(name)).collect());
    }
    decks.push(
        plan.boss_deck()
            .iter()
            .flat_map(|(name, n)| (0..*n).map(move |_| card(name)))
            .collect(),
    );
    commanders.push(Vec::new());
    let setup = GameSetup::new(Arc::new(Commander), seed, decks)
        .with_commanders(commanders)
        .with_satchels(satchels);
    plan.apply(content, setup).unwrap()
}

/// The plan without the boss's modifier, relics and phases `DELVE_SIM_DROP` names (comma-separated ids), with the
/// boss's life changed by `DELVE_SIM_LIFE`: tuning knobs to measure what each is worth.
fn dropped(plan: &FightPlan) -> FightPlan {
    let mut plan = plan.clone();
    if let Ok(drop) = std::env::var("DELVE_SIM_DROP") {
        let drop: Vec<&str> = drop.split(',').collect();
        plan.boss.relics.retain(|id| !drop.contains(&id.as_str()));
        plan.boss
            .round_phases
            .retain(|(id, _)| !drop.contains(&id.as_str()));
        plan.boss.modifier = plan.boss.modifier.filter(|id| !drop.contains(&id.as_str()));
    }
    // `DELVE_SIM_HAND` sets the boss's opening hand.
    if let Ok(hand) = std::env::var("DELVE_SIM_HAND") {
        plan.boss.hand = hand.parse().unwrap();
    }
    // `DELVE_SIM_LIFE` sets the boss's life to this percent of the plan's.
    if let Ok(percent) = std::env::var("DELVE_SIM_LIFE") {
        let percent: i32 = percent.parse().unwrap();
        plan.boss.life = plan.boss.life * percent / 100;
    }
    plan
}

/// Plays one planned fight.
fn fight(plan: &FightPlan, cards: &[Vec<String>], seed: u64, heroes: Level) -> Fought {
    let plan = &dropped(plan);
    let setup = setup_of(plan, cards, seed);
    let party = plan.heroes.len();
    let mut brains: Vec<Box<dyn SeatBrain>> = (0..plan.seats())
        .map(|seat| {
            let player = PlayerId::from_index(seat);
            let level = match seat.checked_sub(party) {
                None => heroes,
                Some(_) => boss_level(plan.boss.brain),
            };
            brain(level, seed, player, false)
        })
        .collect();
    let options = PlayOptions {
        max_answers: 60_000,
        runaway_fails: true,
        ..PlayOptions::default()
    };
    match play_brains(setup, seed, options, &mut brains) {
        Ok(outcome) => {
            let won = matches!(outcome.result, GameResult::Team(0));
            let mut lives = Vec::new();
            let mut spirits = Vec::new();
            // The outcome carries the events; the end state comes from replaying them.
            let boss_turns = u32::try_from(
                outcome
                    .events
                    .iter()
                    .filter(|e| {
                        matches!(e, mtg_engine::events::Event::TurnBegan { active, .. } if active.index() >= party)
                    })
                    .count(),
            )
            .unwrap();
            if std::env::var_os("DELVE_SIM_EVENTS").is_some() {
                use mtg_engine::events::Event;
                let count =
                    |f: &dyn Fn(&Event) -> bool| outcome.events.iter().filter(|e| f(e)).count();
                println!(
                    "seed {seed}: plan innate {}, modifier {:?}, relics {:?}",
                    plan.boss.innate, plan.boss.modifier, plan.boss.relics
                );
                println!(
                    "seed {seed}: phases {}, combats {}",
                    count(&|e| matches!(e, Event::PhaseBegan { .. })),
                    count(&|e| matches!(
                        e,
                        Event::StepBegan {
                            step: mtg_engine::turn::Step::BeginCombat
                        }
                    )),
                );
            }
            let state = mtg_engine::replay(&setup_of(plan, cards, seed), &outcome.events);
            if let Ok(engine) = &state
                && std::env::var_os("DELVE_SIM_EVENTS").is_some()
            {
                let lives: Vec<i32> = (0..plan.seats())
                    .map(|at| engine.player(PlayerId::from_index(at)).unwrap().life())
                    .collect();
                println!(
                    "seed {seed}: end lives {lives:?} of boss {}",
                    plan.boss.life
                );
            }
            let mut pressure = Vec::new();
            if let Ok(engine) = state {
                for at in 0..plan.heroes.len() {
                    let player = engine.player(PlayerId::from_index(at)).unwrap();
                    lives.push(player.life());
                    if player.is_spirit() {
                        spirits.push(u8::try_from(at).unwrap());
                    }
                    pressure.push(mtg_engine::spotlight::received(
                        &engine,
                        PlayerId::from_index(at),
                    ));
                }
            }
            let falls = outcome
                .events
                .iter()
                .filter(|e| matches!(e, mtg_engine::events::Event::BecameSpirit { .. }))
                .count() as u64;
            let rises = outcome
                .events
                .iter()
                .filter(|e| matches!(e, mtg_engine::events::Event::SpiritRevived { .. }))
                .count() as u64;
            let emblems: Vec<_> = outcome
                .events
                .iter()
                .filter_map(|e| match e {
                    mtg_engine::events::Event::BecameSpirit { emblem, .. } => Some(*emblem),
                    _ => None,
                })
                .collect();
            let haunts = outcome
                .events
                .iter()
                .filter(|e| {
                    matches!(e, mtg_engine::events::Event::AbilityActivated { source, .. } if emblems.contains(&source.id))
                })
                .count() as u64;
            let (first_half, first_fall) =
                life_marks(&setup_of(plan, cards, seed), &outcome.events, party);
            Fought {
                won,
                lives,
                spirits,
                turns: outcome.turns,
                boss_turns,
                failed: false,
                falls,
                rises,
                haunts,
                pressure,
                first_half,
                first_fall,
            }
        }
        Err(failure) => {
            println!("seed {seed}: FAILED {:?}", failure.kind);
            Fought {
                won: false,
                lives: vec![0; plan.heroes.len()],
                spirits: Vec::new(),
                turns: 0,
                boss_turns: 0,
                failed: true,
                falls: 0,
                rises: 0,
                haunts: 0,
                pressure: Vec::new(),
                first_half: None,
                first_fall: None,
            }
        }
    }
}

/// The fights against one boss: played, won, failed, turns, the boss side's turns.
#[derive(Default)]
struct Tally {
    n: u64,
    won: u64,
    failed: u64,
    turns: u64,
    boss_turns: u64,
    falls: u64,
    rises: u64,
    haunts: u64,
    /// The largest share of the boss's pressure one hero received, per fight with any pressure.
    max_shares: Vec<f64>,
    /// The round a hero first fell to half life, per fight in which one did.
    halves: Vec<u32>,
    /// The round a hero first fell, per fight in which one did.
    first_falls: Vec<u32>,
}

impl Tally {
    fn add(&mut self, fought: &Fought) {
        self.n += 1;
        self.won += u64::from(fought.won);
        self.failed += u64::from(fought.failed);
        self.turns += u64::from(fought.turns);
        self.boss_turns += u64::from(fought.boss_turns);
        self.falls += fought.falls;
        self.rises += fought.rises;
        self.haunts += fought.haunts;
        let total: u32 = fought.pressure.iter().sum();
        if total > 0 {
            let most = *fought.pressure.iter().max().unwrap();
            self.max_shares.push(f64::from(most) / f64::from(total));
        }
        self.halves.extend(fought.first_half);
        self.first_falls.extend(fought.first_fall);
    }

    fn merge(&mut self, other: &Tally) {
        self.n += other.n;
        self.won += other.won;
        self.failed += other.failed;
        self.turns += other.turns;
        self.boss_turns += other.boss_turns;
        self.falls += other.falls;
        self.rises += other.rises;
        self.haunts += other.haunts;
        self.max_shares.extend(&other.max_shares);
        self.halves.extend(&other.halves);
        self.first_falls.extend(&other.first_falls);
    }

    /// The spotlight's measures: the mean largest share of pressure on one hero, the round a hero first fell to half
    /// life and first fell (means over the fights in which it happened, with their count).
    fn spotlight_line(&self) -> String {
        let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len().max(1) as f64;
        let rounds =
            |v: &[u32]| v.iter().map(|r| f64::from(*r)).sum::<f64>() / v.len().max(1) as f64;
        format!(
            "max hero share {:.0}% (worst {:.0}%), half life in round {:.1} ({}/{}), first fall in round {:.1} ({}/{})",
            100.0 * mean(&self.max_shares),
            100.0 * self.max_shares.iter().copied().fold(0.0, f64::max),
            rounds(&self.halves),
            self.halves.len(),
            self.n,
            rounds(&self.first_falls),
            self.first_falls.len(),
            self.n
        )
    }

    fn line(&self) -> String {
        let n = self.n.max(1) as f64;
        let hero_turns = (self.turns - self.boss_turns) as f64 / n;
        let boss_turns = self.boss_turns as f64 / n;
        format!(
            "{}/{} won ({:.0}%), {:.1} turns ({:.1} boss side), ~{:.0} min, {} falls, {} rises, {} haunts{}",
            self.won,
            self.n,
            100.0 * self.won as f64 / n,
            self.turns as f64 / n,
            boss_turns,
            hero_turns * HERO_TURN_MINUTES + boss_turns * BOSS_TURN_MINUTES,
            self.falls,
            self.rises,
            self.haunts,
            if self.failed > 0 {
                format!(", {} FAILED", self.failed)
            } else {
                String::new()
            }
        )
    }
}

/// A run in its lobby with `heroes` bot heroes on the precons; the first hero counts as the player.
fn lobby(seed: u64, heroes: usize, heat: u8) -> (Run, Vec<RunEvent>) {
    let content = content().unwrap();
    // `DELVE_SIM_CONTENT` plays a run of that content generation; a new run's by default.
    let version = std::env::var("DELVE_SIM_CONTENT")
        .map_or(mtg_campaign::run::CONTENT, |v| v.parse().unwrap());
    let mut events = vec![RunEvent::Content { version }];
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
                commanders: Vec::new(),
                makeup: None,
            },
        });
    }
    events.push(RunEvent::Started);
    (
        Run::replay(content, "warrens", seed, heat, &events).unwrap(),
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
    let heat: u8 = args.get(6).map_or(0, |a| a.parse().unwrap());
    let content = content().unwrap();
    // The bosses' bots read their Satchel cues from the content, as the server's do.
    mtg_testkit::brain::satchel::install(content.satchel_cues());
    let theme = &content.themes[0];
    // `DELVE_SIM_BOSSES` (comma-separated ids) plays only those bosses.
    let only = std::env::var("DELVE_SIM_BOSSES").ok();
    let bosses: Vec<String> = theme
        .bosses
        .iter()
        .map(|b| b.id.clone())
        .filter(|id| {
            only.as_deref()
                .is_none_or(|only| only.split(',').any(|o| o == id))
        })
        .collect();
    // (boss, won, turns) per fight, and per run the cleared bosses.
    let fights: Mutex<BTreeMap<String, Tally>> = Mutex::new(BTreeMap::new());
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
                        // The boss follows the seed, so one game from any seed meets the same boss as in a longer series.
                        let boss = &bosses[usize::try_from(seed - 1).unwrap() % bosses.len()];
                        let (mut run, _) = lobby(seed, heroes, heat);
                        // A boss off this seed's ladder takes the place of the boss of its rank (the final one for a
                        // final boss, else the first).
                        let node = match run.map.nodes.iter().find(|n| n.boss.as_deref() == Some(boss)) {
                            Some(node) => node.id,
                            None => {
                                let last = theme.boss(boss).unwrap().final_boss;
                                let node = run
                                    .map
                                    .nodes
                                    .iter_mut()
                                    .filter(|n| n.boss.is_some())
                                    .reduce(|first, n| if last { n } else { first })
                                    .unwrap();
                                node.boss = Some(boss.clone());
                                node.id
                            }
                        };
                        run.at = Some(node);
                        run.status = Status::Fight { game: None };
                        let mut plan = run.fight_plan(content).unwrap().unwrap();
                        // `DELVE_SIM_DEPTH` meets the boss at that depth (bosses defeated before it).
                        if let Ok(depth) = std::env::var("DELVE_SIM_DEPTH") {
                            let spec = theme.boss(boss).unwrap();
                            plan.boss = run
                                .boss_seat(content, spec, node, (heroes, depth.parse().unwrap()))
                                .unwrap();
                        }
                        let t = Instant::now();
                        let fought = fight(&plan, &vec![Vec::new(); heroes], seed, hero_level);
                        println!(
                            "seed {seed} {boss}: {} in {} turns ({} boss side, {:.0}s), pressure {:?}, half {:?}, fall {:?}{}",
                            if fought.won { "win" } else { "loss" },
                            fought.turns,
                            fought.boss_turns,
                            t.elapsed().as_secs_f64(),
                            fought.pressure,
                            fought.first_half,
                            fought.first_fall,
                            if fought.failed { " FAILED" } else { "" }
                        );
                        fights
                            .lock()
                            .unwrap()
                            .entry(boss.clone())
                            .or_default()
                            .add(&fought);
                    } else {
                        let (mut run, _) = lobby(seed, heroes, heat);
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
                                        run.heroes.iter().map(|h| h.satchel.clone()).collect();
                                    fights_played += 1;
                                    let fought = fight(
                                        &plan,
                                        &cards,
                                        seed * 100 + fights_played,
                                        hero_level,
                                    );
                                    fights
                                        .lock()
                                        .unwrap()
                                        .entry(plan.boss.boss.clone())
                                        .or_default()
                                        .add(&fought);
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
        "\n{heroes} heroes ({hero_level:?}), Heat {heat}, {:.0}s",
        started.elapsed().as_secs_f64()
    );
    let table = fights.lock().unwrap();
    let mut all = Tally::default();
    for (boss, tally) in table.iter() {
        println!("{boss:>16}: {}", tally.line());
        all.merge(tally);
    }
    println!("{:>16}: {}", "all", all.line());
    println!("{:>16}: {}", "spotlight", all.spotlight_line());
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
