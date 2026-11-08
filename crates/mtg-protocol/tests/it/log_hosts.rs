//! Goal: a snapshot's log window names, for each attach event, the host the attachment hung on just before it (a permanent, a player or nothing), following the attach events of the window itself, and leaves out every event that attaches nothing.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::GameState;
use mtg_engine::events::Event;
use mtg_engine::zones::Zone;
use mtg_protocol::{LogHost, project_window};
use mtg_testkit::cards::free_creature;
use mtg_testkit::{Scenario, ready};
use rstest::rstest;

const OWNER: PlayerId = PlayerId(0);

/// Three permanents of seat 0: `a`, `b` and `gear`, which gets attached.
struct Table {
    state: GameState,
    a: ObjectId,
    b: ObjectId,
    gear: ObjectId,
}

fn table() -> Table {
    let state = Scenario::new(2)
        .battlefield(
            0,
            [
                ready(free_creature("A", 1, 1)),
                ready(free_creature("B", 1, 1)),
                ready(free_creature("Gear", 0, 1)),
            ],
        )
        .build()
        .unwrap()
        .state()
        .clone();
    let find = |name: &str| {
        *state
            .zone(Zone::Battlefield)
            .iter()
            .find(|id| state.object(**id).unwrap().card().name == name)
            .unwrap()
    };
    Table {
        a: find("A"),
        b: find("B"),
        gear: find("Gear"),
        state,
    }
}

fn host(at: u32, card: Option<ObjectId>, player: Option<PlayerId>) -> LogHost {
    LogHost { at, card, player }
}

#[rstest]
#[case::first_attach_has_no_old_host(1)]
#[case::moves_from_the_earlier_attach(2)]
#[case::a_player_host_replaces_a_card_host(3)]
fn the_old_host_follows_the_window(#[case] events: usize) {
    let t = table();
    let all = [
        Event::Attached {
            object: t.gear,
            to: t.a,
        },
        Event::Attached {
            object: t.gear,
            to: t.b,
        },
        Event::AttachedToPlayer {
            object: t.gear,
            player: OWNER,
        },
    ];
    let window = project_window(&t.state, &all[..events], OWNER).unwrap();
    let expected = [
        host(0, None, None),
        host(1, Some(t.a), None),
        host(2, Some(t.b), None),
    ];
    assert_eq!(window.hosts, expected[..events]);
}

#[test]
fn events_that_attach_nothing_have_no_entry() {
    let t = table();
    let events = [
        Event::Attached {
            object: t.gear,
            to: t.a,
        },
        Event::Unattached { object: t.gear },
        Event::Attached {
            object: t.gear,
            to: t.b,
        },
    ];
    let window = project_window(&t.state, &events, OWNER).unwrap();
    assert_eq!(window.hosts, [host(0, None, None), host(2, None, None)]);
}
