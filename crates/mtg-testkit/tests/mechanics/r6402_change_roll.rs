//! Goal: a permanent's rule that changes a die's result after its controller rolls (CR 706.2) plays at 2 to 4 seats: Night Shift of the Living Dead asks after the roll, its life is paid, the changed number is the result its own "whenever you roll a 6" sees while the natural result is logged beside it, and it is offered only once each turn; Xenosquirrels pays a +1/+1 counter for each change, any number of times a turn; declining changes nothing; and the game replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::choose::{ChoiceOption, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_engine::{Event, replay};
use mtg_testkit::cards::cost;
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

/// An instant for nothing that rolls a d6 and does nothing else.
fn roller(name: &str) -> Arc<CardDefinition> {
    let rules: Ability =
        serde_json::from_str(r#"{"Spell":{"effects":[{"RollDice":{"sides":6,"results":[]}}]}}"#)
            .unwrap();
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[]))
            .with_rules(rules),
    )
}

fn table(seats: usize, seed: u64, permanent: &str) -> Scenario {
    Scenario::new(seats)
        .seed(seed)
        .hand(
            0,
            vec![roller("Roll A"), roller("Roll B"), roller("Roll C")],
        )
        .battlefield(
            0,
            [mtg_testkit::scenario::with_counters(
                mtg_engine::PresetPermanent::new(mtg_testkit::corpus::card(permanent).unwrap()),
                CounterKind::PlusOnePlusOne,
                if permanent == "Xenosquirrels" { 3 } else { 0 },
            )],
        )
}

/// What happened when the change was offered: the labels offered, and the answer given.
#[derive(Debug, Default)]
struct Offers {
    asked: Vec<Vec<String>>,
}

/// Passes priority until the stack is empty; a change of a roll is answered with the option `pick` makes of the labels.
fn settle(
    driver: &mut Driver,
    offers: &mut Offers,
    pick: &dyn Fn(&[String]) -> usize,
) -> Result<(), ScriptError> {
    for _ in 0..200 {
        let Some(decision) = driver.engine().decision().cloned() else {
            return Ok(());
        };
        match decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty() && driver.state().resolving().is_none() =>
            {
                return Ok(());
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            DecisionKind::ChooseOption(options) if options.text.starts_with("After you roll") => {
                let labels: Vec<String> = options
                    .options
                    .iter()
                    .map(|option| match option {
                        ChoiceOption::Label(label) => label.clone(),
                        other => panic!("{other:?}"),
                    })
                    .collect();
                let at = pick(&labels);
                offers.asked.push(labels);
                driver.answer(Answer::ChooseOption(OptionChoice(at)))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// The rolls logged since `from`: (result, natural).
fn rolls(driver: &Driver, from: usize) -> Vec<(u32, Option<u32>)> {
    driver.engine().events()[from..]
        .iter()
        .filter_map(|event| match event {
            Event::DieRolled {
                result, natural, ..
            } => Some((*result, *natural)),
            _ => None,
        })
        .collect()
}

fn zombies(driver: &Driver) -> usize {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| driver.state().object(**id).unwrap().card().name == "Zombie Employee")
        .count()
}

fn life(driver: &Driver) -> i32 {
    driver.state().player(crate::common::pid(0)).unwrap().life()
}

#[rstest]
fn night_shift_changes_one_roll_each_turn_for_a_life(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 3, 11)] seed: u64,
) -> Result<(), ScriptError> {
    let name = "Night Shift of the Living Dead";
    let mut driver = Driver::from_scenario(table(seats, seed, name))?;
    let mut offers = Offers::default();
    let from = driver.engine().events().len();
    driver.cast("Roll A")?;
    // Up when the die shows 5 (a 6 for the token), else down.
    settle(&mut driver, &mut offers, &|labels: &[String]| {
        if labels[1].contains("the 5 becomes 6") {
            1
        } else {
            labels.len() - 1
        }
    })?;
    let [(result, natural)] = rolls(&driver, from)[..] else {
        panic!("one roll");
    };
    let shown = natural.expect("the result was changed");
    assert_eq!(offers.asked.len(), 1);
    assert_eq!(offers.asked[0][0], "Keep the result");
    assert_eq!(result, if shown == 5 { 6 } else { shown - 1 });
    assert_eq!(life(&driver), 19, "1 life paid");
    assert_eq!(
        zombies(&driver),
        usize::from(result == 6),
        "the trigger sees the changed result"
    );
    // Once each turn: the next roll is not offered.
    let from = driver.engine().events().len();
    driver.cast("Roll B")?;
    settle(&mut driver, &mut offers, &|_: &[String]| 1)?;
    assert_eq!(offers.asked.len(), 1);
    let [(_, None)] = rolls(&driver, from)[..] else {
        panic!("an unchanged roll");
    };
    assert_eq!(life(&driver), 19);
    let setup = table(seats, seed, name).setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}

#[rstest]
fn xenosquirrels_pays_a_counter_for_each_change(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 5)] seed: u64,
) -> Result<(), ScriptError> {
    let name = "Xenosquirrels";
    let mut driver = Driver::from_scenario(table(seats, seed, name))?;
    let mut offers = Offers::default();
    let counters = |driver: &Driver| {
        driver
            .state()
            .zone(Zone::Battlefield)
            .iter()
            .find_map(|id| {
                let object = driver.state().object(*id).unwrap();
                (object.card().name == name).then(|| {
                    object
                        .permanent()
                        .unwrap()
                        .counters_of(&CounterKind::PlusOnePlusOne)
                })
            })
            .unwrap()
    };
    assert_eq!(counters(&driver), 3);
    // Declining changes nothing and keeps the counter.
    let from = driver.engine().events().len();
    driver.cast("Roll A")?;
    settle(&mut driver, &mut offers, &|_: &[String]| 0)?;
    assert_eq!(rolls(&driver, from)[0].1, None);
    assert_eq!(counters(&driver), 3);
    // Two changes, a counter each.
    for (spell, left) in [("Roll B", 2), ("Roll C", 1)] {
        let from = driver.engine().events().len();
        driver.cast(spell)?;
        settle(&mut driver, &mut offers, &|_: &[String]| 1)?;
        let [(result, Some(shown))] = rolls(&driver, from)[..] else {
            panic!("a changed roll");
        };
        assert_eq!(result, shown + 1);
        assert_eq!(counters(&driver), left);
    }
    assert_eq!(offers.asked.len(), 3);
    assert_eq!(life(&driver), 20);
    let setup = table(seats, seed, name).setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
