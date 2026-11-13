//! Goal: Gaea's Liege counts the Forests of its controller while it is not attacking and those of the player it attacks
//! while it attacks (dying at 0/0), at tables of two to four seats; the log replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{forest, island};
use mtg_testkit::{Driver, Scenario, corpus, ready};
use rstest::rstest;

const LIEGE: &str = "Gaea's Liege";

fn stats(driver: &Driver) -> (Option<i32>, Option<i32>) {
    let id = driver.id_of(LIEGE).unwrap();
    let shown = mtg_engine::characteristics::characteristics(driver.state(), id).unwrap();
    (shown.power, shown.toughness)
}

#[rstest]
fn the_liege_counts_the_forests_of_whoever_it_faces(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 3)] mine: usize,
    #[values(0, 2, 5)] theirs: usize,
) {
    let defender = seats - 1;
    let game = || {
        let mut own = vec![ready(corpus::card(LIEGE).unwrap())];
        own.extend((0..mine).map(|_| ready(forest())));
        let mut scenario = Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .battlefield(0, own)
            .battlefield(defender, (0..theirs).map(|_| ready(forest())));
        if defender != 1 {
            // A bystander's Forests never count.
            scenario = scenario.battlefield(1, (0..4).map(|_| ready(forest())));
        }
        for seat in 0..seats {
            scenario = scenario.library(seat, (0..10).map(|_| island()));
        }
        scenario
    };
    let setup = game().setup().unwrap();
    let mut driver = Driver::from_scenario(game()).unwrap();
    let mine = i32::try_from(mine).unwrap();
    let theirs = i32::try_from(theirs).unwrap();
    assert_eq!(stats(&driver), (Some(mine), Some(mine)));
    driver.idle_until(1, Step::DeclareAttackers).unwrap();
    driver.attack(&[(LIEGE, defender)]).unwrap();
    if theirs == 0 {
        // A 0/0 Liege is put into its owner's graveyard (CR 704.5f).
        assert!(
            driver.id_of(LIEGE).is_err(),
            "the 0/0 Liege left the battlefield"
        );
    } else {
        assert_eq!(stats(&driver), (Some(theirs), Some(theirs)));
    }
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}
