//! Goal: the event of a permanent tapped for mana holds the permanent and the player who tapped it: "that land doesn't untap during its controller's next untap step" (Vorinclex, Voice of Hunger) and "gain control of that artifact" (Treasure Nabber) reach the permanent, and "deals 1 damage to that player" (Manabarbs) reaches the player and spares a land that is also a creature.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::pid;

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::characteristics::controller_of;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_testkit::cards::forest;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Passes priority while the stack holds something, saying yes to every "may".
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..60 {
        match &driver.decision()?.kind {
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(true)))?;
            }
            DecisionKind::Priority(_) if !driver.state().stack().is_empty() => {
                driver.pass()?;
            }
            _ => return Ok(()),
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// A Forest that is also a 1/1 creature, as Dryad Arbor is.
fn forest_creature() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::basic_land(Some(Color::Green))
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1),
    )
}

/// Whether the named permanent is tapped.
fn is_tapped(driver: &Driver, name: &str) -> bool {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .tapped()
}

/// The seat that controls the named permanent.
fn controller(driver: &Driver, name: &str) -> usize {
    let id = driver.id_of(name).unwrap();
    controller_of(driver.state(), id).unwrap().index()
}

/// Seat 1 taps `source` for mana while seat 0 controls `card`; then the stack resolves.
fn opponent_taps(
    card_name: &str,
    source: Arc<CardDefinition>,
    source_name: &str,
) -> Result<Driver, ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .battlefield(0, [ready(card(card_name).unwrap())])
            .battlefield(1, [ready(source)]),
    )?;
    // The other seat has priority once seat 0 has passed in its main phase.
    driver.pass()?;
    driver.tap_for_mana(source_name)?;
    settle(&mut driver)?;
    Ok(driver)
}

#[test]
fn an_opponents_land_tapped_for_mana_skips_its_next_untap_with_vorinclex() -> Result<(), ScriptError>
{
    let mut driver = opponent_taps("Vorinclex, Voice of Hunger", forest(), "Forest")?;
    assert!(is_tapped(&driver, "Forest"));
    driver.idle_until(2, Step::Main1)?;
    assert!(
        is_tapped(&driver, "Forest"),
        "its controller's next untap step"
    );
    driver.idle_until(4, Step::Main1)?;
    assert!(!is_tapped(&driver, "Forest"), "the untap step after it");
    Ok(())
}

#[test]
fn an_opponents_artifact_tapped_for_mana_changes_control_with_treasure_nabber()
-> Result<(), ScriptError> {
    let driver = opponent_taps("Treasure Nabber", card("Sol Ring").unwrap(), "Sol Ring")?;
    assert_eq!(controller(&driver, "Sol Ring"), 0);
    Ok(())
}

#[rstest]
#[case::a_land(forest())]
#[case::a_land_that_is_a_creature(forest_creature())]
fn manabarbs_damages_the_player_who_taps_and_never_the_permanent(
    #[case] source: Arc<CardDefinition>,
) -> Result<(), ScriptError> {
    let driver = opponent_taps("Manabarbs", source, "Forest")?;
    assert_eq!(driver.state().player(pid(1)).unwrap().life(), 19);
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), 20);
    let id = driver.id_of("Forest")?;
    assert_eq!(
        driver
            .state()
            .object(id)
            .unwrap()
            .permanent()
            .unwrap()
            .damage(),
        0
    );
    Ok(())
}
