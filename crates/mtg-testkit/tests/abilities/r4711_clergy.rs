//! Goal: a printed "If this creature would be destroyed, regenerate it." (`ReplaceBy::Regenerate`, Clergy of the Holy
//! Nimbus) saves the creature from every destruction, tapped, without being used up, and an opponent's activation of
//! "{1}: This creature can't be regenerated this turn. Only your opponents may activate this ability." lets the next
//! destruction through, at two to four seats; the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::swamp;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// Priority passes until the stack is empty.
fn r4711_settle(driver: &mut Driver) {
    for _ in 0..60 {
        if !matches!(driver.decision().unwrap().kind, DecisionKind::Priority(_))
            || driver.state().stack().is_empty()
        {
            break;
        }
        driver.pass().unwrap();
    }
}

/// Whether Clergy of the Holy Nimbus is on the battlefield.
fn r4711_alive(driver: &Driver) -> bool {
    driver.state().zone(Zone::Battlefield).iter().any(|id| {
        driver
            .state()
            .object(*id)
            .is_some_and(|object| object.card().name == "Clergy of the Holy Nimbus")
    })
}

#[rstest]
fn r4711_clergy_regenerates_unless_an_opponent_paid(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] activated: bool,
) {
    let owner = seats - 1;
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(0, [(); 6].map(|()| ready(swamp())))
        .battlefield(owner, [ready(card("Clergy of the Holy Nimbus").unwrap())])
        .hand(
            0,
            [card("Doom Blade").unwrap(), card("Doom Blade").unwrap()],
        );
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    // The active player is an opponent of the Clergy's controller and may activate its second ability.
    assert_eq!(
        driver
            .offered_abilities("Clergy of the Holy Nimbus")
            .unwrap(),
        vec![1]
    );
    if activated {
        driver
            .begin_ability("Clergy of the Holy Nimbus", 1)
            .unwrap();
        driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
        r4711_settle(&mut driver);
    }
    for _ in 0..2 {
        if !r4711_alive(&driver) {
            break;
        }
        let clergy = driver.target_named("Clergy of the Holy Nimbus").unwrap();
        driver.cast_at("Doom Blade", &[clergy]).unwrap();
        r4711_settle(&mut driver);
    }
    assert_eq!(r4711_alive(&driver), !activated, "activated: {activated}");
    if !activated {
        let clergy = driver.id_of("Clergy of the Holy Nimbus").unwrap();
        assert!(
            driver
                .state()
                .object(clergy)
                .unwrap()
                .permanent()
                .unwrap()
                .tapped(),
            "a regenerated creature is tapped"
        );
    }
    let graveyard = driver
        .state()
        .zone(Zone::Graveyard(PlayerId::from_index(owner)));
    assert_eq!(graveyard.is_empty(), !activated);
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}
