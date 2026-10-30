//! Goal: a cast or activation taken back before the stack (`AnnouncementRewound`) is told whole, card and reason, to the seat that began it, and to every other seat only as that seat having taken back a play, at every table size and for every reason.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::Event;
use mtg_engine::decision::Unplayable;
use mtg_engine::zones::Zone;
use mtg_protocol::{EventView, HiddenEvent, project_events};
use mtg_testkit::Scenario;
use mtg_testkit::cards::free_creature;
use rstest::rstest;

#[rstest]
fn a_rewind_names_the_card_to_its_player_alone(
    #[values(2, 3, 4)] seats: usize,
    #[values(Unplayable::Mana, Unplayable::Targets, Unplayable::Cost)] reason: Unplayable,
) {
    let engine = Scenario::new(seats)
        .hand(0, [free_creature("Secret", 1, 1)])
        .build()
        .unwrap();
    let state = engine.state();
    let object = state.zone(Zone::Hand(PlayerId(0)))[0];
    let rewound = Event::AnnouncementRewound {
        player: PlayerId(0),
        object,
        reason,
    };
    let viewers: Vec<PlayerId> = (0..seats).map(PlayerId::from_index).collect();
    let batches = project_events(state, std::slice::from_ref(&rewound), &viewers, 0).unwrap();
    for (viewer, batch) in viewers.iter().zip(&batches) {
        let expected = if viewer.index() == 0 {
            EventView::Public(rewound.clone())
        } else {
            EventView::Hidden(HiddenEvent::AnnouncementRewound {
                player: PlayerId(0),
            })
        };
        assert_eq!(batch.events, [expected], "{viewer:?}");
    }
}
