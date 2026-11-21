//! Goal: a replacement effect that reaches a spell cast in the middle of a resolution hands the cast and the steps after
//! it to the resolving ability (CR 608.2g, 614.1) at 2 to 4 seats: Unpredictable Cyclone's replaced cycling draw exiles
//! until a card sharing a card type with the cycled card, offers it free, then puts the rest on the bottom; a cast card
//! goes on the stack above the cycling ability and resolves after it, a declined one goes to the bottom with the rest; a
//! land cycled draws as usual; each log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_engine::GameSetup;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::replays;
use crate::common::{names, pid};

fn cyclone() -> Arc<CardDefinition> {
    let mut definition = Arc::unwrap_or_clone(card("Unpredictable Cyclone").unwrap());
    definition.manual.clear();
    Arc::new(definition)
}

/// Seat 0 controls the cyclone with a Swamp and a Plains, holds `cycled`, and its library is (top first) a Forest, a
/// Forest, Grizzly Bears, then a Forest.
fn table(seats: usize, cycled: &str) -> Scenario {
    Scenario::new(seats)
        .hand_named(0, [cycled])
        .battlefield(0, [ready(cyclone()), ready(swamp()), ready(plains())])
        .library(
            0,
            [forest(), forest(), card("Grizzly Bears").unwrap(), forest()],
        )
}

/// Cycles the card in hand, answers the free cast with `cast`, and plays on until the stack is empty; how many times the
/// cast was offered.
fn cycle(
    driver: &mut Driver,
    setup: &GameSetup,
    name: &str,
    cast: bool,
) -> Result<usize, ScriptError> {
    driver.begin_activation(name)?;
    let mut offered = 0;
    for _ in 0..100 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(offered),
            // The cast spell waits above the cycling ability, which put the rest on the bottom before anyone got priority.
            DecisionKind::Priority(_)
                if names(driver.state(), Zone::Stack) == ["Grizzly Bears"] =>
            {
                assert_eq!(driver.state().zone(Zone::Library(pid(0))).len(), 3);
                assert_eq!(driver.state().zone(Zone::Exile).len(), 0);
                driver.answer_passively()?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::YesNo(_) => {
                assert_eq!(decision.player, pid(0));
                offered += 1;
                // The question of the cast hung on the resolution is asked again from the log.
                replays(driver, setup);
                driver.answer(Answer::YesNo(YesNoChoice(cast)))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

#[rstest]
fn the_card_found_is_cast_free_and_the_rest_goes_to_the_bottom(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let setup = table(seats, "Lurching Rotbeast").setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, "Lurching Rotbeast"))?;
    assert_eq!(cycle(&mut driver, &setup, "Lurching Rotbeast", true)?, 1);
    // Nothing was drawn: the Bears were cast free and resolved, the two Forests exiled before them went to the bottom.
    assert_eq!(names(driver.state(), Zone::Hand(pid(0))).len(), 0);
    assert!(names(driver.state(), Zone::Battlefield).contains(&"Grizzly Bears".to_owned()));
    assert_eq!(
        names(driver.state(), Zone::Library(pid(0))),
        ["Forest", "Forest", "Forest"]
    );
    assert_eq!(driver.state().zone(Zone::Exile).len(), 0);
    assert!(driver.state().resolving().is_none());
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn a_declined_cast_puts_the_card_found_on_the_bottom_with_the_rest(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let setup = table(seats, "Lurching Rotbeast").setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, "Lurching Rotbeast"))?;
    assert_eq!(cycle(&mut driver, &setup, "Lurching Rotbeast", false)?, 1);
    assert_eq!(names(driver.state(), Zone::Hand(pid(0))).len(), 0);
    assert!(!names(driver.state(), Zone::Battlefield).contains(&"Grizzly Bears".to_owned()));
    let mut library = names(driver.state(), Zone::Library(pid(0)));
    library.sort();
    assert_eq!(library, ["Forest", "Forest", "Forest", "Grizzly Bears"]);
    assert_eq!(driver.state().zone(Zone::Exile).len(), 0);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn a_land_cycled_draws(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let setup = table(seats, "Secluded Steppe").setup().unwrap();
    let mut driver = Driver::from_scenario(table(seats, "Secluded Steppe"))?;
    assert_eq!(cycle(&mut driver, &setup, "Secluded Steppe", true)?, 0);
    assert_eq!(names(driver.state(), Zone::Hand(pid(0))), ["Forest"]);
    assert_eq!(driver.state().zone(Zone::Library(pid(0))).len(), 3);
    replays(&driver, &setup);
    Ok(())
}
