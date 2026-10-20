//! Goal: over real HTTP and sockets the holder of a seat chooses the deck it plays, as the host, as a guest and when joining, and changes it until the seat is ready; Ready is refused without a deck; a stranger, another seat and a deck nobody has are refused alike; a player who leaves takes the choice with them; a game made with a deck for some seats and none for others says which; and neither the roster nor any answer names a card of any deck.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod net;

use mtg_protocol::{ClientMessage, ErrorCode, LobbySeat, ServerMessage};
use mtg_server::demo;
use net::lobby::{self, Made, bearer, call, join};
use net::{TestServer, WsClient, bot_config, connect};
use serde_json::{Value, json};

/// The next roster a socket is sent, after any welcome.
async fn roster(socket: &mut WsClient) -> Vec<LobbySeat> {
    loop {
        match socket.message().await {
            ServerMessage::Lobby { seats, .. } => return seats,
            ServerMessage::Welcome { .. } => {}
            other => panic!("expected a roster, got {other:?}"),
        }
    }
}

/// The name of each seat's deck, `None` for a seat without one.
fn deck_names(seats: &[LobbySeat]) -> Vec<Option<&str>> {
    seats
        .iter()
        .map(|seat| seat.deck.as_ref().map(|deck| deck.name.as_str()))
        .collect()
}

async fn put_deck(
    server: &TestServer,
    game: &str,
    seat: &str,
    token: &str,
    deck: &str,
) -> net::Reply {
    let body = json!({ "deck": deck }).to_string();
    call(
        server.addr,
        "PUT",
        &format!("/api/games/{game}/seats/{seat}/deck"),
        Some(&body),
        &[("Authorization", &bearer(token))],
    )
    .await
}

async fn seated(server: &TestServer, made: &Made, seat: usize) -> WsClient {
    let mut socket = connect(server.addr, &made.id, &made.tokens[seat]).await;
    assert_eq!(socket.welcome(&made.id).await.index(), seat);
    socket
}

/// A guest's token from a successful join.
fn token_of(reply: &net::Reply) -> String {
    assert_eq!(reply.status, 200, "{}", reply.text());
    reply.json()["token"].as_str().unwrap().to_owned()
}

/// Every card name of every built-in deck.
fn card_names() -> Vec<String> {
    demo::all()
        .iter()
        .flat_map(|deck| deck.list().into_iter().map(|(name, _)| name))
        .collect()
}

fn assert_names_no_card(texts: &[String]) {
    for text in texts {
        for name in card_names() {
            assert!(!text.contains(&name), "{name} in {text}");
        }
    }
}

#[tokio::test]
async fn a_host_and_a_guest_choose_and_change_their_decks_and_the_table_hears_each_choice() {
    let server = TestServer::start(bot_config()).await;
    let made = lobby::make(server.addr, "Ann", 2, 1, "public").await;
    let mut seen: Vec<String> = Vec::new();
    let mut host = seated(&server, &made, 0).await;
    let first = roster(&mut host).await;
    assert_eq!(deck_names(&first), [Some("Mono Green Stompers"), None]);

    // A guest joins bringing a deck of their own choice.
    let joined = join(
        server.addr,
        &made.id,
        &json!({ "name": "Bo", "deck": "mono-white" }),
        &[],
    )
    .await;
    seen.push(joined.text());
    let bo = token_of(&joined);
    let told = roster(&mut host).await;
    assert_eq!(
        deck_names(&told),
        [Some("Mono Green Stompers"), Some("Mono White Weenies")]
    );
    let deck = told[1].deck.as_ref().unwrap();
    assert_eq!((deck.format.as_str(), deck.manual_cards), ("normal", 0));

    // The guest changes it, and so does the host; the other seat hears each time.
    let mut guest = connect(server.addr, &made.id, &bo).await;
    guest.welcome(&made.id).await;
    assert_eq!(
        deck_names(&roster(&mut guest).await),
        [Some("Mono Green Stompers"), Some("Mono White Weenies")]
    );
    assert!(roster(&mut host).await[1].connected);
    let changed = put_deck(&server, &made.id, "1", &bo, "mono-red").await;
    seen.push(changed.text());
    assert_eq!(changed.status, 200, "{}", changed.text());
    assert_eq!(
        changed.json(),
        json!({ "name": "Mono Red Giants", "format": "normal", "manual_cards": 0 })
    );
    let after_guest = roster(&mut host).await;
    assert_eq!(
        deck_names(&after_guest),
        [Some("Mono Green Stompers"), Some("Mono Red Giants")]
    );
    assert_eq!(roster(&mut guest).await, after_guest);
    let by_host = put_deck(&server, &made.id, "0", made.host(), "mono-white").await;
    assert_eq!(by_host.status, 200, "{}", by_host.text());
    let after_host = roster(&mut guest).await;
    assert_eq!(
        deck_names(&after_host),
        [Some("Mono White Weenies"), Some("Mono Red Giants")]
    );
    assert_eq!(roster(&mut host).await, after_host);

    // The listing, the lookup and the roster carry no card of any deck.
    for path in [format!("/api/games/{}", made.id), "/api/lobbies".to_owned()] {
        seen.push(call(server.addr, "GET", &path, None, &[]).await.text());
    }
    seen.push(serde_json::to_string(&after_host).unwrap());
    assert_names_no_card(&seen);
}

#[tokio::test]
async fn ready_is_refused_without_a_deck_and_a_ready_seat_keeps_its_deck_until_it_takes_ready_back()
{
    let server = TestServer::start(bot_config()).await;
    let seats = json!([
        { "name": "Ann", "deck": "mono-green" },
        { "name": "Bo" },
    ]);
    let body = json!({ "format": "normal", "seats": seats });
    let made = net::create_game(server.addr, body).await;
    let made = Made {
        id: made.0,
        code: String::new(),
        tokens: made.1,
    };
    let mut ann = seated(&server, &made, 0).await;
    let mut bo = seated(&server, &made, 1).await;
    let table = roster(&mut ann).await;
    assert_eq!(deck_names(&table), [Some("Mono Green Stompers"), None]);
    roster(&mut bo).await;

    bo.send(&ClientMessage::Ready).await;
    loop {
        match bo.message().await {
            ServerMessage::Error { code, message } => {
                assert_eq!(code, ErrorCode::BadMessage);
                assert!(message.contains("choose a deck"), "{message}");
                break;
            }
            ServerMessage::Lobby { .. } => {}
            other => panic!("expected the refusal, got {other:?}"),
        }
    }
    let handle = server.registry.handle(made.id.parse().unwrap()).unwrap();
    assert!(handle.status().await.unwrap().ready.is_empty());

    // With a deck the same Ready counts; then the deck is fixed until Ready is taken back.
    assert_eq!(
        put_deck(&server, &made.id, "1", &made.tokens[1], "mono-red")
            .await
            .status,
        200
    );
    bo.send(&ClientMessage::Ready).await;
    assert!(
        loop {
            let seats = roster(&mut bo).await;
            if seats[1].ready {
                break seats[1].deck.is_some();
            }
        },
        "a ready seat shows its deck"
    );
    let refused = put_deck(&server, &made.id, "1", &made.tokens[1], "mono-white").await;
    assert_eq!(refused.status, 409);
    assert_eq!(refused.error_code(), "seat_ready");
    bo.send(&ClientMessage::NotReady).await;
    loop {
        if !roster(&mut bo).await[1].ready {
            break;
        }
    }
    let changed = put_deck(&server, &made.id, "1", &made.tokens[1], "mono-white").await;
    assert_eq!(changed.status, 200, "{}", changed.text());

    // Once everybody is ready the game starts, and the decks no longer change.
    for seat in [&mut ann, &mut bo] {
        seat.send(&ClientMessage::Ready).await;
    }
    for _ in 0..200 {
        if handle.status().await.unwrap().started {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(handle.status().await.unwrap().started);
    let late = put_deck(&server, &made.id, "1", &made.tokens[1], "mono-red").await;
    assert_eq!((late.status, late.error_code().as_str()), (409, "started"));
}

#[tokio::test]
async fn only_the_holder_of_a_seat_chooses_its_deck_and_a_deck_nobody_has_is_one_answer() {
    let server = TestServer::start(bot_config()).await;
    let made = lobby::make(server.addr, "Ann", 3, 3, "invite").await;
    let other = lobby::make(server.addr, "Cy", 2, 2, "invite").await;
    let path = |game: &str, seat: &str| format!("/api/games/{game}/seats/{seat}/deck");
    let body = json!({ "deck": "mono-red" }).to_string();

    // Whoever holds no seat of the game is a stranger: one 404, the one of an unknown game.
    let unknown_game = put_deck(
        &server,
        "6f1e0c1a-3b1f-4d9e-8a3e-0d3c2d1b7a55",
        "0",
        made.host(),
        "mono-red",
    )
    .await;
    for token in [&other.tokens[0], &"0".repeat(64)] {
        let reply = put_deck(&server, &made.id, "0", token, "mono-red").await;
        assert_eq!(reply.status, 404, "{}", reply.text());
        assert_eq!(reply.text(), unknown_game.text());
    }
    let nobody = call(server.addr, "PUT", &path(&made.id, "0"), Some(&body), &[]).await;
    assert_eq!(
        nobody.status,
        503,
        "no credential and no accounts: {}",
        nobody.text()
    );

    // A holder chooses for their own seat only.
    let elsewhere = put_deck(&server, &made.id, "2", &made.tokens[1], "mono-red").await;
    assert_eq!(
        (elsewhere.status, elsewhere.error_code().as_str()),
        (403, "forbidden")
    );
    for seat in ["01", "12", "-1", "x", ""] {
        let reply = put_deck(&server, &made.id, seat, made.host(), "mono-red").await;
        assert_eq!(reply.status, 404, "seat {seat:?}");
    }

    // A deck that is no built-in deck and no stored deck of the caller's is one 404, whatever it looks like.
    let nothing = put_deck(&server, &made.id, "0", made.host(), "mono-purple").await;
    assert_eq!(
        (nothing.status, nothing.error_code().as_str()),
        (404, "unknown_deck")
    );
    for id in [
        "4457ed35-7c10-48c8-9776-456485fdf070",
        "MONO-RED",
        "",
        "../../x",
    ] {
        let reply = put_deck(&server, &made.id, "0", made.host(), id).await;
        assert_eq!(reply.status, 404, "{id:?}");
        assert_eq!(reply.text(), nothing.text(), "{id:?}");
    }

    // The body is judged as a body.
    for bad in [
        "{}",
        r#"{"deck": 3}"#,
        r#"{"deck":"mono-red","seat":1}"#,
        "[]",
        "",
    ] {
        let headers = [("Authorization", bearer(made.host()))];
        let headers: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let reply = call(
            server.addr,
            "PUT",
            &path(&made.id, "0"),
            Some(bad),
            &headers,
        )
        .await;
        assert_eq!(reply.status, 400, "{bad:?}: {}", reply.text());
    }
    // Nothing above changed a deck.
    assert_eq!(
        put_deck(&server, &made.id, "0", made.host(), "mono-white")
            .await
            .status,
        200
    );
}

#[tokio::test]
async fn a_player_who_leaves_takes_their_deck_with_them_and_the_seat_returns_to_the_one_it_was_made_with()
 {
    let server = TestServer::start(bot_config()).await;
    let made = lobby::make(server.addr, "Ann", 2, 1, "public").await;
    let mut host = seated(&server, &made, 0).await;
    roster(&mut host).await;

    let bo = token_of(
        &join(
            server.addr,
            &made.id,
            &json!({ "name": "Bo", "deck": "mono-white" }),
            &[],
        )
        .await,
    );
    assert_eq!(
        deck_names(&roster(&mut host).await),
        [Some("Mono Green Stompers"), Some("Mono White Weenies")]
    );
    let left = call(
        server.addr,
        "POST",
        &format!("/api/games/{}/leave", made.id),
        None,
        &[("Authorization", &bearer(&bo))],
    )
    .await;
    assert_eq!(left.status, 204);
    // An open seat is listed without a deck.
    assert_eq!(
        deck_names(&roster(&mut host).await),
        [Some("Mono Green Stompers"), None]
    );

    // Whoever takes it next gets the deck the game was made with, never the last player's.
    let cy = join(server.addr, &made.id, &json!({ "name": "Cy" }), &[]).await;
    assert_eq!(cy.status, 200);
    assert_eq!(
        deck_names(&roster(&mut host).await),
        [Some("Mono Green Stompers"), Some("Mono Green Stompers")]
    );
    // Bo's old token opens nothing, so it cannot choose a deck either.
    assert_eq!(
        put_deck(&server, &made.id, "1", &bo, "mono-red")
            .await
            .status,
        404
    );
}

#[tokio::test]
async fn a_game_is_made_with_a_deck_for_some_seats_and_none_for_others_and_a_bad_deck_makes_nothing()
 {
    let server = TestServer::start(bot_config()).await;
    let seats = json!([
        { "name": "Ann", "deck": "mono-red" },
        { "name": "Player 2" },
        { "name": "Player 3", "deck": "mono-white" },
    ]);
    let body = json!({ "format": "normal", "visibility": "public", "held": 1, "seats": seats });
    let created = call(
        server.addr,
        "POST",
        "/api/games",
        Some(&body.to_string()),
        &[],
    )
    .await;
    assert_eq!(created.status, 201, "{}", created.text());
    let id = created.json()["game"].as_str().unwrap().to_owned();
    let token = created.json()["seats"][0]["token"]
        .as_str()
        .unwrap()
        .to_owned();
    let made = Made {
        id: id.clone(),
        code: String::new(),
        tokens: vec![token],
    };
    let mut host = seated(&server, &made, 0).await;
    // Open seats are listed without a deck; the creator's has the one it asked for.
    assert_eq!(
        deck_names(&roster(&mut host).await),
        [Some("Mono Red Giants"), None, None]
    );

    // A guest who joins the seat made without a deck has none and cannot be ready; the other open seat keeps its own.
    let bo = token_of(&join(server.addr, &id, &json!({ "name": "Bo" }), &[]).await);
    assert_eq!(
        deck_names(&roster(&mut host).await),
        [Some("Mono Red Giants"), None, None]
    );
    let cy = token_of(&join(server.addr, &id, &json!({ "name": "Cy" }), &[]).await);
    assert_eq!(
        deck_names(&roster(&mut host).await),
        [Some("Mono Red Giants"), None, Some("Mono White Weenies")]
    );
    assert_eq!(
        put_deck(&server, &id, "1", &bo, "mono-green").await.status,
        200
    );
    assert_eq!(
        put_deck(&server, &id, "2", &cy, "mono-green").await.status,
        200
    );

    // A game is not made for a deck nobody has: the deck of a stored id, an unknown id, another seat's stored deck.
    for (seat, deck) in [
        (0, "nope"),
        (0, "4457ed35-7c10-48c8-9776-456485fdf070"),
        (1, "4457ed35-7c10-48c8-9776-456485fdf070"),
    ] {
        let mut wrong = body.clone();
        wrong["seats"][seat]["deck"] = json!(deck);
        let reply = call(
            server.addr,
            "POST",
            "/api/games",
            Some(&wrong.to_string()),
            &[],
        )
        .await;
        assert_eq!(reply.status, 400, "{deck}: {}", reply.text());
        assert_eq!(reply.error_code(), "unknown_deck");
    }
    assert_eq!(
        server.registry.len(),
        1,
        "a refused request creates nothing"
    );

    // A join that brings a deck nobody has claims nothing.
    let bad = join(
        server.addr,
        &id,
        &json!({ "name": "Di", "deck": "nope" }),
        &[],
    )
    .await;
    assert_eq!(
        (bad.status, bad.error_code().as_str()),
        (404, "unknown_deck")
    );
    let none: Value = call(server.addr, "GET", &format!("/api/games/{id}"), None, &[])
        .await
        .json();
    assert_eq!(none["seats"].as_array().unwrap().len(), 3);
}
