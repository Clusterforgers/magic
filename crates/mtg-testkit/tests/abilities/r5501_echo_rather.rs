//! Goal: Thick-Skinned Goblin's "You may pay {0} rather than pay the echo cost for permanents you control." plays at tables of two, three and four seats: the echo payment of a permanent its controller controls offers {0} beside the echo cost (only {0} when the echo cost can't be paid), paying {0} keeps the permanent (the ruling: it counts as paying echo), paying none sacrifices it, a Goblin an opponent controls changes nothing, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use crate::common;

use common::{name_of, pid};
use mtg_engine::choose::{ChoiceOption, OptionChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, replay};
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::corpus::card;
use mtg_testkit::upkeep_cards::{echoer, mana};
use mtg_testkit::{Driver, Scenario, ScriptError, ready, sick};
use rstest::rstest;

/// What seat 0 does when its Echoer's echo is due.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reply {
    /// Pays {0} through the Goblin's rule.
    Zero,
    /// Pays the echo cost itself.
    Echo,
    /// Pays none of them.
    Neither,
}

/// Seat 0 holds an Echoer with echo {1} that came under its control this turn and `lands` Forests; the Goblin is
/// on the battlefield under the control of seat `goblin_seat`, and every other seat holds a creature.
fn table(seats: usize, lands: usize, goblin_seat: usize) -> Scenario {
    let goblin = card("Thick-Skinned Goblin").unwrap();
    let mut mine = vec![sick(echoer("Echoer", mana("{1}")))];
    mine.extend(vec![ready(forest()); lands]);
    let mut table = Scenario::new(seats).at(1, 0, Step::Main1);
    for seat in 0..seats {
        let mut field = if seat == 0 {
            mine.clone()
        } else {
            vec![ready(free_creature("Bystander", 1, 1))]
        };
        if seat == goblin_seat {
            field.push(ready(goblin.clone()));
        }
        table = table.battlefield(seat, field);
    }
    table
}

/// Passes until a question that is no priority decision waits, or the stack is empty.
fn until_question(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..60 {
        match &driver.decision()?.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(()),
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => return Ok(()),
        }
    }
    Err(ScriptError::Stuck("a question".into()))
}

fn in_graveyard(driver: &Driver, name: &str) -> bool {
    driver
        .state()
        .zone(Zone::Graveyard(pid(0)))
        .iter()
        .any(|id| name_of(driver.state(), *id) == name)
}

/// The labels of the pending choice of an option.
fn labels(driver: &Driver) -> Vec<String> {
    match &driver.decision().map(|decision| &decision.kind) {
        Ok(DecisionKind::ChooseOption(options)) => options
            .options
            .iter()
            .map(|option| match option {
                ChoiceOption::Label(label) => label.clone(),
                other => format!("{other:?}"),
            })
            .collect(),
        other => panic!("a choice of costs was expected, not {other:?}"),
    }
}

#[rstest]
fn the_goblin_offers_zero_beside_the_echo_cost(
    #[values(2, 3, 4)] seats: usize,
    #[values(
        (2, Reply::Zero),
        (2, Reply::Echo),
        (2, Reply::Neither),
        (0, Reply::Zero),
        (0, Reply::Neither)
    )]
    case: (usize, Reply),
) -> Result<(), ScriptError> {
    let (lands, reply) = case;
    let setup = table(seats, lands, 0).setup()?;
    let mut driver = Driver::from_scenario(table(seats, lands, 0))?;
    driver.idle_until(1 + u32::try_from(seats).unwrap(), Step::Upkeep)?;
    until_question(&mut driver)?;
    let offered = labels(&driver);
    let expected: Vec<&str> = if lands > 0 {
        vec!["Pay {1}", "Pay {0}", "Pay none of them"]
    } else {
        vec!["Pay {0}", "Pay none of them"]
    };
    assert_eq!(offered, expected);
    let pick = match reply {
        Reply::Zero => offered.iter().position(|label| label == "Pay {0}"),
        Reply::Echo => offered.iter().position(|label| label == "Pay {1}"),
        Reply::Neither => Some(offered.len() - 1),
    }
    .unwrap();
    driver.answer(Answer::ChooseOption(OptionChoice(pick)))?;
    while let DecisionKind::PayMana(_) = driver.decision()?.kind {
        driver.answer(Answer::PayMana(PayChoice::Auto))?;
    }
    until_question(&mut driver)?;
    let kept = reply != Reply::Neither;
    assert_eq!(driver.id_of("Echoer").is_ok(), kept);
    assert_eq!(in_graveyard(&driver, "Echoer"), !kept);
    let log = driver.engine().events().to_vec();
    assert_eq!(
        replay(&setup, &log).unwrap().state_hash(),
        driver.state().state_hash()
    );
    let resumed = Engine::resume(setup, log).unwrap();
    assert_eq!(resumed.state_hash(), driver.state().state_hash());
    Ok(())
}

#[rstest]
fn a_goblin_an_opponent_controls_changes_nothing(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, 2, seats - 1))?;
    driver.idle_until(1 + u32::try_from(seats).unwrap(), Step::Upkeep)?;
    until_question(&mut driver)?;
    let decision = driver.decision()?;
    assert_eq!(decision.player, pid(0));
    assert!(
        matches!(decision.kind, DecisionKind::YesNo(_)),
        "the plain echo question: {:?}",
        decision.kind.name()
    );
    driver.answer(Answer::YesNo(YesNoChoice(false)))?;
    until_question(&mut driver)?;
    assert!(in_graveyard(&driver, "Echoer"));
    Ok(())
}
