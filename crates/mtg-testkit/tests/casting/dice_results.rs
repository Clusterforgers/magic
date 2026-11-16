//! Goal: a trigger that judges the result of a die ("whenever you roll a 4 or higher", "a 1", "a 6", "equal to the result") triggers once for each die of its controller that qualifies, for one die and for several rolled together, at tables of two and four seats whoever rolls, reads that die's result in its "if you rolled 6 or higher" and "equal to the result", never fires for another player's die, and the log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::names;
use crate::l291_dice::{l291_replays, l291_roll};

/// The card that watches the dice of seat 0: one of the corpus, or a card with these rules (JSON).
#[derive(Debug, Clone, Copy)]
enum Watcher {
    Corpus(&'static str),
    Rules(&'static str, &'static str),
}

impl Watcher {
    fn definition(self) -> Arc<CardDefinition> {
        match self {
            Watcher::Corpus(name) => card(name).unwrap(),
            Watcher::Rules(name, rules) => Arc::new(
                CardDefinition::new(name)
                    .with_type(CardType::Artifact)
                    .with_rules(serde_json::from_str::<Ability>(rules).unwrap()),
            ),
        }
    }
}

/// What the watcher's rules did, counted.
#[derive(Debug, Clone, Copy)]
enum Measure {
    /// The permanents of this name on the battlefield.
    Named(&'static str),
    /// The charge counters on the watcher.
    Charge,
    /// The +1/+1 counters on the watcher.
    PlusOne,
}

/// "{0}: Roll `dice` `sides`-sided dice."
fn roller(sides: u32, dice: u32) -> Arc<CardDefinition> {
    let count = if dice > 1 {
        format!(r#","count":{dice}"#)
    } else {
        String::new()
    };
    let roll: Ability = serde_json::from_str(&format!(
        r#"{{"Activated":{{"cost":"{{0}}","effects":[{{"RollDice":{{"sides":{sides}{count},"results":[]}}}}]}}}}"#
    ))
    .unwrap();
    Arc::new(
        CardDefinition::new("Die Roller")
            .with_type(CardType::Artifact)
            .with_rules(roll),
    )
}

/// Seat 0 controls the watcher and seat `roller` the Die Roller, in the roller's first main phase.
fn table(
    seats: usize,
    seed: u64,
    roller_seat: usize,
    watcher: Watcher,
    sides: u32,
    dice: u32,
) -> Scenario {
    let scenario = Scenario::new(seats)
        .seed(seed)
        .at(2, roller_seat, Step::Main1);
    let mut mine = vec![ready(watcher.definition())];
    if roller_seat == 0 {
        mine.push(ready(roller(sides, dice)));
        scenario.battlefield(0, mine)
    } else {
        scenario
            .battlefield(roller_seat, [ready(roller(sides, dice))])
            .battlefield(0, mine)
    }
}

fn measured(driver: &Driver, watcher: Watcher, measure: Measure) -> usize {
    let counters = |kind: CounterKind| {
        let (Watcher::Corpus(name) | Watcher::Rules(name, _)) = watcher;
        let id = driver.id_of(name).unwrap();
        let held = driver
            .state()
            .object(id)
            .and_then(|object| object.permanent())
            .map_or(0, |permanent| permanent.counters_of(&kind));
        usize::try_from(held).unwrap()
    };
    match measure {
        Measure::Named(name) => names(driver.state(), Zone::Battlefield)
            .iter()
            .filter(|n| *n == name)
            .count(),
        Measure::Charge => counters(CounterKind::Charge),
        Measure::PlusOne => counters(CounterKind::PlusOnePlusOne),
    }
}

/// Rolls `dice` dice 30 times; `expected` of the dice of seat 0 is what the watcher must have done.
fn play(
    seats: usize,
    seed: u64,
    roller_seat: usize,
    (watcher, measure): (Watcher, Measure),
    (sides, dice): (u32, u32),
    expected: fn(&[u32]) -> usize,
) -> Result<Vec<u32>, ScriptError> {
    let scenario = || table(seats, seed, roller_seat, watcher, sides, dice);
    let mut driver = Driver::from_scenario(scenario())?;
    let rolled = l291_roll(&mut driver, 30)?;
    let mine: Vec<u32> = rolled
        .iter()
        .filter(|(seat, _)| *seat == 0)
        .map(|(_, result)| *result)
        .collect();
    assert_eq!(
        rolled.len(),
        30 * usize::try_from(dice).unwrap(),
        "{rolled:?}"
    );
    assert_eq!(mine.len() == rolled.len(), roller_seat == 0);
    assert_eq!(
        measured(&driver, watcher, measure),
        expected(&mine),
        "{rolled:?}"
    );
    l291_replays(&driver, scenario());
    Ok(mine)
}

fn count(dice: &[u32], judge: impl Fn(u32) -> bool) -> usize {
    dice.iter().filter(|die| judge(**die)).count()
}

const HOUSE: Watcher = Watcher::Corpus("Mr. House, President and CEO");
const SQUIRRELS: Watcher = Watcher::Rules(
    "Squirrel Burrow",
    r#"{"Triggered":{"when":{"DiceRolled":{"who":"You","at_least":4,"on_a_die":true}},"effects":[{"CreateToken":{"token":{"Custom":{"name":"Squirrel","colors":["Green"],"types":["Creature"],"subtypes":["Squirrel"],"power":1,"toughness":1}}}}]}}"#,
);
const SUBJECT: Watcher = Watcher::Rules(
    "Test Rig",
    r#"{"Triggered":{"when":{"DiceRolled":{"who":"You","at_least":4,"on_a_die":true}},"effects":[{"PutCounters":{"on":"This","counter":"+1/+1"}}]}}"#,
);
const OLD_HOUSE: Watcher = Watcher::Rules(
    "Old House",
    r#"{"Triggered":{"when":{"DiceRolled":{"who":"You","each":true}},"if":{"Compare":{"left":"Triggering","op":"Ge","right":5}},"effects":[{"PutCounters":{"on":"This","counter":"+1/+1"}}]}}"#,
);
const LUCK: Watcher = Watcher::Rules(
    "Luck Counter",
    r#"{"Triggered":{"when":{"DiceRolled":{"who":"You","each":true}},"effects":[{"PutCounters":{"on":"This","counter":"charge","count":{"DieResult":"Only"}}}]}}"#,
);

/// Each die of seat 0 that meets a threshold, a number or a size triggers once with its own result, below it, at it and
/// above it, one die at a time and three at once.
#[rstest]
#[case::mr_house_robots(HOUSE, Measure::Named("Robot"), |dice: &[u32]| count(dice, |d| d >= 4))]
#[case::mr_house_treasures(HOUSE, Measure::Named("Treasure"), |dice: &[u32]| count(dice, |d| d >= 6))]
#[case::a_four_or_higher_on_a_die(SQUIRRELS, Measure::Named("Squirrel"), |dice: &[u32]| count(dice, |d| d >= 4))]
#[case::a_counter_for_each_such_die(SUBJECT, Measure::PlusOne, |dice: &[u32]| count(dice, |d| d >= 4))]
#[case::the_result_read_in_an_if(OLD_HOUSE, Measure::PlusOne, |dice: &[u32]| count(dice, |d| d >= 5))]
#[case::exactly_a_one(Watcher::Corpus("Complaints Clerk"), Measure::Named("Clown Robot"), |dice: &[u32]| count(dice, |d| d == 1))]
#[case::exactly_a_six(Watcher::Corpus("Night Shift of the Living Dead"), Measure::Named("Zombie Employee"), |dice: &[u32]| count(dice, |d| d == 6))]
fn l291_each_die_that_qualifies_triggers_once(
    #[case] watcher: Watcher,
    #[case] measure: Measure,
    #[case] expected: fn(&[u32]) -> usize,
    #[values(2, 4)] seats: usize,
    #[values(1, 2)] seed: u64,
    #[values(0, 1)] roller_seat: usize,
    #[values(1, 3)] dice: u32,
) -> Result<(), ScriptError> {
    let mine = play(
        seats,
        seed,
        roller_seat,
        (watcher, measure),
        (6, dice),
        expected,
    )?;
    // The sample holds dice that qualify and dice that do not, so the count is no accident.
    if roller_seat == 0 {
        let hit = expected(&mine);
        assert!(0 < hit && hit < mine.len(), "{mine:?}");
    }
    Ok(())
}

fn total(dice: &[u32]) -> usize {
    dice.iter().map(|d| usize::try_from(*d).unwrap()).sum()
}

/// "Equal to the result" reads the die that triggered the ability, which rolls nothing itself: each die of a roll of
/// several for "a die", the die of a single roll for "one or more dice".
#[rstest]
#[case::each_die_is_its_result(LUCK, 1)]
#[case::each_of_three_dice(LUCK, 3)]
#[case::one_or_more_dice(Watcher::Corpus("Vexing Puzzlebox"), 1)]
fn l291_the_result_of_the_die_that_triggered_is_the_amount(
    #[case] watcher: Watcher,
    #[case] dice: u32,
    #[values(2, 4)] seats: usize,
    #[values(1, 2)] seed: u64,
    #[values(0, 1)] roller_seat: usize,
) -> Result<(), ScriptError> {
    let mine = play(
        seats,
        seed,
        roller_seat,
        (watcher, Measure::Charge),
        (6, dice),
        total,
    )?;
    assert_eq!(mine.is_empty(), roller_seat != 0);
    Ok(())
}
