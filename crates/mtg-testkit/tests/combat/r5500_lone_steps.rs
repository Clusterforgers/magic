//! Goal: steps added on their own after a phase play as printed at tables of two, three and four seats: when Obeka,
//! Splitter of Seconds deals N combat damage to a player, N upkeep steps follow the combat phase, each alone (no draw
//! step follows it), then the postcombat main phase; upkeep triggers trigger in each (As Foretold gets a time counter
//! each time); the log replays to the state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_engine::events::Event;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::corpus::card;
use mtg_testkit::scenario::with_counters;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const OBEKA: &str = "Obeka, Splitter of Seconds";

fn r5500_time() -> CounterKind {
    CounterKind::Other("time".to_owned())
}

/// Obeka (a 2/5) with `boost` +1/+1 counters attacks seat 1 alone and nothing blocks; the steps begun from the combat
/// damage step on are `upkeeps` upkeep steps after the end of combat, then the postcombat main phase.
#[rstest]
#[case::two_damage(0, 2)]
#[case::three_damage(1, 3)]
fn r5500_obeka_adds_that_many_upkeep_steps_after_combat(
    #[values(2, 3, 4)] seats: usize,
    #[case] boost: u32,
    #[case] upkeeps: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats).at(2, 0, Step::Main1).battlefield(
        0,
        [
            with_counters(
                ready(card(OBEKA).unwrap()),
                CounterKind::PlusOnePlusOne,
                boost,
            ),
            ready(card("As Foretold").unwrap()),
        ],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::DeclareAttackers)?;
    let from = driver.engine().events().len();
    driver.attack(&[(OBEKA, 1)])?;
    for _ in 0..200 {
        if driver.state().turn().step() == Step::Main2 && driver.state().stack().is_empty() {
            break;
        }
        driver.answer_passively()?;
    }
    assert_eq!(driver.state().turn().step(), Step::Main2);
    let begun: Vec<Step> = driver.engine().events()[from..]
        .iter()
        .filter_map(|event| match event {
            Event::StepBegan { step } => Some(*step),
            _ => None,
        })
        .skip_while(|step| *step != Step::EndCombat)
        .collect();
    let mut wanted = vec![Step::EndCombat];
    wanted.extend(std::iter::repeat_n(Step::Upkeep, upkeeps));
    wanted.push(Step::Main2);
    assert_eq!(begun, wanted);
    let foretold = driver.id_of("As Foretold")?;
    let counters = driver
        .state()
        .object(foretold)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&r5500_time());
    assert_eq!(
        counters as usize, upkeeps,
        "one time counter for each upkeep"
    );
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    Ok(())
}
