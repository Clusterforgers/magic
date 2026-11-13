//! Goal: Blinding Beam's "Creatures don't untap during target player's next untap step" holds at two to four seats for
//! that player's next untap step only, for creatures, not lands, and not for another player's untap step (the Blinding
//! Beam ruling: it affects all creatures during that player's next untap step).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::choose::{CostsChoice, ModeChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, island, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::scenario::tapped;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0 casts Blinding Beam's second mode at `victim`; every other seat has a tapped Bear and a tapped Island.
fn r5508_beamed(seats: usize, victim: usize) -> Result<Driver, ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, (0..3).map(|_| ready(plains())).collect::<Vec<_>>())
        .hand(0, [card("Blinding Beam").unwrap()]);
    for seat in 1..seats {
        scenario = scenario.battlefield(
            seat,
            vec![tapped(free_creature("Bear", 2, 2)), tapped(island())],
        );
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Blinding Beam")?;
    for _ in 0..20 {
        match driver.decision()?.kind {
            DecisionKind::ChooseCosts(_) => {
                driver.answer(Answer::ChooseCosts(CostsChoice::Costs(Vec::new())))?;
            }
            DecisionKind::ChooseMode(_) => {
                driver.answer(Answer::ChooseMode(ModeChoice::Modes(vec![1])))?;
            }
            DecisionKind::ChooseTargets(_) => {
                let player = driver.target_player(victim);
                driver.choose_targets(&[player])?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => break,
        }
    }
    Ok(driver)
}

/// Whether seat `seat`'s permanent of that name is tapped.
fn r5508_tapped(driver: &Driver, seat: usize, name: &str) -> bool {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .find(|object| {
            object.card().name == name && object.controller() == PlayerId::from_index(seat)
        })
        .and_then(|object| object.permanent().map(|p| p.tapped()))
        .unwrap()
}

#[rstest]
#[case::two_seats(2, 1)]
#[case::three_seats(3, 2)]
#[case::four_seats(4, 1)]
fn r5508_the_targets_creatures_stay_tapped_for_one_untap_step(
    #[case] seats: usize,
    #[case] victim: usize,
) -> Result<(), ScriptError> {
    let mut driver = r5508_beamed(seats, victim)?;
    // Through the victim's next untap step, into their upkeep.
    let victims_turn = 2 + u32::try_from(victim).unwrap();
    driver.idle_until(victims_turn, Step::Upkeep)?;
    assert!(
        r5508_tapped(&driver, victim, "Bear"),
        "the creature stays tapped"
    );
    assert!(!r5508_tapped(&driver, victim, "Island"), "a land untaps");
    for other in (1..seats).filter(|seat| *seat < victim) {
        assert!(
            !r5508_tapped(&driver, other, "Bear"),
            "another player's creature untapped in their own step"
        );
    }
    // The rule is spent: the victim's following untap step untaps the creature.
    let next = victims_turn + u32::try_from(seats).unwrap();
    driver.idle_until(next, Step::Upkeep)?;
    assert!(!r5508_tapped(&driver, victim, "Bear"));
    Ok(())
}
