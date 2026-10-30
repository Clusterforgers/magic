//! Goal: a coin flipped or a die rolled by hand reaches every seat and spectator as the same public event with the same result, and no other card, count or order in the batches or the views changes or leaks because of it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::Event;
use mtg_engine::manual::{ManualAction, ManualRoll};
use mtg_protocol::{EventView, StateView};
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::{Scenario, ready};
use rstest::rstest;

use crate::common::table::{Table, seats};

const HIDDEN: &str = "Hidden card";

fn table(seat_count: usize) -> Table {
    let mut scenario = Scenario::new(seat_count)
        .seed(9)
        .hand(0, [free_creature(HIDDEN, 1, 1)])
        .library(0, vec![forest(); 4])
        .battlefield(0, [ready(free_creature("Elf", 1, 1))]);
    for other in 1..seat_count {
        scenario = scenario.library(other, vec![forest(); 4]);
    }
    Table::new(scenario)
}

#[rstest]
fn every_seat_and_spectator_reads_the_same_roll_and_nothing_else_changes(
    #[values(2, 3, 4)] seat_count: usize,
    #[values(
        ManualRoll::Coin {},
        ManualRoll::Die { sides: 6 },
        ManualRoll::Die { sides: 100 }
    )]
    kind: ManualRoll,
) {
    let mut table = table(seat_count);
    let before: Vec<StateView> = seats(seat_count)
        .into_iter()
        .map(|viewer| StateView::for_player(table.driver.state(), viewer))
        .collect();
    let run = table.run(|driver| driver.manual(ManualAction::Roll { kind }));
    let Event::ManualRolled { result, .. } = &run.events[1] else {
        panic!("not a roll: {:?}", run.events);
    };
    for (viewer, batch) in seats(seat_count).into_iter().zip(&run.batches) {
        // The action and the roll, both exactly as logged, for every viewer.
        let told: Vec<&EventView> = batch.events.iter().collect();
        assert_eq!(told.len(), 2, "{viewer:?}");
        assert!(matches!(
            told[1],
            EventView::Public(Event::ManualRolled { result: told, kind: seen, player })
                if told == result && *seen == kind && *player == PlayerId(0)
        ));
        // The card in the hand of seat 0 stays hidden from the others, and the batch brings no card.
        if viewer != PlayerId(0) {
            assert!(
                !serde_json::to_string(&batch.objects)
                    .unwrap()
                    .contains(HIDDEN)
            );
        }
        assert!(
            batch.objects.is_empty() && batch.gone.is_empty(),
            "{viewer:?}"
        );
        let after = StateView::for_player(table.driver.state(), viewer);
        assert_eq!(
            serde_json::to_value(&after).unwrap(),
            serde_json::to_value(&before[viewer.index()]).unwrap(),
            "{viewer:?} sees the same table"
        );
    }
    let spectator = StateView::for_spectator(table.driver.state());
    assert!(!serde_json::to_string(&spectator).unwrap().contains(HIDDEN));
}
