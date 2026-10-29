//! Goal: on Postgres an account plays one of its stored decks: it chooses it at creation, when joining and later by token or session, the table is told the deck's name, format and a count of manual cards and never a card, another account's deck is the 404 of a deck nobody has, a blocked deck is refused with its issues to its owner alone, the recipe names the decks at the start with the fingerprint of their cards, a started game survives the deletion of its stored decks and resumes with the same decks and state, a start whose recipe cannot be stored is refused and tried again, and a table with a deck played by hand lists as one, stores that when it starts and resumes as one after a restart, and the printing a deck line chose is the printing of the cards in the game's views (a line with none leaves it out, a built-in deck has none) and in the recipe, and the same after a restart.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::net;

use std::collections::BTreeSet;
use std::sync::OnceLock;

use mtg_cards::Caps;
use mtg_core::ids::PlayerId;
use mtg_db::Database;
use mtg_import::scryfall::cards;
use mtg_protocol::{ClientMessage, ErrorCode, LobbySeat, ServerMessage};
use mtg_server::{CardSource, DeckFingerprint, GameId, GameRecords, RecordStatus, migrate};
use net::lobby::{self, bearer};
use net::{Browser, PASSWORD, TestServer, WsClient, bot_config, register};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

/// The Scryfall objects (trimmed, without legalities) of the corpus cards: the fixture files as one JSON array.
fn scryfall() -> String {
    mtg_testkit::corpus::scryfall_array().unwrap()
}

fn objects() -> &'static Vec<Value> {
    static OBJECTS: OnceLock<Vec<Value>> = OnceLock::new();
    OBJECTS.get_or_init(|| serde_json::from_str(&scryfall()).unwrap())
}

fn oracle(name: &str) -> String {
    objects()
        .iter()
        .find(|card| card["name"] == name)
        .unwrap_or_else(|| panic!("the corpus has no {name}"))["oracle_id"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// The corpus with the legalities the tests need: Sol Ring is not legal in Modern.
fn fixture() -> String {
    let mut all = objects().clone();
    for card in &mut all {
        let mut formats = json!({"commander": "legal", "modern": "legal", "legacy": "legal"});
        if card["name"] == "Sol Ring" {
            formats["modern"] = json!("not_legal");
        }
        card["legalities"] = formats;
    }
    serde_json::to_string(&all).unwrap()
}

fn source(pool: &PgPool) -> CardSource {
    CardSource::new(pool.clone()).with_caps(Caps::everything())
}

async fn start(pool: &PgPool) -> TestServer {
    migrate(pool).await.unwrap();
    let db = Database::from_pool(pool.clone());
    db.migrate().await.unwrap();
    db.importer()
        .import_cards(cards(fixture().as_bytes()))
        .await
        .unwrap();
    TestServer::start_with_cards(pool.clone(), bot_config(), source(pool)).await
}

fn line(zone: &str, name: &str, count: u32) -> Value {
    json!({"zone": zone, "oracle_id": oracle(name), "printing_id": null, "count": count})
}

fn deck(name: &str, format: &str, lines: &[Value]) -> Value {
    json!({"name": name, "format": format, "notes": "", "cards": lines})
}

/// Sixty Forests: nothing for the table to resolve by hand.
fn forests() -> Value {
    deck("Evergreen", "modern", &[line("main", "Forest", 60)])
}

/// Fifty-six Forests and four Grizzly Bears, which have no text.
fn bears() -> Value {
    deck(
        "Beasts and Bears",
        "modern",
        &[line("main", "Forest", 56), line("main", "Grizzly Bears", 4)],
    )
}

/// Four copies of a card the rules model in part.
fn quest() -> Value {
    deck(
        "Quest",
        "modern",
        &[
            line("main", "Forest", 56),
            line("main", "Aven Mindcensor", 4),
        ],
    )
}

/// A card that is not legal in Modern.
fn banned() -> Value {
    deck(
        "Banned",
        "modern",
        &[line("main", "Forest", 59), line("main", "Sol Ring", 1)],
    )
}

fn short() -> Value {
    deck("Short", "freeform", &[line("main", "Forest", 20)])
}

fn commander() -> Value {
    deck(
        "Commander",
        "commander",
        &[
            line("commander", "Questing Beast", 1),
            line("main", "Forest", 99),
        ],
    )
}

async fn store(server: &TestServer, who: &Browser, deck: &Value) -> String {
    let reply = who.post(server.addr, "/api/decks", &deck.to_string()).await;
    assert_eq!(reply.status, 201, "{}", reply.text());
    reply.json()["id"].as_str().unwrap().to_owned()
}

fn seat_path(game: &str, seat: usize) -> String {
    format!("/api/games/{game}/seats/{seat}/deck")
}

async fn choose(
    server: &TestServer,
    who: &Browser,
    game: &str,
    seat: usize,
    id: &str,
) -> net::Reply {
    who.put(
        server.addr,
        &seat_path(game, seat),
        &json!({ "deck": id }).to_string(),
    )
    .await
}

/// A public game of two seats made by `host`, who holds seat 0 and starts with `deck`.
async fn create(server: &TestServer, host: &Browser, deck: &str) -> String {
    let body = json!({
        "format": "normal", "visibility": "public", "held": 1,
        "seats": [{"name": host.username, "deck": deck}, {"deck": "mono-red"}],
    });
    host.create_game(server.addr, &body).await.0
}

async fn roster(socket: &mut WsClient) -> Vec<LobbySeat> {
    loop {
        match socket.message().await {
            ServerMessage::Lobby { seats, .. } => return seats,
            ServerMessage::Welcome { .. } => {}
            other => panic!("expected a roster, got {other:?}"),
        }
    }
}

fn corpus_names() -> BTreeSet<String> {
    objects()
        .iter()
        .map(|card| card["name"].as_str().unwrap().to_owned())
        .collect()
}

fn assert_no_card_name(texts: &[String]) {
    for text in texts {
        for name in corpus_names() {
            assert!(!text.contains(&name), "{name} in {text}");
        }
    }
}

#[sqlx::test(migrations = false)]
async fn an_account_plays_its_stored_deck_and_the_table_hears_a_count_and_never_a_card(
    pool: PgPool,
) {
    let server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let bo = register(server.addr, "bob", PASSWORD).await;
    let dan = register(server.addr, "dan", PASSWORD).await;
    let (quest_id, forests_id) = (
        store(&server, &ann, &quest()).await,
        store(&server, &ann, &forests()).await,
    );
    let bears_id = store(&server, &bo, &bears()).await;
    let mut seen: Vec<String> = Vec::new();

    // The host starts with a stored deck; the table is told its name, format and four manual copies.
    let game = create(&server, &ann, &quest_id).await;
    let mut socket = net::open(
        server.addr,
        &format!("/ws/{game}"),
        &[("Cookie", &ann.cookie)],
    )
    .await;
    socket.welcome(&game).await;
    let first = roster(&mut socket).await;
    let told = first[0].deck.as_ref().unwrap();
    assert_eq!(
        (told.name.as_str(), told.format.as_str(), told.manual_cards),
        ("Quest", "modern", 4)
    );
    assert!(first[1].deck.is_none(), "an open seat plays nothing yet");

    // A joiner brings theirs, and the host changes hers by session alone.
    let body = json!({ "deck": bears_id }).to_string();
    let joined = bo
        .post(server.addr, &format!("/api/games/{game}/join"), &body)
        .await;
    assert_eq!(joined.status, 200, "{}", joined.text());
    seen.push(joined.text());
    let table = roster(&mut socket).await;
    assert_eq!(
        table[1]
            .deck
            .as_ref()
            .map(|d| (d.name.as_str(), d.manual_cards)),
        Some(("Beasts and Bears", 0))
    );
    let changed = choose(&server, &ann, &game, 0, &forests_id).await;
    assert_eq!(changed.status, 200, "{}", changed.text());
    seen.push(changed.text());
    assert_eq!(
        changed.json(),
        json!({"name": "Evergreen", "format": "modern", "manual_cards": 0})
    );
    let table = roster(&mut socket).await;
    assert_eq!(
        table[0].deck.as_ref().map(|d| d.name.as_str()),
        Some("Evergreen")
    );

    // Another account's deck is the 404 of a deck nobody has, for a player, a guest and a stranger alike.
    let nobody = choose(
        &server,
        &ann,
        &game,
        0,
        "4457ed35-7c10-48c8-9776-456485fdf070",
    )
    .await;
    assert_eq!(
        (nobody.status, nobody.error_code().as_str()),
        (404, "unknown_deck")
    );
    let theirs = choose(&server, &bo, &game, 1, &forests_id).await;
    assert_eq!(theirs.status, 404);
    assert_eq!(
        theirs.text(),
        nobody.text(),
        "a deck of another account looks like no deck"
    );
    let stranger = choose(&server, &dan, &game, 1, &bears_id).await;
    assert_eq!(stranger.status, 404);
    assert_eq!(
        stranger.error_code(),
        "not_found",
        "a stranger is told nothing about decks"
    );
    let token = joined.json()["token"].as_str().unwrap().to_owned();
    let as_guest = net::lobby::call(
        server.addr,
        "PUT",
        &seat_path(&game, 1),
        Some(&json!({ "deck": bears_id }).to_string()),
        &[("Authorization", &bearer(&token))],
    )
    .await;
    assert_eq!(
        as_guest.status, 404,
        "a seat token without the session brings no stored deck"
    );
    assert_eq!(as_guest.text(), nobody.text());
    seen.extend([
        nobody.text(),
        theirs.text(),
        stranger.text(),
        as_guest.text(),
    ]);

    // A deck of the same account is found with the token when the session rides along.
    let with_both = bo
        .send(
            server.addr,
            "PUT",
            &seat_path(&game, 1),
            &json!({ "deck": bears_id }).to_string(),
            &[("Authorization", &bearer(&token))],
        )
        .await;
    assert_eq!(with_both.status, 200, "{}", with_both.text());

    for path in [format!("/api/games/{game}"), "/api/lobbies".to_owned()] {
        seen.push(net::get(server.addr, &path).await.text());
    }
    seen.push(serde_json::to_string(&roster(&mut socket).await).unwrap());
    assert_no_card_name(&seen);
}

#[sqlx::test(migrations = false)]
async fn a_blocked_deck_is_refused_with_its_issues_for_its_owner_and_nobody_is_seated_by_it(
    pool: PgPool,
) {
    let server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let bo = register(server.addr, "bob", PASSWORD).await;
    let game = create(&server, &ann, "mono-green").await;

    for (stored, code, name) in [
        (banned(), "not_legal", Some("Sol Ring")),
        (short(), "deck_size", None),
        (commander(), "no_commander_zone", None),
    ] {
        let id = store(&server, &ann, &stored).await;
        let refused = choose(&server, &ann, &game, 0, &id).await;
        assert_eq!(refused.status, 422, "{}", refused.text());
        assert_eq!(refused.error_code(), "deck_blocked");
        let issues = refused.json()["issues"].clone();
        assert_eq!(issues[0]["code"], code, "{issues}");
        if let Some(name) = name {
            assert_eq!(issues[0]["name"], name);
        }
        // The same refusal when a game is made with the deck and when a joiner brings it.
        let body = json!({"format": "normal", "seats": [{"name": "Ann", "deck": id}, {"name": "B", "deck": "mono-red"}]});
        let at_creation = ann.post(server.addr, "/api/games", &body.to_string()).await;
        assert_eq!(
            (at_creation.status, at_creation.error_code().as_str()),
            (422, "deck_blocked")
        );
    }
    // A blocked deck of the joiner's own seats nobody.
    let blocked = store(&server, &bo, &banned()).await;
    let join = bo
        .post(
            server.addr,
            &format!("/api/games/{game}/join"),
            &json!({ "deck": blocked }).to_string(),
        )
        .await;
    assert_eq!(
        (join.status, join.error_code().as_str()),
        (422, "deck_blocked")
    );
    let listed = net::get(server.addr, &format!("/api/games/{game}"))
        .await
        .json();
    assert_eq!(listed["seats"][1]["name"], "", "the seat is still open");
    assert_eq!(
        server.registry.len(),
        1,
        "no game was made by a refused deck"
    );

    // The deck stayed what it was.
    let still = ann
        .put(
            server.addr,
            &seat_path(&game, 0),
            &json!({ "deck": "mono-red" }).to_string(),
        )
        .await;
    assert_eq!(still.status, 200);
}

/// The recipe of a game as the database holds it.
async fn recipe(pool: &PgPool, game: &str) -> (Value, RecordStatus) {
    let record = GameRecords::new(pool.clone())
        .get(game.parse::<GameId>().unwrap())
        .await
        .unwrap()
        .unwrap();
    (record.recipe, record.status)
}

#[sqlx::test(migrations = false)]
async fn a_started_game_survives_the_deletion_of_its_stored_decks_and_resumes_with_the_same_decks(
    pool: PgPool,
) {
    let mut server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let bo = register(server.addr, "bob", PASSWORD).await;
    let forests_id = store(&server, &ann, &forests()).await;
    let bears_id = store(&server, &bo, &bears()).await;
    let game = create(&server, &ann, "mono-green").await;
    let joined = bo
        .post(
            server.addr,
            &format!("/api/games/{game}/join"),
            &json!({ "deck": bears_id }).to_string(),
        )
        .await;
    assert_eq!(joined.status, 200, "{}", joined.text());
    assert_eq!(
        choose(&server, &ann, &game, 0, &forests_id).await.status,
        200
    );

    // The lobby's recipe still names the decks the game was made with.
    let (before, status) = recipe(&pool, &game).await;
    assert_eq!(status, RecordStatus::Lobby);
    assert_eq!(
        before["decks"][0],
        json!({"kind": "demo", "id": "mono-green"})
    );

    lobby::start(&server.registry, &game, 2).await;
    let (stored, status) = recipe(&pool, &game).await;
    assert_eq!(status, RecordStatus::Running);

    // Each seat's deck is the oracle id of every card, with the fingerprint of the cards as assembled.
    let assembler = source(&pool);
    let expected: [Vec<String>; 2] = [
        vec![oracle("Forest"); 60],
        [vec![oracle("Forest"); 56], vec![oracle("Grizzly Bears"); 4]].concat(),
    ];
    for (seat, expected) in expected.into_iter().enumerate() {
        assert_eq!(stored["decks"][seat]["kind"], "oracle");
        let ids: Vec<String> = stored["decks"][seat]["cards"]
            .as_array()
            .unwrap()
            .iter()
            .map(|id| id.as_str().unwrap().to_owned())
            .collect();
        let (mut sorted, mut wanted) = (ids.clone(), expected);
        sorted.sort();
        wanted.sort();
        assert_eq!(sorted, wanted, "seat {seat} plays the cards of its deck");
        let cards = assembler.deck(&ids).await.unwrap();
        let fingerprint = DeckFingerprint::of(&cards).unwrap().to_string();
        assert_eq!(stored["fingerprints"][seat], fingerprint, "seat {seat}");
    }
    assert_eq!(stored["seed"], before["seed"]);

    // The decks are deleted; the game is not affected.
    for (who, id) in [(&ann, &forests_id), (&bo, &bears_id)] {
        let gone = who.delete(server.addr, &format!("/api/decks/{id}")).await;
        assert_eq!(gone.status, 204);
    }
    let handle = server.registry.handle(game.parse().unwrap()).unwrap();
    let hash = handle.status().await.unwrap().state_hash;
    assert!(hash.is_some());
    server.shutdown();
    server.finished().await.unwrap();
    drop(handle);
    drop(server);

    let server = TestServer::start_with_cards(pool.clone(), bot_config(), assembler).await;
    let handle = server
        .registry
        .handle(game.parse().unwrap())
        .expect("the game came back");
    let status = handle.status().await.unwrap();
    assert!(status.started);
    assert_eq!(status.state_hash, hash, "the same decks make the same game");
    assert_eq!(recipe(&pool, &game).await, (stored, RecordStatus::Running));
}

#[sqlx::test(migrations = false)]
async fn a_start_whose_recipe_cannot_be_stored_is_refused_and_tried_again(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let forests_id = store(&server, &ann, &forests()).await;
    let game = create(&server, &ann, "mono-green").await;
    assert_eq!(
        choose(&server, &ann, &game, 0, &forests_id).await.status,
        200
    );

    // The record no longer shows a lobby (the janitor expired it, say), so the decks cannot be stored.
    let token = {
        let joined = net::lobby::join(server.addr, &game, &json!({ "name": "Cy" }), &[]).await;
        joined.json()["token"].as_str().unwrap().to_owned()
    };
    sqlx::query("UPDATE games SET status = 'expired' WHERE id = $1")
        .bind(Uuid::parse_str(&game).unwrap())
        .execute(&pool)
        .await
        .unwrap();
    let mut cy = net::connect(server.addr, &game, &token).await;
    cy.welcome(&game).await;
    roster(&mut cy).await;
    let handle = server.registry.handle(game.parse().unwrap()).unwrap();
    handle
        .send(PlayerId(0), ClientMessage::Ready)
        .await
        .unwrap();
    cy.send(&ClientMessage::Ready).await;
    loop {
        match cy.message().await {
            ServerMessage::Error { code, .. } => {
                assert_eq!(code, ErrorCode::Internal);
                break;
            }
            ServerMessage::Lobby { .. } => {}
            other => panic!("expected the refusal, got {other:?}"),
        }
    }
    assert!(
        !handle.status().await.unwrap().started,
        "the game did not start"
    );

    sqlx::query("UPDATE games SET status = 'lobby' WHERE id = $1")
        .bind(Uuid::parse_str(&game).unwrap())
        .execute(&pool)
        .await
        .unwrap();
    cy.send(&ClientMessage::NotReady).await;
    cy.send(&ClientMessage::Ready).await;
    for _ in 0..200 {
        if handle.status().await.unwrap().started {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(
        handle.status().await.unwrap().started,
        "a repeated Ready starts it"
    );
    let (stored, status) = recipe(&pool, &game).await;
    assert_eq!(status, RecordStatus::Running);
    assert_eq!(stored["decks"][0]["kind"], "oracle");
}

/// `GET /api/games/{id}` and `GET /api/lobbies`: whether each says the table plays by hand (`None` for a game not listed).
async fn flags(server: &TestServer, game: &str) -> (bool, Option<bool>) {
    let looked =
        net::lobby::call(server.addr, "GET", &format!("/api/games/{game}"), None, &[]).await;
    assert_eq!(looked.status, 200, "{}", looked.text());
    let listing = net::lobby::lobbies(server.addr, &[]).await;
    assert_eq!(listing.status, 200, "{}", listing.text());
    let rows = listing.json()["games"].clone();
    let listed = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["game"] == game)
        .map(|row| row["manual_play"].as_bool().unwrap());
    (looked.json()["manual_play"].as_bool().unwrap(), listed)
}

async fn stored_flag(pool: &PgPool, game: &str) -> bool {
    GameRecords::new(pool.clone())
        .get(game.parse::<GameId>().unwrap())
        .await
        .unwrap()
        .unwrap()
        .manual_play
}

/// A game of a host who plays `host_deck` and a guest who plays `guest_deck`, both stored decks: the lobby says
/// whether it is played by hand, the start stores that, and a restarted server resumes the game with it.
async fn plays_by_hand_through_a_restart(
    pool: PgPool,
    host_deck: Value,
    guest_deck: Value,
    expected: bool,
) {
    let mut server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let bo = register(server.addr, "bob", PASSWORD).await;
    let host_id = store(&server, &ann, &host_deck).await;
    let guest_id = store(&server, &bo, &guest_deck).await;
    let game = create(&server, &ann, "mono-green").await;
    assert_eq!(
        flags(&server, &game).await,
        (false, Some(false)),
        "built-in decks"
    );
    assert_eq!(choose(&server, &ann, &game, 0, &host_id).await.status, 200);
    let joined = bo
        .post(
            server.addr,
            &format!("/api/games/{game}/join"),
            &json!({ "deck": guest_id }).to_string(),
        )
        .await;
    assert_eq!(joined.status, 200, "{}", joined.text());
    assert_eq!(flags(&server, &game).await, (expected, Some(expected)));
    assert!(
        !stored_flag(&pool, &game).await,
        "a lobby stores nothing yet"
    );

    lobby::start(&server.registry, &game, 2).await;
    assert_eq!(
        stored_flag(&pool, &game).await,
        expected,
        "the start stores it"
    );
    assert_eq!(flags(&server, &game).await, (expected, None));

    let handle = server.registry.handle(game.parse().unwrap()).unwrap();
    let hash = handle.status().await.unwrap().state_hash;
    server.shutdown();
    server.finished().await.unwrap();
    drop(handle);
    drop(server);

    let server = TestServer::start_with_cards(pool.clone(), bot_config(), source(&pool)).await;
    let handle = server
        .registry
        .handle(game.parse().unwrap())
        .expect("the game came back");
    let status = handle.status().await.unwrap();
    assert!(status.started);
    assert_eq!(status.state_hash, hash);
    assert_eq!(status.manual_play, expected, "the resumed game keeps it");
    assert_eq!(flags(&server, &game).await, (expected, None));
    assert_eq!(stored_flag(&pool, &game).await, expected);
}

#[sqlx::test(migrations = false)]
async fn a_table_with_a_deck_played_by_hand_starts_and_resumes_as_one(pool: PgPool) {
    plays_by_hand_through_a_restart(pool, quest(), forests(), true).await;
}

#[sqlx::test(migrations = false)]
async fn a_table_where_the_guest_plays_by_hand_starts_and_resumes_as_one(pool: PgPool) {
    plays_by_hand_through_a_restart(pool, forests(), quest(), true).await;
}

#[sqlx::test(migrations = false)]
async fn a_table_whose_decks_are_run_whole_starts_and_resumes_as_one(pool: PgPool) {
    plays_by_hand_through_a_restart(pool, forests(), bears(), false).await;
}

/// The Scryfall id of the corpus printing of `name`.
fn printing(name: &str) -> String {
    objects()
        .iter()
        .find(|card| card["name"] == name)
        .unwrap_or_else(|| panic!("the corpus has no {name}"))["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// What seat 0 sees of its own cards: the cards' names with their identity.
async fn identities(handle: &mtg_server::GameHandle) -> Vec<mtg_protocol::ObjectView> {
    let mut queue = handle.join(PlayerId::from_index(0)).await.unwrap();
    loop {
        if let ServerMessage::Snapshot { state, .. } = queue.recv().await.unwrap() {
            return state.objects;
        }
    }
}

#[sqlx::test(migrations = false)]
async fn the_printing_a_deck_line_chose_names_the_cards_of_the_game_and_survives_a_restart(
    pool: PgPool,
) {
    let mut server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    // Fifty-six Forests of the corpus printing, four Grizzly Bears of no printing.
    let lines = [
        json!({"zone": "main", "oracle_id": oracle("Forest"), "printing_id": printing("Forest"), "count": 56}),
        line("main", "Grizzly Bears", 4),
    ];
    let stored = store(&server, &ann, &deck("Printed", "modern", &lines)).await;
    let game = create(&server, &ann, "mono-green").await;
    let bo = register(server.addr, "bob", PASSWORD).await;
    let path = format!("/api/games/{game}/join");
    let joined = bo
        .post(server.addr, &path, &json!({"deck": "mono-red"}).to_string())
        .await;
    assert_eq!(joined.status, 200, "{}", joined.text());
    assert_eq!(choose(&server, &ann, &game, 0, &stored).await.status, 200);
    lobby::start(&server.registry, &game, 2).await;

    // The recipe keeps one printing for each card of the library, none for the demo deck.
    let (recipe, _) = recipe(&pool, &game).await;
    let cards = recipe["decks"][0]["cards"].as_array().unwrap();
    let printings = recipe["decks"][0]["printings"].as_array().unwrap();
    assert_eq!(printings.len(), cards.len());
    for (card, printing_id) in cards.iter().zip(printings) {
        let forest = card == &json!(oracle("Forest"));
        assert_eq!(printing_id, &json!(forest.then(|| printing("Forest"))));
    }
    assert_eq!(
        recipe["decks"][1],
        json!({"kind": "demo", "id": "mono-red"})
    );

    // The views name the printing of every card seat 0 sees, and the cards of no printing name only the card.
    let handle = server.registry.handle(game.parse().unwrap()).unwrap();
    let hash = handle.status().await.unwrap().state_hash;
    let before = identities(&handle).await;
    assert!(before.iter().any(|object| object.name == "Forest"));
    for object in &before {
        let identity = object.identity.as_ref().expect("a stored card has one");
        let (card, wanted) = if object.name == "Forest" {
            ("Forest", Some(printing("Forest")))
        } else {
            ("Grizzly Bears", None)
        };
        assert_eq!(identity.oracle_id, oracle(card), "{}", object.name);
        assert_eq!(identity.printing_id, wanted, "{}", object.name);
    }

    server.shutdown();
    server.finished().await.unwrap();
    drop(handle);
    drop(server);
    let server = TestServer::start_with_cards(pool.clone(), bot_config(), source(&pool)).await;
    let handle = server.registry.handle(game.parse().unwrap()).unwrap();
    assert_eq!(handle.status().await.unwrap().state_hash, hash);
    assert_eq!(identities(&handle).await, before);
}
