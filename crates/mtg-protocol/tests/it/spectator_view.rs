//! Goal: a spectator's view says it is a spectator's and a seat's does not, a spectator sees every public thing and no seat's hand or library (the hands too when the table has open hands), its batches and snapshots stay equal to the true view, and the city's blessing is public to seats and spectator alike, at 2, 3 and 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_core::house::HouseRules;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::events::Event;
use mtg_engine::state::GameState;
use mtg_engine::zones::Zone;
use mtg_protocol::{EventView, StateView, project_events, spectator_viewer};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

fn seat(index: usize) -> PlayerId {
    PlayerId::from_index(index)
}

fn card(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1),
    )
}

/// A creature with ascend, which gives its controller the city's blessing at ten permanents.
fn ascender() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Ascender")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(Ability::Keyword(Keyword::Ascend)),
    )
}

/// Every seat holds uniquely named cards in hand, library, graveyard and play; seat 0 controls ten permanents, one with ascend.
fn scenario(seats: usize, open_hands: bool) -> Scenario {
    let house = HouseRules {
        open_hands: open_hands.then_some(true),
        ..HouseRules::default()
    };
    let mut scenario = Scenario::new(seats).house_rules(house);
    for n in 0..seats {
        let mut board = vec![ready(card(&format!("Board {n}")))];
        if n == 0 {
            board = (0..9)
                .map(|extra| ready(card(&format!("Extra {extra}"))))
                .chain([ready(ascender())])
                .collect();
        }
        scenario = scenario
            .hand(n, [card(&format!("Hand {n}"))])
            .library(n, (0..3).map(|c| card(&format!("Library {n} {c}"))))
            .graveyard(n, [card(&format!("Grave {n}"))])
            .battlefield(n, board);
    }
    scenario
}

/// The game before the first engine step and a driver that ran it.
fn begin(seats: usize, open_hands: bool) -> (GameState, Driver) {
    let setup = scenario(seats, open_hands).setup().unwrap();
    let before = GameState::from_setup(&setup).unwrap();
    (
        before,
        Driver::from_scenario(scenario(seats, open_hands)).unwrap(),
    )
}

fn ids(view: &StateView) -> BTreeSet<ObjectId> {
    view.objects.iter().map(|object| object.id).collect()
}

fn json(view: &StateView) -> String {
    serde_json::to_string(view).unwrap()
}

#[rstest]
fn a_spectators_view_says_so_and_a_seats_view_does_not(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 5)] past: usize,
) {
    let (_, driver) = begin(seats, false);
    let state = driver.state();
    for n in 0..seats {
        let view = StateView::for_player(state, seat(n));
        assert!(!view.table.spectator, "seat {n}");
        assert!(
            !json(&view).contains("\"spectator\""),
            "seat {n} is told nothing about spectators"
        );
    }
    let spectator = StateView::for_spectator(state);
    assert!(spectator.table.spectator);
    assert_eq!(spectator.table.viewer, spectator_viewer(state));
    assert_eq!(spectator.table.viewer, seat(seats));
    assert!(json(&spectator).contains("\"spectator\":true"));
    // Any viewer that is no seat is a spectator, whatever number it carries.
    let other = StateView::for_player(state, seat(seats + past));
    assert!(other.table.spectator);
    assert_eq!(other.objects, spectator.objects);
    assert_eq!(other.table.players, spectator.table.players);
    let back: StateView = serde_json::from_str(&json(&spectator)).unwrap();
    assert_eq!(back, spectator);
}

#[rstest]
fn a_spectator_sees_every_public_zone_and_no_seats_hidden_cards(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] open_hands: bool,
) {
    let (_, driver) = begin(seats, open_hands);
    let state = driver.state();
    let spectator = StateView::for_spectator(state);
    let known = ids(&spectator);
    let text = json(&spectator);
    for owner in 0..seats {
        let player = &spectator.table.players[owner];
        let hand = state.zone(Zone::Hand(seat(owner))).to_vec();
        assert_eq!(player.hand.count as usize, hand.len());
        assert_eq!(player.hand.cards, open_hands.then(|| hand.clone()));
        assert_eq!(text.contains(&format!("Hand {owner}")), open_hands);
        let library = state.zone(Zone::Library(seat(owner)));
        assert_eq!(player.library.count as usize, library.len());
        assert!(player.library.cards.is_none());
        assert!(library.iter().all(|id| !known.contains(id)));
        assert!(!text.contains(&format!("Library {owner}")));
        let grave = state.zone(Zone::Graveyard(seat(owner))).to_vec();
        assert_eq!(player.graveyard.cards, Some(grave.clone()));
        assert!(grave.iter().all(|id| known.contains(id)));
    }
    let board = state.zone(Zone::Battlefield).to_vec();
    assert_eq!(spectator.table.battlefield.cards, Some(board.clone()));
    assert!(board.iter().all(|id| known.contains(id)));
    // Everything the spectator knows, every seat knows, as the seat sees it.
    for n in 0..seats {
        let view = StateView::for_player(state, seat(n));
        for object in &spectator.objects {
            assert_eq!(
                view.objects.iter().find(|o| o.id == object.id),
                Some(object),
                "seat {n} and {:?}",
                object.name
            );
        }
        let own = !open_hands;
        // A seat reads its own hand and a spectator does not, unless every hand is open.
        assert_eq!(
            ids(&view).len() > known.len(),
            own,
            "seat {n} against the spectator"
        );
    }
}

#[rstest]
fn a_spectators_batches_are_its_snapshots(#[values(2, 3, 4)] seats: usize) {
    let (before, mut driver) = begin(seats, false);
    driver.answer_passively().unwrap();
    let viewers: Vec<PlayerId> = (0..seats)
        .map(seat)
        .chain([spectator_viewer(&before)])
        .collect();
    let batches = project_events(&before, driver.engine().events(), &viewers, 0).unwrap();
    assert_eq!(batches.len(), seats + 1);
    for (n, batch) in batches.iter().enumerate() {
        assert_eq!(batch.table.spectator, n == seats, "batch {n}");
    }
    let mut folded = StateView::for_spectator(&before);
    folded.apply(&batches[seats]);
    assert_eq!(folded, StateView::for_spectator(driver.state()));
    assert!(folded.table.spectator);
    // No batch to the spectator names a hidden card.
    let text = serde_json::to_string(&batches[seats]).unwrap();
    for n in 0..seats {
        assert!(!text.contains(&format!("Hand {n}")), "hand {n}");
        assert!(!text.contains(&format!("Library {n}")), "library {n}");
    }
}

#[rstest]
fn the_citys_blessing_is_told_to_every_seat_and_a_spectator(#[values(2, 3, 4)] seats: usize) {
    let (before, driver) = begin(seats, false);
    let state = driver.state();
    assert!(state.has_citys_blessing(seat(0)));
    let views: Vec<StateView> = (0..seats)
        .map(|n| StateView::for_player(state, seat(n)))
        .chain([StateView::for_spectator(state)])
        .collect();
    for (viewer, view) in views.iter().enumerate() {
        let blessed: Vec<bool> = view
            .table
            .players
            .iter()
            .map(|player| player.citys_blessing)
            .collect();
        let mut wanted = vec![false; seats];
        wanted[0] = true;
        assert_eq!(blessed, wanted, "viewer {viewer}");
        let text = json(view);
        assert_eq!(text.matches("\"citys_blessing\":true").count(), 1);
        assert!(!text.contains("\"citys_blessing\":false"));
    }
    // Before the check that gives it, nobody has it and the field is left out.
    let early = StateView::for_spectator(&before);
    assert!(early.table.players.iter().all(|p| !p.citys_blessing));
    assert!(!json(&early).contains("citys_blessing"));
    // The event that gives it reaches every viewer whole.
    let viewers: Vec<PlayerId> = (0..=seats).map(seat).collect();
    for (viewer, batch) in viewers
        .iter()
        .zip(project_events(&before, driver.engine().events(), &viewers, 0).unwrap())
    {
        let gained = Event::CitysBlessingGained { player: seat(0) };
        assert!(
            batch.events.contains(&EventView::Public(gained)),
            "{viewer:?}"
        );
        assert!(batch.table.players[0].citys_blessing);
    }
}
