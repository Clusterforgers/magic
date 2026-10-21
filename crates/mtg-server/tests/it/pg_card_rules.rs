//! Goal: the rules the compiler stored in `card_rules` reach the server without a restart: a card with no rules file is played by hand until its row is stored, then it is verified in the card list, the deck check, a saved deck's marker and the precon list, and a changed or removed row moves them back; a rules file wins over a row for the same card; bots play whole games with a compiled card; and a game never changes under new rows: a running game keeps its definitions, and a restart that finds the rows changed fails the game with the deck named, as it does for any changed rules.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;
use crate::net;

use std::collections::BTreeSet;

use common::{Bot, drive, roomy};
use mtg_cards::card::ImplStatus as RulesStatus;
use mtg_cards::generated::{COMPILER_VERSION, Generation, generate, input_fingerprint};
use mtg_cards::load::{Library, Source, slug};
use mtg_cards::{Caps, builtin};
use mtg_db::{Database, ImplStatus, NewRules};
use mtg_import::ids::OracleId;
use mtg_import::mtgjson::{PreconCard, PreconDeck};
use mtg_import::scryfall::{CardRecord, cards};
use mtg_protocol::ClientMessage;
use mtg_server::{
    CardSource, DeckRef, GameId, GameRecipe, GameRecords, RecordStatus, Registry, ServerConfig,
    migrate, printed_card,
};
use mtg_testkit::corpus::card;
use mtg_testkit::play_game;
use net::{Browser, PASSWORD, TestServer, bot_config, register};
use serde_json::{Value, json};
use sqlx::PgPool;

/// Scryfall objects (trimmed) of the corpus cards, as the importer reads them.
const SCRYFALL: &str = include_str!("../../../mtg-cards/tests/fixtures/corpus_scryfall.json");

/// The card the tests keep out of the rules files and compile instead.
const GROWTH: &str = "Giant Growth";

fn records() -> Vec<CardRecord> {
    cards(SCRYFALL.as_bytes()).map(Result::unwrap).collect()
}

fn id_of(name: &str) -> String {
    let record = records().into_iter().find(|record| record.name == name);
    record.unwrap().oracle_id.to_string()
}

/// The embedded rules without the files of these cards: the cards have no hand-written rules.
fn without(names: &[&str]) -> Library {
    let skipped: Vec<String> = names
        .iter()
        .map(|name| format!("cards/{}.json", slug(name)))
        .collect();
    let sources: Vec<Source> = builtin::sources()
        .into_iter()
        .filter(|source| !skipped.contains(&source.path))
        .collect();
    Library::from_sources(&sources).unwrap_or_else(|errors| panic!("{errors}"))
}

/// Both schemas migrated and the corpus imported, with the legalities the deck check reads.
async fn seeded(pool: &PgPool) -> Database {
    migrate(pool).await.unwrap();
    let db = Database::from_pool(pool.clone());
    db.migrate().await.unwrap();
    db.importer()
        .import_cards(cards(SCRYFALL.as_bytes()))
        .await
        .unwrap();
    db
}

/// The rows `mtg-cli compile` would write for the card, written by the same functions.
async fn store(db: &Database, hand_written: &Library, name: &str) {
    let stored = db
        .cards()
        .get(&OracleId(id_of(name)))
        .await
        .unwrap()
        .unwrap();
    let printed = printed_card(&stored);
    let names = BTreeSet::from([name.to_lowercase()]);
    let Generation::Rules(rules) = generate(&printed, hand_written, &names) else {
        panic!("{name} compiles to valid rules");
    };
    let status = match rules.impl_status {
        RulesStatus::Full => ImplStatus::Full,
        _ => ImplStatus::Partial,
    };
    db.card_rules()
        .put(&[NewRules {
            oracle_id: stored.oracle_id.clone(),
            rules: serde_json::to_value(&*rules).unwrap(),
            status,
            fingerprint: input_fingerprint(&printed),
            compiler_version: COMPILER_VERSION.to_owned(),
        }])
        .await
        .unwrap();
}

/// Rewrites the stored rules of Giant Growth to give +4/+4.
async fn make_it_plus_four(pool: &PgPool) {
    sqlx::query(
        "UPDATE card_rules SET updated_at = now(),
            rules = jsonb_set(rules, '{faces,0,abilities,0,Spell,effects,0,Continuous,mods,0,ModifyPT,power}', '4')
         WHERE oracle_id = $1::uuid",
    )
    .bind(id_of(GROWTH))
    .execute(pool)
    .await
    .unwrap();
}

async fn remove_row(db: &Database) {
    let removed = db
        .card_rules()
        .remove(&[OracleId(id_of(GROWTH))])
        .await
        .unwrap();
    assert_eq!(removed, 1);
}

// ---- the router: card list, deck check, saved decks and precons ----

fn line(zone: &str, name: &str, count: u32) -> Value {
    json!({"zone": zone, "oracle_id": id_of(name), "printing_id": null, "count": count})
}

/// Four Giant Growth and fifty-six Forests, under the rules of a Normal game and no format's legality.
fn growth_deck() -> Value {
    json!({
        "name": "Growth", "format": "freeform", "notes": "",
        "cards": [line("main", "Forest", 56), line("main", GROWTH, 4)],
    })
}

async fn serve(pool: &PgPool) -> (TestServer, Database, Browser) {
    let db = seeded(pool).await;
    let source = CardSource::new(pool.clone())
        .with_rules(without(&[GROWTH]))
        .with_caps(Caps::everything());
    let server = TestServer::start_with_cards(pool.clone(), bot_config(), source).await;
    let ann = register(server.addr, "ann", PASSWORD).await;
    (server, db, ann)
}

async fn support_of_growth(server: &TestServer) -> Value {
    let reply = net::get(server.addr, &format!("/api/cards/{}", id_of(GROWTH))).await;
    assert_eq!(reply.status, 200, "{}", reply.text());
    reply.json()["card"]["support"].clone()
}

async fn check(server: &TestServer, ann: &Browser) -> Value {
    let deck = growth_deck();
    let unsaved = json!({"format": deck["format"], "cards": deck["cards"]});
    let reply = ann
        .post(server.addr, "/api/decks/check", &unsaved.to_string())
        .await;
    assert_eq!(reply.status, 200, "{}", reply.text());
    reply.json()
}

#[sqlx::test(migrations = false)]
async fn a_stored_row_moves_a_card_from_played_by_hand_to_verified_in_the_card_list_and_the_deck_check(
    pool: PgPool,
) {
    let (server, db, ann) = serve(&pool).await;
    assert_eq!(support_of_growth(&server).await, "manual");
    let report = check(&server, &ann).await;
    assert_eq!(report["summary"]["state"], "manual_play");
    assert_eq!(report["summary"]["counts"]["manual"], 4);

    store(&db, &without(&[GROWTH]), GROWTH).await;

    assert_eq!(support_of_growth(&server).await, "verified");
    let report = check(&server, &ann).await;
    assert_eq!(report["summary"]["state"], "verified");
    assert_eq!(report["summary"]["counts"]["verified"], 60);
    let search = net::get(server.addr, "/api/cards?q=giant+growth")
        .await
        .json();
    assert_eq!(search["cards"][0]["support"], "verified");

    remove_row(&db).await;

    assert_eq!(support_of_growth(&server).await, "manual");
    assert_eq!(
        check(&server, &ann).await["summary"]["state"],
        "manual_play"
    );
}

#[sqlx::test(migrations = false)]
async fn a_saved_deck_is_marked_changed_when_the_rules_of_a_card_in_it_change(pool: PgPool) {
    let (server, db, ann) = serve(&pool).await;
    let hand_written = without(&[GROWTH]);
    store(&db, &hand_written, GROWTH).await;
    let saved = ann
        .post(server.addr, "/api/decks", &growth_deck().to_string())
        .await;
    assert_eq!(saved.status, 201, "{}", saved.text());
    let (id, view) = (
        saved.json()["id"].as_str().unwrap().to_owned(),
        saved.json(),
    );
    assert_eq!(
        (view["state"].as_str(), view["verified"].as_bool()),
        (Some("verified"), Some(true))
    );
    let path = format!("/api/decks/{id}");

    make_it_plus_four(&pool).await;
    let after = ann.get(server.addr, &path).await.json();
    assert_eq!(after["state"], "unchecked", "other rules, another deck");
    assert_eq!(after["changed"], true);
    assert_eq!(after["check"]["coverage_hash"].as_str().is_some(), true);
    assert_ne!(
        after["check"]["coverage_hash"],
        after["report"]["coverage_hash"]
    );

    remove_row(&db).await;
    let gone = ann.get(server.addr, &path).await.json();
    assert_eq!(
        (gone["state"].as_str(), gone["changed"].as_bool()),
        (Some("unchecked"), Some(true))
    );

    let again = ann
        .put(server.addr, &path, &growth_deck().to_string())
        .await
        .json();
    assert_eq!(
        again["state"], "manual_play",
        "saved again under the rules of now"
    );
}

fn growth_pack() -> PreconDeck {
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
        name: "Growth Pack".to_owned(),
        code: "TST".to_owned(),
        deck_type: Some("Starter Deck".to_owned()),
        release_date: Some("2024-05-01".to_owned()),
        main_board: vec![entry("Forest", 56), entry(GROWTH, 4)],
        side_board: Vec::new(),
        commander: Vec::new(),
        display_commander: Vec::new(),
    }
}

#[sqlx::test(migrations = false)]
async fn the_precon_list_and_the_precon_view_follow_the_compiled_rules(pool: PgPool) {
    let (server, db, _ann) = serve(&pool).await;
    db.importer()
        .import_precons([Ok(growth_pack())])
        .await
        .unwrap();
    let state = |list: &Value| list["precons"][0]["state"].clone();

    let list = net::get(server.addr, "/api/precons").await.json();
    assert_eq!(state(&list), "manual_play");
    assert_eq!(list["precons"][0]["summary"]["counts"]["manual"], 4);

    store(&db, &without(&[GROWTH]), GROWTH).await;

    let list = net::get(server.addr, "/api/precons").await.json();
    assert_eq!(state(&list), "verified");
    assert_eq!(list["precons"][0]["verified"], true);
    let code = list["precons"][0]["code"].as_str().unwrap().to_owned();
    let view = net::get(server.addr, &format!("/api/precons/{code}"))
        .await
        .json();
    let growth = view["boards"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|board| board["cards"].as_array().unwrap())
        .find(|entry| entry["card"]["name"] == GROWTH)
        .unwrap_or_else(|| panic!("{view}"));
    assert_eq!(growth["card"]["support"], "verified");

    make_it_plus_four(&pool).await;
    remove_row(&db).await;
    let list = net::get(server.addr, "/api/precons").await.json();
    assert_eq!(state(&list), "manual_play", "the index was built again");
}

// ---- the definitions a game is given ----

/// Forty Forests and four each of Giant Growth and four cards a game can do without.
fn ids() -> Vec<String> {
    let mut names = vec!["Forest"; 40];
    for name in [
        GROWTH,
        "Grizzly Bears",
        "Llanowar Elves",
        "Sol Ring",
        "Fireball",
    ] {
        names.extend([name; 4]);
    }
    names.into_iter().map(id_of).collect()
}

fn oracle() -> DeckRef {
    DeckRef::Oracle { cards: ids() }
}

#[sqlx::test(migrations = false)]
async fn a_compiled_card_has_its_abilities_and_a_rules_file_wins_over_its_row(pool: PgPool) {
    let db = seeded(&pool).await;
    let hand_written = without(&[GROWTH]);
    let by_hand = CardSource::new(pool.clone()).with_rules(hand_written.clone());
    let abilities = |source: CardSource| async move {
        let deck = source.deck(&[id_of(GROWTH)]).await.unwrap();
        deck[0].abilities.len() + deck[0].rules.len()
    };
    assert_eq!(abilities(by_hand.clone()).await, 0, "played by hand");

    store(&db, &hand_written, GROWTH).await;

    // The same source reads the row it did not have when it was made.
    assert_eq!(abilities(by_hand).await, 1, "the compiled spell");
    let compiled = CardSource::new(pool.clone())
        .with_rules(hand_written)
        .deck(&[id_of(GROWTH)])
        .await
        .unwrap();
    assert_eq!(*compiled[0], *card(GROWTH).unwrap(), "what the file says");

    // With the file back, a row that says something else loses to it.
    make_it_plus_four(&pool).await;
    let deck = CardSource::new(pool).deck(&[id_of(GROWTH)]).await.unwrap();
    assert_eq!(
        *deck[0],
        *card(GROWTH).unwrap(),
        "the hand-written file wins"
    );
}

#[sqlx::test(migrations = false)]
async fn bots_play_whole_games_with_a_compiled_card(pool: PgPool) {
    let db = seeded(&pool).await;
    let hand_written = without(&[GROWTH]);
    store(&db, &hand_written, GROWTH).await;
    let source = CardSource::new(pool).with_rules(hand_written);
    let catalog = source.catalog(&ids()).await.unwrap();
    for seed in 0..10 {
        let recipe = GameRecipe::normal(seed, [oracle(), oracle()])
            .with_fingerprints_from(&catalog)
            .unwrap();
        let setup = recipe.build_with(&catalog).unwrap();
        play_game(setup, seed).unwrap_or_else(|failure| panic!("seed {seed}: {failure}"));
    }
}

// ---- games and new rows ----

fn config(seed: u64) -> ServerConfig {
    ServerConfig {
        fixed_seed: Some(seed),
        game: roomy(),
        ..ServerConfig::default()
    }
}

fn registry(pool: &PgPool, config: &ServerConfig) -> Registry {
    let source = CardSource::new(pool.clone()).with_rules(without(&[GROWTH]));
    Registry::with_card_source(config, pool.clone(), source)
}

/// A recorded two-seat game with a compiled card in both decks: the bots are seated, ready, and
/// have given `answers` answers.
async fn started(registry: &Registry, seed: u64, answers: usize) -> (GameId, Vec<Bot>) {
    let recipe = GameRecipe::normal(seed, [oracle(), oracle()]);
    let names = vec!["Bot 0".to_owned(), "Bot 1".to_owned()];
    let created = registry
        .create_from_recipe(recipe, names, None)
        .await
        .unwrap();
    let handle = registry.handle(created.id).unwrap();
    let mut bots = vec![
        Bot::join(&handle, 0, seed).await,
        Bot::join(&handle, 1, seed).await,
    ];
    for bot in &bots {
        handle.send(bot.seat, ClientMessage::Ready).await.unwrap();
    }
    assert_eq!(drive(&mut bots, &handle, answers).await, None);
    (created.id, bots)
}

async fn fingerprints(pool: &PgPool, game: GameId) -> Value {
    let record = GameRecords::new(pool.clone())
        .get(game)
        .await
        .unwrap()
        .unwrap();
    record.recipe["fingerprints"].clone()
}

#[sqlx::test(migrations = false)]
async fn a_game_with_a_compiled_card_resumes_unchanged_while_its_rows_are_unchanged(pool: PgPool) {
    let db = seeded(&pool).await;
    store(&db, &without(&[GROWTH]), GROWTH).await;
    let config = config(51);
    let first = registry(&pool, &config);
    let (game, _bots) = started(&first, 51, 40).await;
    let before = first.handle(game).unwrap().status().await.unwrap();
    assert!(before.started && before.events > 10);
    drop(first);

    let second = registry(&pool, &config);
    let report = second.resume().await.unwrap();

    assert_eq!((report.resumed, report.failed.len()), (1, 0), "{report:?}");
    let after = second.handle(game).unwrap().status().await.unwrap();
    assert_eq!(
        (after.events, after.state_hash),
        (before.events, before.state_hash)
    );
}

#[sqlx::test(migrations = false)]
async fn a_row_written_again_while_a_game_is_down_fails_it_at_boot(pool: PgPool) {
    let db = seeded(&pool).await;
    store(&db, &without(&[GROWTH]), GROWTH).await;
    let config = config(52);
    let first = registry(&pool, &config);
    let (game, _bots) = started(&first, 52, 40).await;
    let events = event_count(&pool, game).await;
    drop(first);

    make_it_plus_four(&pool).await;
    let second = registry(&pool, &config);
    let report = second.resume().await.unwrap();

    assert_eq!(report.failed, [game], "{report:?}");
    let record = GameRecords::new(pool.clone())
        .get(game)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(record.status, RecordStatus::Failed);
    let reason = record.failure.unwrap();
    assert!(
        reason.starts_with("the deck changed: seat 0 (deck of 60 cards by oracle id)"),
        "{reason}"
    );
    assert!(second.handle(game).is_none());
    assert_eq!(
        event_count(&pool, game).await,
        events,
        "the log is as it was"
    );
}

async fn event_count(pool: &PgPool, game: GameId) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM game_events WHERE game_id = $1")
        .bind(game.uuid())
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test(migrations = false)]
async fn a_row_removed_while_a_game_is_down_fails_it_at_boot(pool: PgPool) {
    let db = seeded(&pool).await;
    store(&db, &without(&[GROWTH]), GROWTH).await;
    let config = config(54);
    let first = registry(&pool, &config);
    let (game, _bots) = started(&first, 54, 40).await;
    drop(first);

    remove_row(&db).await;
    let report = registry(&pool, &config).resume().await.unwrap();

    assert_eq!(report.failed, [game], "{report:?}");
}

#[sqlx::test(migrations = false)]
async fn a_game_that_is_running_keeps_the_rules_it_began_with_and_a_new_game_takes_the_new_ones(
    pool: PgPool,
) {
    let db = seeded(&pool).await;
    store(&db, &without(&[GROWTH]), GROWTH).await;
    let config = config(55);
    let registry = registry(&pool, &config);
    let (old, mut bots) = started(&registry, 55, 40).await;
    let handle = registry.handle(old).unwrap();
    let before = fingerprints(&pool, old).await;

    // Mid-game the compiler writes the card again.
    make_it_plus_four(&pool).await;
    let (new, _others) = started(&registry, 55, 10).await;

    assert_ne!(
        before,
        fingerprints(&pool, new).await,
        "the new game was given the new rules"
    );
    assert_eq!(
        fingerprints(&pool, old).await,
        before,
        "the old recipe is as it was"
    );
    // The bots held back their last answers at the limit; asking again for the table brings them back.
    for bot in &mut bots {
        bot.rejoin(&handle).await;
    }
    assert!(
        drive(&mut bots, &handle, usize::MAX).await.is_some(),
        "the old game plays to its end under its own rules"
    );
    let record = GameRecords::new(pool).get(old).await.unwrap().unwrap();
    assert_eq!(record.status, RecordStatus::Finished);
}
