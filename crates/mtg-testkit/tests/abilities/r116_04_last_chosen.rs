//! Goal: a permanent that chooses its value again keeps the last one chosen (`Effect::R111KeepChosen` over the permanent's choice, "the last chosen color") at tables of two to four seats: Chromatic Armor chooses a color as it enters and prevents the damage sources of that color would deal to the enchanted creature; its {X} ability, X the number of sleight counters on it, puts another sleight counter and chooses a new color, after which the old color's damage is dealt again and the new one's is prevented; the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_core::color::Color;
use mtg_engine::choose::{ChoiceOption, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, island, mountain, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn table(seats: usize) -> Scenario {
    let mut mine = vec![ready(bear())];
    mine.extend((0..3).map(|_| ready(plains())));
    mine.extend((0..2).map(|_| ready(island())));
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, mine)
        .hand(0, [card("Chromatic Armor").unwrap()]);
    for seat in 1..seats {
        scenario = scenario
            .battlefield(seat, [ready(mountain()), ready(mountain())])
            .hand(
                seat,
                [
                    card("Lightning Bolt").unwrap(),
                    card("Lightning Bolt").unwrap(),
                ],
            );
    }
    scenario
}

/// Answers until the stack is empty: a color is `color`, targets are the Bear, mana is paid automatically.
fn settle(driver: &mut Driver, color: Color) -> Result<(), ScriptError> {
    for _ in 0..80 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty()
                    && driver.state().pending_triggers().is_empty() =>
            {
                return Ok(());
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            DecisionKind::ChooseOption(options) => {
                let at = options
                    .options
                    .iter()
                    .position(|option| *option == ChoiceOption::Color(color))
                    .unwrap();
                driver.answer(Answer::ChooseOption(OptionChoice(at)))?;
            }
            DecisionKind::ChooseTargets(_) => {
                let bear = driver.target_named("Grizzly Bears")?;
                driver.choose_targets(&[bear])?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    panic!("the stack did not empty");
}

/// Seat 1 bolts the Bear at once on its own priority; whether the Bear survives.
fn bolt(driver: &mut Driver) -> Result<bool, ScriptError> {
    while driver.decision()?.player != mtg_core::ids::PlayerId::from_index(1) {
        driver.pass()?;
    }
    let bear = driver.target_named("Grizzly Bears")?;
    driver.cast_at("Lightning Bolt", &[bear])?;
    settle(driver, Color::Red)?;
    Ok(driver.id_of("Grizzly Bears").is_ok())
}

#[rstest]
fn the_last_chosen_color_is_prevented(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let scenario = table(seats);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    driver.begin_cast("Chromatic Armor")?;
    settle(&mut driver, Color::Red)?;
    let armor = driver.id_of("Chromatic Armor")?;
    let sleight = CounterKind::Other("sleight".to_owned());
    let counters = |driver: &Driver| {
        driver
            .state()
            .object(armor)
            .unwrap()
            .permanent()
            .unwrap()
            .counters_of(&sleight)
    };
    assert_eq!(counters(&driver), 1);
    assert!(bolt(&mut driver)?, "red damage is prevented");
    while driver.decision()?.player != mtg_core::ids::PlayerId::from_index(0) {
        driver.pass()?;
    }
    driver.begin_activation("Chromatic Armor")?;
    settle(&mut driver, Color::Blue)?;
    assert_eq!(counters(&driver), 2, "X = 1 paid, a second sleight counter");
    let lands_tapped = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            let object = driver.state().object(**id).unwrap();
            object.controller() == mtg_core::ids::PlayerId::from_index(0)
                && object
                    .permanent()
                    .is_some_and(mtg_engine::object::PermanentState::tapped)
        })
        .count();
    assert_eq!(lands_tapped, 4, "three for the Aura and X = 1");
    assert!(
        !bolt(&mut driver)?,
        "red is no longer the last chosen color"
    );
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}
