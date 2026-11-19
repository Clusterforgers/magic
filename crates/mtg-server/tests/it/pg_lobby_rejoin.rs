//! Goal: a signed-in player who holds a seat of a lobby that bots filled, and who left the page without pressing Ready, always finds it again: in every kind of lobby (a duel, a free-for-all, a team game, Commander) the list marks it as theirs while it shows full to everyone else, a join by a seated player hands back their own seat without a new one, a stranger is still turned away, leaving frees the seat, and a Delves party takes a member who joins again only once.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::net;
use crate::pg_commander::{RED, create_body, led_by, on, start, store};

use net::lobby::call;
use net::{Browser, PASSWORD, TestServer, register};
use serde_json::{Value, json};
use sqlx::PgPool;

/// A lobby kind: its format, its seats (`None` an open seat, `Some(true)` a bot, `Some(false)` the host) and its
/// teams.
#[derive(Debug, Clone, Copy)]
struct Kind {
    commander: bool,
    seats: &'static [Option<bool>],
    teams: Option<&'static [u32]>,
}

const HOST: Option<bool> = Some(false);
const BOT: Option<bool> = Some(true);
const OPEN: Option<bool> = None;

/// The creation request of `kind`: `deck` for the host and the open seats, which come last, and a precon for the bots.
fn creation(kind: Kind, deck: &str) -> Value {
    let seats: Vec<Value> = kind
        .seats
        .iter()
        .map(|seat| match seat {
            Some(false) => json!({ "name": "ann", "deck": deck }),
            Some(true) => json!({ "deck": on(RED), "bot": true }),
            None => json!({}),
        })
        .collect();
    let held = kind.seats.iter().filter(|seat| seat.is_some()).count();
    let mut body = if kind.commander {
        create_body(&seats, held)
    } else {
        json!({ "format": "normal", "visibility": "public", "held": held, "seats": seats })
    };
    if let Some(teams) = kind.teams {
        body["house_rules"] = json!({ "teams": { "mode": "teams", "seats": teams } });
    }
    body
}

/// The listed row of `game` as `who` (a guest when `None`) sees it.
async fn row(server: &TestServer, who: Option<&Browser>, game: &str) -> Value {
    let reply = match who {
        Some(who) => who.get(server.addr, "/api/lobbies").await,
        None => call(server.addr, "GET", "/api/lobbies", None, &[]).await,
    };
    assert_eq!(reply.status, 200, "{}", reply.text());
    reply.json()["games"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["game"] == game)
        .cloned()
        .unwrap_or_else(|| panic!("{game} is not listed"))
}

fn full(row: &Value) -> bool {
    row["filled"] == row["total"]
}

fn seated(row: &Value) -> bool {
    row.get("seated") == Some(&json!(true))
}

async fn join(server: &TestServer, who: &Browser, game: &str, deck: Option<&str>) -> net::Reply {
    let body = deck.map_or_else(|| json!({}), |deck| json!({ "deck": deck }));
    who.post(
        server.addr,
        &format!("/api/games/{game}/join"),
        &body.to_string(),
    )
    .await
}

/// The lobby kinds: a duel against a bot, a free-for-all, a team game and Commander, each filled by bots and, where
/// a seat is open, by Bob.
const KINDS: [(&str, Kind); 4] = [
    (
        "a duel against a bot",
        Kind {
            commander: false,
            seats: &[HOST, BOT],
            teams: None,
        },
    ),
    (
        "a free-for-all",
        Kind {
            commander: false,
            seats: &[HOST, BOT, BOT, OPEN],
            teams: None,
        },
    ),
    (
        "a team game",
        Kind {
            commander: false,
            seats: &[HOST, BOT, BOT, OPEN],
            teams: Some(&[0, 1, 1, 0]),
        },
    ),
    (
        "Commander",
        Kind {
            commander: true,
            seats: &[HOST, BOT, BOT, OPEN],
            teams: None,
        },
    ),
];

/// The players and, for Commander, their decks.
struct Players {
    ann: Browser,
    bob: Browser,
    cyd: Browser,
    decks: [String; 3],
}

#[sqlx::test(migrations = false)]
async fn a_seated_player_finds_and_reopens_a_lobby_bots_filled(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let bob = register(server.addr, "bob", PASSWORD).await;
    let cyd = register(server.addr, "cyd", PASSWORD).await;
    let decks = [
        store(&server, &ann, &led_by("Questing Beast")).await,
        store(&server, &bob, &led_by("Rosheen Meanderer")).await,
        store(&server, &cyd, &led_by("Questing Beast")).await,
    ];
    let players = Players {
        ann,
        bob,
        cyd,
        decks,
    };
    for (name, kind) in KINDS {
        eprintln!("{name}");
        rejoin_case(&server, &players, kind).await;
    }
}

async fn rejoin_case(server: &TestServer, players: &Players, kind: Kind) {
    let Players {
        ann,
        bob,
        cyd,
        decks,
    } = players;
    let deck = |n: usize| kind.commander.then(|| decks[n].as_str());
    let (ann_deck, bob_deck, cyd_deck) = (deck(0), deck(1), deck(2));
    let precon = on(RED);
    let body = creation(kind, ann_deck.unwrap_or(&precon));
    let (game, _) = ann.create_game(server.addr, &body).await;
    let open = kind.seats.iter().filter(|seat| seat.is_none()).count();
    if open > 0 {
        let joined = join(server, bob, &game, bob_deck).await;
        assert_eq!(joined.status, 200, "{}", joined.text());
    }

    // Everybody in it sees the lobby as theirs; it is full to everyone else.
    for who in [Some(ann), Some(bob), Some(cyd), None] {
        let listed = row(server, who, &game).await;
        assert!(full(&listed), "{listed}");
        let inside = who.is_some_and(|w| w.username == "ann" || (open > 0 && w.username == "bob"));
        assert_eq!(
            seated(&listed),
            inside,
            "{:?}: {listed}",
            who.map(|w| &w.username)
        );
    }

    // Coming back after closing the page: a join hands back the own seat, with no new token.
    let back = join(server, ann, &game, None).await;
    assert_eq!(back.status, 200, "{}", back.text());
    assert_eq!(
        (back.json()["seat"].clone(), back.json()["token"].clone()),
        (json!(0), Value::Null)
    );
    if open > 0 {
        let back = join(server, bob, &game, None).await;
        assert_eq!(back.status, 200, "{}", back.text());
        let seat = kind.seats.len() - 1;
        assert_eq!(
            (back.json()["seat"].clone(), back.json()["token"].clone()),
            (json!(seat), Value::Null)
        );
    }
    let stranger = join(server, cyd, &game, cyd_deck).await;
    assert_eq!(
        (stranger.status, stranger.error_code().as_str()),
        (409, "full")
    );

    // Leaving gives the seat back: the lobby has room again and is no longer Bob's.
    if open > 0 {
        let left = bob
            .post(server.addr, &format!("/api/games/{game}/leave"), "")
            .await;
        assert_eq!(left.status, 204, "{}", left.text());
        let listed = row(server, Some(bob), &game).await;
        assert!(!full(&listed) && !seated(&listed), "{listed}");
        let taken = join(server, cyd, &game, cyd_deck).await;
        assert_eq!(taken.status, 200, "{}", taken.text());
        assert!(taken.json()["token"].is_string());
        assert!(seated(&row(server, Some(cyd), &game).await));
    }
}

#[sqlx::test(migrations = false)]
async fn a_delves_member_who_joins_a_full_party_again_keeps_their_place(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let bob = register(server.addr, "bob", PASSWORD).await;
    let cyd = register(server.addr, "cyd", PASSWORD).await;
    let created = ann
        .post(server.addr, "/api/delves", r#"{"heat":0,"seed":"DELVE"}"#)
        .await;
    assert_eq!(created.status, 201, "{}", created.text());
    let created = created.json();
    let (id, code) = (
        created["id"].as_str().unwrap(),
        created["code"].as_str().unwrap(),
    );
    let path = format!("/api/delves/join/{code}");
    assert_eq!(bob.post(server.addr, &path, "").await.json()["me"], 1);
    // Bot allies fill the party.
    let bot = json!({ "kind": "add_bot", "level": "Normal" }).to_string();
    let mut size = 2;
    while ann
        .post(server.addr, &format!("/api/delves/{id}/act"), &bot)
        .await
        .status
        == 200
    {
        size += 1;
        assert!(size <= 16, "the party never fills");
    }
    assert!(size > 2);

    // Both members come back to a full party in their own places; a stranger finds no room.
    for (who, me) in [(&ann, 0), (&bob, 1)] {
        let again = who.post(server.addr, &path, "").await;
        assert_eq!(again.status, 200, "{}", again.text());
        assert_eq!(again.json()["me"], me);
        assert_eq!(
            again.json()["run"]["heroes"].as_array().unwrap().len(),
            size
        );
    }
    let stranger = cyd.post(server.addr, &path, "").await;
    assert_ne!(stranger.status, 200, "{}", stranger.text());
}
