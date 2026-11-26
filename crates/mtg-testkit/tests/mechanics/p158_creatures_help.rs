//! Goal: an activated ability whose cost lets creatures pay its mana (`CostItem::CreaturesHelpPay`, Heirloom Epic) is
//! offered exactly when lands and untapped creatures (summoning sick ones too) reach the cost between them, never lets
//! an artifact help, never counts the source that the cost taps, taps each named creature once, refuses an artifact
//! named as a helper and more helpers than the cost holds, and is no waterbend; at two to four seats, each log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::decision::{Answer, PayChoice};
use mtg_engine::events::Event;
use mtg_engine::replay;
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::{Driver, Scenario, ScriptError, ready, sick};
use rstest::rstest;

use crate::common::pid;

/// An artifact: "{3}, {T}: You gain 1 life. For each mana in this ability's activation cost, you may tap an untapped
/// creature you control rather than pay that mana."
fn heirloom() -> Arc<CardDefinition> {
    let json = r#"{"Activated":{"cost":[{"CreaturesHelpPay":3},"Tap"],"effects":[{"GainLife":{"amount":1}}]}}"#;
    Arc::new(
        CardDefinition::new("Heirloom")
            .with_type(CardType::Artifact)
            .with_rules(serde_json::from_str::<Ability>(json).unwrap()),
    )
}

fn relic(n: usize) -> Arc<CardDefinition> {
    Arc::new(CardDefinition::new(format!("Relic {n}")).with_type(CardType::Artifact))
}

/// Seat 0 has the Heirloom and, besides it, `lands` Forests, `relics` artifacts, `bears` creatures and `cubs` creatures
/// that came this turn.
fn game(
    seats: usize,
    lands: usize,
    relics: usize,
    bears: usize,
    cubs: usize,
) -> Result<Driver, ScriptError> {
    let mut board = vec![ready(heirloom())];
    board.extend((0..lands).map(|_| ready(forest())));
    board.extend((1..=relics).map(|n| ready(relic(n))));
    board.extend((1..=bears).map(|n| ready(free_creature(&format!("Bear {n}"), 2, 2))));
    board.extend((1..=cubs).map(|n| sick(free_creature(&format!("Cub {n}"), 1, 1))));
    Driver::from_scenario(Scenario::new(seats).battlefield(0, board))
}

fn offered(driver: &Driver) -> bool {
    !driver.offered_abilities("Heirloom").unwrap().is_empty()
}

fn life(driver: &Driver) -> i32 {
    driver.state().player(pid(0)).unwrap().life()
}

fn tapped(driver: &Driver, name: &str) -> bool {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .is_some_and(mtg_engine::object::PermanentState::tapped)
}

fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    while !driver.state().stack().is_empty() {
        driver.answer_passively()?;
    }
    Ok(())
}

fn replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

#[rstest]
#[case::lands_pay_it_all(3, 0, 0, 0, true)]
#[case::creatures_pay_it_all(0, 0, 3, 0, true)]
#[case::sick_creatures_help(0, 0, 1, 2, true)]
#[case::a_mix(1, 0, 1, 1, true)]
#[case::artifacts_do_not_help(0, 3, 0, 0, false)]
#[case::one_short(1, 2, 1, 0, false)]
fn the_cost_is_offered_exactly_when_lands_and_creatures_reach_it(
    #[values(2, 3, 4)] seats: usize,
    #[case] lands: usize,
    #[case] relics: usize,
    #[case] bears: usize,
    #[case] cubs: usize,
    #[case] payable: bool,
) -> Result<(), ScriptError> {
    let driver = game(seats, lands, relics, bears, cubs)?;
    assert_eq!(offered(&driver), payable);
    Ok(())
}

#[rstest]
fn a_manual_payment_taps_the_named_creatures_and_is_no_waterbend(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = game(seats, 1, 1, 2, 0)?;
    driver.begin_activation("Heirloom")?;
    driver.pay_helped_by(&["Forest"], &[], &["Bear 1", "Bear 2"], &[], 0)?;
    settle(&mut driver)?;
    assert_eq!(life(&driver), 21);
    for name in ["Forest", "Bear 1", "Bear 2", "Heirloom"] {
        assert!(tapped(&driver, name), "{name}");
    }
    assert!(!tapped(&driver, "Relic 1"));
    assert!(
        !driver
            .engine()
            .events()
            .iter()
            .any(|event| matches!(event, Event::Bent { .. })),
        "paying it is not waterbending"
    );
    replays(&driver);
    Ok(())
}

#[rstest]
#[case::an_artifact_named(&["Relic 1", "Bear 1", "Bear 2"])]
#[case::more_than_the_cost(&["Bear 1", "Bear 2", "Bear 3", "Bear 4"])]
fn a_wrong_helper_is_refused(
    #[values(2, 3, 4)] seats: usize,
    #[case] helpers: &[&str],
) -> Result<(), ScriptError> {
    let mut driver = game(seats, 0, 1, 4, 0)?;
    driver.begin_activation("Heirloom")?;
    let before = driver.state().state_hash();
    assert!(driver.pay_helped_by(&[], &[], helpers, &[], 0).is_err());
    assert_eq!(driver.state().state_hash(), before);
    Ok(())
}

#[rstest]
fn the_automatic_payment_taps_creatures(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = game(seats, 0, 2, 3, 0)?;
    driver.begin_activation("Heirloom")?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver)?;
    assert_eq!(life(&driver), 21);
    for n in 1..=3 {
        assert!(tapped(&driver, &format!("Bear {n}")));
    }
    assert!(!tapped(&driver, "Relic 1") && !tapped(&driver, "Relic 2"));
    replays(&driver);
    Ok(())
}
