//! Goal: a seat that does not own a card sees exactly what the rules of the table say and no more: open hands show every hand and the cards dealt, bottomed and drawn to every seat while the library order stays unknown, a revealed library top shows the top card of every library after each deal, draw and shuffle and nothing under it, the card a seat scries after a mulligan is shown to that seat alone, and the hand that opening-hand smoothing does not keep is shown to nobody, its owner included.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::cast_possible_truncation
)]

use std::collections::BTreeSet;

use mtg_core::house::{HouseRules, MulliganStyle};
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind, MulliganChoice};
use mtg_engine::state::Stage;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, Event, GameState};
use mtg_protocol::{EventBatch, EventView, HiddenEvent, StateView, project_events};
use mtg_testkit::{Driver, fuzz_setup};
use rstest::rstest;

fn viewers(players: usize) -> Vec<PlayerId> {
    (0..players).map(PlayerId::from_index).collect()
}

fn seat(n: usize) -> PlayerId {
    PlayerId::from_index(n)
}

fn rules(set: impl FnOnce(&mut HouseRules)) -> HouseRules {
    let mut rules = HouseRules::default();
    set(&mut rules);
    rules
}

/// The game before anything happened and a driver at its first question.
fn begin(players: usize, seed: u64, house: HouseRules) -> (GameState, Driver) {
    let setup = fuzz_setup(players, seed).with_house_rules(house);
    let before = GameState::from_setup(&setup).unwrap();
    (before, Driver::new(Engine::new(setup).unwrap()).unwrap())
}

fn batches(before: &GameState, driver: &Driver, players: usize) -> Vec<EventBatch> {
    project_events(before, driver.engine().events(), &viewers(players), 0).unwrap()
}

fn named(batch: &EventBatch) -> BTreeSet<ObjectId> {
    batch.objects.iter().map(|object| object.id).collect()
}

/// Answers the opening until turn 1 begins, keeping every hand and bottoming the first cards.
fn finish_opening(driver: &mut Driver) {
    while driver.state().stage() == Stage::Mulligan {
        driver.answer_passively().unwrap();
    }
}

// Open hands

#[rstest]
fn open_hands_show_every_hand_to_every_seat_and_no_library(
    #[values(2, 3, 4, 5)] players: usize,
    #[values(false, true)] open: bool,
) {
    let (_, driver) = begin(players, 3, rules(|r| r.open_hands = open.then_some(true)));
    let state = driver.state();
    // Seats count from 0; a spectator is a viewer that is no seat.
    for viewer in viewers(players).into_iter().chain([seat(9)]) {
        let view = StateView::for_player(state, viewer);
        let known: BTreeSet<ObjectId> = view.objects.iter().map(|object| object.id).collect();
        for owner in 0..players {
            let hand = state.zone(Zone::Hand(seat(owner))).to_vec();
            let shown = view.table.players[owner].hand.cards.clone();
            let sees = open || viewer == seat(owner);
            assert_eq!(
                shown,
                sees.then(|| hand.clone()),
                "{viewer:?} and hand {owner}"
            );
            assert!(hand.iter().all(|card| known.contains(card) == sees));
            let library = &view.table.players[owner].library;
            assert!(library.cards.is_none(), "{viewer:?} sees a library");
            assert!(
                state
                    .zone(Zone::Library(seat(owner)))
                    .iter()
                    .all(|card| !known.contains(card)),
                "{viewer:?} names a card of a library"
            );
        }
    }
}

#[rstest]
fn the_cards_dealt_are_told_whole_to_every_seat_with_open_hands(
    #[values(2, 3, 4)] players: usize,
    #[values(false, true)] open: bool,
) {
    let (before, driver) = begin(players, 4, rules(|r| r.open_hands = open.then_some(true)));
    let events = driver.engine().events();
    for (viewer, batch) in viewers(players)
        .into_iter()
        .zip(batches(&before, &driver, players))
    {
        for (event, shown) in events.iter().zip(&batch.events) {
            match event {
                Event::HandDealt { player, cards } if open || *player == viewer => {
                    assert_eq!(shown, &EventView::Public(event.clone()));
                    assert!(cards.iter().all(|card| named(&batch).contains(card)));
                }
                Event::HandDealt { player, cards } => assert_eq!(
                    shown,
                    &EventView::Hidden(HiddenEvent::HandDealt {
                        player: *player,
                        count: cards.len() as u32
                    })
                ),
                // The order of a library is known to nobody, whoever's hand is open.
                Event::LibraryShuffled { player, .. } => assert_eq!(
                    shown,
                    &EventView::Hidden(HiddenEvent::LibraryShuffled { player: *player })
                ),
                _ => {}
            }
        }
        let json = serde_json::to_string(&batch.events).unwrap();
        assert!(!json.contains("\"order\""), "{viewer:?} was told an order");
    }
}

#[rstest]
fn a_card_that_leaves_an_open_hand_for_the_library_is_told_as_it_is_to_its_owner(
    #[values(2, 3, 4)] players: usize,
    #[values(false, true)] open: bool,
) {
    let house = rules(|r| r.open_hands = open.then_some(true));
    let (before, mut driver) = begin(players, 6, house);
    // Seat 0 mulligans once and bottoms a card; everyone else keeps.
    let mut mulliganed = false;
    while driver.state().stage() == Stage::Mulligan {
        let decision = driver.decision().unwrap().clone();
        match decision.kind {
            DecisionKind::Mulligan(_) if decision.player == seat(0) && !mulliganed => {
                mulliganed = true;
                driver
                    .answer(Answer::Mulligan(MulliganChoice::Mulligan))
                    .unwrap();
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    let events = driver.engine().events();
    let bottomed = events
        .iter()
        .position(|event| matches!(event, Event::CardsBottomed { .. }))
        .expect("the mulligan costs a card");
    let taken = events
        .iter()
        .position(|event| matches!(event, Event::MulliganTaken { .. }))
        .unwrap();
    for (viewer, batch) in viewers(players)
        .into_iter()
        .zip(batches(&before, &driver, players))
    {
        let sees = open || viewer == seat(0);
        let shown = &batch.events[bottomed];
        if sees {
            assert_eq!(shown, &EventView::Public(events[bottomed].clone()));
        } else {
            assert_eq!(
                shown,
                &EventView::Hidden(HiddenEvent::CardsBottomed {
                    player: seat(0),
                    count: 1
                })
            );
        }
        // The hand shuffled into the library names no card, to anyone.
        assert_eq!(
            batch.events[taken],
            EventView::Hidden(HiddenEvent::MulliganTaken { player: seat(0) })
        );
    }
}

#[rstest]
fn a_card_drawn_is_named_to_every_seat_with_open_hands(
    #[values(3, 4)] players: usize,
    #[values(false, true)] open: bool,
) {
    let (before, mut driver) = begin(players, 2, rules(|r| r.open_hands = open.then_some(true)));
    finish_opening(&mut driver);
    driver.pass_until(Step::Main1).unwrap();
    // Three seats or more: the first player draws on turn 1.
    let events = driver.engine().events();
    let (at, drew) = events
        .iter()
        .enumerate()
        .find_map(|(at, event)| match event {
            Event::CardDrawn { player, .. } => Some((at, *player)),
            _ => None,
        })
        .expect("the first player draws");
    for (viewer, batch) in viewers(players)
        .into_iter()
        .zip(batches(&before, &driver, players))
    {
        let sees = open || viewer == drew;
        let want = if sees {
            EventView::Public(events[at].clone())
        } else {
            EventView::Hidden(HiddenEvent::CardDrawn { player: drew })
        };
        assert_eq!(batch.events[at], want, "{viewer:?}");
    }
}

// A revealed library top

#[rstest]
fn the_top_of_every_library_is_shown_to_every_seat_and_nothing_under_it(
    #[values(2, 3, 4, 5)] players: usize,
    #[values(false, true)] revealed: bool,
) {
    let house = rules(|r| r.revealed_library_top = revealed.then_some(true));
    let (_, driver) = begin(players, 5, house);
    let state = driver.state();
    for viewer in viewers(players) {
        let view = StateView::for_player(state, viewer);
        let known: BTreeSet<ObjectId> = view.objects.iter().map(|object| object.id).collect();
        for owner in 0..players {
            let library = state.zone(Zone::Library(seat(owner)));
            let top = library.last().copied();
            let shown = &view.table.players[owner];
            assert_eq!(
                shown.revealed_top,
                top.filter(|_| revealed),
                "{viewer:?}, library {owner}"
            );
            assert!(shown.library.cards.is_none());
            assert_eq!(shown.library.count as usize, library.len());
            for card in library {
                assert_eq!(
                    known.contains(card),
                    revealed && Some(*card) == top,
                    "{viewer:?} and a card of library {owner}"
                );
            }
        }
    }
}

#[rstest]
fn the_library_top_is_not_shown_before_the_first_shuffle(#[values(2, 4)] players: usize) {
    let setup =
        fuzz_setup(players, 1).with_house_rules(rules(|r| r.revealed_library_top = Some(true)));
    let state = GameState::from_setup(&setup).unwrap();
    assert_eq!(state.stage(), Stage::PreGame);
    for viewer in viewers(players) {
        let view = StateView::for_player(&state, viewer);
        assert!(view.table.players.iter().all(|p| p.revealed_top.is_none()));
        assert_eq!(view.objects.len(), 0);
    }
}

#[rstest]
fn the_new_top_is_shown_after_a_deal_a_shuffle_and_a_draw(#[values(3, 4)] players: usize) {
    let house = rules(|r| r.revealed_library_top = Some(true));
    let (before, mut driver) = begin(players, 8, house);
    // Seat 0 takes one mulligan: its hand is shuffled into the library and the next hand drawn.
    let mut mulliganed = false;
    while driver.state().stage() == Stage::Mulligan {
        let decision = driver.decision().unwrap().clone();
        if matches!(decision.kind, DecisionKind::Mulligan(_))
            && decision.player == seat(0)
            && !mulliganed
        {
            mulliganed = true;
            driver
                .answer(Answer::Mulligan(MulliganChoice::Mulligan))
                .unwrap();
        } else {
            driver.answer_passively().unwrap();
        }
    }
    driver.pass_until(Step::Main1).unwrap();
    let state = driver.state();
    for (viewer, batch) in viewers(players)
        .into_iter()
        .zip(batches(&before, &driver, players))
    {
        // The table closes on the tops of the libraries as they lie now, each seat's own included.
        for owner in 0..players {
            let top = state.zone(Zone::Library(seat(owner))).last().copied();
            assert_eq!(
                batch.table.players[owner].revealed_top, top,
                "{viewer:?}, library {owner}"
            );
            assert!(top.is_none_or(|top| named(&batch).contains(&top)));
        }
        // The card drawn this turn is in a hand the other seats cannot see into, and so gone from their views.
        let drawn = driver
            .engine()
            .events()
            .iter()
            .rev()
            .find_map(|event| match event {
                Event::CardDrawn { player, object } => Some((*player, *object)),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            batch.objects.iter().any(|object| object.id == drawn.1),
            viewer == drawn.0
        );
    }
}

// The scry of the Vancouver style

#[rstest]
fn the_card_a_seat_scries_after_a_mulligan_is_shown_to_that_seat_alone(
    #[values(2, 3, 4)] players: usize,
) {
    let house = rules(|r| r.mulligan_style = Some(MulliganStyle::Vancouver));
    let (before, mut driver) = begin(players, 7, house);
    let mut mulliganed = false;
    let mut scryed = None;
    while driver.state().stage() == Stage::Mulligan {
        let decision = driver.decision().unwrap().clone();
        match &decision.kind {
            DecisionKind::Mulligan(_) if decision.player == seat(0) && !mulliganed => {
                mulliganed = true;
                driver
                    .answer(Answer::Mulligan(MulliganChoice::Mulligan))
                    .unwrap();
            }
            DecisionKind::ChooseCards(options) => {
                assert_eq!(decision.player, seat(0));
                let top = options.from[0];
                scryed = Some(top);
                // Every seat's view while the question is open: only the asked seat names the card.
                for viewer in viewers(players) {
                    let view = StateView::for_player(driver.state(), viewer);
                    let known = view.objects.iter().any(|object| object.id == top);
                    assert_eq!(known, viewer == seat(0), "{viewer:?} and the card scried");
                }
                driver
                    .answer(Answer::ChooseCards(CardsChoice(options.from.clone())))
                    .unwrap();
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    let top = scryed.expect("a seat that kept after a mulligan scries");
    let events = driver.engine().events();
    let at = events
        .iter()
        .position(|e| matches!(e, Event::Scried(_)))
        .unwrap();
    for (viewer, batch) in viewers(players)
        .into_iter()
        .zip(batches(&before, &driver, players))
    {
        let shown = &batch.events[at];
        if viewer == seat(0) {
            assert_eq!(shown, &EventView::Public(events[at].clone()));
        } else {
            assert_eq!(
                shown,
                &EventView::Hidden(HiddenEvent::Scried {
                    player: seat(0),
                    top: 0,
                    bottom: 1
                })
            );
        }
        // The card went to the bottom, which nobody sees into.
        assert!(batch.objects.iter().all(|object| object.id != top));
    }
}

// Opening-hand smoothing

/// The two hands a seat could be dealt: the top seven of its library as the first shuffle left it, and the seven
/// below them.
fn candidate_hands(events: &[Event], seat: PlayerId) -> (Vec<ObjectId>, Vec<ObjectId>) {
    let base = events
        .iter()
        .find_map(|event| match event {
            Event::LibraryShuffled { player, order } if *player == seat => Some(order.clone()),
            _ => None,
        })
        .unwrap();
    let first = base.iter().rev().take(7).copied().collect();
    let second = base.iter().rev().skip(7).take(7).copied().collect();
    (first, second)
}

#[rstest]
fn the_hand_smoothing_does_not_keep_is_shown_to_nobody_not_even_its_owner(
    #[values(2, 3, 4, 5)] players: usize,
    #[values(1, 2, 3, 4, 5, 6)] seed: u64,
) {
    let house = rules(|r| r.opening_hand_smoothing = Some(true));
    let (before, driver) = begin(players, seed, house);
    let events = driver.engine().events();
    let all = batches(&before, &driver, players);
    for owner in 0..players {
        let (first, second) = candidate_hands(events, seat(owner));
        let dealt = driver.state().zone(Zone::Hand(seat(owner))).to_vec();
        assert_eq!(dealt.len(), 7);
        let library_len = driver.state().zone(Zone::Library(seat(owner))).len();
        let mut kept = dealt.clone();
        kept.sort_unstable();
        let sorted = |hand: &[ObjectId]| {
            let mut hand = hand.to_vec();
            hand.sort_unstable();
            hand
        };
        let unused = if kept == sorted(&first) {
            second
        } else {
            assert_eq!(
                kept,
                sorted(&second),
                "the hand dealt is one of the two drawn"
            );
            first
        };
        for (viewer, batch) in viewers(players).into_iter().zip(&all) {
            let shown = named(batch);
            // The seat sees the hand it was dealt and nobody else does.
            assert_eq!(
                dealt.iter().all(|card| shown.contains(card)),
                viewer == seat(owner)
            );
            for card in &unused {
                assert!(
                    !shown.contains(card),
                    "{viewer:?} names a card of the hand not kept"
                );
                assert!(
                    batch.events.iter().all(|event| match event {
                        EventView::Public(event) => !event.touched_objects().contains(card),
                        EventView::Hidden(_) => true,
                    }),
                    "{viewer:?} is told of a card of the hand not kept"
                );
                assert!(
                    batch
                        .table
                        .players
                        .iter()
                        .all(|p| p.revealed_top != Some(*card))
                );
                let view = StateView::for_player(driver.state(), viewer);
                assert!(view.objects.iter().all(|object| object.id != *card));
            }
            // The deal is a shuffle nobody can read and the hand dealt, a count to the others.
            let json = serde_json::to_string(&batch.events).unwrap();
            assert!(!json.contains("\"order\""), "{viewer:?} was told an order");
            let library = &batch.table.players[owner].library;
            assert_eq!(
                (library.count as usize, library.cards.is_none()),
                (library_len, true)
            );
        }
    }
}

#[rstest]
fn the_deal_of_a_smoothed_hand_is_a_shuffle_and_a_hand_to_every_seat(
    #[values(2, 4)] players: usize,
) {
    let house = rules(|r| r.opening_hand_smoothing = Some(true));
    let (before, driver) = begin(players, 3, house);
    let events = driver.engine().events();
    let deal = events
        .iter()
        .position(|event| matches!(event, Event::HandDealt { .. }))
        .unwrap();
    for (viewer, batch) in viewers(players)
        .into_iter()
        .zip(batches(&before, &driver, players))
    {
        let Event::HandDealt { player, cards } = &events[deal] else {
            unreachable!();
        };
        // Whatever the seat, the shuffle before its deal is hidden, and the deal a count unless it is theirs.
        assert!(matches!(
            batch.events[deal - 1],
            EventView::Hidden(HiddenEvent::LibraryShuffled { .. })
        ));
        if *player == viewer {
            assert_eq!(batch.events[deal], EventView::Public(events[deal].clone()));
        } else {
            assert_eq!(
                batch.events[deal],
                EventView::Hidden(HiddenEvent::HandDealt {
                    player: *player,
                    count: cards.len() as u32
                })
            );
        }
    }
}
