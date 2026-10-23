//! Goal: a lobby plays the companions its seats chose: a deck that names one puts it in the game at two, three and four seats while a seat without one has none, the lobby and the lookups never say that a companion exists, the opening reveals each one to every seat and the snapshot names it, and a seat that changes its deck or leaves takes the companion with it and gives the next player the deck the game was made with.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::Client;
use crate::net;

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_engine::companions::CompanionStage;
use mtg_engine::rng::GameRng;
use mtg_protocol::{SeatDeck, ServerMessage};
use mtg_server::registry::{ChosenDeck, Entrance, GameOptions, SeatClaim};
use mtg_server::{GameId, Visibility};
use mtg_testkit::companion_cards::{small_companion, small_deck};
use net::lobby::{self, call};
use net::{TestServer, bot_config};
use rstest::rstest;

const NAME: &str = "Small Companion";

/// Sixty small permanents, with the small companion when `companion` is set.
fn deck(companion: bool) -> ChosenDeck {
    let cards: Vec<Arc<CardDefinition>> = small_deck(&mut GameRng::substream(4, 4));
    let ids = cards.iter().map(|card| card.name.clone()).collect();
    let note = SeatDeck {
        name: "Small Permanents".to_owned(),
        format: "normal".to_owned(),
        manual_cards: 0,
    };
    let chosen = ChosenDeck::stored(ids, cards, note).unwrap();
    if companion {
        chosen
            .with_companion("small-companion".to_owned(), small_companion())
            .unwrap()
    } else {
        chosen
    }
}

async fn make(server: &TestServer, decks: Vec<Option<ChosenDeck>>, held: usize) -> GameId {
    let names = (0..decks.len()).map(|n| format!("Player {n}")).collect();
    let options = GameOptions {
        visibility: Visibility::Public,
        held: Some(held),
        ..GameOptions::default()
    };
    server
        .registry
        .create_decked(names, decks, options, None)
        .await
        .unwrap()
        .id
}

fn text_of(client: &mut Client) -> String {
    client
        .drain()
        .iter()
        .map(|message| serde_json::to_string(message).unwrap())
        .collect()
}

/// The companion of each seat in the snapshot a new connection of `seat` is sent.
async fn companions_seen_by(
    server: &TestServer,
    game: GameId,
    seat: usize,
) -> Vec<Option<(CompanionStage, Option<String>)>> {
    let handle = server.registry.handle(game).unwrap();
    let mut client = Client::join(&handle, seat).await;
    let state = client
        .until(|message| match message {
            ServerMessage::Snapshot { state, .. } => Some(state.clone()),
            _ => None,
        })
        .await;
    state
        .table
        .players
        .iter()
        .map(|player| {
            player
                .companion
                .as_ref()
                .map(|companion| (companion.stage, companion.name.clone()))
        })
        .collect()
}

#[rstest]
#[tokio::test]
async fn the_opening_reveals_each_companion_to_every_seat_and_the_lobby_never_names_one(
    #[values(2, 3, 4)] seats: usize,
) {
    let server = TestServer::start(bot_config()).await;
    let named: Vec<bool> = (0..seats).map(|seat| seat + 1 < seats).collect();
    let decks = named.iter().map(|with| Some(deck(*with))).collect();
    let game = make(&server, decks, seats).await;
    let handle = server.registry.handle(game).unwrap();

    let mut clients = Vec::new();
    for seat in 0..seats {
        clients.push(Client::join(&handle, seat).await);
    }
    let mut lobby_texts: Vec<String> = clients.iter_mut().map(text_of).collect();
    for path in [format!("/api/games/{game}"), "/api/lobbies".to_owned()] {
        lobby_texts.push(call(server.addr, "GET", &path, None, &[]).await.text());
    }
    for text in &lobby_texts {
        assert!(!text.to_lowercase().contains("companion"), "{text}");
    }

    lobby::start(&server.registry, &game.to_string(), seats).await;
    let reveals = named.iter().filter(|with| **with).count();
    for client in &mut clients {
        let batches: Vec<_> = client
            .drain()
            .into_iter()
            .filter_map(|message| match message {
                ServerMessage::Events(batch) => Some(batch),
                _ => None,
            })
            .collect();
        let events = serde_json::to_string(
            &batches
                .iter()
                .flat_map(|batch| batch.events.iter())
                .collect::<Vec<_>>(),
        )
        .unwrap();
        assert_eq!(events.matches("CompanionRevealed").count(), reveals);
        assert!(!events.contains(NAME), "a reveal event names no card");
        // The table after the opening shows every companion, revealed, with its name.
        let table = &batches.last().expect("the opening was sent").table;
        let shown: Vec<_> = table
            .players
            .iter()
            .map(|player| {
                player
                    .companion
                    .as_ref()
                    .map(|c| (c.stage, c.name.as_deref()))
            })
            .collect();
        let expected: Vec<_> = named
            .iter()
            .map(|with| with.then_some((CompanionStage::Revealed, Some(NAME))))
            .collect();
        assert_eq!(shown, expected);
    }
    for viewer in 0..seats {
        let seen = companions_seen_by(&server, game, viewer).await;
        let expected: Vec<_> = named
            .iter()
            .map(|with| with.then(|| (CompanionStage::Revealed, Some(NAME.to_owned()))))
            .collect();
        assert_eq!(seen, expected, "as seat {viewer} sees them");
    }
}

#[derive(Debug, Clone, Copy)]
enum Step {
    /// The seat chooses a deck, with or without a companion.
    Choose(bool),
    /// The seat is given back and a new player claims it, bringing a deck.
    Replace(bool),
    /// The seat is given back and a new player claims it with the deck the game was made with.
    GiveBack,
}

#[rstest]
#[case::the_deck_it_was_made_with(&[], false)]
#[case::a_joiner_brings_a_companion(&[Step::Replace(true)], true)]
#[case::a_choice_adds_one(&[Step::Choose(true)], true)]
#[case::a_choice_takes_it_away(&[Step::Replace(true), Step::Choose(false)], false)]
#[case::a_second_choice_puts_it_back(&[Step::Choose(true), Step::Choose(false), Step::Choose(true)], true)]
#[case::the_player_who_left_takes_it_along(&[Step::Replace(true), Step::GiveBack], false)]
#[case::the_next_player_chooses_another_deck(&[Step::Replace(true), Step::GiveBack, Step::Choose(false)], false)]
#[case::and_a_third_brings_one_again(&[Step::Replace(false), Step::Replace(true)], true)]
#[tokio::test]
async fn a_seat_that_changes_its_deck_or_leaves_takes_its_companion_with_it(
    #[case] steps: &[Step],
    #[case] companion: bool,
    #[values(2, 3, 4)] seats: usize,
) {
    let server = TestServer::start(bot_config()).await;
    let registry = &server.registry;
    // Seat 0 is held with a companion; the other seats are open, made with a deck without one.
    let mut decks = vec![Some(deck(true))];
    decks.extend((1..seats).map(|_| Some(deck(false))));
    let game = make(&server, decks, 1).await;
    let claim = |name: &str| SeatClaim {
        name: name.to_owned(),
        user: None,
    };
    let mut seated = Vec::new();
    for seat in 1..seats {
        let joined = registry
            .claim(game, Entrance::Code, claim(&format!("Guest {seat}")))
            .await
            .unwrap();
        seated.push(joined.seat);
    }
    let subject = seated[0];
    for (n, step) in steps.iter().enumerate() {
        match *step {
            Step::Choose(with) => {
                registry
                    .choose_deck(game, subject, subject, deck(with))
                    .await
                    .unwrap();
            }
            Step::Replace(with) => {
                registry.release(game, subject, subject).await.unwrap();
                let again = claim(&format!("Next {n}"));
                let taken = registry
                    .claim_with(game, Entrance::Code, again, Some(deck(with)))
                    .await
                    .unwrap();
                assert_eq!(taken.seat, subject);
            }
            Step::GiveBack => {
                registry.release(game, subject, subject).await.unwrap();
                let again = claim(&format!("Next {n}"));
                let taken = registry.claim(game, Entrance::Code, again).await.unwrap();
                assert_eq!(taken.seat, subject);
            }
        }
    }

    lobby::start(registry, &game.to_string(), seats).await;
    let seen = companions_seen_by(&server, game, 0).await;
    assert_eq!(seen.len(), seats);
    assert!(seen[0].is_some(), "the host's companion is there");
    assert_eq!(seen[subject.index()].is_some(), companion, "{steps:?}");
    for (seat, view) in seen.iter().enumerate().skip(1) {
        if seat != subject.index() {
            assert!(view.is_none(), "seat {seat} named none");
        }
    }
}
