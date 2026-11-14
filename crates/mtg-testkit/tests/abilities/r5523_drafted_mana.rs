//! Goal: in a game without a draft no color was chosen for Paliano, the High City, so its mana ability adds no mana
//! (its ruling: "If you control Paliano, the High City but you didn't draft a card named Paliano, the High City, its
//! second ability won't add mana. Notably, Paliano won't produce colorless mana."), on any seat at two to four seats,
//! while a land beside it still taps for its mana.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::decision::DecisionKind;
use mtg_testkit::cards::mountain;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

#[rstest]
fn r5523_paliano_adds_no_mana_without_a_draft(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats).battlefield(
        0,
        [
            ready(card("Paliano, the High City").unwrap()),
            ready(mountain()),
        ],
    );
    let mut driver = Driver::from_scenario(scenario)?;
    let paliano = driver.id_of("Paliano, the High City")?;
    let DecisionKind::Priority(options) = driver.decision()?.kind.clone() else {
        panic!("seat 0 holds priority");
    };
    assert!(
        options
            .mana_abilities
            .iter()
            .all(|m| m.source.object != paliano),
        "Paliano offers no mana"
    );
    assert!(driver.tap_for_mana("Paliano, the High City").is_err());
    driver.tap_for_mana("Mountain")?;
    let pool = driver
        .state()
        .player(mtg_core::ids::PlayerId(0))
        .unwrap()
        .mana()
        .total();
    assert_eq!(pool, 1, "the Mountain adds its mana");
    Ok(())
}
