//! Goal: on Postgres the precon browser lists every stored precon with the deck check of its cards and whether a game takes it, shows one by board with each card's support, follows the precons and the cards without a restart, and answers an unknown code like a malformed one; a seat plays a precon as its deck and a lobby is made with one in a single request, both held to the rules of any seat's deck (the holder alone chooses, a deck the game cannot play is refused with its issues, the recipe names the cards with their fingerprint); the endpoints are public, throttled per address and leak nothing of any account, and the writes keep the origin guard.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::net;

use std::collections::BTreeSet;
use std::sync::OnceLock;
use std::time::Duration;

use mtg_cards::Caps;
use mtg_db::Database;
use mtg_import::mtgjson::{PreconCard, PreconDeck};
use mtg_import::scryfall::cards;
use mtg_protocol::{LobbySeat, ServerMessage};
use mtg_server::{
    CardSource, DeckFingerprint, GameId, GameRecords, RecordStatus, ServerConfig, migrate,
};
use net::lobby::{self, bearer};
use net::{Browser, PASSWORD, TestServer, WsClient, bot_config, register};
use serde_json::{Value, json};
use sqlx::PgPool;

/// The 40 real cards of the rules corpus, trimmed Scryfall objects without legalities.
const CORPUS: &str = include_str!("../../../mtg-cards/tests/fixtures/corpus_scryfall.json");

fn objects() -> &'static Vec<Value> {
    static OBJECTS: OnceLock<Vec<Value>> = OnceLock::new();
    OBJECTS.get_or_init(|| serde_json::from_str(CORPUS).unwrap())
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

/// The corpus with legalities: every card is legal in Commander.
fn fixture(change: impl Fn(&mut Value)) -> String {
    let mut all = objects().clone();
    for card in &mut all {
        card["legalities"] = json!({"commander": "legal", "modern": "legal"});
        change(card);
    }
    serde_json::to_string(&all).unwrap()
}

fn source(pool: &PgPool) -> CardSource {
    CardSource::new(pool.clone()).with_caps(Caps::everything())
}

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

fn deck(
    name: &str,
    kind: &str,
    release: &str,
    commander: &[(&str, u32)],
    main: &[(&str, u32)],
) -> PreconDeck {
    let cards = |list: &[(&str, u32)]| list.iter().map(|(n, c)| entry(n, *c)).collect();
    PreconDeck {
        name: name.to_owned(),
        code: "TST".to_owned(),
        deck_type: Some(kind.to_owned()),
        release_date: Some(release.to_owned()),
        main_board: cards(main),
        side_board: Vec::new(),
        commander: cards(commander),
        display_commander: Vec::new(),
    }
}

/// Sixty Forests: nothing for the table to resolve by hand, and a Normal game takes it.
fn evergreen() -> PreconDeck {
    deck(
        "Evergreen Pack",
        "Starter Deck",
        "2024-05-01",
        &[],
        &[("Forest", 60)],
    )
}

/// Four copies of a card the rules model in part: playable, with four manual cards.
fn quest() -> PreconDeck {
    deck(
        "Quest Pack",
        "Starter Deck",
        "2023-03-01",
        &[],
        &[("Forest", 56), ("Aven Mindcensor", 4)],
    )
}

/// A Commander deck: legal and playable by hand, but a Normal game has no command zone.
fn commander() -> PreconDeck {
    deck(
        "Commander Pack",
        "Commander Deck",
        "2022-02-02",
        &[("Questing Beast", 1)],
        &[("Forest", 98), ("Courser of Kruphix", 1)],
    )
}

/// A card nobody stored: the deck is blocked.
fn ghost() -> PreconDeck {
    deck(
        "Ghost Pack",
        "Starter Deck",
        "2024-06-01",
        &[],
        &[("Forest", 59), ("No Such Card", 1)],
    )
}

/// Verified for the deck check, but too short for a Normal game.
fn short() -> PreconDeck {
    deck(
        "Short Pack",
        "Starter Deck",
        "2021-01-01",
        &[],
        &[("Forest", 20)],
    )
}

async fn seed(database: &Database, decks: Vec<PreconDeck>) {
    database
        .importer()
        .import_precons(decks.into_iter().map(Ok))
        .await
        .unwrap();
}

async fn start_with(pool: &PgPool, change: impl FnOnce(&mut ServerConfig)) -> TestServer {
    migrate(pool).await.unwrap();
    let database = Database::from_pool(pool.clone());
    database.migrate().await.unwrap();
    database
        .importer()
        .import_cards(cards(fixture(|_| {}).as_bytes()))
        .await
        .unwrap();
    seed(
        &database,
        vec![evergreen(), quest(), commander(), ghost(), short()],
    )
    .await;
    let mut config = bot_config();
    change(&mut config);
    TestServer::start_with_cards(pool.clone(), config, source(pool)).await
}

async fn start(pool: &PgPool) -> TestServer {
    start_with(pool, |_| {}).await
}

const EVERGREEN: &str = "tst-evergreen-pack";
const QUEST: &str = "tst-quest-pack";
const COMMANDER: &str = "tst-commander-pack";
const GHOST: &str = "tst-ghost-pack";
const SHORT: &str = "tst-short-pack";

fn on(code: &str) -> String {
    format!("precon:{code}")
}

async fn precons(server: &TestServer) -> net::Reply {
    net::get(server.addr, "/api/precons").await
}

async fn precon(server: &TestServer, code: &str) -> net::Reply {
    net::get(server.addr, &format!("/api/precons/{code}")).await
}

fn item<'a>(list: &'a Value, code: &str) -> &'a Value {
    list["precons"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["code"] == code)
        .unwrap_or_else(|| panic!("{code} is not listed: {list}"))
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

fn card_names() -> BTreeSet<String> {
    objects()
        .iter()
        .map(|card| card["name"].as_str().unwrap().to_owned())
        .collect()
}

// ---- the list ----

#[sqlx::test(migrations = false)]
async fn the_list_names_every_precon_with_its_deck_check_and_whether_a_game_takes_it(pool: PgPool) {
    let server = start(&pool).await;

    let reply = precons(&server).await;
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert_eq!(reply.header("cache-control"), Some("no-store"));
    let list = reply.json();

    // Newest release first.
    let codes: Vec<&str> = list["precons"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["code"].as_str().unwrap())
        .collect();
    assert_eq!(codes, [GHOST, EVERGREEN, QUEST, COMMANDER, SHORT]);

    let green = item(&list, EVERGREEN);
    assert_eq!(green["name"], "Evergreen Pack");
    assert_eq!(green["set"], "TST");
    assert_eq!(green["deck_type"], "Starter Deck");
    assert_eq!(green["release_date"], "2024-05-01");
    assert_eq!(green["card_count"], 60);
    assert_eq!(green["commanders"], json!([]));
    assert_eq!(green["format"], "freeform");
    assert_eq!(green["state"], "verified");
    assert_eq!(green["verified"], true);
    assert_eq!(green["playable"], true);
    assert_eq!(green["summary"]["counts"]["total"], 60);
    assert_eq!(green["summary"]["counts"]["verified"], 60);

    let quest = item(&list, QUEST);
    assert_eq!(
        (
            quest["state"].clone(),
            quest["verified"].clone(),
            quest["playable"].clone()
        ),
        (json!("manual_play"), json!(false), json!(true))
    );
    assert_eq!(quest["summary"]["counts"]["verified"], 56);
    let flagged = quest["summary"]["counts"]["partial"].as_u64().unwrap()
        + quest["summary"]["counts"]["manual"].as_u64().unwrap();
    assert_eq!(flagged, 4);

    // A Commander deck is judged as one, and a Normal game has no command zone for it.
    let boss = item(&list, COMMANDER);
    assert_eq!(boss["format"], "commander");
    assert_eq!(boss["commanders"], json!(["Questing Beast"]));
    assert_eq!(boss["card_count"], 100);
    assert_eq!(boss["state"], "manual_play");
    assert_eq!(boss["playable"], false);

    // A card nobody stored blocks the deck.
    let lost = item(&list, GHOST);
    assert_eq!(lost["state"], "blocked");
    assert_eq!(lost["playable"], false);
    assert_eq!(lost["summary"]["counts"]["unplayable"], 1);
    assert_eq!(lost["summary"]["issues"], 1);

    // Clean for the deck check, refused by the game.
    let little = item(&list, SHORT);
    assert_eq!(
        (little["state"].clone(), little["playable"].clone()),
        (json!("verified"), json!(false))
    );
}

#[sqlx::test(migrations = false)]
async fn the_endpoints_are_public_and_answer_a_signed_in_account_as_they_answer_anyone(
    pool: PgPool,
) {
    let server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;

    let anonymous = precons(&server).await;
    let signed_in = ann.get(server.addr, "/api/precons").await;
    assert_eq!(signed_in.status, 200);
    assert_eq!(signed_in.text(), anonymous.text());
    let one = ann
        .get(server.addr, &format!("/api/precons/{COMMANDER}"))
        .await;
    assert_eq!(one.text(), precon(&server, COMMANDER).await.text());

    // Nothing about an account is in them.
    let text = format!("{}{}", anonymous.text(), one.text()).to_lowercase();
    for word in ["username", "session", "cookie", &ann.id.to_lowercase()] {
        assert!(!text.contains(word), "{word} in the precons");
    }
}

#[sqlx::test(migrations = false)]
async fn one_precon_lists_its_cards_by_board_with_each_cards_support(pool: PgPool) {
    let server = start(&pool).await;

    let reply = precon(&server, COMMANDER).await;
    assert_eq!(reply.status, 200, "{}", reply.text());
    let view = reply.json();
    assert_eq!(view["precon"]["code"], COMMANDER);
    assert_eq!(view["precon"]["state"], "manual_play");
    let boards: Vec<(&str, u64)> = view["boards"]
        .as_array()
        .unwrap()
        .iter()
        .map(|board| {
            (
                board["board"].as_str().unwrap(),
                board["copies"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(boards, [("commander", 1), ("main", 99)]);

    let lead = &view["boards"][0]["cards"][0];
    assert_eq!(lead["name"], "Questing Beast");
    assert_eq!(lead["count"], 1);
    assert_eq!(lead["card"]["oracle_id"], oracle("Questing Beast"));
    assert_eq!(lead["card"]["type_line"], "Legendary Creature — Beast");
    assert_eq!(lead["support"], "verified", "the rules model it in full");
    let main_board = view["boards"][1]["cards"].as_array().unwrap();
    let find = |name: &str| main_board.iter().find(|card| card["name"] == name).unwrap();
    let land = find("Forest");
    assert_eq!(
        (
            land["name"].clone(),
            land["count"].clone(),
            land["support"].clone()
        ),
        (json!("Forest"), json!(98), json!("verified"))
    );
    assert_eq!(land["reasons"], json!([]));
    let partial = find("Courser of Kruphix");
    assert_ne!(partial["support"], "verified", "the rules model it in part");
    assert!(
        !partial["reasons"].as_array().unwrap().is_empty(),
        "an unsupported clause comes with its reason"
    );

    // Why a game does not take it.
    let issues = view["issues"].as_array().unwrap();
    assert_eq!(issues.len(), 1, "{issues:?}");
    assert_eq!(issues[0]["code"], "no_commander_zone");

    // A playable deck lists nothing against it, and its one board.
    let green = precon(&server, EVERGREEN).await.json();
    assert_eq!(green["issues"], json!([]));
    assert_eq!(green["boards"].as_array().unwrap().len(), 1);
    assert_eq!(green["boards"][0]["board"], "main");
}

#[sqlx::test(migrations = false)]
async fn a_card_the_database_lacks_is_shown_unplayable_without_a_card(pool: PgPool) {
    let server = start(&pool).await;
    let view = precon(&server, GHOST).await.json();
    let cards = view["boards"][0]["cards"].as_array().unwrap();
    let missing = cards
        .iter()
        .find(|card| card["name"] == "No Such Card")
        .unwrap();
    assert_eq!(missing["card"], json!(null));
    assert_eq!(missing["support"], "unplayable");
    assert_eq!(missing["count"], 1);
    assert!(missing["reasons"][0]["text"].is_string());
    assert_eq!(view["issues"][0]["code"], "unknown_card");
    assert_eq!(view["issues"][0]["name"], "No Such Card");
}

#[sqlx::test(migrations = false)]
async fn an_unknown_code_and_a_malformed_one_are_one_404(pool: PgPool) {
    let server = start(&pool).await;
    let unknown = precon(&server, "tst-nothing-here").await;
    assert_eq!(unknown.status, 404);
    assert_eq!(unknown.error_code(), "not_found");
    for code in [
        "",
        "%00",
        "TST_EVERGREEN_PACK",
        "tst-evergreen-pack%2f..",
        "x".repeat(300).as_str(),
    ] {
        let reply = precon(&server, code).await;
        assert!(
            reply.status == 404,
            "{code:?}: {} {}",
            reply.status,
            reply.text()
        );
        if !code.is_empty() {
            assert_eq!(reply.text(), unknown.text(), "{code:?}");
        }
    }
    // The code is not case sensitive: the web may show it as it likes.
    assert_eq!(precon(&server, "TST-EVERGREEN-PACK").await.status, 200);
}

#[sqlx::test(migrations = false)]
async fn the_list_follows_the_precons_and_the_cards_without_a_restart(pool: PgPool) {
    let server = start(&pool).await;
    let database = Database::from_pool(pool.clone());
    let before = precons(&server).await.json();
    assert_eq!(before["precons"].as_array().unwrap().len(), 5);
    assert_eq!(precons(&server).await.json(), before);

    // A new deck shows at once, and a deck that changed shows as it is now.
    seed(
        &database,
        vec![
            deck(
                "Bears Pack",
                "Starter Deck",
                "2025-01-01",
                &[],
                &[("Forest", 56), ("Grizzly Bears", 4)],
            ),
            deck(
                "Evergreen Pack",
                "Starter Deck",
                "2024-05-01",
                &[],
                &[("Forest", 40)],
            ),
        ],
    )
    .await;
    let after = precons(&server).await.json();
    assert_eq!(after["precons"].as_array().unwrap().len(), 6);
    assert_eq!(after["precons"][0]["code"], "tst-bears-pack");
    assert_eq!(item(&after, "tst-bears-pack")["state"], "verified");
    assert_eq!(item(&after, EVERGREEN)["card_count"], 40);
    assert_eq!(item(&after, EVERGREEN)["playable"], false);

    // Rules text on a card that had none: the deck holding it needs the table now.
    database
        .importer()
        .import_cards(cards(
            fixture(|card| {
                if card["name"] == "Grizzly Bears" {
                    card["oracle_text"] = json!("Flying, vigilance and a very long sentence.");
                }
            })
            .as_bytes(),
        ))
        .await
        .unwrap();
    let changed = precons(&server).await.json();
    let bears = item(&changed, "tst-bears-pack");
    assert_eq!(bears["state"], "manual_play");
    assert_eq!(bears["summary"]["counts"]["verified"], 56);
    assert_eq!(
        precon(&server, "tst-bears-pack").await.json()["precon"],
        *bears
    );

    // A deck removed from the database leaves the list and its code is unknown.
    sqlx::query("DELETE FROM precon_decks WHERE name = 'Short Pack'")
        .execute(&pool)
        .await
        .unwrap();
    let gone = precons(&server).await.json();
    assert_eq!(gone["precons"].as_array().unwrap().len(), 5);
    assert_eq!(precon(&server, SHORT).await.status, 404);
}

#[tokio::test]
async fn a_server_without_a_database_has_no_precons_and_names_none_as_a_deck() {
    let server = TestServer::start(bot_config()).await;
    for path in ["/api/precons", "/api/precons/tst-evergreen-pack"] {
        let reply = net::get(server.addr, path).await;
        assert_eq!(
            (reply.status, reply.error_code().as_str()),
            (503, "cards_unavailable"),
            "{path}"
        );
    }
    let body = json!({"format": "normal", "seats": [{"name": "Ann", "deck": on(EVERGREEN)}, {"name": "Bo", "deck": "mono-red"}]});
    let reply = net::post_json(server.addr, "/api/games", &body.to_string()).await;
    assert_eq!(
        (reply.status, reply.error_code().as_str()),
        (400, "unknown_deck")
    );
}

// ---- a seat plays a precon ----

#[sqlx::test(migrations = false)]
async fn a_seat_takes_a_precon_as_its_deck_and_the_table_hears_a_count_and_never_a_card(
    pool: PgPool,
) {
    let server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let mut seen: Vec<String> = Vec::new();

    let game = create(&server, &ann, "mono-green").await;
    let mut socket = net::open(
        server.addr,
        &format!("/ws/{game}"),
        &[("Cookie", &ann.cookie)],
    )
    .await;
    socket.welcome(&game).await;
    roster(&mut socket).await;

    let chosen = choose(&server, &ann, &game, 0, &on(QUEST)).await;
    assert_eq!(chosen.status, 200, "{}", chosen.text());
    assert_eq!(
        chosen.json(),
        json!({"name": "Quest Pack", "format": "freeform", "manual_cards": 4})
    );
    seen.push(chosen.text());
    let table = roster(&mut socket).await;
    let told = table[0].deck.as_ref().unwrap();
    assert_eq!(
        (told.name.as_str(), told.format.as_str(), told.manual_cards),
        ("Quest Pack", "freeform", 4)
    );

    // Any seat may change to another precon until it is ready, by token too, and the code's case does not matter.
    let joined = net::lobby::join(server.addr, &game, &json!({ "name": "Cy" }), &[]).await;
    assert_eq!(joined.status, 200, "{}", joined.text());
    let token = joined.json()["token"].as_str().unwrap().to_owned();
    let guest = net::lobby::call(
        server.addr,
        "PUT",
        &seat_path(&game, 1),
        Some(&json!({ "deck": on(&EVERGREEN.to_uppercase()) }).to_string()),
        &[("Authorization", &bearer(&token))],
    )
    .await;
    assert_eq!(guest.status, 200, "{}", guest.text());
    assert_eq!(
        guest.json(),
        json!({"name": "Evergreen Pack", "format": "freeform", "manual_cards": 0})
    );
    seen.push(guest.text());

    for path in [format!("/api/games/{game}"), "/api/lobbies".to_owned()] {
        seen.push(net::get(server.addr, &path).await.text());
    }
    seen.push(serde_json::to_string(&roster(&mut socket).await).unwrap());
    for text in &seen {
        for name in card_names() {
            assert!(!text.contains(&name), "{name} in {text}");
        }
    }
}

#[sqlx::test(migrations = false)]
async fn only_the_holder_of_a_seat_chooses_its_deck_and_a_precon_changes_nothing_about_that(
    pool: PgPool,
) {
    let server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let dan = register(server.addr, "dan", PASSWORD).await;
    let game = create(&server, &ann, &on(EVERGREEN)).await;
    let joined = net::lobby::join(server.addr, &game, &json!({ "name": "Cy" }), &[]).await;
    let token = joined.json()["token"].as_str().unwrap().to_owned();

    // The host may not choose for the guest's seat, a stranger is told nothing, and so is a visitor with no credential.
    let other = choose(&server, &ann, &game, 1, &on(QUEST)).await;
    assert_eq!(
        (other.status, other.error_code().as_str()),
        (403, "forbidden")
    );
    let stranger = choose(&server, &dan, &game, 1, &on(QUEST)).await;
    assert_eq!(stranger.status, 404);
    let anonymous = net::lobby::call(
        server.addr,
        "PUT",
        &seat_path(&game, 1),
        Some(&json!({ "deck": on(QUEST) }).to_string()),
        &[],
    )
    .await;
    assert_eq!(anonymous.status, 401);
    let unknown_game = choose(
        &server,
        &ann,
        "00000000000000000000000000000000",
        0,
        &on(QUEST),
    )
    .await;
    assert_eq!(unknown_game.status, 404);
    assert_eq!(stranger.text(), unknown_game.text());

    // A seat that pressed Ready keeps its deck until it takes that back.
    let mut socket = net::connect(server.addr, &game, &token).await;
    socket.welcome(&game).await;
    roster(&mut socket).await;
    socket.send(&mtg_protocol::ClientMessage::Ready).await;
    roster(&mut socket).await;
    let late = net::lobby::call(
        server.addr,
        "PUT",
        &seat_path(&game, 1),
        Some(&json!({ "deck": on(QUEST) }).to_string()),
        &[("Authorization", &bearer(&token))],
    )
    .await;
    assert_eq!(
        (late.status, late.error_code().as_str()),
        (409, "seat_ready")
    );
}

#[sqlx::test(migrations = false)]
async fn a_precon_the_game_cannot_take_is_refused_with_its_issues_and_nobody_is_seated_by_it(
    pool: PgPool,
) {
    let server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let game = create(&server, &ann, &on(EVERGREEN)).await;

    for (code, issue) in [
        (COMMANDER, "no_commander_zone"),
        (SHORT, "deck_size"),
        (GHOST, "unknown_card"),
    ] {
        // As a seat's deck.
        let refused = choose(&server, &ann, &game, 0, &on(code)).await;
        assert_eq!(refused.status, 422, "{code}: {}", refused.text());
        assert_eq!(refused.error_code(), "deck_blocked");
        assert_eq!(refused.json()["issues"][0]["code"], issue, "{code}");
        // When a game is made with it.
        let body = json!({"format": "normal", "seats": [{"name": "Ann", "deck": on(code)}, {"name": "B", "deck": "mono-red"}]});
        let at_creation = ann.post(server.addr, "/api/games", &body.to_string()).await;
        assert_eq!(
            (at_creation.status, at_creation.error_code().as_str()),
            (422, "deck_blocked"),
            "{code}"
        );
        // When a joiner brings it.
        let join = net::lobby::join(
            server.addr,
            &game,
            &json!({ "name": "Cy", "deck": on(code) }),
            &[],
        )
        .await;
        assert_eq!(
            (join.status, join.error_code().as_str()),
            (422, "deck_blocked"),
            "{code}"
        );
    }
    let listed = net::get(server.addr, &format!("/api/games/{game}"))
        .await
        .json();
    assert_eq!(listed["seats"][1]["name"], "", "the seat is still open");
    assert_eq!(
        server.registry.len(),
        1,
        "no game was made by a refused deck"
    );

    // An unknown code is the 404 of a deck nobody has; at creation it is a 400.
    let nobody = choose(&server, &ann, &game, 0, &on("tst-nothing-here")).await;
    assert_eq!(
        (nobody.status, nobody.error_code().as_str()),
        (404, "unknown_deck")
    );
    assert_eq!(choose(&server, &ann, &game, 0, "precon:").await.status, 404);
    let body = json!({"format": "normal", "seats": [{"name": "Ann", "deck": on("tst-nothing-here")}, {"name": "B"}]});
    let at_creation = ann.post(server.addr, "/api/games", &body.to_string()).await;
    assert_eq!(
        (at_creation.status, at_creation.error_code().as_str()),
        (400, "unknown_deck")
    );

    // The deck stayed what it was.
    assert_eq!(
        choose(&server, &ann, &game, 0, &on(EVERGREEN)).await.status,
        200
    );
}

// ---- one click ----

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
async fn one_request_makes_a_lobby_with_the_precon_as_the_hosts_deck_and_the_game_starts_with_its_cards(
    pool: PgPool,
) {
    let server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;

    let body = json!({
        "format": "normal", "visibility": "public", "held": 1,
        "seats": [{"name": "ann", "deck": on(QUEST)}, {"deck": "mono-red"}],
    });
    let created = ann.post(server.addr, "/api/games", &body.to_string()).await;
    assert_eq!(created.status, 201, "{}", created.text());
    let game = created.json()["game"].as_str().unwrap().to_owned();
    assert!(created.json()["code"].is_string());

    // The lobby is open to join, and the host's seat shows the precon.
    let listed = lobby::listed(&net::get(server.addr, "/api/lobbies").await);
    assert_eq!(listed, std::slice::from_ref(&game));
    let mut socket = net::open(
        server.addr,
        &format!("/ws/{game}"),
        &[("Cookie", &ann.cookie)],
    )
    .await;
    socket.welcome(&game).await;
    let seats = roster(&mut socket).await;
    assert_eq!(
        seats[0].deck.as_ref().map(|d| d.name.as_str()),
        Some("Quest Pack")
    );

    // The game was made with its recipe; the deck the host chose goes in at the start.
    let joined = net::lobby::join(server.addr, &game, &json!({ "name": "Cy" }), &[]).await;
    assert_eq!(joined.status, 200, "{}", joined.text());
    lobby::start(&server.registry, &game, 2).await;
    let (stored, status) = recipe(&pool, &game).await;
    assert_eq!(status, RecordStatus::Running);
    assert_eq!(stored["decks"][0]["kind"], "oracle");
    let ids: Vec<String> = stored["decks"][0]["cards"]
        .as_array()
        .unwrap()
        .iter()
        .map(|id| id.as_str().unwrap().to_owned())
        .collect();
    let mut expected = [
        vec![oracle("Forest"); 56],
        vec![oracle("Aven Mindcensor"); 4],
    ]
    .concat();
    let mut sorted = ids.clone();
    sorted.sort();
    expected.sort();
    assert_eq!(sorted, expected, "the seat plays the cards of the precon");
    let cards = source(&pool).deck(&ids).await.unwrap();
    assert_eq!(
        stored["fingerprints"][0],
        DeckFingerprint::of(&cards).unwrap().to_string()
    );
}

#[sqlx::test(migrations = false)]
async fn a_guest_makes_a_lobby_with_a_precon_for_every_seat_and_a_stored_deck_is_still_the_hosts_alone(
    pool: PgPool,
) {
    let server = start(&pool).await;

    // No account: precons are public data, so any seat of a new game may start with one.
    let body = json!({
        "format": "normal", "held": 2,
        "seats": [{"name": "Ann", "deck": on(EVERGREEN)}, {"name": "Bo", "deck": on(QUEST)}],
    });
    let made = net::post_json(server.addr, "/api/games", &body.to_string()).await;
    assert_eq!(made.status, 201, "{}", made.text());
    let game = made.json()["game"].as_str().unwrap().to_owned();
    let tokens: Vec<String> = made.json()["seats"]
        .as_array()
        .unwrap()
        .iter()
        .map(|seat| seat["token"].as_str().unwrap().to_owned())
        .collect();
    lobby::start(&server.registry, &game, 2).await;
    let (stored, _) = recipe(&pool, &game).await;
    assert_eq!(stored["decks"][0]["kind"], "oracle");
    assert_eq!(stored["decks"][1]["kind"], "oracle");
    assert_eq!(tokens.len(), 2);

    // A stored deck of the account is the creator's seat alone, as before.
    let ann = register(server.addr, "ann", PASSWORD).await;
    let own = json!({"name": "Mine", "format": "freeform", "notes": "", "cards": [
        {"zone": "main", "oracle_id": oracle("Forest"), "printing_id": null, "count": 60}]});
    let stored_id = ann
        .post(server.addr, "/api/decks", &own.to_string())
        .await
        .json()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let body =
        json!({"format": "normal", "seats": [{"name": "Ann"}, {"name": "Bo", "deck": stored_id}]});
    let refused = ann.post(server.addr, "/api/games", &body.to_string()).await;
    assert_eq!(
        (refused.status, refused.error_code().as_str()),
        (400, "unknown_deck")
    );
}

// ---- access ----

#[sqlx::test(migrations = false)]
async fn the_precon_endpoints_turn_an_address_away_at_its_limit_and_count_every_request(
    pool: PgPool,
) {
    let server = start_with(&pool, |config| {
        config.auth.limits.precon_requests_per_ip = 3;
        config.auth.limits.lobby_window = Duration::from_secs(2);
    })
    .await;

    assert_eq!(precons(&server).await.status, 200);
    assert_eq!(precon(&server, EVERGREEN).await.status, 200);
    // Unknown codes count too, so the endpoint cannot be probed for free.
    assert_eq!(precon(&server, "tst-nothing-here").await.status, 404);
    let turned_away = precons(&server).await;
    assert_eq!(turned_away.status, 429);
    assert_eq!(turned_away.error_code(), "rate_limited");
    assert!(matches!(turned_away.header("retry-after"), Some("1" | "2")));
    assert_eq!(precon(&server, EVERGREEN).await.status, 429);

    // The lobby list and the writes have budgets of their own.
    assert_eq!(net::get(server.addr, "/api/lobbies").await.status, 200);
    let ann = register(server.addr, "ann", PASSWORD).await;
    let game = create(&server, &ann, &on(EVERGREEN)).await;
    assert_eq!(
        choose(&server, &ann, &game, 0, &on(QUEST)).await.status,
        200
    );

    tokio::time::sleep(Duration::from_millis(2_300)).await;
    assert_eq!(precons(&server).await.status, 200);
}

#[sqlx::test(migrations = false)]
async fn a_request_from_another_site_cannot_set_a_precon_or_make_a_lobby_with_one(pool: PgPool) {
    let server = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let game = create(&server, &ann, "mono-green").await;
    let foreign = [("Origin", "https://evil.example")];

    let put = ann
        .send(
            server.addr,
            "PUT",
            &seat_path(&game, 0),
            &json!({ "deck": on(QUEST) }).to_string(),
            &foreign,
        )
        .await;
    assert_eq!(
        (put.status, put.error_code().as_str()),
        (403, "forbidden_origin")
    );
    let body = json!({"format": "normal", "seats": [{"name": "Ann", "deck": on(EVERGREEN)}, {"name": "Bo", "deck": "mono-red"}]});
    let before = server.registry.len();
    let post = ann
        .send(
            server.addr,
            "POST",
            "/api/games",
            &body.to_string(),
            &foreign,
        )
        .await;
    assert_eq!(
        (post.status, post.error_code().as_str()),
        (403, "forbidden_origin")
    );
    assert_eq!(server.registry.len(), before, "no game was made");
    let join = ann
        .send(
            server.addr,
            "POST",
            &format!("/api/games/{game}/join"),
            &json!({ "deck": on(QUEST) }).to_string(),
            &foreign,
        )
        .await;
    assert_eq!(join.status, 403);

    // The seat kept the deck it had, and the server's own site is no foreign site.
    let same = format!("http://{}", server.addr);
    let own = ann
        .send(
            server.addr,
            "PUT",
            &seat_path(&game, 0),
            &json!({ "deck": on(QUEST) }).to_string(),
            &[("Origin", &same)],
        )
        .await;
    assert_eq!(own.status, 200, "{}", own.text());
    assert_eq!(own.json()["name"], "Quest Pack");
}
