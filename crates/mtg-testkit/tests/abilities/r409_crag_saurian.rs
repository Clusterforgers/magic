//! Goal: "Whenever a source deals damage to this creature, that source's controller gains control of this creature." plays
//! at two to four seats: Crag Saurian goes to the controller of whatever damages it, an opponent's Lightning Bolt or its own
//! controller's (which changes nothing), and stays with that player afterwards.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::characteristics::controller_of;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_testkit::cards::mountain;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Priority passes until the stack is empty.
fn r409_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..60 {
        if !matches!(driver.decision()?.kind, DecisionKind::Priority(_))
            || driver.state().stack().is_empty()
        {
            break;
        }
        driver.pass()?;
    }
    Ok(())
}

/// Seat 0 controls Crag Saurian; the active seat `caster` holds a Lightning Bolt and a Mountain and bolts the Saurian (a 4/4).
#[rstest]
#[case::an_opponent_takes_it(2, 1, 1)]
#[case::the_third_player_takes_it(3, 2, 2)]
#[case::its_controller_keeps_it(4, 0, 0)]
fn r409_crag_saurian_goes_to_the_controller_of_the_source(
    #[case] seats: usize,
    #[case] caster: usize,
    #[case] owner: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .battlefield(0, [ready(card("Crag Saurian").unwrap())])
        .battlefield(caster, [ready(mountain())])
        .hand(caster, [card("Lightning Bolt").unwrap()])
        .at(1, caster, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    let saurian = driver.id_of("Crag Saurian")?;
    driver.begin_cast("Lightning Bolt")?;
    let target = driver.target_named("Crag Saurian")?;
    driver.choose_targets(&[target])?;
    driver.answer(mtg_engine::decision::Answer::PayMana(
        mtg_engine::decision::PayChoice::Auto,
    ))?;
    r409_settle(&mut driver)?;
    assert_eq!(
        controller_of(driver.state(), saurian),
        Some(PlayerId::from_index(owner))
    );
    Ok(())
}
