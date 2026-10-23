//! Goal: on Postgres a stored deck names its companion in the game: the card's compiled rules reach the seat's game at two, three and four seats beside a precon and built-in decks that have none, the table is told the deck and never a companion before the opening reveals it, the recipe names it with a fingerprint over it, a restart brings the game back with the companion at the seat, a companion that breaks its condition or a second one refuses the deck with its code in every way a deck is chosen, and a card without a companion ability is no companion for the game.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::Client;
use crate::net;

use std::collections::BTreeSet;
use std::sync::OnceLock;

use mtg_cards::generated::{COMPILER_VERSION, Generation, generate, input_fingerprint};
use mtg_cards::load::Library;
use mtg_cards::{Caps, builtin};
use mtg_db::{Database, ImplStatus, NewRules};
use mtg_engine::companions::CompanionStage;
use mtg_import::ids::OracleId;
use mtg_import::mtgjson::{PreconCard, PreconDeck};
use mtg_import::scryfall::cards;
use mtg_protocol::ServerMessage;
use mtg_server::{
    CardSource, DeckFingerprint, GameId, GameRecords, RecordStatus, migrate, printed_card,
};
use net::lobby::{self, join};
use net::{Browser, PASSWORD, TestServer, bot_config, register};
use serde_json::{Value, json};
use sqlx::PgPool;

const TINY: &str = "Tiny Companion";
const TINY_ID: &str = "00000000-0000-4000-8000-0000000000c1";

/// The Scryfall objects (trimmed, without legalities) of the corpus cards: the fixture files as one JSON array.
fn objects() -> &'static Vec<Value> {
    static OBJECTS: OnceLock<Vec<Value>> = OnceLock::new();
    OBJECTS.get_or_init(|| {
        serde_json::from_str(&mtg_testkit::corpus::scryfall_array().unwrap()).unwrap()
    })
}

/// A card that is no card of the corpus: a companion for decks of permanents with mana value 2 or less.
fn tiny() -> Value {
    json!({
        "id": "00000000-0000-4000-8000-0000000000d1", "oracle_id": TINY_ID, "name": TINY,
        "lang": "en", "layout": "normal", "set": "tst", "collector_number": "1", "rarity": "rare",
        "released_at": "2020-01-01", "digital": false, "mana_cost": "{1}{G}", "cmc": 2.0,
        "type_line": "Creature \u{2014} Cat",
        "oracle_text": "Companion \u{2014} Each permanent card in your starting deck has mana value 2 or less. (If this card is your chosen companion, you may put it into your hand from outside the game for {3} as a sorcery.)",
        "power": "2", "toughness": "2", "colors": ["G"], "color_identity": ["G"],
        "keywords": ["Companion"],
    })
}

fn oracle(name: &str) -> String {
    if name == TINY {
        return TINY_ID.to_owned();
    }
    objects()
        .iter()
        .find(|card| card["name"] == name)
        .unwrap_or_else(|| panic!("the corpus has no {name}"))["oracle_id"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// The corpus and the companion, legal in the formats the decks use.
fn fixture() -> String {
    let mut all = objects().clone();
    all.push(tiny());
    for card in &mut all {
        card["legalities"] = json!({"commander": "legal", "modern": "legal", "legacy": "legal"});
    }
    serde_json::to_string(&all).unwrap()
}

fn source(pool: &PgPool) -> CardSource {
    CardSource::new(pool.clone()).with_caps(Caps::everything())
}

/// The rows `mtg-cli compile` would write for the companion, written by the same functions.
async fn compile_tiny(db: &Database) {
    let stored = db.cards().get(&OracleId(TINY_ID.to_owned())).await.unwrap();
    let stored = stored.unwrap();
    let printed = printed_card(&stored);
    let names = BTreeSet::from([TINY.to_lowercase()]);
    let hand_written = Library::from_sources(&builtin::sources()).unwrap();
    let Generation::Rules(rules) = generate(&printed, &hand_written, &names) else {
        panic!("{TINY} compiles to valid rules");
    };
    db.card_rules()
        .put(&[NewRules {
            oracle_id: stored.oracle_id.clone(),
            rules: serde_json::to_value(&*rules).unwrap(),
            status: ImplStatus::Full,
            fingerprint: input_fingerprint(&printed),
            compiler_version: COMPILER_VERSION.to_owned(),
        }])
        .await
        .unwrap();
}

async fn start(pool: &PgPool) -> (TestServer, Database) {
    migrate(pool).await.unwrap();
    let db = Database::from_pool(pool.clone());
    db.migrate().await.unwrap();
    db.importer()
        .import_cards(cards(fixture().as_bytes()))
        .await
        .unwrap();
    compile_tiny(&db).await;
    let server = TestServer::start_with_cards(pool.clone(), bot_config(), source(pool)).await;
    (server, db)
}

fn line(zone: &str, name: &str, count: u32) -> Value {
    json!({"zone": zone, "oracle_id": oracle(name), "printing_id": null, "count": count})
}

fn deck(name: &str, lines: &[Value]) -> Value {
    json!({"name": name, "format": "modern", "notes": "", "cards": lines})
}

/// Fifty-six Forests and four Grizzly Bears, which the companion's condition allows.
fn fitting() -> Value {
    deck(
        "Small Things",
        &[
            line("main", "Forest", 56),
            line("main", "Grizzly Bears", 4),
            line("companion", TINY, 1),
        ],
    )
}

/// Four Aven Mindcensors have mana value 3, which the condition forbids.
fn too_big() -> Value {
    deck(
        "Big Things",
        &[
            line("main", "Forest", 56),
            line("main", "Aven Mindcensor", 4),
            line("companion", TINY, 1),
        ],
    )
}

fn two_companions() -> Value {
    deck(
        "Two Friends",
        &[line("main", "Forest", 60), line("companion", TINY, 2)],
    )
}

/// A card with no companion ability in the companion slot.
fn no_ability() -> Value {
    deck(
        "Wishful",
        &[
            line("main", "Forest", 60),
            line("companion", "Grizzly Bears", 1),
        ],
    )
}

async fn store(server: &TestServer, who: &Browser, deck: &Value) -> String {
    let reply = who.post(server.addr, "/api/decks", &deck.to_string()).await;
    assert_eq!(reply.status, 201, "{}", reply.text());
    reply.json()["id"].as_str().unwrap().to_owned()
}

fn precon_pack() -> PreconDeck {
    let entry = |name: &str, count| PreconCard {
        count,
        uuid: format!("uuid-{name}"),
        name: name.to_owned(),
        set_code: None,
        number: None,
        is_foil: false,
        scryfall_id: None,
        oracle_id: None,
    };
    PreconDeck {
        name: "Starter Pack".to_owned(),
        code: "TST".to_owned(),
        deck_type: Some("Starter Deck".to_owned()),
        release_date: Some("2024-05-01".to_owned()),
        main_board: vec![entry("Forest", 56), entry("Grizzly Bears", 4)],
        side_board: Vec::new(),
        commander: Vec::new(),
        display_commander: Vec::new(),
    }
}

/// The body of a public game of `seats` seats; seat 0 plays `deck`.
fn body(host: &Browser, seats: usize, deck: &str) -> Value {
    let mut decks = vec![json!({"name": host.username, "deck": deck})];
    decks.extend((1..seats).map(|_| json!({"deck": "mono-red"})));
    json!({"format": "normal", "visibility": "public", "held": 1, "seats": decks})
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

/// The companion of each seat in the snapshot a connection of `seat` is sent.
async fn companions_seen_by(
    server: &TestServer,
    game: &str,
    seat: usize,
) -> Vec<Option<(CompanionStage, Option<String>)>> {
    let handle = server.registry.handle(game.parse().unwrap()).unwrap();
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

/// Seat 0 shows its companion, revealed and named, to every seat of the game; the others show none.
async fn assert_only_the_host_has_one(server: &TestServer, game: &str, seats: usize) {
    for viewer in 0..seats {
        let seen = companions_seen_by(server, game, viewer).await;
        assert_eq!(seen.len(), seats);
        assert_eq!(
            seen[0],
            Some((CompanionStage::Revealed, Some(TINY.to_owned()))),
            "seen by seat {viewer}"
        );
        assert!(seen[1..].iter().all(Option::is_none), "{seen:?}");
    }
}

#[sqlx::test(migrations = false)]
async fn a_stored_deck_plays_its_companion_at_every_table_size_and_a_restart_keeps_it(
    pool: PgPool,
) {
    let (mut server, db) = start(&pool).await;
    db.importer()
        .import_precons([Ok(precon_pack())])
        .await
        .unwrap();
    let ann = register(server.addr, "ann", PASSWORD).await;
    let deck_id = store(&server, &ann, &fitting()).await;
    let listed = net::get(server.addr, "/api/precons").await.json();
    let precon = format!("precon:{}", listed["precons"][0]["code"].as_str().unwrap());
    let mut games = Vec::new();
    let mut seen_texts: Vec<String> = Vec::new();

    for seats in 2..=4 {
        let created = ann
            .post(
                server.addr,
                "/api/games",
                &body(&ann, seats, &deck_id).to_string(),
            )
            .await;
        assert_eq!(created.status, 201, "{}", created.text());
        seen_texts.push(created.text());
        let game = created.json()["game"].as_str().unwrap().to_owned();
        // The last seat brings a precon, which has no companion board; the others keep mono-red.
        for seat in 1..seats {
            let mut joiner = json!({"name": format!("Guest {seat}")});
            if seat + 1 == seats {
                joiner["deck"] = json!(precon);
            }
            let joined = join(server.addr, &game, &joiner, &[]).await;
            assert_eq!(joined.status, 200, "{}", joined.text());
            seen_texts.push(joined.text());
        }
        seen_texts.push(
            net::get(server.addr, &format!("/api/games/{game}"))
                .await
                .text(),
        );
        lobby::start(&server.registry, &game, seats).await;
        assert_only_the_host_has_one(&server, &game, seats).await;
        games.push((game, seats));
    }
    seen_texts.push(net::get(server.addr, "/api/lobbies").await.text());
    for text in &seen_texts {
        assert!(
            !text.to_lowercase().contains("companion") && !text.contains(TINY),
            "the table is told the deck and nothing of its companion: {text}"
        );
    }

    // The recipe names the companion beside the library, and its fingerprint covers the card.
    let assembler = source(&pool);
    let companion = assembler
        .deck(&[TINY_ID.to_owned()])
        .await
        .unwrap()
        .remove(0);
    let mut stored = Vec::new();
    for (game, _) in &games {
        let (recipe, status) = recipe(&pool, game).await;
        assert_eq!(status, RecordStatus::Running);
        assert_eq!(recipe["decks"][0]["companion"], TINY_ID);
        assert!(recipe["decks"][1].get("companion").is_none());
        let ids: Vec<String> = recipe["decks"][0]["cards"]
            .as_array()
            .unwrap()
            .iter()
            .map(|id| id.as_str().unwrap().to_owned())
            .collect();
        assert_eq!(ids.len(), 60, "the companion is no card of the deck");
        let cards = assembler.deck(&ids).await.unwrap();
        let fingerprint = DeckFingerprint::of_seat_with(&cards, &[], Some(&companion))
            .unwrap()
            .to_string();
        assert_eq!(recipe["fingerprints"][0], fingerprint);
        stored.push(recipe);
    }

    // The server stops and starts again: every game is back with its companion at its seat.
    let hashes: Vec<_> = {
        let mut hashes = Vec::new();
        for (game, _) in &games {
            let handle = server.registry.handle(game.parse().unwrap()).unwrap();
            hashes.push(handle.status().await.unwrap().state_hash);
        }
        hashes
    };
    server.shutdown();
    server.finished().await.unwrap();
    drop(server);
    let server = TestServer::start_with_cards(pool.clone(), bot_config(), assembler).await;
    for (((game, seats), hash), before) in games.iter().zip(hashes).zip(stored) {
        let handle = server
            .registry
            .handle(game.parse().unwrap())
            .expect("it came back");
        let status = handle.status().await.unwrap();
        assert!(status.started);
        assert_eq!(status.state_hash, hash, "{seats} seats");
        assert_only_the_host_has_one(&server, game, *seats).await;
        assert_eq!(recipe(&pool, game).await, (before, RecordStatus::Running));
    }
}

#[sqlx::test(migrations = false)]
async fn a_companion_that_breaks_its_condition_refuses_the_deck_wherever_it_is_chosen(
    pool: PgPool,
) {
    let (server, _db) = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let ok = store(&server, &ann, &fitting()).await;
    let game = ann.create_game(server.addr, &body(&ann, 2, &ok)).await.0;
    for (stored, code, name) in [
        (too_big(), "companion_condition", Some(TINY)),
        (two_companions(), "companion_count", None),
    ] {
        let id = store(&server, &ann, &stored).await;
        let by_put = ann
            .put(
                server.addr,
                &format!("/api/games/{game}/seats/0/deck"),
                &json!({ "deck": id }).to_string(),
            )
            .await;
        let at_creation = ann
            .post(server.addr, "/api/games", &body(&ann, 2, &id).to_string())
            .await;
        let by_join = join(server.addr, &game, &json!({"name": "Bo", "deck": id}), &[]).await;
        for reply in [by_put, at_creation] {
            assert_eq!(
                (reply.status, reply.error_code().as_str()),
                (422, "deck_blocked"),
                "{}",
                reply.text()
            );
            let issue = reply.json()["issues"][0].clone();
            assert_eq!(issue["code"], code, "{issue}");
            assert_eq!(issue["zone"], "companion");
            if let Some(name) = name {
                assert_eq!(issue["name"], name);
            }
        }
        // A guest has no session, so a stored deck is the one of nobody.
        assert_eq!(by_join.status, 404, "{}", by_join.text());
    }
    let listed = net::get(server.addr, &format!("/api/games/{game}"))
        .await
        .json();
    assert_eq!(listed["seats"][1]["name"], "", "no seat was taken");
    assert_eq!(
        server.registry.len(),
        1,
        "no game was made by a refused deck"
    );
}

#[sqlx::test(migrations = false)]
async fn a_card_without_a_companion_ability_is_no_companion_for_the_game(pool: PgPool) {
    let (server, _db) = start(&pool).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    let id = store(&server, &ann, &no_ability()).await;
    let game = ann.create_game(server.addr, &body(&ann, 2, &id)).await.0;
    join(server.addr, &game, &json!({"name": "Bo"}), &[]).await;
    lobby::start(&server.registry, &game, 2).await;
    let (recipe, _) = recipe(&pool, &game).await;
    assert!(recipe["decks"][0].get("companion").is_none(), "{recipe}");
    for viewer in 0..2 {
        let seen = companions_seen_by(&server, &game, viewer).await;
        assert_eq!(seen, [None, None]);
    }
}
