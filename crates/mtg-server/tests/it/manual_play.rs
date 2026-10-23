//! Goal: a table plays with manual cards when a seat somebody holds plays a deck with cards the game does not run whole; for rosters of two to four seats, with and without such decks and with open seats, the game browser and the lookup of a game say so, they follow a seat that changes its deck, is claimed with such a deck or is given back, and the lookup keeps the flag once the game has started.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::seat;
use crate::net;

use mtg_cards::{CardDefinition, vanilla};
use mtg_core::color::Color;
use mtg_protocol::SeatDeck;
use mtg_server::registry::{ChosenDeck, Entrance, GameOptions, SeatClaim};
use mtg_server::{GameId, Visibility};
use net::lobby::{self, call};
use net::{TestServer, bot_config};
use rstest::rstest;

/// Sixty Forests that tell the table `manual` of their cards are not run whole.
fn deck(manual: u32) -> ChosenDeck {
    let cards: Vec<Arc<CardDefinition>> =
        std::iter::repeat_n(vanilla::basic_land(Color::Green), 60).collect();
    let note = SeatDeck {
        name: format!("By hand {manual}"),
        format: "normal".to_owned(),
        manual_cards: manual,
    };
    ChosenDeck::stored(vec!["forest".to_owned(); 60], cards, note).unwrap()
}

/// A public game with a seat for each entry of `manual`, the first `held` of them held, whose seats start
/// with decks holding that many manual cards.
async fn make(server: &TestServer, manual: &[u32], held: usize) -> GameId {
    let names = (0..manual.len()).map(|n| format!("Player {n}")).collect();
    let decks = manual.iter().map(|count| Some(deck(*count))).collect();
    let options = GameOptions {
        visibility: Visibility::Public,
        held: Some(held),
        ..GameOptions::default()
    };
    let created = server
        .registry
        .create_decked(names, decks, options, None)
        .await
        .unwrap();
    created.id
}

/// What the game browser says of `game`: `None` while it is not listed.
async fn listed(server: &TestServer, game: GameId) -> Option<bool> {
    let reply = lobby::lobbies(server.addr, &[]).await;
    assert_eq!(reply.status, 200, "{}", reply.text());
    let id = game.to_string();
    let json = reply.json();
    let row = json["games"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["game"] == id.as_str())?
        .clone();
    let flag = row["manual_play"].as_bool().expect("a listed row says");
    let registry = server.registry.lobbies().await;
    let own = registry.iter().find(|row| row.game == game).unwrap();
    assert_eq!(own.manual_play, flag, "the registry and the wire agree");
    Some(flag)
}

/// What `GET /api/games/{id}` says of `game`.
async fn looked_up(server: &TestServer, game: GameId) -> bool {
    let reply = call(server.addr, "GET", &format!("/api/games/{game}"), None, &[]).await;
    assert_eq!(reply.status, 200, "{}", reply.text());
    reply.json()["manual_play"]
        .as_bool()
        .expect("a lookup says")
}

/// Both answers for a game in its lobby, which must agree.
async fn flag(server: &TestServer, game: GameId) -> bool {
    let shown = listed(server, game).await.expect("the game is listed");
    assert_eq!(looked_up(server, game).await, shown);
    shown
}

#[rstest]
#[case::two_seats_without(&[0, 0], 2, false)]
#[case::two_seats_one_with(&[0, 2], 2, true)]
#[case::the_host_alone_holds_it(&[3, 0], 1, true)]
#[case::the_host_alone_holds_none(&[0, 0], 1, false)]
#[case::three_seats_without(&[0, 0, 0], 3, false)]
#[case::three_seats_the_middle_one(&[0, 4, 0], 3, true)]
#[case::an_open_seat_does_not_count(&[0, 0, 5], 2, false)]
#[case::four_seats_without(&[0, 0, 0, 0], 4, false)]
#[case::four_seats_all_with(&[1, 1, 1, 1], 4, true)]
#[case::four_seats_the_last_one_open(&[0, 0, 0, 9], 3, false)]
#[case::four_seats_the_last_one_held(&[0, 0, 0, 9], 4, true)]
#[case::a_single_manual_card(&[0, 1, 0, 0], 4, true)]
#[tokio::test]
async fn the_browser_and_the_lookup_say_whether_a_held_seat_plays_manual_cards(
    #[case] manual: &[u32],
    #[case] held: usize,
    #[case] expected: bool,
) {
    let server = TestServer::start(bot_config()).await;
    let game = make(&server, manual, held).await;
    assert_eq!(flag(&server, game).await, expected);
}

#[tokio::test]
async fn the_flag_follows_the_decks_as_seats_change_them_arrive_and_leave() {
    let server = TestServer::start(bot_config()).await;
    let registry = &server.registry;
    let game = make(&server, &[0, 0, 0], 1).await;
    assert!(!flag(&server, game).await);

    // A joiner who brings such a deck makes the table play by hand, and who leaves takes it away again.
    let claim = SeatClaim {
        name: "Bo".to_owned(),
        user: None,
    };
    let bo = registry
        .claim_with(game, Entrance::Code, claim, Some(deck(2)))
        .await
        .unwrap()
        .seat;
    assert!(flag(&server, game).await);
    registry.release(game, bo, bo).await.unwrap();
    assert!(!flag(&server, game).await, "the seat went back to its deck");

    // A seat that chooses such a deck, and then another, moves it either way.
    let claim = SeatClaim {
        name: "Cy".to_owned(),
        user: None,
    };
    let cy = registry
        .claim(game, Entrance::Code, claim)
        .await
        .unwrap()
        .seat;
    for (count, expected) in [(5, true), (0, false), (1, true)] {
        registry
            .choose_deck(game, cy, cy, deck(count))
            .await
            .unwrap();
        assert_eq!(flag(&server, game).await, expected, "{count} manual cards");
    }

    // The host's own deck counts like anybody's, and one clean deck among them changes nothing.
    registry
        .choose_deck(game, seat(0), seat(0), deck(7))
        .await
        .unwrap();
    registry.choose_deck(game, cy, cy, deck(0)).await.unwrap();
    assert!(flag(&server, game).await);
    registry
        .choose_deck(game, seat(0), seat(0), deck(0))
        .await
        .unwrap();
    assert!(!flag(&server, game).await);
}

#[rstest]
#[case::two_seats_without(&[0, 0], false)]
#[case::two_seats_one_with(&[0, 2], true)]
#[case::three_seats_the_first_with(&[1, 0, 0], true)]
#[case::three_seats_without(&[0, 0, 0], false)]
#[case::four_seats_the_last_with(&[0, 0, 0, 3], true)]
#[case::four_seats_without(&[0, 0, 0, 0], false)]
#[tokio::test]
async fn a_game_that_has_started_keeps_the_flag_it_started_with(
    #[case] manual: &[u32],
    #[case] expected: bool,
) {
    let server = TestServer::start(bot_config()).await;
    let game = make(&server, manual, manual.len()).await;
    assert_eq!(flag(&server, game).await, expected);

    lobby::start(&server.registry, &game.to_string(), manual.len()).await;
    assert_eq!(
        listed(&server, game).await,
        None,
        "a started game is not listed"
    );
    assert_eq!(looked_up(&server, game).await, expected);
    // Asking again, as the page does, says the same.
    assert_eq!(looked_up(&server, game).await, expected);
}
