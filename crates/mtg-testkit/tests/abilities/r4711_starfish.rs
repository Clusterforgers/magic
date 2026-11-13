//! Goal: the turn log counts each regeneration of a permanent (`TallyEvent::Regenerated`), so Spiny Starfish ("At the
//! beginning of each end step, if this creature regenerated this turn, create a 0/1 blue Starfish creature token for each
//! time it regenerated this turn.") makes one token for each regeneration, and none and no trigger without one, at two
//! to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{island, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// Priority passes until the stack is empty, other questions answered passively.
fn r4711_settle(driver: &mut Driver) {
    for _ in 0..60 {
        match driver.decision().unwrap().kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::Priority(_) => {
                driver.pass().unwrap();
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
}

/// The permanents named Starfish (the tokens).
fn r4711_starfish_tokens(driver: &Driver) -> usize {
    driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            driver
                .state()
                .object(**id)
                .is_some_and(|object| object.card().name == "Starfish")
        })
        .count()
}

#[rstest]
fn r4711_spiny_starfish_counts_its_regenerations(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 2)] regenerations: usize,
) {
    let mut lands = vec![ready(island()), ready(island())];
    lands.extend((0..4).map(|_| ready(swamp())));
    lands.push(ready(card("Spiny Starfish").unwrap()));
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(0, lands)
        .hand(
            0,
            [card("Doom Blade").unwrap(), card("Doom Blade").unwrap()],
        );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    for _ in 0..regenerations {
        driver.begin_activation("Spiny Starfish").unwrap();
        driver
            .pay_tapping(&["Island"], 0)
            .or_else(|_| driver.answer(Answer::PayMana(PayChoice::Auto)))
            .unwrap();
        r4711_settle(&mut driver);
        let starfish = driver.target_named("Spiny Starfish").unwrap();
        driver.cast_at("Doom Blade", &[starfish]).unwrap();
        r4711_settle(&mut driver);
    }
    assert!(driver.id_of("Spiny Starfish").is_ok(), "it regenerated");
    driver.idle_until(3, Step::End).unwrap();
    r4711_settle(&mut driver);
    assert_eq!(r4711_starfish_tokens(&driver), regenerations);
}
