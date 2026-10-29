//! Goal: on Postgres a Commander game is made, listed, joined and started at 2, 3, 4 and 6 seats from stored decks and from precons, each seat starts with its commander in the command zone and every seat sees all of them, the recipe stores the commanders with the decks and a resumed lobby and game come back as they were, a deck that breaks a rule of the format (no commander, size, singleton, colour identity, an ineligible commander, a command zone in a Normal game, a demo deck) is refused with the reasons of the check at creation, on a join and on a deck change, the Commander precons are playable or blocked by the same report with the manual cards marked, house rules layer over the format, and bots that only pass play through several turns of a precon with manual cards.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::{common, net};

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use common::passive;
use futures_util::future::join_all;
use mtg_cards::Caps;
use mtg_db::Database;
use mtg_import::mtgjson::{PreconCard, PreconDeck};
use mtg_import::scryfall::cards;
use mtg_protocol::{ClientMessage, LobbySeat, ServerMessage, StateView};
use mtg_server::{CardSource, GameId, GameRecords, RecordStatus, ServerConfig, migrate};
use net::lobby::{self, bearer};
use net::{Browser, Frame, PASSWORD, TestServer, WsClient, bot_config, connect, register};
use serde_json::{Value, json};
use sqlx::PgPool;

/// The real cards of the rules corpus, trimmed Scryfall objects without legalities.
fn objects() -> &'static Vec<Value> {
    static OBJECTS: OnceLock<Vec<Value>> = OnceLock::new();
    OBJECTS.get_or_init(|| {
        serde_json::from_str(&mtg_testkit::corpus::scryfall_array().unwrap()).unwrap()
    })
}

pub(crate) fn oracle(name: &str) -> String {
    objects()
        .iter()
        .find(|card| card["name"] == name)
        .unwrap_or_else(|| panic!("the corpus has no {name}"))["oracle_id"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// The corpus with legalities: every card is legal in Commander and Modern.
pub(crate) fn fixture() -> String {
    let mut all = objects().clone();
    for card in &mut all {
        card["legalities"] = json!({"commander": "legal", "modern": "legal"});
    }
    serde_json::to_string(&all).unwrap()
}

pub(crate) fn source(pool: &PgPool) -> CardSource {
    CardSource::new(pool.clone()).with_caps(Caps::everything())
}

// ---- precons ----

fn entry(name: &str, count: u32) -> PreconCard {
    PreconCard {
        count,
        uuid: format!("uuid-{name}"),
        name: name.to_owned(),
        set_code: None,
        number: None,
        is_foil: false,
        scryfall_id: None,
        oracle_id: None,
    }
}

pub(crate) fn precon_deck(
    name: &str,
    kind: &str,
    commander: &[(&str, u32)],
    main: &[(&str, u32)],
) -> PreconDeck {
    let cards = |list: &[(&str, u32)]| list.iter().map(|(n, c)| entry(n, *c)).collect();
    PreconDeck {
        name: name.to_owned(),
        code: "CMD".to_owned(),
        deck_type: Some(kind.to_owned()),
        release_date: Some("2024-01-01".to_owned()),
        main_board: cards(main),
        side_board: Vec::new(),
        commander: cards(commander),
        display_commander: Vec::new(),
    }
}

/// A legal Commander deck with one card the rules model in part, so one copy is played by hand.
pub(crate) fn beast_pack() -> PreconDeck {
    precon_deck(
        "Beast Pack",
        "Commander Deck",
        &[("Questing Beast", 1)],
        &[("Forest", 98), ("Auspicious Starrix", 1)],
    )
}

/// Too few cards for the format.
pub(crate) fn short_pack() -> PreconDeck {
    precon_deck(
        "Short Pack",
        "Commander Deck",
        &[("Questing Beast", 1)],
        &[("Forest", 50)],
    )
}

/// A red card under a green commander.
pub(crate) fn red_pack() -> PreconDeck {
    precon_deck(
        "Red Pack",
        "Commander Deck",
        &[("Questing Beast", 1)],
        &[("Forest", 98), ("Lightning Bolt", 1)],
    )
}

/// A commander that cannot lead a deck.
pub(crate) fn bear_pack() -> PreconDeck {
    precon_deck(
        "Bear Pack",
        "Commander Deck",
        &[("Grizzly Bears", 1)],
        &[("Forest", 99)],
    )
}

/// A card nobody stored.
pub(crate) fn ghost_pack() -> PreconDeck {
    precon_deck(
        "Ghost Pack",
        "Commander Deck",
        &[("Questing Beast", 1)],
        &[("Forest", 98), ("No Such Card", 1)],
    )
}

/// A Starter Deck of sixty Forests: a Normal game takes it, a Commander game does not.
fn evergreen() -> PreconDeck {
    precon_deck("Evergreen Pack", "Starter Deck", &[], &[("Forest", 60)])
}

pub(crate) const BEAST: &str = "cmd-beast-pack";
pub(crate) const SHORT: &str = "cmd-short-pack";
pub(crate) const RED: &str = "cmd-red-pack";
pub(crate) const BEAR: &str = "cmd-bear-pack";
pub(crate) const GHOST: &str = "cmd-ghost-pack";
const EVERGREEN: &str = "cmd-evergreen-pack";

pub(crate) fn on(code: &str) -> String {
    format!("precon:{code}")
}

async fn start(pool: &PgPool) -> TestServer {
    start_with(pool, bot_config()).await
}

async fn start_with(pool: &PgPool, config: ServerConfig) -> TestServer {
    migrate(pool).await.unwrap();
    let database = Database::from_pool(pool.clone());
    database.migrate().await.unwrap();
    database
        .importer()
        .import_cards(cards(fixture().as_bytes()))
        .await
        .unwrap();
    database
        .importer()
        .import_precons(
            [
                beast_pack(),
                short_pack(),
                red_pack(),
                bear_pack(),
                ghost_pack(),
                evergreen(),
            ]
            .into_iter()
            .map(Ok),
        )
        .await
        .unwrap();
    TestServer::start_with_cards(pool.clone(), config, source(pool)).await
}

// ---- stored decks ----

pub(crate) fn line(zone: &str, name: &str, count: u32) -> Value {
    json!({"zone": zone, "oracle_id": oracle(name), "printing_id": null, "count": count})
}

pub(crate) fn deck(name: &str, format: &str, lines: &[Value]) -> Value {
    json!({"name": name, "format": format, "notes": "", "cards": lines})
}

/// A legal Commander deck led by `commander`: 99 Forests, which the identity of the commander allows.
pub(crate) fn led_by(commander: &str) -> Value {
    deck(
        commander,
        "commander",
        &[line("commander", commander, 1), line("main", "Forest", 99)],
    )
}

/// The three commanders the players take turns with; each has green in its identity.
const LEADERS: [&str; 3] = [
    "Questing Beast",
    "The Thing, Ben Grimm",
    "Rosheen Meanderer",
];

pub(crate) async fn store(server: &TestServer, who: &Browser, deck: &Value) -> String {
    let reply = who.post(server.addr, "/api/decks", &deck.to_string()).await;
    assert_eq!(reply.status, 201, "{}", reply.text());
    reply.json()["id"].as_str().unwrap().to_owned()
}

/// Six accounts, each with a Commander deck of its own.
async fn players(server: &TestServer) -> Vec<(Browser, String, &'static str)> {
    let mut all = Vec::new();
    for (index, name) in ["ann", "bob", "cyd", "dee", "eve", "fay"]
        .into_iter()
        .enumerate()
    {
        let browser = register(server.addr, name, PASSWORD).await;
        let leader = LEADERS[index % LEADERS.len()];
        let id = store(server, &browser, &led_by(leader)).await;
        all.push((browser, id, leader));
    }
    all
}

pub(crate) fn create_body(seats: &[Value], held: usize) -> Value {
    json!({"format": "commander", "visibility": "public", "held": held, "seats": seats})
}

pub(crate) async fn join(server: &TestServer, who: &Browser, game: &str, deck: &str) -> net::Reply {
    who.post(
        server.addr,
        &format!("/api/games/{game}/join"),
        &json!({ "deck": deck }).to_string(),
    )
    .await
}

fn seat_path(game: &str, seat: usize) -> String {
    format!("/api/games/{game}/seats/{seat}/deck")
}

pub(crate) async fn choose(
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

/// The recipe of a game as the database holds it.
async fn recipe(pool: &PgPool, game: &str) -> (Value, RecordStatus) {
    let record = GameRecords::new(pool.clone())
        .get(game.parse::<GameId>().unwrap())
        .await
        .unwrap()
        .unwrap();
    (record.recipe, record.status)
}

fn row<'a>(listing: &'a Value, game: &str) -> Option<&'a Value> {
    listing["games"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["game"] == game)
}

/// The first snapshot a seat is sent after it joins a started game.
async fn snapshot(socket: &mut WsClient, game: &str) -> StateView {
    socket.join(game).await;
    loop {
        match socket.message().await {
            ServerMessage::Snapshot { state, .. } => return *state,
            ServerMessage::Welcome { .. } | ServerMessage::Lobby { .. } => {}
            other => panic!("expected a snapshot, got {other:?}"),
        }
    }
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

/// Every seat sees one commander for every seat, in its command zone, and 40 life.
fn assert_commanders(view: &StateView, seats: usize) {
    assert_eq!(view.table.format, "commander");
    assert_eq!(view.table.players.len(), seats);
    for (seat, player) in view.table.players.iter().enumerate() {
        assert_eq!(player.commanders.len(), 1, "seat {seat}");
        assert_eq!(player.command.count, 1, "seat {seat}");
        assert_eq!(player.life, 40, "seat {seat}");
        assert_eq!(player.commanders[0].casts, 0);
        assert_eq!(player.commanders[0].tax, 0);
        assert_eq!(player.commander_damage, []);
        assert_eq!(player.hand.count, 7, "seat {seat}: a hand of seven");
    }
}

// ---- stored decks at every table size ----

#[sqlx::test(migrations = false)]
async fn stored_decks_make_a_commander_game_at_every_table_size(pool: PgPool) {
    let server = start(&pool).await;
    let all = players(&server).await;

    for seats in [2usize, 3, 4, 6] {
        let (host, host_deck, _) = &all[0];
        let seat_list: Vec<Value> = (0..seats)
            .map(|seat| {
                if seat == 0 {
                    json!({"name": host.username, "deck": host_deck})
                } else {
                    json!({})
                }
            })
            .collect();
        let (game, tokens) = host
            .create_game(server.addr, &create_body(&seat_list, 1))
            .await;

        // The lobby row names the format and the table.
        let listing = lobby::lobbies(server.addr, &[]).await.json();
        let listed = row(&listing, &game).unwrap_or_else(|| panic!("{game} is not listed"));
        assert_eq!(listed["format"], "commander");
        assert_eq!(listed["total"], seats);
        assert_eq!(listed["filled"], 1);

        // The others join with their own decks.
        let mut tokens = tokens;
        for (who, deck, _) in &all[1..seats] {
            let joined = join(&server, who, &game, deck).await;
            assert_eq!(joined.status, 200, "{}", joined.text());
            tokens.push(joined.json()["token"].as_str().unwrap().to_owned());
        }

        // The roster tells each seat's deck, with the format of the deck.
        let mut host_socket = connect(server.addr, &game, &tokens[0]).await;
        host_socket.welcome(&game).await;
        let seated = roster(&mut host_socket).await;
        assert_eq!(seated.len(), seats);
        for (seat, listed) in seated.iter().enumerate() {
            let deck = listed
                .deck
                .as_ref()
                .unwrap_or_else(|| panic!("seat {seat} has no deck"));
            assert_eq!(deck.format, "commander", "seat {seat}");
        }
        host_socket.close().await;

        lobby::start(&server.registry, &game, seats).await;
        assert!(
            row(&lobby::lobbies(server.addr, &[]).await.json(), &game).is_none(),
            "a started game leaves the listing"
        );

        // The recipe stores each seat's commander with its library, and the fingerprints.
        let (stored, status) = recipe(&pool, &game).await;
        assert_eq!(status, RecordStatus::Running);
        assert_eq!(stored["format"], "commander");
        for (seat, (_, _, leader)) in all[..seats].iter().enumerate() {
            let deck = &stored["decks"][seat];
            assert_eq!(deck["kind"], "oracle", "seat {seat}");
            assert_eq!(deck["commanders"], json!([oracle(leader)]), "seat {seat}");
            assert_eq!(deck["cards"].as_array().unwrap().len(), 99, "seat {seat}");
            assert!(
                stored["fingerprints"][seat]
                    .as_str()
                    .is_some_and(|text| text.starts_with("v1:")),
                "seat {seat}"
            );
        }

        // Every seat is shown every commander.
        for (seat, token) in tokens.iter().enumerate() {
            let mut socket = connect(server.addr, &game, token).await;
            let view = snapshot(&mut socket, &game).await;
            assert_eq!(view.table.viewer.index(), seat);
            assert_commanders(&view, seats);
            socket.close().await;
        }
    }
}

// ---- precons at every table size ----

#[sqlx::test(migrations = false)]
async fn a_precon_makes_a_commander_game_in_one_request_at_every_table_size(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;

    for seats in [2usize, 3, 4, 6] {
        let seat_list: Vec<Value> = (0..seats)
            .map(|seat| json!({"name": format!("Player {seat}"), "deck": on(BEAST)}))
            .collect();
        let (game, tokens) = ann
            .create_game(server.addr, &create_body(&seat_list, seats))
            .await;
        assert_eq!(tokens.len(), seats);

        let mut socket = connect(server.addr, &game, &tokens[0]).await;
        socket.welcome(&game).await;
        let seated = roster(&mut socket).await;
        for listed in &seated {
            let note = listed.deck.as_ref().unwrap();
            assert_eq!(note.name, "Beast Pack");
            assert_eq!(note.format, "commander");
            assert_eq!(note.manual_cards, 1, "Auspicious Starrix is played by hand");
        }
        socket.close().await;

        lobby::start(&server.registry, &game, seats).await;
        let (stored, _) = recipe(&pool, &game).await;
        assert_eq!(stored["format"], "commander");
        for seat in 0..seats {
            assert_eq!(
                stored["decks"][seat]["commanders"],
                json!([oracle("Questing Beast")])
            );
            assert_eq!(stored["decks"][seat]["cards"].as_array().unwrap().len(), 99);
        }
        let mut socket = connect(server.addr, &game, &tokens[seats - 1]).await;
        assert_commanders(&snapshot(&mut socket, &game).await, seats);
        socket.close().await;
    }
}

#[sqlx::test(migrations = false)]
async fn a_joiner_brings_a_precon_to_a_commander_game_and_a_normal_game_refuses_it(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let seat_list = [json!({"name": "Ann", "deck": on(BEAST)}), json!({})];
    let (game, _) = ann
        .create_game(server.addr, &create_body(&seat_list, 1))
        .await;
    let joined = lobby::join(
        server.addr,
        &game,
        &json!({"name": "Guest", "deck": on(BEAST)}),
        &[],
    )
    .await;
    assert_eq!(joined.status, 200, "{}", joined.text());

    // The same precon in a Normal game is a command zone the game does not have.
    let normal = json!({"format": "normal", "seats": [
        {"name": "Ann", "deck": on(BEAST)}, {"name": "B", "deck": "mono-red"}]});
    let refused = ann
        .post(server.addr, "/api/games", &normal.to_string())
        .await;
    assert_eq!(refused.status, 422, "{}", refused.text());
    assert_eq!(refused.error_code(), "deck_blocked");
    assert_eq!(refused.json()["issues"][0]["code"], "no_commander_zone");

    // A Normal precon is no Commander deck.
    let plain = json!({"format": "commander", "seats": [
        {"name": "Ann", "deck": on(EVERGREEN)}, {"name": "B"}]});
    let refused = ann
        .post(server.addr, "/api/games", &plain.to_string())
        .await;
    assert_eq!(refused.status, 422, "{}", refused.text());
    assert_eq!(refused.json()["issues"][0]["code"], "commander_count");
}

// ---- resume ----

#[sqlx::test(migrations = false)]
async fn a_commander_lobby_and_a_running_game_come_back_after_a_restart(pool: PgPool) {
    let mut server = start(&pool).await;
    let all = players(&server).await;
    let (ann, ann_deck, _) = &all[0];

    // A lobby with one seat open, and a started game of four seats.
    let lobby_seats = [json!({"name": "ann", "deck": ann_deck}), json!({})];
    let (waiting, _) = ann
        .create_game(server.addr, &create_body(&lobby_seats, 1))
        .await;
    let seat_list: Vec<Value> = (0..4)
        .map(|seat| json!({"name": format!("Player {seat}"), "deck": on(BEAST)}))
        .collect();
    let (running, tokens) = ann
        .create_game(server.addr, &create_body(&seat_list, 4))
        .await;
    lobby::start(&server.registry, &running, 4).await;
    let handle = server.registry.handle(running.parse().unwrap()).unwrap();
    let hash = handle.status().await.unwrap().state_hash;
    assert!(hash.is_some());
    let (before, status) = recipe(&pool, &running).await;
    assert_eq!(status, RecordStatus::Running);

    server.shutdown();
    server.finished().await.unwrap();
    drop(handle);
    drop(server);

    let server = TestServer::start_with_cards(pool.clone(), bot_config(), source(&pool)).await;
    let handle = server
        .registry
        .handle(running.parse().unwrap())
        .expect("the running game came back");
    let status = handle.status().await.unwrap();
    assert!(status.started);
    assert_eq!(
        status.state_hash, hash,
        "the same commanders make the same game"
    );
    assert_eq!(
        recipe(&pool, &running).await,
        (before, RecordStatus::Running)
    );
    let mut socket = connect(server.addr, &running, &tokens[2]).await;
    assert_commanders(&snapshot(&mut socket, &running).await, 4);
    socket.close().await;

    // The lobby is listed again as a Commander game; its seat chooses a deck again, a friend joins
    // and the game starts with both commanders.
    let listing = lobby::lobbies(server.addr, &[]).await.json();
    assert_eq!(row(&listing, &waiting).unwrap()["format"], "commander");
    assert_eq!(
        choose(&server, ann, &waiting, 0, ann_deck).await.status,
        200
    );
    let (bob, bob_deck, _) = &all[1];
    let joined = join(&server, bob, &waiting, bob_deck).await;
    assert_eq!(joined.status, 200, "{}", joined.text());
    lobby::start(&server.registry, &waiting, 2).await;
    let (stored, status) = recipe(&pool, &waiting).await;
    assert_eq!(status, RecordStatus::Running);
    assert_eq!(
        stored["decks"][0]["commanders"],
        json!([oracle("Questing Beast")])
    );
    assert_eq!(
        stored["decks"][1]["commanders"],
        json!([oracle("The Thing, Ben Grimm")])
    );
}

// ---- refused decks ----

/// Decks that break a rule of the format, with the code of the first issue the table is told.
fn refused_decks() -> Vec<(&'static str, Value, &'static str)> {
    vec![
        (
            "no commander",
            deck("None", "commander", &[line("main", "Forest", 100)]),
            "commander_count",
        ),
        (
            "too few cards",
            deck(
                "Short",
                "commander",
                &[
                    line("commander", "Questing Beast", 1),
                    line("main", "Forest", 50),
                ],
            ),
            "deck_size",
        ),
        (
            "a second copy",
            deck(
                "Twice",
                "commander",
                &[
                    line("commander", "Questing Beast", 1),
                    line("main", "Auspicious Starrix", 2),
                    line("main", "Forest", 97),
                ],
            ),
            "copies",
        ),
        (
            "outside the colour identity",
            deck(
                "Red",
                "commander",
                &[
                    line("commander", "Questing Beast", 1),
                    line("main", "Lightning Bolt", 1),
                    line("main", "Forest", 98),
                ],
            ),
            "color_identity",
        ),
        (
            "a commander that cannot lead",
            deck(
                "Bears",
                "commander",
                &[
                    line("commander", "Grizzly Bears", 1),
                    line("main", "Forest", 99),
                ],
            ),
            "commander_eligibility",
        ),
        (
            "a command zone in a constructed deck",
            deck(
                "Modern",
                "modern",
                &[
                    line("commander", "Questing Beast", 1),
                    line("main", "Forest", 99),
                ],
            ),
            "no_commander_zone",
        ),
        (
            "a deck for another format",
            deck("Sixty", "freeform", &[line("main", "Forest", 60)]),
            "commander_count",
        ),
    ]
}

#[sqlx::test(migrations = false)]
async fn a_deck_that_breaks_the_format_is_refused_with_the_reasons_of_the_check(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let bob = register(server.addr, "bob", PASSWORD).await;
    let good = store(&server, &ann, &led_by("Questing Beast")).await;
    let seat_list = [json!({"name": "Ann", "deck": good}), json!({})];
    let (game, _) = ann
        .create_game(server.addr, &create_body(&seat_list, 1))
        .await;

    for (why, bad, code) in refused_decks() {
        let id = store(&server, &ann, &bad).await;
        let theirs = store(&server, &bob, &bad).await;

        // At creation.
        let body = create_body(&[json!({"name": "Ann", "deck": id}), json!({})], 1);
        let made = ann.post(server.addr, "/api/games", &body.to_string()).await;
        assert_eq!(made.status, 422, "{why}: {}", made.text());
        assert_eq!(made.error_code(), "deck_blocked", "{why}");
        assert_eq!(made.json()["issues"][0]["code"], code, "{why}");
        assert!(
            made.json()["issues"][0]["message"]
                .as_str()
                .is_some_and(|m| !m.is_empty()),
            "{why}: the issue says what is wrong"
        );

        // On a deck change.
        let changed = choose(&server, &ann, &game, 0, &id).await;
        assert_eq!(changed.status, 422, "{why}: {}", changed.text());
        assert_eq!(changed.json()["issues"][0]["code"], code, "{why}");

        // On a join; nobody is seated by it.
        let joined = join(&server, &bob, &game, &theirs).await;
        assert_eq!(joined.status, 422, "{why}: {}", joined.text());
        assert_eq!(joined.json()["issues"][0]["code"], code, "{why}");
    }
    let info = net::get(server.addr, &format!("/api/games/{game}"))
        .await
        .json();
    assert_eq!(info["format"], "commander");
    assert_eq!(
        info["seats"][1]["name"], "",
        "no refused deck took the open seat"
    );

    // A built-in deck has no commander.
    let demo = ann
        .post(
            server.addr,
            "/api/games",
            &json!({"format": "commander", "seats": [
                {"name": "Ann", "deck": "mono-red"}, {"name": "B"}]})
            .to_string(),
        )
        .await;
    assert_eq!(demo.status, 422, "{}", demo.text());
    assert_eq!(demo.json()["issues"][0]["code"], "commander_count");
    let late = lobby::join(
        server.addr,
        &game,
        &json!({"name": "G", "deck": "mono-red"}),
        &[],
    )
    .await;
    assert_eq!(late.status, 422, "{}", late.text());

    // The seat that was left alone keeps what it had, and the good deck still plays.
    assert_eq!(choose(&server, &ann, &game, 0, &good).await.status, 200);
    let joined = join(
        &server,
        &bob,
        &game,
        &store(&server, &bob, &led_by("Rosheen Meanderer")).await,
    )
    .await;
    assert_eq!(joined.status, 200, "{}", joined.text());
}

#[sqlx::test(migrations = false)]
async fn a_commander_deck_has_no_place_in_a_normal_game(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let id = store(&server, &ann, &led_by("Questing Beast")).await;
    let body = json!({"format": "normal", "seats": [
        {"name": "Ann", "deck": id}, {"name": "B", "deck": "mono-red"}]});
    let made = ann.post(server.addr, "/api/games", &body.to_string()).await;
    assert_eq!(made.status, 422, "{}", made.text());
    assert_eq!(made.json()["issues"][0]["code"], "no_commander_zone");
}

// ---- the format and the table size ----

#[sqlx::test(migrations = false)]
async fn the_format_key_and_the_seat_count_are_checked_before_anything_is_made(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let seats =
        |n: usize| -> Vec<Value> { (0..n).map(|i| json!({"name": format!("P{i}")})).collect() };
    for (format, count, status, code) in [
        ("commander", 1, 400, "invalid_request"),
        ("modern", 2, 400, "unsupported_format"),
        ("Commander", 2, 400, "unsupported_format"),
    ] {
        let body = json!({"format": format, "seats": seats(count)});
        let reply = ann.post(server.addr, "/api/games", &body.to_string()).await;
        assert_eq!(reply.status, status, "{format} {count}: {}", reply.text());
        assert_eq!(reply.error_code(), code, "{format} {count}");
    }
    // Seats without a deck wait for their holders to choose one; no format has a most.
    for (format, count) in [
        ("commander", 2),
        ("commander", 6),
        ("commander", 24),
        ("normal", 12),
        ("normal", 24),
    ] {
        let body = json!({"format": format, "seats": seats(count)});
        let reply = ann.post(server.addr, "/api/games", &body.to_string()).await;
        assert_eq!(reply.status, 201, "{format} {count}: {}", reply.text());
    }
    assert_eq!(server.registry.len(), 5);
}

#[sqlx::test(migrations = false)]
async fn a_seat_without_a_deck_cannot_start_a_commander_game_and_house_rules_layer_over_it(
    pool: PgPool,
) {
    let server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let id = store(&server, &ann, &led_by("Questing Beast")).await;
    let body = json!({
        "format": "commander", "visibility": "public", "held": 2,
        "seats": [{"name": "Ann", "deck": id}, {"name": "Bo"}],
        "house_rules": {"starting_life": 30},
    });
    let (game, tokens) = ann.create_game(server.addr, &body).await;
    let listing = lobby::lobbies(server.addr, &[]).await.json();
    let listed = row(&listing, &game).unwrap();
    assert_eq!(listed["format"], "commander");
    assert_eq!(listed["house_rules"]["starting_life"], 30);

    // Bo has no deck: Ready is refused until they choose one.
    let mut bo = connect(server.addr, &game, &tokens[1]).await;
    bo.welcome(&game).await;
    bo.send(&ClientMessage::Ready).await;
    loop {
        match bo.message().await {
            ServerMessage::Error { .. } => break,
            ServerMessage::Lobby { .. } => {}
            other => panic!("expected the refusal, got {other:?}"),
        }
    }
    let chosen = lobby::call(
        server.addr,
        "PUT",
        &seat_path(&game, 1),
        Some(&json!({ "deck": on(BEAST) }).to_string()),
        &[("Authorization", &bearer(&tokens[1]))],
    )
    .await;
    assert_eq!(chosen.status, 200, "{}", chosen.text());
    lobby::start(&server.registry, &game, 2).await;

    let mut socket = connect(server.addr, &game, &tokens[0]).await;
    let view = snapshot(&mut socket, &game).await;
    assert_eq!(view.table.format, "commander");
    assert!(
        view.table.players.iter().all(|player| player.life == 30),
        "the creator's starting life replaces the format's 40"
    );
    assert!(
        view.table
            .players
            .iter()
            .all(|player| player.commanders.len() == 1)
    );
}

// ---- the precons ----

fn item<'a>(list: &'a Value, code: &str) -> &'a Value {
    list["precons"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["code"] == code)
        .unwrap_or_else(|| panic!("{code} is not listed: {list}"))
}

#[sqlx::test(migrations = false)]
async fn a_commander_precon_is_playable_when_the_check_passes_and_blocked_with_its_reason_when_not(
    pool: PgPool,
) {
    let server = start(&pool).await;
    let list = net::get(server.addr, "/api/precons").await.json();

    let beast = item(&list, BEAST);
    assert_eq!(beast["format"], "commander");
    assert_eq!(beast["state"], "manual_play", "one card is played by hand");
    assert_eq!(beast["playable"], true);
    assert_eq!(beast["card_count"], 100);
    assert_eq!(beast["commanders"], json!(["Questing Beast"]));
    let counts = &beast["summary"]["counts"];
    assert_eq!(counts["total"], 100);
    assert_eq!(
        counts["partial"].as_u64().unwrap() + counts["manual"].as_u64().unwrap(),
        1
    );

    let evergreen = item(&list, EVERGREEN);
    assert_eq!(
        (
            evergreen["format"].clone(),
            evergreen["state"].clone(),
            evergreen["playable"].clone()
        ),
        (json!("freeform"), json!("verified"), json!(true))
    );

    for (code, issue) in [
        (SHORT, "deck_size"),
        (RED, "color_identity"),
        (BEAR, "commander_eligibility"),
        (GHOST, "unknown_card"),
    ] {
        let listed = item(&list, code);
        assert_eq!(listed["state"], "blocked", "{code}");
        assert_eq!(listed["playable"], false, "{code}");
        let view = net::get(server.addr, &format!("/api/precons/{code}"))
            .await
            .json();
        let codes = |issues: &Value| -> Vec<String> {
            issues
                .as_array()
                .unwrap()
                .iter()
                .map(|issue| issue["code"].as_str().unwrap().to_owned())
                .collect()
        };
        assert!(
            codes(&view["issues"]).contains(&issue.to_owned()),
            "{code}: {}",
            view["issues"]
        );
        // A game refuses it with the same reasons.
        let body = create_body(&[json!({"name": "Ann", "deck": on(code)}), json!({})], 1);
        let made = net::post_json(server.addr, "/api/games", &body.to_string()).await;
        assert_eq!(made.status, 422, "{code}: {}", made.text());
        assert_eq!(
            codes(&made.json()["issues"]),
            codes(&view["issues"]),
            "{code}"
        );
    }
}

// ---- bots ----

/// Seat `seat` passes every decision until some table has reached `turn`; the highest turn it saw.
async fn pass_until(
    addr: std::net::SocketAddr,
    game: String,
    token: String,
    turn: u32,
    seen: Arc<AtomicU32>,
) {
    let mut client = connect(addr, &game, &token).await;
    client.welcome(&game).await;
    loop {
        if seen.load(Ordering::SeqCst) >= turn {
            return;
        }
        match client.next().await {
            Frame::Message(ServerMessage::Decision { id, decision, .. }) => {
                let answer = passive(&decision);
                client
                    .send(&ClientMessage::Answer {
                        decision: id,
                        answer,
                    })
                    .await;
            }
            Frame::Message(ServerMessage::Events(batch)) => {
                seen.fetch_max(batch.table.turn.number, Ordering::SeqCst);
            }
            Frame::Message(ServerMessage::Snapshot { state, .. }) => {
                seen.fetch_max(state.table.turn.number, Ordering::SeqCst);
            }
            Frame::Message(ServerMessage::GameOver { result }) => {
                panic!("a table of passing players ended: {result:?}")
            }
            Frame::Message(_) => {}
            other => panic!("the socket of a passing seat ended: {other:?}"),
        }
    }
}

#[sqlx::test(migrations = false)]
async fn players_who_only_pass_play_turns_of_a_precon_with_manual_cards_without_wedging(
    pool: PgPool,
) {
    let server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    for seats in [2usize, 4] {
        let seat_list: Vec<Value> = (0..seats)
            .map(|seat| json!({"name": format!("Player {seat}"), "deck": on(BEAST)}))
            .collect();
        let (game, tokens) = ann
            .create_game(server.addr, &create_body(&seat_list, seats))
            .await;
        let turn = u32::try_from(seats).unwrap() + 2;
        let seen = Arc::new(AtomicU32::new(0));
        // The decision a seat waits on is sent again when it connects, so the seats may come after the start.
        lobby::start(&server.registry, &game, seats).await;
        let players = tokens
            .iter()
            .cloned()
            .map(|token| pass_until(server.addr, game.clone(), token, turn, Arc::clone(&seen)));
        tokio::time::timeout(Duration::from_secs(120), join_all(players))
            .await
            .unwrap_or_else(|_| {
                panic!(
                    "{seats} seats wedged at turn {}",
                    seen.load(Ordering::SeqCst)
                )
            });
        assert!(seen.load(Ordering::SeqCst) >= turn);
    }
}

#[test]
fn the_decks_of_the_tests_name_cards_of_the_corpus() {
    let names: BTreeSet<&str> = objects()
        .iter()
        .map(|card| card["name"].as_str().unwrap())
        .collect();
    for name in LEADERS.into_iter().chain([
        "Forest",
        "Auspicious Starrix",
        "Lightning Bolt",
        "Grizzly Bears",
    ]) {
        assert!(names.contains(name), "{name}");
    }
}
