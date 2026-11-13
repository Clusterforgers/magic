//! Goal: Tomik, Distinguished Advokist's rules (`GameRule::Cant` over lands on the battlefield and land cards in
//! graveyards) stop an opponent from targeting its controller's land and from playing a land card from a graveyard,
//! while its controller still does both, at two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::pid;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, mountain, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// Seat 0 to act with Crucible of Worlds, a Forest in its graveyard and Devastate in hand; Tomik on `tomik`'s side and a
/// Plains on the last seat (`tomik` is 0 or the last seat).
fn r5519_tomik_table(seats: usize, tomik: usize) -> Scenario {
    let mut mine: Vec<_> = (0..5).map(|_| ready(mountain())).collect();
    mine.push(ready(card("Crucible of Worlds").unwrap()));
    let mut theirs = vec![ready(plains())];
    let advokist = ready(card("Tomik, Distinguished Advokist").unwrap());
    if tomik == 0 {
        mine.push(advokist);
    } else {
        theirs.push(advokist);
    }
    Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .hand(0, [card("Devastate").unwrap()])
        .graveyard(0, [forest()])
        .battlefield(0, mine)
        .battlefield(seats - 1, theirs)
}

#[rstest]
fn r5519_tomik_stops_only_opponents(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] tomik_is_mine: bool,
) {
    let tomik = if tomik_is_mine { 0 } else { seats - 1 };
    let mut driver = Driver::from_scenario(r5519_tomik_table(seats, tomik)).unwrap();
    // A land card in a graveyard: played only by Tomik's controller.
    assert_eq!(driver.play_land("Forest").is_ok(), tomik_is_mine);
    assert_eq!(
        driver.state().zone(Zone::Graveyard(pid(0))).is_empty(),
        tomik_is_mine
    );
    // A land on the battlefield: targeted only by Tomik's controller (the Plains is never seat 0's own when Tomik is
    // across the table).
    let mut driver = Driver::from_scenario(r5519_tomik_table(seats, tomik)).unwrap();
    let plains = driver.target_named("Plains").unwrap();
    assert_eq!(
        driver.cast_at("Devastate", &[plains]).is_ok(),
        tomik_is_mine
    );
}
