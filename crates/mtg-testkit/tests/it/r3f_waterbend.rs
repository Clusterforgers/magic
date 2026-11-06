//! Goal: an ability whose cost is waterbend {N} is offered exactly when lands, untapped artifacts and untapped creatures (summoning sick ones too) can pay N between them, never counts the source of an ability that also taps it, pays with the objects the player names or picked by the engine, taps each of them once and refuses a payment that taps more than the cost holds.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::decision::{Answer, PayChoice};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::{Driver, Scenario, ScriptError, ready, sick};
use rstest::rstest;

use crate::common::pid;

/// A creature: "Waterbend {3}[, {T}]: You gain 1 life."
fn bender(tap: bool) -> Arc<CardDefinition> {
    let cost = if tap {
        r#"[{"Waterbend":3},"Tap"]"#
    } else {
        r#"[{"Waterbend":3}]"#
    };
    let json =
        format!(r#"{{"Activated":{{"cost":{cost},"effects":[{{"GainLife":{{"amount":1}}}}]}}}}"#);
    Arc::new(
        CardDefinition::new("Bender")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(serde_json::from_str::<Ability>(&json).unwrap()),
    )
}

fn relic(n: usize) -> Arc<CardDefinition> {
    Arc::new(CardDefinition::new(format!("Relic {n}")).with_type(CardType::Artifact))
}

/// Seat 0 has the Bender and, besides it, `lands` Forests, `relics` untapped artifacts, `bears` creatures and `sick`
/// creatures that came this turn.
fn game(
    tap: bool,
    lands: usize,
    relics: usize,
    bears: usize,
    sick_bears: usize,
) -> Result<Driver, ScriptError> {
    let mut board = vec![ready(bender(tap))];
    board.extend((0..lands).map(|_| ready(forest())));
    board.extend((1..=relics).map(|n| ready(relic(n))));
    board.extend((1..=bears).map(|n| ready(free_creature(&format!("Bear {n}"), 2, 2))));
    board.extend((1..=sick_bears).map(|n| sick(free_creature(&format!("Cub {n}"), 1, 1))));
    Driver::from_scenario(Scenario::new(2).battlefield(0, board))
}

fn offered(driver: &Driver) -> bool {
    !driver.offered_abilities("Bender").unwrap().is_empty()
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

#[rstest]
#[case::lands_pay_it_all(false, 3, 0, 0, 0, true)]
#[case::artifacts_pay_it_all(false, 0, 3, 0, 0, true)]
#[case::creatures_pay_it_all(false, 0, 0, 3, 0, true)]
#[case::sick_creatures_help(false, 0, 1, 0, 2, true)]
#[case::a_mix(false, 1, 1, 1, 0, true)]
#[case::one_short(false, 0, 1, 0, 0, false)]
#[case::the_source_is_not_a_helper_when_it_taps(true, 0, 2, 0, 0, false)]
#[case::the_source_taps_and_others_help(true, 0, 3, 0, 0, true)]
#[case::the_source_helps_when_it_does_not_tap(false, 0, 2, 0, 0, true)]
fn the_cost_is_offered_exactly_when_lands_and_helpers_reach_it(
    #[case] tap: bool,
    #[case] lands: usize,
    #[case] relics: usize,
    #[case] bears: usize,
    #[case] sick_bears: usize,
    #[case] payable: bool,
) -> Result<(), ScriptError> {
    let driver = game(tap, lands, relics, bears, sick_bears)?;
    assert_eq!(offered(&driver), payable);
    Ok(())
}

#[test]
fn a_manual_payment_taps_the_named_helpers_and_the_land() -> Result<(), ScriptError> {
    let mut driver = game(false, 1, 1, 1, 0)?;
    driver.begin_activation("Bender")?;
    driver.pay_helped_by(&["Forest"], &[], &["Relic 1", "Bear 1"], &[], 0)?;
    while !driver.state().stack().is_empty() {
        driver.answer_passively()?;
    }
    assert_eq!(life(&driver), 21);
    for name in ["Forest", "Relic 1", "Bear 1"] {
        assert!(tapped(&driver, name), "{name}");
    }
    assert!(!tapped(&driver, "Bender"));
    Ok(())
}

#[test]
fn a_payment_that_taps_more_than_the_cost_holds_is_refused() -> Result<(), ScriptError> {
    let mut driver = game(false, 0, 2, 2, 0)?;
    driver.begin_activation("Bender")?;
    let before = driver.state().state_hash();
    let result = driver.pay_helped_by(
        &[],
        &[],
        &["Relic 1", "Relic 2", "Bear 1", "Bear 2"],
        &[],
        0,
    );
    assert!(result.is_err());
    assert_eq!(driver.state().state_hash(), before);
    Ok(())
}

#[test]
fn the_automatic_payment_uses_what_there_is() -> Result<(), ScriptError> {
    let mut driver = game(true, 0, 3, 0, 0)?;
    driver.begin_activation("Bender")?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    while !driver.state().stack().is_empty() {
        driver.answer_passively()?;
    }
    assert_eq!(life(&driver), 21);
    assert!(tapped(&driver, "Bender"));
    for n in 1..=3 {
        assert!(tapped(&driver, &format!("Relic {n}")));
    }
    assert_eq!(
        driver.state().zone(Zone::Battlefield).len(),
        4,
        "nothing left play"
    );
    Ok(())
}

/// A free sorcery: "As an additional cost to cast this spell, waterbend {2}."
fn rite() -> Arc<CardDefinition> {
    let extra = r#"{"AdditionalCost":{"cost":[{"Waterbend":2}],"required":true}}"#;
    Arc::new(
        CardDefinition::new("Rite")
            .with_type(CardType::Sorcery)
            .with_mana_cost(mtg_testkit::cards::cost(0, &[]))
            .with_rules(serde_json::from_str::<Ability>(extra).unwrap()),
    )
}

#[test]
fn a_waterbend_part_outside_an_ability_is_never_offered() -> Result<(), ScriptError> {
    let driver = Driver::from_scenario(
        Scenario::new(2)
            .hand(0, [rite()])
            .battlefield(0, [ready(forest()), ready(forest()), ready(relic(1))]),
    )?;
    let mtg_engine::decision::DecisionKind::Priority(options) = &driver.decision()?.kind else {
        panic!("priority was expected");
    };
    assert!(options.castable.is_empty(), "no helper could be offered");
    Ok(())
}
