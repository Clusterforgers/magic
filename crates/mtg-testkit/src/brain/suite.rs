//! The decision-quality suite: hand-made positions, each with the answers a strong player accepts, for scoring a
//! brain's judgement (counters, holding mana, removal, protection, attacks, blocks, wipes, development, tricks and
//! multiplayer targeting) apart from the luck of whole games.
//!
//! Seat 0 is always the brain under test. A [`Judge::Answer`] case reads the brain's answer to the pending decision;
//! a [`Judge::Outcome`] case lets the brain play seat 0 while every other seat answers passively
//! ([`Driver::passive_answer`]) until [`Until`], then reads the game.

use std::sync::Arc;
use std::time::{Duration, Instant};

use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::color::Color;
use mtg_core::house::HouseRules;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::teams::{TeamMode, Teams};
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{Answer, PriorityAction};
use mtg_engine::format::Commander;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, GameResult, PresetPermanent};
use thiserror::Error;

use super::{Level, SeatBrain, brain};
use crate::bot::apply_answer;
use crate::cards::{forest, island, keyworded_creature, mountain, plains, swamp};
use crate::corpus::{CorpusError, card};
use crate::driver::Driver;
use crate::scenario::{Scenario, ScenarioError, ready, with_counters};
use crate::script_error::ScriptError;

/// Why a case could not be set up or played.
#[derive(Debug, Error)]
pub enum SuiteError {
    /// A corpus card is missing.
    #[error("corpus: {0}")]
    Corpus(#[from] CorpusError),
    /// The table could not be built.
    #[error("scenario: {0}")]
    Scenario(#[from] ScenarioError),
    /// The setup's scripted moves failed.
    #[error("script: {0}")]
    Script(#[from] ScriptError),
    /// The position has no decision for seat 0.
    #[error("no decision for seat 0 in {0}")]
    NoDecision(&'static str),
}

/// The skill a case measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Theme {
    /// Which spells to counter and which to let resolve.
    Counter,
    /// Keeping mana open for interaction, or using it at the end of a turn.
    Hold,
    /// Which permanent removal goes at.
    Removal,
    /// Answering removal aimed at the seat's own creature.
    Protect,
    /// Which creatures attack, and whom.
    Attack,
    /// Which creatures block.
    Block,
    /// When a board wipe is worth casting.
    Wipe,
    /// Mana, lands and spells in a good order.
    Develop,
    /// Combat tricks and fogs.
    Trick,
}

/// When an outcome case stops playing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Until {
    /// The step the case started in is over.
    StepChanges,
    /// Combat damage has been dealt this turn.
    CombatEnds,
    /// The turn the case started in is over.
    TurnEnds,
}

/// A check on the answer seat 0 gave.
pub type AnswerCheck = Box<dyn Fn(&Engine, &Answer) -> bool>;
/// A check on the game after seat 0 played.
pub type OutcomeCheck = Box<dyn Fn(&Engine) -> bool>;

/// How a case is judged.
pub enum Judge {
    /// The answer to the pending decision passes the check.
    Answer(AnswerCheck),
    /// The game after playing on to [`Until`] passes the check.
    Outcome(Until, OutcomeCheck),
}

/// A position with its judge.
pub struct Table {
    /// The game, with a decision of seat 0 pending.
    pub engine: Engine,
    /// How the answer is judged.
    pub judge: Judge,
}

/// One case of the suite.
#[derive(Debug, Clone, Copy)]
pub struct Case {
    /// A short name of what the right play is.
    pub name: &'static str,
    /// The skill it measures.
    pub theme: Theme,
    /// Builds the position.
    pub build: fn() -> Result<Table, SuiteError>,
}

/// The result of one case.
#[derive(Debug, Clone, Copy)]
pub struct Verdict {
    /// The case.
    pub case: Case,
    /// Whether the brain's play was acceptable.
    pub passed: bool,
    /// The longest single answer.
    pub longest: Duration,
}

/// The most answers an outcome case plays.
const LIMIT: usize = 400;

/// Plays `case` with `brain` at seat 0.
pub fn run(case: &Case, brain: &mut dyn SeatBrain) -> Result<Verdict, SuiteError> {
    let Table { mut engine, judge } = (case.build)()?;
    let seat = PlayerId::from_index(0);
    let mut longest = Duration::ZERO;
    let passed = match judge {
        Judge::Answer(check) => {
            let decision = engine
                .decision()
                .filter(|d| d.player == seat)
                .cloned()
                .ok_or(SuiteError::NoDecision(case.name))?;
            let start = Instant::now();
            let answer = brain.choose(&decision, Some(&engine));
            longest = start.elapsed();
            check(&engine, &answer)
        }
        Judge::Outcome(until, check) => {
            let turn = engine.state().turn().number();
            let step = engine.state().turn().step();
            for _ in 0..LIMIT {
                if engine.result().is_some() || stopped(&engine, until, turn, step) {
                    break;
                }
                let Some(decision) = engine.decision().cloned() else {
                    break;
                };
                let answer = if decision.player == seat {
                    let start = Instant::now();
                    let answer = brain.choose(&decision, Some(&engine));
                    longest = longest.max(start.elapsed());
                    answer
                } else {
                    Driver::passive_answer(&decision)
                };
                if apply_answer(&mut engine, &decision, answer).is_err() {
                    let fallback = Driver::passive_answer(&decision);
                    if apply_answer(&mut engine, &decision, fallback).is_err() {
                        break;
                    }
                }
            }
            check(&engine)
        }
    };
    Ok(Verdict {
        case: *case,
        passed,
        longest,
    })
}

/// Whether an outcome case that started in `turn` at `step` has reached `until`.
fn stopped(engine: &Engine, until: Until, turn: u32, step: Step) -> bool {
    let now = engine.state().turn();
    if now.number() != turn {
        return true;
    }
    match until {
        Until::StepChanges => now.step() != step,
        Until::CombatEnds => super::sim::after_combat(now.step()),
        Until::TurnEnds => false,
    }
}

/// Plays every case with a fresh brain of `level` (seeded with `seed`, untimed unless `timed`).
pub fn score(level: Level, seed: u64, timed: bool) -> Vec<Result<Verdict, SuiteError>> {
    cases()
        .iter()
        .map(|case| {
            let mut bot = brain(level, seed, PlayerId::from_index(0), timed);
            run(case, bot.as_mut())
        })
        .collect()
}

// ---- Position helpers ----

/// `n` untapped copies of a land.
fn lands(land: fn() -> Arc<CardDefinition>, n: usize) -> Vec<PresetPermanent> {
    (0..n).map(|_| ready(land())).collect()
}

/// A creature costing `generic` and one `color`, with keywords.
fn beast(
    name: &str,
    generic: u32,
    color: Color,
    power: i32,
    toughness: i32,
    keys: &[Keyword],
) -> Arc<CardDefinition> {
    keyworded_creature(name, generic, &[color], power, toughness, keys)
}

/// An untapped creature on the battlefield.
fn body(name: &str, power: i32, toughness: i32, keys: &[Keyword]) -> PresetPermanent {
    ready(beast(name, 2, Color::Green, power, toughness, keys))
}

/// Cards that stand for an opponent's unknown hand.
fn filler(n: usize) -> Vec<Arc<CardDefinition>> {
    (0..n).map(|_| forest()).collect()
}

/// A corpus card.
fn real(name: &str) -> Result<Arc<CardDefinition>, SuiteError> {
    Ok(card(name)?)
}

/// Gives priority passively until seat 0 has a decision.
fn to_seat0(driver: &mut Driver) -> Result<(), SuiteError> {
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        if decision.player.index() == 0 {
            return Ok(());
        }
        driver.answer(Driver::passive_answer(&decision))?;
    }
    Err(SuiteError::NoDecision("setup"))
}

/// The id of the object named `name` in any zone.
pub fn named(engine: &Engine, name: &str) -> Option<ObjectId> {
    let state = engine.state();
    state
        .objects()
        .find(|object| object.card().name == name)
        .map(mtg_engine::object::GameObject::id)
}

/// Whether a permanent named `name` is on the battlefield.
fn on_battlefield(engine: &Engine, name: &str) -> bool {
    let state = engine.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .any(|id| state.object(*id).is_some_and(|o| o.card().name == name))
}

/// Whether a card named `name` is in seat 0's hand.
fn in_hand(engine: &Engine, name: &str) -> bool {
    let state = engine.state();
    state
        .zone(Zone::Hand(PlayerId::from_index(0)))
        .iter()
        .any(|id| state.object(*id).is_some_and(|o| o.card().name == name))
}

/// Whether `answer` casts the spell named `name`.
fn casts(engine: &Engine, answer: &Answer, name: &str) -> bool {
    matches!(answer, Answer::Priority(PriorityAction::CastSpell(id))
        if engine.state().object(*id).is_some_and(|o| o.card().name == name))
}

/// Whether `answer` is a pass.
fn passes(answer: &Answer) -> bool {
    matches!(answer, Answer::Priority(PriorityAction::Pass))
}

/// The attackers of an attack declaration, with what each attacks.
fn attackers(answer: &Answer) -> Vec<(ObjectId, AttackTarget)> {
    match answer {
        Answer::DeclareAttackers(attack) => attack.clone(),
        Answer::DeclareExerting { attackers, .. } => attackers.clone(),
        _ => Vec::new(),
    }
}

/// The names of the attackers in `answer`.
fn attacking_names(engine: &Engine, answer: &Answer) -> Vec<String> {
    attackers(answer)
        .iter()
        .filter_map(|(id, _)| engine.state().object(*id).map(|o| o.card().name.clone()))
        .collect()
}

/// The (blocker, attacker) names of a block declaration.
fn blocks(engine: &Engine, answer: &Answer) -> Vec<(String, String)> {
    let Answer::DeclareBlockers(pairs) = answer else {
        return Vec::new();
    };
    let name = |id: &ObjectId| {
        engine
            .state()
            .object(*id)
            .map_or_else(String::new, |o| o.card().name.clone())
    };
    pairs.iter().map(|(b, a)| (name(b), name(a))).collect()
}

/// Whether seat `seat` has lost or the game went to someone else.
fn lost(engine: &Engine, seat: usize) -> bool {
    let player = PlayerId::from_index(seat);
    engine
        .state()
        .player(player)
        .is_none_or(|p| !p.is_in_game() || p.life() <= 0)
        || matches!(engine.result(), Some(GameResult::Winner(w)) if w != player)
}

/// A judge on the answer.
fn answer(check: impl Fn(&Engine, &Answer) -> bool + 'static) -> Judge {
    Judge::Answer(Box::new(check))
}

/// A judge on the outcome.
fn outcome(until: Until, check: impl Fn(&Engine) -> bool + 'static) -> Judge {
    Judge::Outcome(until, Box::new(check))
}

/// A table with its judge; a `Result` so every builder ends the same way.
#[allow(clippy::unnecessary_wraps)]
fn table(driver: &Driver, judge: Judge) -> Result<Table, SuiteError> {
    Ok(Table {
        engine: driver.engine().clone(),
        judge,
    })
}

const NONE: &[Keyword] = &[];
const FLYING: &[Keyword] = &[Keyword::Flying];
const DEATHTOUCH: &[Keyword] = &[Keyword::Deathtouch];

/// The opponent's bomb: a 6/6 flyer.
fn bomb() -> Arc<CardDefinition> {
    beast("Dread Dragon", 4, Color::Red, 6, 6, FLYING)
}

/// Seat 1 casts the spell `name` (aimed at `targets`, by name; a number names a seat) in its main phase, then seat 0 has
/// priority with it on the stack. Seat 0 holds `hand` with `mine` in play; seat 1 has `theirs` and `cards` unknown.
fn on_stack(scenario: Scenario, name: &str, targets: &[&str]) -> Result<Driver, SuiteError> {
    let mut driver = Driver::from_scenario(scenario)?;
    if targets.is_empty() {
        driver.cast(name)?;
    } else {
        let mut aimed = Vec::new();
        for target in targets {
            aimed.push(match target.parse::<usize>() {
                Ok(seat) => driver.target_player(seat),
                Err(_) => driver.target_named(target)?,
            });
        }
        driver.cast_at(name, &aimed)?;
    }
    to_seat0(&mut driver)?;
    Ok(driver)
}

/// A two-seat table in seat 1's first main phase, seat 0 holding Counterspell over two Islands.
fn counter_table(
    mine: Vec<PresetPermanent>,
    theirs: Vec<PresetPermanent>,
    spell: Arc<CardDefinition>,
    unknown: usize,
) -> Result<Scenario, SuiteError> {
    let mut own = lands(island, 2);
    own.extend(mine);
    Ok(Scenario::new(2)
        .at(4, 1, Step::Main1)
        .hand(0, [real("Counterspell")?])
        .battlefield(0, own)
        .hand(1, std::iter::once(spell).chain(filler(unknown)))
        .battlefield(1, theirs))
}

// ---- Counter ----

fn counter_the_bomb() -> Result<Table, SuiteError> {
    let scenario = counter_table(vec![], lands(mountain, 6), bomb(), 4)?;
    let driver = on_stack(scenario, &bomb().name, &[])?;
    table(&driver, answer(|e, a| casts(e, a, "Counterspell")))
}

fn let_the_mana_dork_resolve() -> Result<Table, SuiteError> {
    let elves = real("Llanowar Elves")?;
    let scenario = counter_table(vec![], lands(forest, 5), Arc::clone(&elves), 5)?;
    let driver = on_stack(scenario, &elves.name, &[])?;
    table(&driver, answer(|_, a| passes(a)))
}

fn let_the_bear_resolve() -> Result<Table, SuiteError> {
    let bears = real("Grizzly Bears")?;
    let scenario = counter_table(vec![], lands(forest, 5), Arc::clone(&bears), 5)?;
    let driver = on_stack(scenario, &bears.name, &[])?;
    table(&driver, answer(|_, a| passes(a)))
}

fn counter_the_card_engine() -> Result<Table, SuiteError> {
    let study = real("Rhystic Study")?;
    let scenario = counter_table(vec![], lands(island, 4), Arc::clone(&study), 4)?;
    let driver = on_stack(scenario, &study.name, &[])?;
    table(&driver, answer(|e, a| casts(e, a, "Counterspell")))
}

fn counter_the_wrath_that_hits_us() -> Result<Table, SuiteError> {
    let wrath = real("Wrath of God")?;
    let mine = vec![
        body("Our Knight", 3, 3, NONE),
        body("Our Ogre", 3, 3, NONE),
        body("Our Drake", 3, 3, FLYING),
    ];
    let scenario = counter_table(mine, lands(plains, 4), Arc::clone(&wrath), 3)?;
    let driver = on_stack(scenario, &wrath.name, &[])?;
    table(&driver, answer(|e, a| casts(e, a, "Counterspell")))
}

fn let_the_wrath_that_hits_them_resolve() -> Result<Table, SuiteError> {
    let wrath = real("Wrath of God")?;
    let mut theirs = lands(plains, 4);
    theirs.extend([
        body("Their Knight", 3, 3, NONE),
        body("Their Ogre", 3, 3, NONE),
        body("Their Drake", 3, 3, FLYING),
    ]);
    let scenario = counter_table(vec![], theirs, Arc::clone(&wrath), 3)?;
    let driver = on_stack(scenario, &wrath.name, &[])?;
    table(&driver, answer(|_, a| passes(a)))
}

fn counter_removal_on_the_key_creature() -> Result<Table, SuiteError> {
    let blade = real("Doom Blade")?;
    let mine = vec![body("Sky Tyrant", 5, 5, FLYING)];
    let scenario = counter_table(mine, lands(swamp, 4), Arc::clone(&blade), 4)?;
    let driver = on_stack(scenario, &blade.name, &["Sky Tyrant"])?;
    table(&driver, answer(|e, a| casts(e, a, "Counterspell")))
}

fn let_removal_on_a_small_creature_resolve() -> Result<Table, SuiteError> {
    let blade = real("Doom Blade")?;
    let mine = vec![body("Squire", 1, 1, NONE)];
    let scenario = counter_table(mine, lands(swamp, 5), Arc::clone(&blade), 5)?;
    let driver = on_stack(scenario, &blade.name, &["Squire"])?;
    table(&driver, answer(|_, a| passes(a)))
}

fn counter_lethal_burn() -> Result<Table, SuiteError> {
    let bolt = real("Lightning Bolt")?;
    let scenario = counter_table(vec![], lands(mountain, 3), Arc::clone(&bolt), 3)?.life(0, 3);
    let driver = on_stack(scenario, &bolt.name, &["0"])?;
    table(&driver, answer(|e, a| casts(e, a, "Counterspell")))
}

fn let_burn_at_high_life_resolve() -> Result<Table, SuiteError> {
    let bolt = real("Lightning Bolt")?;
    let scenario = counter_table(vec![], lands(mountain, 5), Arc::clone(&bolt), 5)?;
    let driver = on_stack(scenario, &bolt.name, &["0"])?;
    table(&driver, answer(|_, a| passes(a)))
}

fn counter_the_combo_piece() -> Result<Table, SuiteError> {
    let vizier = real("Vizier of Remedies")?;
    let mut theirs = lands(plains, 4);
    theirs.push(ready(real("Devoted Druid")?));
    let scenario = counter_table(vec![], theirs, Arc::clone(&vizier), 3)?;
    let driver = on_stack(scenario, &vizier.name, &[])?;
    table(&driver, answer(|e, a| casts(e, a, "Counterspell")))
}

fn counter_the_counter_on_our_bomb() -> Result<Table, SuiteError> {
    let scenario = Scenario::new(2)
        .at(5, 0, Step::Main1)
        .hand(0, [real("Counterspell")?, bomb()])
        .battlefield(0, {
            let mut own = lands(island, 2);
            own.extend(lands(mountain, 6));
            own
        })
        .hand(1, std::iter::once(real("Counterspell")?).chain(filler(3)))
        .battlefield(1, lands(island, 3));
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast_tapping(
        "Dread Dragon",
        &["Mountain", "Mountain", "Mountain", "Mountain", "Mountain"],
    )?;
    driver.pass()?;
    let dragon = driver.engine().state().stack().last().copied();
    let spell = dragon
        .and_then(|id| {
            driver
                .state()
                .object(id)
                .map(mtg_engine::object::GameObject::reference)
        })
        .ok_or(SuiteError::NoDecision("dragon on the stack"))?;
    driver.cast_at("Counterspell", &[mtg_engine::choose::Target::Object(spell)])?;
    to_seat0(&mut driver)?;
    table(&driver, answer(|e, a| casts(e, a, "Counterspell")))
}

fn let_a_teammates_spell_resolve() -> Result<Table, SuiteError> {
    let house = HouseRules {
        teams: Some(Teams::even(TeamMode::Teams, 2, 2)),
        ..HouseRules::default()
    };
    let scenario = Scenario::new(4)
        .house_rules(house)
        .at(4, 2, Step::Main1)
        .hand(0, [real("Counterspell")?])
        .battlefield(0, lands(island, 2))
        .hand(2, [bomb()])
        .battlefield(2, lands(mountain, 6));
    let driver = on_stack(scenario, &bomb().name, &[])?;
    table(&driver, answer(|_, a| passes(a)))
}

fn counter_the_leaders_bomb() -> Result<Table, SuiteError> {
    let mut leader = lands(mountain, 7);
    leader.extend([
        body("Leader Drake", 3, 3, FLYING),
        body("Leader Ogre", 4, 4, NONE),
        body("Leader Brute", 3, 3, NONE),
    ]);
    let scenario = Scenario::new(4)
        .at(6, 2, Step::Main1)
        .hand(0, [real("Counterspell")?])
        .battlefield(0, lands(island, 2))
        .hand(2, std::iter::once(bomb()).chain(filler(5)))
        .battlefield(2, leader)
        .life(2, 34)
        .battlefield(1, vec![body("Small Fry", 1, 1, NONE)])
        .life(1, 9);
    let driver = on_stack(scenario, &bomb().name, &[])?;
    table(&driver, answer(|e, a| casts(e, a, "Counterspell")))
}

fn let_the_weakest_player_develop() -> Result<Table, SuiteError> {
    let ogre = beast("Weak Ogre", 2, Color::Red, 3, 3, NONE);
    let mut leader = lands(island, 8);
    leader.extend([
        body("Leader Drake", 3, 3, FLYING),
        body("Leader Ogre", 4, 4, NONE),
    ]);
    let scenario = Scenario::new(4)
        .at(6, 1, Step::Main1)
        .hand(0, [real("Counterspell")?])
        .battlefield(0, lands(island, 2))
        .hand(1, [Arc::clone(&ogre)])
        .battlefield(1, lands(mountain, 3))
        .life(1, 5)
        .hand(2, filler(6))
        .battlefield(2, leader)
        .life(2, 33);
    let driver = on_stack(scenario, &ogre.name, &[])?;
    table(&driver, answer(|_, a| passes(a)))
}

// ---- Hold ----

/// Seat 0's first main phase on turn 5 with `hand` and `mine`, seat 1 with `theirs` and `unknown` cards.
fn own_main(
    hand: Vec<Arc<CardDefinition>>,
    mine: Vec<PresetPermanent>,
    theirs: Vec<PresetPermanent>,
    unknown: usize,
) -> Scenario {
    Scenario::new(2)
        .at(5, 0, Step::Main1)
        .hand(0, hand)
        .battlefield(0, mine)
        .hand(1, filler(unknown))
        .battlefield(1, theirs)
}

fn hold_the_counter_over_a_weak_creature() -> Result<Table, SuiteError> {
    let merfolk = beast("Mediocre Merfolk", 1, Color::Blue, 2, 1, NONE);
    let mut theirs = lands(mountain, 6);
    theirs.push(body("Their Bear", 2, 2, NONE));
    let scenario = own_main(
        vec![real("Counterspell")?, merfolk],
        lands(island, 3),
        theirs,
        6,
    );
    let driver = Driver::from_scenario(scenario)?;
    table(
        &driver,
        outcome(Until::TurnEnds, |e| in_hand(e, "Mediocre Merfolk")),
    )
}

fn tap_out_for_a_bomb_into_an_empty_hand() -> Result<Table, SuiteError> {
    let tyrant = keyworded_creature("Sky Tyrant", 4, &[Color::Blue, Color::Blue], 5, 5, FLYING);
    let scenario = own_main(
        vec![real("Counterspell")?, tyrant],
        lands(island, 6),
        lands(mountain, 2),
        0,
    );
    let driver = Driver::from_scenario(scenario)?;
    table(
        &driver,
        outcome(Until::TurnEnds, |e| on_battlefield(e, "Sky Tyrant")),
    )
}

fn hold_removal_for_a_real_threat() -> Result<Table, SuiteError> {
    let mut theirs = lands(mountain, 6);
    theirs.push(body("Lone Squirrel", 1, 1, NONE));
    let scenario = own_main(vec![real("Doom Blade")?], lands(swamp, 2), theirs, 6);
    let driver = Driver::from_scenario(scenario)?;
    table(
        &driver,
        outcome(Until::TurnEnds, |e| in_hand(e, "Doom Blade")),
    )
}

fn flash_in_at_the_end_step() -> Result<Table, SuiteError> {
    let drake = beast(
        "Ambush Drake",
        2,
        Color::Blue,
        3,
        3,
        &[Keyword::Flash, Keyword::Flying],
    );
    let scenario = Scenario::new(2)
        .at(5, 1, Step::End)
        .hand(0, [drake])
        .battlefield(0, lands(island, 3))
        .hand(1, filler(3))
        .battlefield(1, lands(mountain, 4));
    let mut driver = Driver::from_scenario(scenario)?;
    to_seat0(&mut driver)?;
    table(&driver, answer(|e, a| casts(e, a, "Ambush Drake")))
}

fn hold_the_flash_creature_for_their_end_step() -> Result<Table, SuiteError> {
    let drake = beast(
        "Ambush Drake",
        2,
        Color::Blue,
        3,
        3,
        &[Keyword::Flash, Keyword::Flying],
    );
    let scenario = own_main(
        vec![real("Counterspell")?, drake],
        lands(island, 4),
        lands(mountain, 6),
        6,
    );
    let driver = Driver::from_scenario(scenario)?;
    table(
        &driver,
        outcome(Until::TurnEnds, |e| in_hand(e, "Ambush Drake")),
    )
}

// ---- Removal ----

/// Seat 0 in its main phase with removal `spell` over `mana`, seat 1 with `theirs`; judged once the step is over.
fn removal_table(
    spell: &str,
    mana: Vec<PresetPermanent>,
    mine: Vec<PresetPermanent>,
    theirs: Vec<PresetPermanent>,
) -> Result<Driver, SuiteError> {
    let mut own = mana;
    own.extend(mine);
    Ok(Driver::from_scenario(own_main(
        vec![real(spell)?],
        own,
        theirs,
        4,
    ))?)
}

fn kill_the_flyer_not_the_walled_ground_creature() -> Result<Table, SuiteError> {
    let driver = removal_table(
        "Doom Blade",
        lands(swamp, 2),
        vec![body("Our Wall", 1, 6, &[Keyword::Defender])],
        vec![
            body("Sky Raider", 3, 3, FLYING),
            body("Ground Pounder", 3, 3, NONE),
        ],
    )?;
    table(
        &driver,
        outcome(Until::StepChanges, |e| {
            !on_battlefield(e, "Sky Raider") && on_battlefield(e, "Ground Pounder")
        }),
    )
}

fn bolt_the_lifelinker_not_the_chaff() -> Result<Table, SuiteError> {
    let driver = removal_table(
        "Lightning Bolt",
        lands(mountain, 1),
        vec![],
        vec![
            body("Chaff Thopter", 1, 1, NONE),
            body("Leech Vampire", 2, 3, &[Keyword::Lifelink]),
        ],
    )?;
    table(
        &driver,
        outcome(Until::StepChanges, |e| !on_battlefield(e, "Leech Vampire")),
    )
}

fn bolt_the_face_for_lethal() -> Result<Table, SuiteError> {
    let scenario = own_main(
        vec![real("Lightning Bolt")?],
        lands(mountain, 1),
        vec![body("Their Bear", 2, 2, NONE)],
        4,
    )
    .life(1, 3);
    let driver = Driver::from_scenario(scenario)?;
    table(&driver, outcome(Until::StepChanges, |e| lost(e, 1)))
}

fn kill_the_card_engine_not_the_vanilla() -> Result<Table, SuiteError> {
    let driver = removal_table(
        "Doom Blade",
        lands(swamp, 2),
        vec![body("Our Guard", 4, 4, NONE)],
        vec![
            ready(real("Kynaios and Tiro of Meletis")?),
            body("Plain Brute", 4, 4, NONE),
        ],
    )?;
    table(
        &driver,
        outcome(Until::StepChanges, |e| {
            !on_battlefield(e, "Kynaios and Tiro of Meletis")
        }),
    )
}

fn kill_the_commander_not_the_vanilla() -> Result<Table, SuiteError> {
    let general = keyworded_creature("Warlord General", 2, &[Color::Red], 3, 3, NONE);
    let mut general = Arc::unwrap_or_clone(general);
    general
        .supertypes
        .insert(mtg_core::card::Supertype::Legendary);
    let general = Arc::new(general);
    let scenario = Scenario::new(2)
        .format(Arc::new(Commander))
        .at(5, 1, Step::Main1)
        .commanders(1, [general])
        .battlefield(1, {
            let mut theirs = lands(mountain, 3);
            theirs.push(body("Plain Soldier", 3, 3, NONE));
            theirs
        })
        .hand(0, [real("Doom Blade")?])
        .battlefield(0, {
            let mut own = lands(swamp, 2);
            own.push(body("Our Guard", 3, 3, NONE));
            own
        });
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Warlord General")?;
    for _ in 0..2 {
        driver.pass()?;
    }
    driver.idle_until(6, Step::Main1)?;
    to_seat0(&mut driver)?;
    table(
        &driver,
        outcome(Until::StepChanges, |e| {
            !on_battlefield(e, "Warlord General")
        }),
    )
}

fn break_the_combo() -> Result<Table, SuiteError> {
    let ballista = with_counters(
        ready(real("Walking Ballista")?),
        CounterKind::PlusOnePlusOne,
        1,
    );
    let driver = removal_table(
        "Doom Blade",
        lands(swamp, 2),
        vec![],
        vec![
            ready(real("Heliod, Sun-Crowned")?),
            ballista,
            body("Plain Brute", 4, 4, NONE),
        ],
    )?;
    table(
        &driver,
        outcome(Until::StepChanges, |e| {
            !on_battlefield(e, "Walking Ballista")
        }),
    )
}

fn pacify_the_attacker_not_the_wall() -> Result<Table, SuiteError> {
    let driver = removal_table(
        "Pacifism",
        lands(plains, 2),
        vec![],
        vec![
            body("Stone Wall", 0, 6, &[Keyword::Defender]),
            body("Raider", 3, 3, NONE),
        ],
    )?;
    table(
        &driver,
        outcome(Until::StepChanges, |e| {
            let state = e.state();
            named(e, "Pacifism")
                .and_then(|id| state.object(id))
                .and_then(|o| o.permanent())
                .and_then(mtg_engine::object::PermanentState::attached_to)
                .and_then(|to| state.object(to.id))
                .is_some_and(|o| o.card().name == "Raider")
        }),
    )
}

fn exile_the_indestructible() -> Result<Table, SuiteError> {
    let golem = ready(beast(
        "Iron Golem",
        3,
        Color::White,
        4,
        4,
        &[Keyword::Indestructible],
    ));
    let mut own = lands(swamp, 2);
    own.push(ready(plains()));
    let scenario = own_main(
        vec![real("Doom Blade")?, real("Swords to Plowshares")?],
        own,
        vec![golem],
        4,
    );
    let driver = Driver::from_scenario(scenario)?;
    table(
        &driver,
        outcome(Until::StepChanges, |e| !on_battlefield(e, "Iron Golem")),
    )
}

fn remove_the_leaders_threat() -> Result<Table, SuiteError> {
    let mut own = lands(swamp, 2);
    own.push(body("Our Guard", 4, 4, NONE));
    let scenario = Scenario::new(4)
        .at(6, 0, Step::Main1)
        .hand(0, [real("Doom Blade")?])
        .battlefield(0, own)
        .battlefield(
            1,
            vec![
                body("Leader Angel", 4, 4, FLYING),
                body("Leader Knight", 3, 3, NONE),
                body("Leader Ogre", 3, 3, NONE),
            ],
        )
        .life(1, 35)
        .hand(1, filler(5))
        .battlefield(2, vec![body("Lonely Wurm", 4, 4, NONE)])
        .life(2, 7)
        .hand(2, filler(1));
    let driver = Driver::from_scenario(scenario)?;
    table(
        &driver,
        outcome(Until::StepChanges, |e| !on_battlefield(e, "Leader Angel")),
    )
}

fn kill_the_attacker_that_would_be_lethal() -> Result<Table, SuiteError> {
    let scenario = Scenario::new(2)
        .at(6, 1, Step::DeclareAttackers)
        .hand(0, [real("Doom Blade")?])
        .battlefield(0, lands(swamp, 2))
        .life(0, 5)
        .battlefield(1, vec![body("Charging Wurm", 6, 6, NONE)]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.attack(&[("Charging Wurm", 0)])?;
    to_seat0(&mut driver)?;
    table(&driver, outcome(Until::CombatEnds, |e| !lost(e, 0)))
}

// ---- Protect ----

fn protect_the_key_creature_from_removal() -> Result<Table, SuiteError> {
    let bolt = real("Lightning Bolt")?;
    let scenario = Scenario::new(2)
        .at(5, 1, Step::Main1)
        .hand(0, [real("Giant Growth")?])
        .battlefield(0, vec![ready(forest()), body("Prized Beast", 3, 3, NONE)])
        .hand(1, std::iter::once(Arc::clone(&bolt)).chain(filler(3)))
        .battlefield(1, lands(mountain, 3));
    let driver = on_stack(scenario, &bolt.name, &["Prized Beast"])?;
    table(
        &driver,
        outcome(Until::StepChanges, |e| on_battlefield(e, "Prized Beast")),
    )
}

fn shield_the_flyer_from_doom_blade() -> Result<Table, SuiteError> {
    let blade = real("Doom Blade")?;
    let scenario = Scenario::new(2)
        .at(5, 1, Step::Main1)
        .hand(0, [real("Apostle's Blessing")?])
        .battlefield(0, {
            let mut own = lands(plains, 2);
            own.push(body("Sky Tyrant", 5, 5, FLYING));
            own
        })
        .hand(1, std::iter::once(Arc::clone(&blade)).chain(filler(3)))
        .battlefield(1, lands(swamp, 3));
    let driver = on_stack(scenario, &blade.name, &["Sky Tyrant"])?;
    table(
        &driver,
        outcome(Until::StepChanges, |e| on_battlefield(e, "Sky Tyrant")),
    )
}

// ---- Attack ----

/// Seat 0 about to declare attackers on turn 5 with `mine` against seat 1 at `life` with `theirs`.
fn attack_table(
    mine: Vec<PresetPermanent>,
    theirs: Vec<PresetPermanent>,
    own_life: i32,
    life: i32,
) -> Result<Engine, SuiteError> {
    Ok(Scenario::new(2)
        .at(5, 0, Step::DeclareAttackers)
        .battlefield(0, mine)
        .battlefield(1, theirs)
        .life(0, own_life)
        .life(1, life)
        .build()?)
}

fn no_attack_into_untapped_deathtouch() -> Result<Table, SuiteError> {
    let engine = attack_table(
        vec![body("Big Brute", 4, 4, NONE)],
        vec![body("Assassin", 1, 1, DEATHTOUCH)],
        20,
        20,
    )?;
    Ok(Table {
        engine,
        judge: answer(|e, a| !attacking_names(e, a).contains(&"Big Brute".to_owned())),
    })
}

fn lethal_through_the_air() -> Result<Table, SuiteError> {
    let engine = attack_table(
        vec![
            body("Drake A", 2, 2, FLYING),
            body("Drake B", 2, 2, FLYING),
            body("Drake C", 2, 2, FLYING),
        ],
        vec![
            body("Ground A", 3, 3, NONE),
            body("Ground B", 3, 3, NONE),
            body("Ground C", 3, 3, NONE),
        ],
        20,
        6,
    )?;
    Ok(Table {
        engine,
        judge: answer(|e, a| attacking_names(e, a).len() == 3),
    })
}

fn keep_blockers_against_the_crack_back() -> Result<Table, SuiteError> {
    let engine = attack_table(
        vec![
            body("Our A", 3, 3, NONE),
            body("Our B", 3, 3, NONE),
            body("Our C", 3, 3, NONE),
        ],
        vec![
            body("Their A", 3, 3, NONE),
            body("Their B", 3, 3, NONE),
            body("Their C", 3, 3, NONE),
        ],
        5,
        20,
    )?;
    Ok(Table {
        engine,
        judge: answer(|e, a| attacking_names(e, a).len() <= 1),
    })
}

fn force_chumps_when_far_ahead() -> Result<Table, SuiteError> {
    let engine = attack_table(
        vec![body("Giant A", 4, 4, NONE), body("Giant B", 4, 4, NONE)],
        vec![body("Pup A", 2, 2, NONE), body("Pup B", 2, 2, NONE)],
        20,
        8,
    )?;
    Ok(Table {
        engine,
        judge: answer(|e, a| attacking_names(e, a).len() == 2),
    })
}

fn trample_through_for_lethal() -> Result<Table, SuiteError> {
    let engine = attack_table(
        vec![
            body("Stomper", 6, 6, &[Keyword::Trample]),
            body("Sidekick", 2, 2, NONE),
        ],
        vec![body("Lone Blocker", 2, 2, NONE)],
        20,
        5,
    )?;
    Ok(Table {
        engine,
        judge: answer(|e, a| attacking_names(e, a).len() == 2),
    })
}

fn a_free_swing() -> Result<Table, SuiteError> {
    let engine = attack_table(
        vec![body("Lone Knight", 3, 3, NONE)],
        lands(mountain, 2),
        20,
        20,
    )?;
    Ok(Table {
        engine,
        judge: answer(|e, a| attacking_names(e, a).len() == 1),
    })
}

fn swing_at_the_open_player() -> Result<Table, SuiteError> {
    let engine = Scenario::new(3)
        .at(5, 0, Step::DeclareAttackers)
        .battlefield(
            0,
            vec![body("Our A", 3, 3, NONE), body("Our B", 3, 3, NONE)],
        )
        .battlefield(1, vec![body("Assassin", 1, 1, DEATHTOUCH)])
        .battlefield(2, lands(mountain, 3))
        .build()?;
    Ok(Table {
        engine,
        judge: answer(|_, a| {
            let attack = attackers(a);
            !attack.is_empty()
                && attack
                    .iter()
                    .all(|(_, target)| *target == AttackTarget::Player(PlayerId::from_index(2)))
        }),
    })
}

fn pressure_the_leader() -> Result<Table, SuiteError> {
    let engine = Scenario::new(4)
        .at(6, 0, Step::DeclareAttackers)
        .battlefield(
            0,
            vec![
                body("Drake A", 2, 2, FLYING),
                body("Drake B", 2, 2, FLYING),
                body("Drake C", 2, 2, FLYING),
                body("Home Guard", 0, 8, &[Keyword::Defender]),
            ],
        )
        .battlefield(
            1,
            vec![
                body("Leader A", 3, 3, NONE),
                body("Leader B", 3, 3, NONE),
                body("Leader C", 4, 4, NONE),
                body("Leader D", 4, 4, NONE),
            ],
        )
        .life(1, 30)
        .hand(1, filler(5))
        .battlefield(2, vec![body("Middle A", 2, 2, NONE)])
        .life(2, 14)
        .battlefield(3, vec![body("Back A", 2, 2, NONE)])
        .life(3, 16)
        .build()?;
    Ok(Table {
        engine,
        judge: answer(|_, a| {
            let attack = attackers(a);
            let at = |seat: usize| {
                attack
                    .iter()
                    .filter(|(_, t)| *t == AttackTarget::Player(PlayerId::from_index(seat)))
                    .count()
            };
            at(1) >= 2 && at(1) > at(2) && at(1) > at(3)
        }),
    })
}

fn menace_gets_past_a_single_blocker() -> Result<Table, SuiteError> {
    let engine = attack_table(
        vec![body("Menacing Brute", 3, 3, &[Keyword::Menace])],
        vec![body("Lone Pup", 2, 2, NONE)],
        20,
        20,
    )?;
    Ok(Table {
        engine,
        judge: answer(|e, a| attacking_names(e, a).len() == 1),
    })
}

fn no_attack_into_a_bigger_first_striker() -> Result<Table, SuiteError> {
    let engine = attack_table(
        vec![body("Our Bear", 3, 3, NONE)],
        vec![body("Fencer", 3, 3, &[Keyword::FirstStrike])],
        20,
        20,
    )?;
    Ok(Table {
        engine,
        judge: answer(|e, a| attacking_names(e, a).is_empty()),
    })
}

// ---- Block ----

/// Seat 1 attacks seat 0 (at `life`) on turn 6 with `attacking` (by name, of `theirs`); seat 0 has `mine`.
fn block_table(
    mine: Vec<PresetPermanent>,
    theirs: Vec<PresetPermanent>,
    attacking: &[&str],
    life: i32,
) -> Result<Driver, SuiteError> {
    let scenario = Scenario::new(2)
        .at(6, 1, Step::DeclareAttackers)
        .battlefield(0, mine)
        .battlefield(1, theirs)
        .life(0, life);
    let mut driver = Driver::from_scenario(scenario)?;
    let attacks: Vec<(&str, usize)> = attacking.iter().map(|name| (*name, 0)).collect();
    driver.attack(&attacks)?;
    to_seat0_blocks(&mut driver)?;
    Ok(driver)
}

/// Passes priority until seat 0 declares blockers.
fn to_seat0_blocks(driver: &mut Driver) -> Result<(), SuiteError> {
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        if decision.player.index() == 0
            && matches!(
                decision.kind,
                mtg_engine::decision::DecisionKind::DeclareBlockers(_)
            )
        {
            return Ok(());
        }
        driver.answer(Driver::passive_answer(&decision))?;
    }
    Err(SuiteError::NoDecision("blocks"))
}

fn chump_to_survive() -> Result<Table, SuiteError> {
    let driver = block_table(
        vec![body("Chump", 1, 1, NONE)],
        vec![body("Crusher", 5, 5, NONE)],
        &["Crusher"],
        4,
    )?;
    table(&driver, answer(|e, a| blocks(e, a).len() == 1))
}

fn no_chump_at_high_life() -> Result<Table, SuiteError> {
    let driver = block_table(
        vec![body("Chump", 1, 1, NONE)],
        vec![body("Crusher", 3, 3, NONE)],
        &["Crusher"],
        20,
    )?;
    table(&driver, answer(|e, a| blocks(e, a).is_empty()))
}

fn take_the_good_trade() -> Result<Table, SuiteError> {
    let driver = block_table(
        vec![body("Our Ogre", 3, 3, NONE)],
        vec![body("Their Bear", 2, 2, NONE)],
        &["Their Bear"],
        20,
    )?;
    table(&driver, answer(|e, a| blocks(e, a).len() == 1))
}

fn keep_the_prize_away_from_deathtouch() -> Result<Table, SuiteError> {
    let driver = block_table(
        vec![body("Prize Beast", 5, 5, NONE)],
        vec![body("Assassin", 1, 1, DEATHTOUCH)],
        &["Assassin"],
        20,
    )?;
    table(&driver, answer(|e, a| blocks(e, a).is_empty()))
}

fn double_block_to_kill() -> Result<Table, SuiteError> {
    let driver = block_table(
        vec![body("Twin A", 3, 3, NONE), body("Twin B", 3, 3, NONE)],
        vec![body("Lone Giant", 4, 4, NONE)],
        &["Lone Giant"],
        20,
    )?;
    table(&driver, answer(|e, a| blocks(e, a).len() == 2))
}

fn chump_the_biggest_attacker() -> Result<Table, SuiteError> {
    let driver = block_table(
        vec![body("Chump", 1, 1, NONE)],
        vec![body("Crusher", 5, 5, NONE), body("Pup", 2, 2, NONE)],
        &["Crusher", "Pup"],
        6,
    )?;
    table(
        &driver,
        answer(|e, a| blocks(e, a) == [("Chump".to_owned(), "Crusher".to_owned())]),
    )
}

fn first_strike_blocks_and_survives() -> Result<Table, SuiteError> {
    let driver = block_table(
        vec![body("Fencer", 2, 2, &[Keyword::FirstStrike])],
        vec![body("Glass Cannon", 3, 2, NONE)],
        &["Glass Cannon"],
        20,
    )?;
    table(&driver, answer(|e, a| blocks(e, a).len() == 1))
}

fn no_block_into_first_strike() -> Result<Table, SuiteError> {
    let driver = block_table(
        vec![body("Our Pup", 2, 1, NONE)],
        vec![body("Fencer", 2, 2, &[Keyword::FirstStrike])],
        &["Fencer"],
        20,
    )?;
    table(&driver, answer(|e, a| blocks(e, a).is_empty()))
}

fn block_enough_to_live() -> Result<Table, SuiteError> {
    let driver = block_table(
        vec![body("Chump A", 1, 1, NONE), body("Chump B", 1, 1, NONE)],
        vec![body("Raider A", 2, 2, NONE), body("Raider B", 2, 2, NONE)],
        &["Raider A", "Raider B"],
        3,
    )?;
    table(&driver, answer(|e, a| !blocks(e, a).is_empty()))
}

// ---- Wipe ----

/// Seat 0 in its main phase with Wrath of God over four Plains, `mine` against `theirs`.
fn wrath_table(
    mine: Vec<PresetPermanent>,
    theirs: Vec<PresetPermanent>,
) -> Result<Driver, SuiteError> {
    let mut own = lands(plains, 4);
    own.extend(mine);
    Ok(Driver::from_scenario(own_main(
        vec![real("Wrath of God")?],
        own,
        theirs,
        4,
    ))?)
}

fn wrath_when_behind() -> Result<Table, SuiteError> {
    let driver = wrath_table(
        vec![],
        vec![
            body("Their A", 3, 3, NONE),
            body("Their B", 3, 3, NONE),
            body("Their C", 4, 4, FLYING),
        ],
    )?;
    table(
        &driver,
        outcome(Until::StepChanges, |e| !on_battlefield(e, "Their C")),
    )
}

fn no_wrath_when_ahead() -> Result<Table, SuiteError> {
    let driver = wrath_table(
        vec![
            body("Our A", 3, 3, NONE),
            body("Our B", 3, 3, NONE),
            body("Our C", 4, 4, FLYING),
        ],
        vec![body("Their A", 2, 2, NONE)],
    )?;
    table(
        &driver,
        outcome(Until::TurnEnds, |e| in_hand(e, "Wrath of God")),
    )
}

fn save_the_wrath_for_more_than_one_bear() -> Result<Table, SuiteError> {
    let driver = wrath_table(vec![], vec![body("Their Bear", 2, 2, NONE)])?;
    table(
        &driver,
        outcome(Until::TurnEnds, |e| in_hand(e, "Wrath of God")),
    )
}

fn wrath_when_the_table_is_ahead() -> Result<Table, SuiteError> {
    let mut own = lands(plains, 4);
    own.push(body("Our Pup", 2, 2, NONE));
    let scenario = Scenario::new(4)
        .at(6, 0, Step::Main1)
        .hand(0, [real("Wrath of God")?])
        .battlefield(0, own)
        .battlefield(
            1,
            vec![body("One A", 3, 3, NONE), body("One B", 3, 3, NONE)],
        )
        .battlefield(
            2,
            vec![body("Two A", 3, 3, NONE), body("Two B", 3, 3, NONE)],
        )
        .battlefield(
            3,
            vec![body("Three A", 3, 3, NONE), body("Three B", 3, 3, NONE)],
        );
    let driver = Driver::from_scenario(scenario)?;
    table(
        &driver,
        outcome(Until::StepChanges, |e| !on_battlefield(e, "Two A")),
    )
}

fn wrath_a_bigger_board() -> Result<Table, SuiteError> {
    let driver = wrath_table(
        vec![body("Our Pup", 2, 2, NONE)],
        vec![
            body("Their A", 3, 3, NONE),
            body("Their B", 3, 3, NONE),
            body("Their Drake", 5, 5, FLYING),
        ],
    )?;
    table(
        &driver,
        outcome(Until::StepChanges, |e| !on_battlefield(e, "Their Drake")),
    )
}

// ---- Develop ----

fn ramp_comes_first() -> Result<Table, SuiteError> {
    let scenario = own_main(
        vec![real("Llanowar Elves")?, real("Grizzly Bears")?],
        lands(forest, 2),
        vec![],
        4,
    );
    let driver = Driver::from_scenario(scenario)?;
    table(&driver, answer(|e, a| casts(e, a, "Llanowar Elves")))
}

fn the_land_comes_first() -> Result<Table, SuiteError> {
    let scenario = own_main(
        vec![forest(), real("Grizzly Bears")?],
        lands(forest, 1),
        vec![],
        4,
    );
    let driver = Driver::from_scenario(scenario)?;
    table(
        &driver,
        answer(|_, a| matches!(a, Answer::Priority(PriorityAction::PlayLand(_)))),
    )
}

fn spend_all_the_mana() -> Result<Table, SuiteError> {
    let centaur = beast("Centaur Courier", 2, Color::Green, 3, 3, NONE);
    let scenario = own_main(
        vec![real("Grizzly Bears")?, centaur],
        lands(forest, 5),
        vec![],
        4,
    );
    let driver = Driver::from_scenario(scenario)?;
    table(
        &driver,
        outcome(Until::TurnEnds, |e| {
            on_battlefield(e, "Grizzly Bears") && on_battlefield(e, "Centaur Courier")
        }),
    )
}

fn the_bigger_spell_when_one_fits() -> Result<Table, SuiteError> {
    let ogre = beast("Hulking Ogre", 3, Color::Green, 4, 4, NONE);
    let scenario = own_main(
        vec![real("Grizzly Bears")?, ogre],
        lands(forest, 4),
        vec![],
        4,
    );
    let driver = Driver::from_scenario(scenario)?;
    table(
        &driver,
        outcome(Until::TurnEnds, |e| on_battlefield(e, "Hulking Ogre")),
    )
}

fn sol_ring_first() -> Result<Table, SuiteError> {
    let scenario = own_main(
        vec![real("Sol Ring")?, real("Grizzly Bears")?],
        lands(forest, 2),
        vec![],
        4,
    );
    let driver = Driver::from_scenario(scenario)?;
    table(
        &driver,
        outcome(Until::TurnEnds, |e| {
            on_battlefield(e, "Sol Ring") && on_battlefield(e, "Grizzly Bears")
        }),
    )
}

// ---- Trick ----

/// Seat 0 attacked with `ours`, seat 1 (at `life`) blocked with `blocker` if any; seat 0 holds `trick` over a Forest.
fn trick_table(
    trick: &str,
    ours: (&str, i32, i32),
    blocker: Option<(&str, i32, i32)>,
    life: i32,
) -> Result<Driver, SuiteError> {
    let mut theirs = Vec::new();
    if let Some((name, p, t)) = blocker {
        theirs.push(body(name, p, t, NONE));
    }
    let scenario = Scenario::new(2)
        .at(5, 0, Step::DeclareAttackers)
        .hand(0, [real(trick)?])
        .battlefield(0, vec![ready(forest()), body(ours.0, ours.1, ours.2, NONE)])
        .battlefield(1, theirs)
        .hand(1, filler(3))
        .life(1, life);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.attack(&[(ours.0, 1)])?;
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        if let mtg_engine::decision::DecisionKind::DeclareBlockers(_) = decision.kind {
            match blocker {
                Some((name, _, _)) => driver.block(&[(name, ours.0)])?,
                None => driver.answer(Answer::DeclareBlockers(Vec::new()))?,
            };
            break;
        }
        driver.answer(Driver::passive_answer(&decision))?;
    }
    to_seat0(&mut driver)?;
    Ok(driver)
}

fn pump_to_win_the_fight() -> Result<Table, SuiteError> {
    let driver = trick_table(
        "Giant Growth",
        ("Brave Bear", 2, 2),
        Some(("Their Ogre", 3, 3)),
        20,
    )?;
    table(
        &driver,
        outcome(Until::CombatEnds, |e| {
            on_battlefield(e, "Brave Bear") && !on_battlefield(e, "Their Ogre")
        }),
    )
}

fn keep_the_pump_when_it_wins_nothing() -> Result<Table, SuiteError> {
    let driver = trick_table("Giant Growth", ("Brave Bear", 2, 2), None, 20)?;
    table(
        &driver,
        outcome(Until::CombatEnds, |e| in_hand(e, "Giant Growth")),
    )
}

fn pump_for_lethal() -> Result<Table, SuiteError> {
    let driver = trick_table("Giant Growth", ("Brave Bear", 2, 2), None, 5)?;
    table(&driver, outcome(Until::CombatEnds, |e| lost(e, 1)))
}

fn fog_the_lethal_attack() -> Result<Table, SuiteError> {
    let scenario = Scenario::new(2)
        .at(6, 1, Step::DeclareAttackers)
        .hand(0, [real("Fog")?])
        .battlefield(0, lands(forest, 1))
        .life(0, 4)
        .battlefield(
            1,
            vec![body("Raider A", 3, 3, NONE), body("Raider B", 3, 3, NONE)],
        );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.attack(&[("Raider A", 0), ("Raider B", 0)])?;
    to_seat0(&mut driver)?;
    table(&driver, outcome(Until::CombatEnds, |e| !lost(e, 0)))
}

fn keep_the_fog_for_a_real_attack() -> Result<Table, SuiteError> {
    let scenario = Scenario::new(2)
        .at(6, 1, Step::DeclareAttackers)
        .hand(0, [real("Fog")?])
        .battlefield(0, lands(forest, 1))
        .battlefield(1, vec![body("Raider A", 2, 2, NONE)]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.attack(&[("Raider A", 0)])?;
    to_seat0(&mut driver)?;
    table(&driver, outcome(Until::CombatEnds, |e| in_hand(e, "Fog")))
}

/// Every case of the suite.
pub fn cases() -> Vec<Case> {
    macro_rules! case {
        ($theme:ident, $build:ident) => {
            Case {
                name: stringify!($build),
                theme: Theme::$theme,
                build: $build,
            }
        };
    }
    vec![
        case!(Counter, counter_the_bomb),
        case!(Counter, let_the_mana_dork_resolve),
        case!(Counter, let_the_bear_resolve),
        case!(Counter, counter_the_card_engine),
        case!(Counter, counter_the_wrath_that_hits_us),
        case!(Counter, let_the_wrath_that_hits_them_resolve),
        case!(Counter, counter_removal_on_the_key_creature),
        case!(Counter, let_removal_on_a_small_creature_resolve),
        case!(Counter, counter_lethal_burn),
        case!(Counter, let_burn_at_high_life_resolve),
        case!(Counter, counter_the_combo_piece),
        case!(Counter, counter_the_counter_on_our_bomb),
        case!(Counter, let_a_teammates_spell_resolve),
        case!(Counter, counter_the_leaders_bomb),
        case!(Counter, let_the_weakest_player_develop),
        case!(Hold, hold_the_counter_over_a_weak_creature),
        case!(Hold, tap_out_for_a_bomb_into_an_empty_hand),
        case!(Hold, hold_removal_for_a_real_threat),
        case!(Hold, flash_in_at_the_end_step),
        case!(Hold, hold_the_flash_creature_for_their_end_step),
        case!(Removal, kill_the_flyer_not_the_walled_ground_creature),
        case!(Removal, bolt_the_lifelinker_not_the_chaff),
        case!(Removal, bolt_the_face_for_lethal),
        case!(Removal, kill_the_card_engine_not_the_vanilla),
        case!(Removal, kill_the_commander_not_the_vanilla),
        case!(Removal, break_the_combo),
        case!(Removal, pacify_the_attacker_not_the_wall),
        case!(Removal, exile_the_indestructible),
        case!(Removal, remove_the_leaders_threat),
        case!(Removal, kill_the_attacker_that_would_be_lethal),
        case!(Protect, protect_the_key_creature_from_removal),
        case!(Protect, shield_the_flyer_from_doom_blade),
        case!(Attack, no_attack_into_untapped_deathtouch),
        case!(Attack, lethal_through_the_air),
        case!(Attack, keep_blockers_against_the_crack_back),
        case!(Attack, force_chumps_when_far_ahead),
        case!(Attack, trample_through_for_lethal),
        case!(Attack, a_free_swing),
        case!(Attack, swing_at_the_open_player),
        case!(Attack, pressure_the_leader),
        case!(Attack, menace_gets_past_a_single_blocker),
        case!(Attack, no_attack_into_a_bigger_first_striker),
        case!(Block, chump_to_survive),
        case!(Block, no_chump_at_high_life),
        case!(Block, take_the_good_trade),
        case!(Block, keep_the_prize_away_from_deathtouch),
        case!(Block, double_block_to_kill),
        case!(Block, chump_the_biggest_attacker),
        case!(Block, first_strike_blocks_and_survives),
        case!(Block, no_block_into_first_strike),
        case!(Block, block_enough_to_live),
        case!(Wipe, wrath_when_behind),
        case!(Wipe, no_wrath_when_ahead),
        case!(Wipe, save_the_wrath_for_more_than_one_bear),
        case!(Wipe, wrath_when_the_table_is_ahead),
        case!(Wipe, wrath_a_bigger_board),
        case!(Develop, ramp_comes_first),
        case!(Develop, the_land_comes_first),
        case!(Develop, spend_all_the_mana),
        case!(Develop, the_bigger_spell_when_one_fits),
        case!(Develop, sol_ring_first),
        case!(Trick, pump_to_win_the_fight),
        case!(Trick, keep_the_pump_when_it_wins_nothing),
        case!(Trick, pump_for_lethal),
        case!(Trick, fog_the_lethal_attack),
        case!(Trick, keep_the_fog_for_a_real_attack),
    ]
}
