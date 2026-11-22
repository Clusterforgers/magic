//! Goal: a die rolled as a cost (`CostItem::R114RollDie`) is offered to the rules that change a roll (CR 706.2, 601.2h) at tables of two to four seats: Clay Golem's d8 asks Night Shift of the Living Dead (pay 1 life, one more or one less) and Xenosquirrels (a +1/+1 counter) before the mana is paid, the changed number is the X of Monstrosity X while the natural result is logged beside it, the payment can no longer be backed out of once the roll was seen, declining keeps the natural result, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::pid;
use mtg_cards::value::CounterKind;
use mtg_engine::choose::OptionChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, PresetPermanent, replay};
use mtg_testkit::cards::{bear, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::scenario::with_counters;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn table(seats: usize, seed: u64, changer: &str) -> Scenario {
    let mut mine = vec![
        ready(card("Clay Golem").unwrap()),
        with_counters(
            PresetPermanent::new(card(changer).unwrap()),
            CounterKind::PlusOnePlusOne,
            2,
        ),
    ];
    mine.extend((0..6).map(|_| ready(plains())));
    let mut scenario = Scenario::new(seats)
        .seed(seed)
        .at(1, 0, Step::Main1)
        .battlefield(0, mine);
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(bear())]);
    }
    scenario
}

/// What the activation asked: the change questions answered, and whether the payment could be backed out of.
#[derive(Debug, Default)]
struct Asked {
    changes: usize,
    cancel: Vec<bool>,
}

/// Answers until the stack is empty: a change of the roll with option `pick`, an opponent's Bear as the target, mana
/// paid automatically.
fn settle(driver: &mut Driver, pick: usize) -> Result<Asked, ScriptError> {
    let mut asked = Asked::default();
    for _ in 0..60 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().pending_triggers().is_empty() =>
            {
                return Ok(asked);
            }
            DecisionKind::ChooseOption(options) if options.text.starts_with("After you roll") => {
                asked.changes += 1;
                let at = if asked.changes == 1 { pick } else { 0 };
                driver.answer(Answer::ChooseOption(OptionChoice(at)))?;
            }
            DecisionKind::ChooseTargets(_) => {
                let bear = driver.target_named("Grizzly Bears")?;
                driver.choose_targets(&[bear])?;
            }
            DecisionKind::PayMana(options) => {
                asked.cancel.push(options.can_cancel);
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    panic!("the stack did not empty");
}

#[rstest]
fn a_cost_roll_is_offered_to_the_rules_that_change_it(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 4, 7)] seed: u64,
    #[values("Night Shift of the Living Dead", "Xenosquirrels")] changer: &str,
    #[values(0, 1, 2)] pick: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats, seed, changer);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    let golem = driver.id_of("Clay Golem")?;
    let squirrels = driver.id_of(changer)?;
    let life = driver.state().player(pid(0)).unwrap().life();
    driver.begin_activation("Clay Golem")?;
    let asked = settle(&mut driver, pick)?;
    assert!(asked.changes >= 1, "the roll is offered to {changer}");
    let events = driver.engine().events();
    let rolled: Vec<(u32, Option<u32>)> = events
        .iter()
        .filter_map(|event| match event {
            Event::DieRolled {
                player,
                sides: 8,
                result,
                natural,
            } if *player == pid(0) => Some((*result, *natural)),
            _ => None,
        })
        .collect();
    let [(result, natural)] = rolled.as_slice() else {
        panic!("one d8: {rolled:?}");
    };
    let shown = natural.unwrap_or(*result);
    // The options are keep, then the die up, then down when it is at least 1.
    let expected = match pick {
        1 => shown + 1,
        2 => shown - 1,
        _ => shown,
    };
    assert_eq!(*result, expected, "the changed number is the result");
    assert_eq!(natural.is_some(), pick != 0);
    let state = driver.state();
    let permanent = state.object(golem).unwrap().permanent().unwrap();
    assert_eq!(
        permanent.counters_of(&CounterKind::PlusOnePlusOne),
        *result,
        "X is the changed result"
    );
    let paid_life = life - state.player(pid(0)).unwrap().life();
    let squirrel_counters = state
        .object(squirrels)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::PlusOnePlusOne);
    match (changer, pick) {
        (_, 0) => {
            assert_eq!(paid_life, 0);
            assert_eq!(squirrel_counters, 2);
        }
        ("Xenosquirrels", _) => {
            assert_eq!(paid_life, 0);
            assert_eq!(squirrel_counters, 1, "a counter pays the change");
        }
        _ => assert_eq!(paid_life, 1, "a life pays the change"),
    }
    assert_eq!(
        asked.cancel,
        vec![false],
        "the payment cannot be backed out of once the roll was seen"
    );
    assert!(state.zone(Zone::Battlefield).len() >= 8);
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}
