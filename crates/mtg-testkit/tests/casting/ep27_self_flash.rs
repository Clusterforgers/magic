//! Goal: "You may cast this spell as though it had flash if you control a Faerie" (a `MayCast` rule of the card that works from its owner's hand) makes the card castable from the hand on another player's turn while the condition holds and not otherwise, never lets another player cast it, still leaves it castable on its owner's main phase, and the cast resolves; at tables of two and three seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::name_of;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, forest};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const SELF_FLASH: &str = r#"{"Rule":{"rule":{"MayCast":{"what":["This"],"flash":true}},"condition":{"Compare":{"left":{"Count":[{"Subtype":"Faerie"},{"ControlledBy":"You"}]},"op":"Ge","right":1}},"from":"Hand"}}"#;

/// A creature for `{G}` its owner may cast as though it had flash while they control a Faerie.
fn spinner() -> Arc<CardDefinition> {
    let rule: Ability = serde_json::from_str(SELF_FLASH).unwrap();
    Arc::new(
        CardDefinition::new("Spinner")
            .with_type(CardType::Creature)
            .with_mana_cost(cost(0, &[Color::Green]))
            .with_power_toughness(1, 1)
            .with_rules(rule),
    )
}

fn faerie() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Faerie")
            .with_type(CardType::Creature)
            .with_subtype("Faerie")
            .with_power_toughness(1, 1),
    )
}

/// Passes until `seat` has priority and says whether it may cast the spinner.
fn may_cast(driver: &mut Driver, seat: usize, seats: usize) -> Result<bool, ScriptError> {
    for _ in 0..seats {
        if driver.decision()?.player.index() == seat {
            break;
        }
        driver.pass()?;
    }
    let decision = driver.decision()?;
    assert_eq!(decision.player.index(), seat, "the seat has priority");
    let DecisionKind::Priority(options) = &decision.kind else {
        panic!("the seat has priority");
    };
    Ok(options
        .castable
        .iter()
        .any(|id| name_of(driver.state(), *id) == "Spinner"))
}

#[rstest]
#[case::with_a_faerie_on_another_turn(true, false, true)]
#[case::without_a_faerie_on_another_turn(false, false, false)]
#[case::without_a_faerie_on_its_own_main_phase(false, true, true)]
fn the_card_is_cast_as_though_it_had_flash_while_the_condition_holds(
    #[case] with_faerie: bool,
    #[case] own_turn: bool,
    #[case] castable: bool,
    #[values(2, 3)] seats: usize,
) -> Result<(), ScriptError> {
    let holder = seats - 1;
    let mut permanents = vec![ready(forest())];
    if with_faerie {
        permanents.push(ready(faerie()));
    }
    let active = if own_turn { holder } else { 0 };
    let scenario = Scenario::new(seats)
        .at(1, active, Step::Main1)
        .battlefield(holder, permanents)
        .hand(holder, [spinner()]);
    let mut driver = Driver::from_scenario(scenario)?;
    assert_eq!(may_cast(&mut driver, holder, seats)?, castable);
    if castable {
        driver.cast("Spinner")?;
        for _ in 0..seats * 2 {
            if driver.state().zone(Zone::Stack).is_empty() {
                break;
            }
            driver.pass()?;
        }
        let entered = driver
            .state()
            .zone(Zone::Battlefield)
            .iter()
            .any(|id| name_of(driver.state(), *id) == "Spinner");
        assert!(entered, "the cast resolves");
    }
    Ok(())
}

/// Another player never casts the card from its owner's hand, whatever they control.
#[rstest]
fn another_player_may_not_cast_it(#[values(2, 3)] seats: usize) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, vec![ready(forest()), ready(faerie())])
        .battlefield(1, vec![ready(faerie())])
        .hand(1, [spinner()]);
    let mut driver = Driver::from_scenario(scenario)?;
    assert!(!may_cast(&mut driver, 0, seats)?);
    Ok(())
}
