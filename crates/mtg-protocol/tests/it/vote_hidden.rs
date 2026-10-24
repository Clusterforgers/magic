//! Goal: a vote tells the table what it should and no more, at two to four seats: an open vote is told to every seat as each player casts it, in the order of the votes, a secret vote is told to the seat that cast it and to nobody else until the vote is done, and then every seat reads the same tally from the finished step.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::choose::{Choice, OptionChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::{Event, GameState};
use mtg_protocol::{EventView, HiddenEvent, project_events};
use mtg_testkit::cards::{free_creature, island};
use mtg_testkit::{Driver, Scenario, open_ballot, ready, secret_ballot};
use rstest::rstest;

fn seat(index: usize) -> PlayerId {
    PlayerId::from_index(index)
}

/// Casts the ballot from seat 0; the seat with number n votes for the option `n % 2` (heads or tails). Returns the log and
/// what each seat is told of it.
fn ballot(seats: usize, secret: bool) -> (Vec<Event>, Vec<Vec<EventView>>) {
    let card = if secret {
        secret_ballot("Ballot")
    } else {
        open_ballot("Ballot")
    };
    let scenario = Scenario::new(seats)
        .hand(0, [card])
        .battlefield(0, [ready(island())])
        .library(
            0,
            (0..10).map(|n| free_creature(&format!("Card {n}"), 1, 1)),
        );
    let before = GameState::from_setup(&scenario.clone().setup().unwrap()).unwrap();
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Ballot").unwrap();
    for _ in 0..100 {
        let decision = driver.decision().unwrap().clone();
        match decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::Priority(_) => {
                driver.pass().unwrap();
            }
            DecisionKind::ChooseOption(_) => {
                let pick = OptionChoice(decision.player.index() % 2);
                driver.answer(Answer::ChooseOption(pick)).unwrap();
            }
            _ => panic!("a vote asks for an option, not {}", decision.kind.name()),
        }
    }
    let events = driver.engine().events().to_vec();
    let viewers: Vec<PlayerId> = (0..seats).map(seat).collect();
    let told = project_events(&before, &events, &viewers, 0)
        .unwrap()
        .into_iter()
        .map(|batch| batch.events)
        .collect();
    (events, told)
}

/// What a seat is told of the votes, in order: the voter and the vote when it was told, none when it was not.
fn votes_told(told: &[EventView]) -> Vec<(PlayerId, Option<Choice>)> {
    told.iter()
        .filter_map(|view| match view {
            EventView::Public(Event::ChoiceMade { player, choice }) => {
                Some((*player, Some(choice.clone())))
            }
            EventView::Hidden(HiddenEvent::ChoiceMade { player }) => Some((*player, None)),
            _ => None,
        })
        .collect()
}

/// The players a seat reads in the finished step as having voted for the name.
fn tally(told: &[EventView], name: &str) -> Vec<PlayerId> {
    told.iter()
        .find_map(|view| match view {
            EventView::Public(Event::StepDone { bound, .. }) => bound
                .iter()
                .find(|(bound_name, _)| bound_name == name)
                .map(|(_, targets)| targets.iter().filter_map(|t| t.player()).collect()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no seat is told the votes for {name}"))
}

#[rstest]
fn an_open_vote_is_told_to_every_seat_as_it_is_cast(#[values(2, 3, 4)] seats: usize) {
    let (_, told) = ballot(seats, false);
    let want: Vec<(PlayerId, Option<Choice>)> = (0..seats)
        .map(|n| (seat(n), Some(Choice::Option(n % 2))))
        .collect();
    for (viewer, views) in told.iter().enumerate() {
        assert_eq!(
            votes_told(views),
            want,
            "seat {viewer} reads every vote, in order"
        );
    }
}

#[rstest]
fn a_secret_vote_is_told_only_to_the_seat_that_cast_it(#[values(2, 3, 4)] seats: usize) {
    let (_, told) = ballot(seats, true);
    for (viewer, views) in told.iter().enumerate() {
        let read = votes_told(views);
        assert_eq!(
            read.len(),
            seats,
            "seat {viewer} knows that every seat voted"
        );
        for (index, (voter, vote)) in read.iter().enumerate() {
            assert_eq!(*voter, seat(index), "in turn order");
            if index == viewer {
                assert_eq!(
                    *vote,
                    Some(Choice::Vote(index % 2)),
                    "seat {viewer} knows its own vote"
                );
            } else {
                assert_eq!(
                    *vote, None,
                    "seat {viewer} is not told the vote of seat {index}"
                );
            }
        }
    }
}

#[rstest]
fn the_finished_vote_shows_the_same_tally_to_every_seat(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] secret: bool,
) {
    let (_, told) = ballot(seats, secret);
    let heads: Vec<PlayerId> = (0..seats).filter(|n| n % 2 == 0).map(seat).collect();
    let tails: Vec<PlayerId> = (0..seats).filter(|n| n % 2 == 1).map(seat).collect();
    for (viewer, views) in told.iter().enumerate() {
        assert_eq!(tally(views, "heads"), heads, "seat {viewer}");
        assert_eq!(tally(views, "tails"), tails, "seat {viewer}");
    }
}
