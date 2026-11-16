//! Goal: Tempt with Discovery plays its tempting offer as printed at two to four seats: its caster finds a land, each
//! opponent decides in turn whether to find one before any of them searches (the offer's ruling), the caster finds one
//! more for each opponent who accepted, and then exactly the players who searched shuffle (the caster once, a declining
//! opponent never); the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::pid;
use mtg_engine::characteristics::controller_of;
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::events::Event;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// How many Forests a seat controls.
fn r6601_forests(driver: &Driver, seat: usize) -> usize {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            state
                .object(**id)
                .is_some_and(|object| object.card().name == "Forest")
                && controller_of(state, **id) == Some(pid(seat))
        })
        .count()
}

#[rstest]
fn r6601_tempt_with_discovery_repays_each_acceptor(
    #[values(2, 3, 4)] seats: usize,
    #[values(0b000, 0b010, 0b100, 0b110, 0b1110)] accepts: usize,
) {
    let mut scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .hand(0, [card("Tempt with Discovery").unwrap()])
        .battlefield(0, (0..4).map(|_| ready(forest())));
    for seat in 0..seats {
        scenario = scenario.library(
            seat,
            [
                forest(),
                forest(),
                forest(),
                forest(),
                free_creature("Top", 1, 1),
            ],
        );
    }
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    driver.cast("Tempt with Discovery").unwrap();
    let mut shuffles = vec![0usize; seats];
    let mut asked = Vec::new();
    let mut opponent_searched_while_asking = false;
    for _ in 0..60 {
        if driver.state().stack().is_empty() {
            break;
        }
        let decision = driver.decision().unwrap().clone();
        let seat = decision.player.index();
        let events = match decision.kind {
            DecisionKind::ChooseCards(options) => {
                opponent_searched_while_asking |= seat != 0 && asked.len() < seats - 1;
                let take: Vec<_> = options
                    .from
                    .iter()
                    .copied()
                    .take(options.max as usize)
                    .collect();
                driver.answer(Answer::ChooseCards(CardsChoice(take)))
            }
            DecisionKind::YesNo(_) => {
                asked.push(seat);
                driver.answer(Answer::YesNo(YesNoChoice(accepts >> seat & 1 == 1)))
            }
            _ => driver.answer_passively(),
        }
        .unwrap();
        for event in events {
            if let Event::LibraryShuffled { player, .. } = event {
                shuffles[player.index()] += 1;
            }
        }
    }
    assert_eq!(driver.state().stack().len(), 0);
    assert_eq!(
        asked,
        (1..seats).collect::<Vec<_>>(),
        "each opponent is asked once, in turn order"
    );
    assert!(!opponent_searched_while_asking);
    let accepted = (1..seats).filter(|seat| accepts >> seat & 1 == 1).count();
    assert_eq!(r6601_forests(&driver, 0), 4 + 1 + accepted);
    assert_eq!(shuffles[0], 1, "the caster shuffles once");
    for (seat, shuffled) in shuffles.iter().enumerate().skip(1) {
        let took = accepts >> seat & 1 == 1;
        assert_eq!(
            r6601_forests(&driver, seat),
            usize::from(took),
            "seat {seat}"
        );
        assert_eq!(
            *shuffled,
            usize::from(took),
            "seat {seat} shuffles only if it searched"
        );
    }
    let replayed = replay(&scenario.setup().unwrap(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}
