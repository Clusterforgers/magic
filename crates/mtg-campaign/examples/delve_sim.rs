//! Plays Delves fights and runs with bot parties to tune the bosses: `cargo run --release -p mtg-campaign --example
//! delve_sim -- <mode> [games] [heroes] [hero level] [first seed] [threads] [heat] [depth] [modifier]`, a mode
//! `fights` (each boss against a fresh party) or `runs` (whole runs, life carried and loot taken), a level `easy`,
//! `normal`, `hard` or `veryhard`. The heroes play the four fixture Commander decks in turn;
//! every game is seeded and paced as at a live table, so a result replays with its seed.
//! The depth is the bosses defeated before the fight; the modifier is `rolled`, `none` or a modifier's id. Each boss
//! gets one `key=value` line: wins, the round each fight ended in, the share that reached each phase, turns, minutes.
//! A run plays the newest content generation (`DELVE_SIM_CONTENT` sets another); `DELVE_SIM_THEME` picks the theme
//! (`warrens` by default) and `DELVE_SIM_BOSSES` the bosses.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::print_stdout,
    clippy::pedantic
)]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use mtg_campaign::sim::{FightLength, Lengths};
use mtg_campaign::{Brain, FightPlan, HeroDeck, Run, RunEvent, Status, content};
use mtg_cards::CardDefinition;
use mtg_core::ids::PlayerId;
use mtg_engine::format::Commander;
use mtg_engine::{GameResult, GameSetup};
use mtg_testkit::brain::{Level, SeatBrain, brain, paced_brain};
use mtg_testkit::corpus;
use mtg_testkit::fixture_decks::{FixtureDeck, fixture_deck};
use mtg_testkit::harness::{PlayOptions, play_brains};

/// The deck cards the corpus lacks (drafts of the read tier), played as Wastes.
static MISSING: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());

/// A deck card by name: a basic land from the testkit, anything else from the corpus; a card the corpus lacks stands
/// in as Wastes and is listed in [`MISSING`].
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
        None => corpus::card(name).unwrap_or_else(|_| {
            MISSING.lock().unwrap().insert(name.to_owned());
            mtg_testkit::cards::wastes()
        }),
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
    /// The round the fight ended in and the boss phases it reached; none when it failed.
    length: Option<FightLength>,
    /// The boss's modifier, if it had one.
    modifier: Option<String>,
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

/// The decisions of one seat and kind: how many, their seconds in all and at most, the tree searches, their
/// rollouts and the answers the searches applied.
type Spent = (u64, f64, f64, u64, u64, u64);

/// What the decisions every seat answered cost, by seat and decision kind (`DELVE_SIM_PROFILE`).
static PROFILE: Mutex<BTreeMap<(usize, String), Spent>> = Mutex::new(BTreeMap::new());

/// A brain that adds the time its inner brain takes to [`PROFILE`].
struct Profiled {
    seat: usize,
    inner: Box<dyn SeatBrain>,
}

impl SeatBrain for Profiled {
    fn choose(
        &mut self,
        decision: &mtg_engine::Decision,
        table: Option<&mtg_engine::Engine>,
    ) -> mtg_engine::decision::Answer {
        let before = (
            mtg_testkit::brain::deep::tally().0,
            mtg_testkit::brain::deep::iterations(),
            mtg_testkit::bot::applied(),
        );
        let t = Instant::now();
        let answer = self.inner.choose(decision, table);
        let secs = t.elapsed().as_secs_f64();
        let kind = format!("{:?}", decision.kind);
        let kind = kind.split(['(', ' ', '{']).next().unwrap_or("").to_owned();
        let mut profile = PROFILE.lock().unwrap();
        let entry = profile.entry((self.seat, kind)).or_default();
        entry.0 += 1;
        entry.1 += secs;
        entry.2 = entry.2.max(secs);
        entry.3 += mtg_testkit::brain::deep::tally().0 - before.0;
        entry.4 += mtg_testkit::brain::deep::iterations() - before.1;
        entry.5 += mtg_testkit::bot::applied() - before.2;
        answer
    }

    fn set_budget(&mut self, share: f64) {
        self.inner.set_budget(share);
    }

    fn set_asked(&mut self, at: Instant) {
        self.inner.set_asked(at);
    }
}

/// The brain of `seat`: paced as at a live table ([`paced_brain`]); `DELVE_SIM_TIMED` plays on the clock instead and
/// `DELVE_SIM_UNPACED` searches the level's whole count.
fn sim_brain(level: Level, seed: u64, seat: PlayerId) -> Box<dyn SeatBrain> {
    if std::env::var_os("DELVE_SIM_TIMED").is_some() {
        brain(level, seed, seat, true)
    } else if std::env::var_os("DELVE_SIM_UNPACED").is_some() {
        brain(level, seed, seat, false)
    } else {
        paced_brain(level, seed, seat)
    }
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
            let inner = sim_brain(level, seed, player);
            if std::env::var_os("DELVE_SIM_PROFILE").is_some() {
                Box::new(Profiled { seat, inner }) as Box<dyn SeatBrain>
            } else {
                inner
            }
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
            let mut round = 0;
            if let Ok(engine) = state {
                round = engine.turn().round();
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
            let length = Some(FightLength::of(round, &outcome.events, party));
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
                length,
                modifier: plan.boss.modifier.clone(),
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
                length: None,
                modifier: plan.boss.modifier.clone(),
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
    /// The round each fight ended in and the phases it reached.
    lengths: Lengths,
    /// The modifiers the boss fought with.
    modifiers: BTreeSet<String>,
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
        if let Some(length) = fought.length {
            self.lengths.add(length);
        }
        self.modifiers
            .insert(fought.modifier.clone().unwrap_or_else(|| "none".into()));
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
        self.lengths.merge(&other.lengths);
        self.modifiers.extend(other.modifiers.iter().cloned());
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

    /// The tally as `key=value` fields after the boss's own: fights, wins, the rounds and phases, turns and minutes.
    fn line(&self) -> String {
        let n = self.n.max(1) as f64;
        let hero_turns = (self.turns - self.boss_turns) as f64 / n;
        let boss_turns = self.boss_turns as f64 / n;
        let modifiers: Vec<&str> = self.modifiers.iter().map(String::as_str).collect();
        format!(
            "modifier={} n={} won={} win={:.0}% {} turns={:.1} boss_turns={:.1} min={:.0} falls={} rises={} haunts={} failed={}",
            modifiers.join("+"),
            self.n,
            self.won,
            100.0 * self.won as f64 / n,
            self.lengths.fields(),
            self.turns as f64 / n,
            boss_turns,
            hero_turns * HERO_TURN_MINUTES + boss_turns * BOSS_TURN_MINUTES,
            self.falls,
            self.rises,
            self.haunts,
            self.failed,
        )
    }
}

/// A run of `theme` in its lobby with `heroes` bot heroes on the precons; the first hero counts as the player.
fn lobby(theme: &str, seed: u64, heroes: usize, heat: u8) -> (Run, Vec<RunEvent>) {
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
        Run::replay(content, theme, seed, heat, &events).unwrap(),
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
    let depth: u32 = args.get(7).map_or(0, |a| a.parse().unwrap());
    // The modifier every fight has: `rolled` (the run's draw), `none` or a modifier's id.
    let modifier = args.get(8).cloned().unwrap_or_else(|| "rolled".into());
    let content = content().unwrap();
    // The bosses' bots read their Satchel cues from the content, as the server's do.
    mtg_testkit::brain::satchel::install(content.satchel_cues());
    // `DELVE_SIM_THEME` plays that theme's bosses; the Warrens by default.
    let theme = content
        .theme(&std::env::var("DELVE_SIM_THEME").unwrap_or_else(|_| "warrens".into()))
        .unwrap();
    let modifier: Option<Option<String>> = match modifier.as_str() {
        "rolled" => None,
        "none" => Some(None),
        id => {
            assert!(
                content.modifiers.iter().any(|m| m.id == id),
                "unknown modifier {id}"
            );
            Some(Some(id.to_owned()))
        }
    };
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
                        let (mut run, _) = lobby(&theme.id, seed, heroes, heat);
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
                        // The boss meets the party at `depth` (bosses defeated before it).
                        let spec = theme.boss(boss).unwrap();
                        plan.boss = run
                            .boss_seat(content, spec, node, (heroes, depth))
                            .unwrap();
                        if let Some(modifier) = &modifier {
                            plan.boss.modifier.clone_from(modifier);
                        }
                        let t = Instant::now();
                        let fought = fight(&plan, &vec![Vec::new(); heroes], seed, hero_level);
                        // `DELVE_SIM_SEEDS` prints a line per fight.
                        if std::env::var_os("DELVE_SIM_SEEDS").is_some() {
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
                        }
                        fights
                            .lock()
                            .unwrap()
                            .entry(boss.clone())
                            .or_default()
                            .add(&fought);
                    } else {
                        let (mut run, _) = lobby(&theme.id, seed, heroes, heat);
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
    // The fields every summary line starts with after the boss.
    let head = format!(
        "theme={} heroes={heroes} level={hero_level:?} heat={heat} depth={} secs={:.0}",
        theme.id,
        if mode == "fights" {
            depth.to_string()
        } else {
            "run".into()
        },
        started.elapsed().as_secs_f64()
    );
    for ((seat, kind), (count, secs, most, searches, iterations, applied)) in
        PROFILE.lock().unwrap().iter()
    {
        println!(
            "profile seat={seat} kind={kind} count={count} secs={secs:.1} max={most:.2} searches={searches} iterations={iterations} applied={applied}"
        );
    }
    let table = fights.lock().unwrap();
    let mut all = Tally::default();
    for (boss, tally) in table.iter() {
        println!("boss={boss} {head} {}", tally.line());
        all.merge(tally);
    }
    println!("boss=all {head} {}", all.line());
    println!("spotlight {}", all.spotlight_line());
    let missing = MISSING.lock().unwrap();
    if !missing.is_empty() {
        let names: Vec<&str> = missing.iter().map(String::as_str).collect();
        println!("missing n={} as Wastes: {}", names.len(), names.join("; "));
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
