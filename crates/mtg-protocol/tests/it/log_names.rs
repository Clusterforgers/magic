//! Goal: the names a snapshot's log window carries are those of the objects the seat saw during the window and cannot read in the state after it (an object that ceased to exist, a card put away), never of a card the seat did not see, never of one the state still names, each object once in id order and at most `SNAPSHOT_NAMES` of them.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::GameState;
use mtg_engine::events::{Event, ZoneCause};
use mtg_engine::zones::Zone;
use mtg_protocol::{LogName, SNAPSHOT_NAMES, project_log, project_window};
use mtg_testkit::cards::{bear, forest, free_creature, giant};
use mtg_testkit::{Scenario, ready};
use rstest::rstest;

const OWNER: PlayerId = PlayerId(0);
const OTHER: PlayerId = PlayerId(1);

/// Seat 0 has a card in hand, two in the library and a creature "Mine"; seat 1 has the creature "Theirs".
struct Table {
    state: GameState,
    hand: ObjectId,
    library: ObjectId,
    mine: ObjectId,
    theirs: ObjectId,
}

fn table() -> Table {
    let state = Scenario::new(2)
        .hand(0, [bear()])
        .library(0, [giant(), forest()])
        .battlefield(0, [ready(free_creature("Mine", 1, 1))])
        .battlefield(1, [ready(free_creature("Theirs", 2, 2))])
        .build()
        .unwrap()
        .state()
        .clone();
    let battlefield = state.zone(Zone::Battlefield);
    let named = |name: &str| {
        *battlefield
            .iter()
            .find(|id| state.object(**id).unwrap().card().name == name)
            .unwrap()
    };
    Table {
        hand: state.zone(Zone::Hand(OWNER))[0],
        library: *state.zone(Zone::Library(OWNER)).last().unwrap(),
        mine: named("Mine"),
        theirs: named("Theirs"),
        state,
    }
}

fn moved(object: ObjectId, from: Zone, to: Zone) -> Event {
    Event::ZoneChanged {
        object,
        from,
        to,
        cause: ZoneCause::Moved,
    }
}

fn name_of(state: &GameState, id: ObjectId) -> String {
    state.object(id).unwrap().card().name.clone()
}

fn named(state: &GameState, ids: &[ObjectId]) -> Vec<LogName> {
    let mut all: Vec<LogName> = ids
        .iter()
        .map(|id| LogName {
            object: *id,
            name: name_of(state, *id),
        })
        .collect();
    all.sort_by_key(|name| name.object);
    all
}

#[test]
fn an_object_that_ceased_to_exist_and_a_card_put_away_keep_their_names() {
    let t = table();
    let events = [
        Event::ObjectLeftGame { object: t.mine },
        moved(t.theirs, Zone::Battlefield, Zone::Library(OTHER)),
    ];
    for viewer in [OWNER, OTHER] {
        let window = project_window(&t.state, &events, viewer).unwrap();
        assert_eq!(
            window.names,
            named(&t.state, &[t.mine, t.theirs]),
            "viewer {viewer:?}"
        );
        assert_eq!(window.log, project_log(&t.state, &events, viewer).unwrap());
    }
}

#[test]
fn a_card_the_seat_never_saw_is_not_named_whatever_happens_to_it() {
    let t = table();
    // Seat 0 puts its hand card back in the library and a library card ceases to exist; seat 1 sees neither.
    let events = [
        moved(t.hand, Zone::Hand(OWNER), Zone::Library(OWNER)),
        Event::ObjectLeftGame { object: t.library },
    ];
    let own = project_window(&t.state, &events, OWNER).unwrap();
    assert_eq!(
        own.names,
        named(&t.state, &[t.hand]),
        "the owner read the card in hand and never the one in the library"
    );
    let other = project_window(&t.state, &events, OTHER).unwrap();
    assert_eq!(other.names, Vec::new(), "the other seat saw neither");
}

#[test]
fn what_the_state_after_the_window_still_names_is_left_out() {
    let t = table();
    // A creature that dies is in the graveyard, which every seat sees: the state names it.
    let dies = [moved(t.mine, Zone::Battlefield, Zone::Graveyard(OWNER))];
    for viewer in [OWNER, OTHER] {
        let window = project_window(&t.state, &dies, viewer).unwrap();
        assert_eq!(window.names, Vec::new(), "viewer {viewer:?}");
    }
    assert_eq!(
        project_window(&t.state, &[], OWNER).unwrap().names,
        Vec::new(),
        "an empty window names nothing"
    );
}

#[rstest]
#[case::few(3)]
#[case::at_the_limit(SNAPSHOT_NAMES)]
#[case::over_the_limit(SNAPSHOT_NAMES + 7)]
fn the_names_are_bounded_and_the_oldest_objects_go_first(#[case] count: usize) {
    let creatures: Vec<_> = (0..count)
        .map(|at| ready(free_creature(&format!("Creature {at}"), 1, 1)))
        .collect();
    let state = Scenario::new(2)
        .battlefield(0, creatures)
        .build()
        .unwrap()
        .state()
        .clone();
    let ids: Vec<ObjectId> = state.zone(Zone::Battlefield).to_vec();
    assert_eq!(ids.len(), count);
    let events: Vec<Event> = ids
        .iter()
        .map(|id| Event::ObjectLeftGame { object: *id })
        .collect();
    let window = project_window(&state, &events, OTHER).unwrap();
    let kept = &window.names;
    assert_eq!(kept.len(), count.min(SNAPSHOT_NAMES));
    assert!(kept.windows(2).all(|pair| pair[0].object < pair[1].object));
    let newest = ids.iter().max().unwrap();
    assert!(kept.iter().any(|name| name.object == *newest));
    let oldest_kept = kept.iter().map(|name| name.object).min().unwrap();
    let dropped = ids.iter().filter(|id| **id < oldest_kept).count();
    assert_eq!(dropped, count - kept.len());
}
