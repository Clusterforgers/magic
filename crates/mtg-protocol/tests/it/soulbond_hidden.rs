//! Goal: a soulbond pairing is public at two to four seats and to a spectator: every seat reads the same `Paired` and `Unpaired` events naming both creatures, the view of each creature shows its partner to every viewer while the pair stands and none once it has ended, the pair of a face-down creature is told as well, and the cards in the hands stay hidden.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::keyword::Keyword;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::{Event, GameState};
use mtg_protocol::{EventView, ObjectView, StateView, project_events};
use mtg_testkit::cards::{forest, free_creature, keyworded_creature, mountain};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::hidden_info::{assert_clean, hidden_across};

fn seat(index: usize) -> PlayerId {
    PlayerId::from_index(index)
}

/// Seat 0 controls a Bond and holds an Ally and a Bolt with a Mountain; every other seat holds a card of its own.
fn table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .library(0, vec![forest(); 8])
        .hand(
            0,
            vec![
                free_creature("Ally", 1, 1),
                mtg_testkit::corpus::card("Lightning Bolt").unwrap(),
            ],
        )
        .battlefield(
            0,
            [
                ready(keyworded_creature(
                    "Bond",
                    0,
                    &[],
                    2,
                    2,
                    &[Keyword::Soulbond],
                )),
                ready(mountain()),
            ],
        );
    let others = [["Wrath of God"], ["Fireball"], ["Opt"]];
    for other in 1..seats {
        scenario = scenario
            .library(other, vec![forest(); 8])
            .hand_named(other, others[other - 1]);
    }
    scenario
}

/// Answers a "may" with yes and everything else passively, until the stack is empty.
fn settle(driver: &mut Driver) {
    for _ in 0..60 {
        match driver.decision().unwrap().kind.clone() {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return,
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(true))).unwrap();
            }
            DecisionKind::Priority(_) => {
                driver.pass().unwrap();
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    panic!("the game did not settle");
}

fn view_of(state: &GameState, viewer: PlayerId, id: ObjectId) -> ObjectView {
    StateView::for_player(state, viewer)
        .objects
        .into_iter()
        .find(|view| view.id == id)
        .unwrap()
}

#[rstest]
fn every_seat_and_a_spectator_reads_the_pairing_and_no_hidden_card(
    #[values(2, 3, 4)] seats: usize,
) {
    let mut driver = Driver::from_scenario(table(seats)).unwrap();
    let start: GameState = driver.state().clone();
    let logged = driver.engine().events().len();
    let bond = driver.id_of("Bond").unwrap();
    driver.cast("Ally").unwrap();
    settle(&mut driver);
    let ally = driver.id_of("Ally").unwrap();
    let paired: GameState = driver.state().clone();
    let target = driver.target_named("Ally").unwrap();
    driver.cast_at("Lightning Bolt", &[target]).unwrap();
    settle(&mut driver);
    let after = driver.state().clone();
    let events = driver.engine().events()[logged..].to_vec();
    let all: Vec<PlayerId> = (0..=seats).map(seat).collect();
    let batches = project_events(&start, &events, &all, u64::try_from(logged).unwrap()).unwrap();
    for (viewer, batch) in all.iter().zip(&batches) {
        let public = |wanted: &dyn Fn(&Event) -> bool| {
            batch
                .events
                .iter()
                .any(|view| matches!(view, EventView::Public(event) if wanted(event)))
        };
        assert!(
            public(&|event| matches!(event, Event::Paired { object, with }
                if (*object, *with) == (ally, bond) || (*object, *with) == (bond, ally))),
            "{viewer:?} reads the pairing in the open"
        );
        assert!(
            public(&|event| matches!(event, Event::Unpaired { object, with }
                if (*object, *with) == (ally, bond) || (*object, *with) == (bond, ally))),
            "{viewer:?} reads the end of the pair in the open"
        );
        assert_clean(batch, &hidden_across(&start, &after, *viewer));
        // While the pair stands each creature shows its partner, and once it has ended neither does.
        if viewer.index() < seats {
            assert_eq!(view_of(&paired, *viewer, bond).paired_with, Some(ally));
            assert_eq!(view_of(&paired, *viewer, ally).paired_with, Some(bond));
            assert_eq!(view_of(&after, *viewer, bond).paired_with, None);
        }
    }
    let spectator = StateView::for_spectator(&paired);
    for (id, partner) in [(bond, ally), (ally, bond)] {
        let view = spectator.objects.iter().find(|view| view.id == id).unwrap();
        assert_eq!(
            view.paired_with,
            Some(partner),
            "the spectator sees the pair"
        );
    }
}

#[test]
fn a_view_without_a_pair_carries_no_partner_field_in_its_json() {
    let driver = Driver::from_scenario(table(2)).unwrap();
    let bond = driver.id_of("Bond").unwrap();
    let view = view_of(driver.state(), seat(1), bond);
    assert_eq!(view.paired_with, None);
    let json = serde_json::to_value(&view).unwrap();
    assert!(json.get("paired_with").is_none(), "{json}");
}
