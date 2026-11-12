//! Goal: a by-hand line of a permanent whose event reads is logged as due (`ByHandDue`, its controller and its words) each time that event happens, at two to four seats; a line whose event did not happen, a line of a card off the battlefield, a line whose event does not read and a line with no event read are never due; a rebuilt engine holds the same log.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::{Engine, Event};
use mtg_testkit::cards::{by_hand, by_hand_due, free_creature};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

const MINE: &str = "Whenever you cast a spell, juggle.";
const THEIRS: &str = "Whenever an opponent casts a spell, juggle.";

/// The dues of the events, as (controller, text).
fn dues(events: &[Event]) -> Vec<(usize, String)> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::ByHandDue(due) => Some((due.controller.index(), due.text.clone())),
            _ => None,
        })
        .collect()
}

/// Seat 0 casts a free creature with `watcher` holding the line `text` for `holder`, on the battlefield or in hand.
fn cast_with(
    seats: usize,
    holder: usize,
    text: &str,
    read: bool,
    in_play: bool,
) -> (Driver, Vec<Event>) {
    let line = if read { by_hand_due } else { by_hand };
    let watcher = line(free_creature("Watcher", 1, 1), text);
    let bear = free_creature("Bear", 2, 2);
    let scenario = if in_play {
        Scenario::new(seats)
            .hand(0, [bear])
            .battlefield(holder, [ready(watcher)])
    } else {
        // A scenario holds one hand per seat, so the watcher waits in the caster's.
        assert_eq!(holder, 0);
        Scenario::new(seats).hand(0, [bear, watcher])
    };
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let events = driver.cast("Bear").unwrap();
    (driver, events)
}

#[rstest]
fn a_line_is_due_when_its_event_happens(#[values(2, 3, 4)] seats: usize) {
    let (_, events) = cast_with(seats, 0, MINE, true, true);
    assert_eq!(dues(&events), [(0, MINE.to_owned())]);
    let last = seats - 1;
    let (_, events) = cast_with(seats, last, THEIRS, true, true);
    assert_eq!(dues(&events), [(last, THEIRS.to_owned())]);
}

#[rstest]
#[case::its_event_did_not_happen(0, THEIRS, true, true)]
#[case::off_the_battlefield(0, MINE, true, false)]
#[case::no_event_read(0, MINE, false, true)]
#[case::an_event_that_does_not_read(0, "Whenever the moon is full, juggle.", true, true)]
fn a_line_is_never_due_otherwise(
    #[case] holder: usize,
    #[case] text: &str,
    #[case] read: bool,
    #[case] in_play: bool,
) {
    let (_, events) = cast_with(2, holder, text, read, in_play);
    assert_eq!(dues(&events), []);
}

#[test]
fn a_rebuilt_engine_holds_the_same_log() {
    let (driver, _) = cast_with(2, 0, MINE, true, true);
    let log = driver.engine().events().to_vec();
    assert_eq!(dues(&log).len(), 1);
    let rebuilt = Engine::resume(driver.engine().setup().clone(), log.clone()).unwrap();
    assert_eq!(rebuilt.events(), log.as_slice());
    assert_eq!(rebuilt.state_hash(), driver.engine().state_hash());
    assert_eq!(rebuilt.decision(), driver.engine().decision());
}
