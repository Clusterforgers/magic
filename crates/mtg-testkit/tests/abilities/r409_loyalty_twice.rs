//! Goal: "You may activate the loyalty abilities of planeswalkers you control twice each turn rather than only once."
//! plays at two to four seats: under Oath of Teferi a planeswalker's loyalty abilities are offered a second time in a
//! turn and never a third; Urza Assembles the Titans' chapter III allows it for that turn alone; without either the
//! planeswalker has its one activation; an opponent's Oath gives its controller nothing.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::PresetPermanent;
use mtg_engine::turn::Step;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

use crate::common::pid;

/// A planeswalker with "+1: You gain 1 life." and loyalty 3.
fn r409_walker() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Walker")
            .with_type(CardType::Planeswalker)
            .with_loyalty(3)
            .with_rules(
                serde_json::from_str::<Ability>(
                    r#"{"Loyalty":{"cost":{"Plus":1},"effects":[{"GainLife":{"amount":1}}]}}"#,
                )
                .unwrap(),
            ),
    )
}

fn r409_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..60 {
        if driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
            && driver.state().pending_triggers().is_empty()
        {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Activates the Walker's loyalty ability as many times as it is offered (at most five); how many times that was.
fn r409_activations(driver: &mut Driver) -> Result<u32, ScriptError> {
    let mut times = 0;
    while times < 5 && !driver.offered_abilities("Walker")?.is_empty() {
        driver.begin_activation("Walker")?;
        r409_settle(driver)?;
        times += 1;
    }
    Ok(times)
}

#[rstest]
#[case::alone(&[], &[], 1)]
#[case::under_an_oath(&["Oath of Teferi"], &[], 2)]
#[case::under_an_opponents_oath(&[], &["Oath of Teferi"], 1)]
fn r409_an_oath_allows_a_second_activation(
    #[case] yours: &[&str],
    #[case] theirs: &[&str],
    #[case] times: u32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut mine: Vec<PresetPermanent> = vec![ready(r409_walker())];
    mine.extend(yours.iter().map(|name| ready(card(name).unwrap())));
    let mut scenario = Scenario::new(seats).battlefield(0, mine);
    if !theirs.is_empty() {
        scenario = scenario.battlefield(
            1,
            theirs
                .iter()
                .map(|name| ready(card(name).unwrap()))
                .collect::<Vec<_>>(),
        );
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let life = driver.state().player(pid(0)).unwrap().life();
    assert_eq!(r409_activations(&mut driver)?, times);
    assert_eq!(
        driver.state().player(pid(0)).unwrap().life() - life,
        i32::try_from(times).unwrap()
    );
    let walker = driver.id_of("Walker")?;
    let loyalty = driver
        .state()
        .object(walker)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::Loyalty);
    assert_eq!(loyalty, 3 + times);
    Ok(())
}

/// Chapter III of Urza Assembles the Titans allows the second activation for its turn only.
#[rstest]
fn r409_urza_chapter_three_allows_two_activations_this_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let urza = with_counters(
        ready(card("Urza Assembles the Titans").unwrap()),
        CounterKind::lore(),
        2,
    );
    let scenario = Scenario::new(seats)
        .battlefield(0, [ready(r409_walker()), urza])
        .at(1, 0, Step::Upkeep);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.pass_until(Step::Main1)?;
    r409_settle(&mut driver)?;
    assert_eq!(r409_activations(&mut driver)?, 2);
    // The next turn of seat 0 gives one again.
    let next = u32::try_from(seats).unwrap() + 1;
    driver.idle_until(next, Step::Main1)?;
    r409_settle(&mut driver)?;
    assert_eq!(r409_activations(&mut driver)?, 1);
    Ok(())
}
