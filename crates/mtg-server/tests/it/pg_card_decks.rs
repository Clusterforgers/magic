//! Goal: a deck of oracle ids is assembled from the card database (the corpus Scryfall fixture imported through `mtg-db`) into the definitions the corpus gives, an id the database lacks is named, bots play whole games with it, a game recorded with such decks resumes unchanged, and a game fails at boot, before anything is replayed, when a card record, a rules file or what the engine runs has changed since it was created.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::{Bot, drive, roomy};
use mtg_cards::load::Library;
use mtg_cards::{Caps, builtin};
use mtg_db::Database;
use mtg_import::scryfall::{CardRecord, cards};
use mtg_protocol::ClientMessage;
use mtg_server::registry::{CreateError, Created};
use mtg_server::{
    CardSource, CatalogError, DeckFingerprint, DeckRef, GameId, GameRecipe, GameRecords,
    RecipeError, RecordStatus, Registry, ServerConfig, migrate, printed_card,
};
use mtg_testkit::corpus::{card, corpus, printed_card as printed_record};
use mtg_testkit::play_game;
use sqlx::PgPool;
use uuid::Uuid;

/// Scryfall objects (trimmed) of the corpus cards, as the importer reads them.
const SCRYFALL: &str = include_str!("../../../mtg-cards/tests/fixtures/corpus_scryfall.json");

fn records() -> Vec<CardRecord> {
    cards(SCRYFALL.as_bytes()).map(Result::unwrap).collect()
}

fn id_of(name: &str) -> String {
    let record = records().into_iter().find(|record| record.name == name);
    record.unwrap().oracle_id.to_string()
}

/// Forty Forests and four each of four cards a Forest can pay for and Fireball, which it cannot.
fn deck_names() -> Vec<&'static str> {
    let mut names = vec!["Forest"; 40];
    for name in [
        "Grizzly Bears",
        "Llanowar Elves",
        "Giant Growth",
        "Sol Ring",
        "Fireball",
    ] {
        names.extend([name; 4]);
    }
    names
}

fn deck_ids() -> Vec<String> {
    deck_names().into_iter().map(id_of).collect()
}

fn oracle(ids: Vec<String>) -> DeckRef {
    DeckRef::oracle(ids)
}

/// Both schemas migrated and the fixture imported: the server's tables for the games and the card
/// database's for the cards.
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

fn config(seed: u64) -> ServerConfig {
    ServerConfig {
        fixed_seed: Some(seed),
        game: roomy(),
        ..ServerConfig::default()
    }
}

#[sqlx::test(migrations = false)]
async fn a_deck_of_oracle_ids_becomes_the_definitions_the_corpus_gives(pool: PgPool) {
    seeded(&pool).await;
    let deck = CardSource::new(pool).deck(&deck_ids()).await.unwrap();
    assert_eq!(deck.len(), 60);
    for (position, (made, name)) in deck.iter().zip(deck_names()).enumerate() {
        assert_eq!(**made, *card(name).unwrap(), "{name} at {position}");
    }
    assert!(
        Arc::ptr_eq(&deck[0], &deck[39]),
        "copies share a definition"
    );
    assert!(!Arc::ptr_eq(&deck[0], &deck[40]));
    let elves = deck.iter().find(|c| c.name == "Llanowar Elves").unwrap();
    assert_eq!(elves.abilities.len(), 1, "the engine runs its mana ability");
}

#[sqlx::test(migrations = false)]
async fn a_stored_card_adapts_to_what_its_import_record_gives(pool: PgPool) {
    let db = seeded(&pool).await;
    let records = records();
    assert_eq!(records.len(), corpus().unwrap().names().count());
    for record in records {
        let row = db.cards().get(&record.oracle_id).await.unwrap().unwrap();
        assert_eq!(
            printed_card(&row),
            printed_record(&record),
            "{}",
            record.name
        );
    }
}

#[sqlx::test(migrations = false)]
async fn every_id_the_database_lacks_is_named_once_in_deck_order(pool: PgPool) {
    seeded(&pool).await;
    let source = CardSource::new(pool);
    let (bears, ghost) = (
        id_of("Grizzly Bears"),
        "00000000-0000-4000-8000-000000000001",
    );
    let ids = vec![
        bears.clone(),
        ghost.to_owned(),
        "not an id".to_owned(),
        bears.clone(),
        ghost.to_owned(),
        id_of("Forest").to_uppercase(),
    ];
    let error = source.deck(&ids).await.unwrap_err();
    assert_eq!(
        error,
        CatalogError::NotFound {
            ids: vec![ghost.to_owned(), "not an id".to_owned()]
        }
    );
    assert_eq!(
        error.to_string(),
        format!("no card for the oracle id(s) {ghost}, not an id")
    );
    // The spelling of an id does not matter, and no ids is no query.
    let known = vec![id_of("Forest").to_uppercase(), bears];
    assert_eq!(source.deck(&known).await.unwrap().len(), 2);
    assert!(
        source.deck(&[]).await.unwrap().is_empty(),
        "no ids, no cards"
    );
}

#[sqlx::test(migrations = false)]
async fn bots_play_whole_games_with_a_deck_of_oracle_ids(pool: PgPool) {
    seeded(&pool).await;
    let source = CardSource::new(pool);
    let catalog = source.catalog(&deck_ids()).await.unwrap();
    assert_eq!(catalog.len(), 6, "six distinct cards");
    for seed in 0..20 {
        let recipe = GameRecipe::normal(seed, [oracle(deck_ids()), oracle(deck_ids())])
            .with_fingerprints_from(&catalog)
            .unwrap();
        let setup = recipe.build_with(&catalog).unwrap();
        play_game(setup, seed).unwrap_or_else(|failure| panic!("seed {seed}: {failure}"));
    }
}

#[sqlx::test(migrations = false)]
async fn the_fingerprint_follows_the_record_the_rules_and_what_the_engine_runs(pool: PgPool) {
    seeded(&pool).await;
    let fingerprint = |source: CardSource| async move {
        DeckFingerprint::of(&source.deck(&deck_ids()).await.unwrap()).unwrap()
    };
    let baseline = fingerprint(CardSource::new(pool.clone())).await;
    assert_eq!(baseline, fingerprint(CardSource::new(pool.clone())).await);

    let other_rules = fingerprint(CardSource::new(pool.clone()).with_rules(elves_make_red())).await;
    let other_engine = fingerprint(CardSource::new(pool.clone()).with_caps(Caps::none())).await;
    // An engine that runs only the plain mana ability gives Llanowar Elves their ability and the other cards no rules.
    let less_engine = fingerprint(
        CardSource::new(pool.clone()).with_caps(Caps::from_names(["Ability.ManaAbility"])),
    )
    .await;
    // The deck is run whole by this engine, so an engine that runs everything changes nothing in it.
    let whole_engine =
        fingerprint(CardSource::new(pool.clone()).with_caps(Caps::everything())).await;
    // No card of the deck has flying, so this engine gives every definition what none does.
    let unrelated_engine =
        fingerprint(CardSource::new(pool.clone()).with_caps(Caps::from_names(["Keyword.Flying"])))
            .await;
    change_bears(&pool).await;
    let other_record = fingerprint(CardSource::new(pool.clone())).await;

    let all = [
        baseline,
        other_rules,
        other_engine,
        other_record,
        less_engine,
    ];
    for (i, a) in all.iter().enumerate() {
        for b in &all[i + 1..] {
            assert_ne!(a, b);
        }
    }
    // A capability no card of the deck needs changes nothing the engine would do, so it changes
    // nothing in the deck either.
    assert_eq!(unrelated_engine, other_engine);
    assert_eq!(whole_engine, baseline);
}

/// The rules of the embedded library with Llanowar Elves tapping for red instead of green.
fn elves_make_red() -> Library {
    let mut sources = builtin::sources();
    let elves = sources
        .iter_mut()
        .find(|source| source.path == "cards/llanowar_elves.json")
        .unwrap();
    assert!(elves.text.contains("\"Green\""));
    elves.text = elves.text.replace("\"Green\"", "\"Red\"");
    Library::from_sources(&sources).unwrap_or_else(|errors| panic!("{errors}"))
}

/// A card record corrected after the game was created: Grizzly Bears is a 3/2 now.
async fn change_bears(pool: &PgPool) {
    let id = Uuid::parse_str(&id_of("Grizzly Bears")).unwrap();
    sqlx::query(
        "UPDATE cards SET faces = jsonb_set(faces, '{0,power}', '\"3\"') WHERE oracle_id = $1",
    )
    .bind(id)
    .execute(pool)
    .await
    .unwrap();
}

fn tokens(created: &Created) -> Vec<String> {
    created
        .seats
        .iter()
        .map(|(_, token)| token.expose().to_owned())
        .collect()
}

/// A recorded two-seat game with decks of oracle ids: the bots are seated, ready, and have given
/// `answers` answers.
async fn started(
    registry: &Registry,
    seed: u64,
    answers: usize,
) -> (GameId, Vec<String>, Vec<Bot>) {
    let recipe = GameRecipe::normal(seed, [oracle(deck_ids()), oracle(deck_ids())]);
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
    (created.id, tokens(&created), bots)
}

#[sqlx::test(migrations = false)]
async fn a_game_with_decks_of_oracle_ids_resumes_with_its_log_and_plays_on(pool: PgPool) {
    seeded(&pool).await;
    let config = config(31);
    let registry = Registry::with_database(&config, pool.clone());
    let (game, tokens, mut bots) = started(&registry, 31, 40).await;
    let before = registry.handle(game).unwrap().status().await.unwrap();
    assert!(before.started && before.events > 10);
    drop(registry);

    let stored = GameRecords::new(pool.clone())
        .get(game)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.recipe["decks"][0]["kind"], "oracle");
    assert_eq!(stored.recipe["fingerprints"].as_array().unwrap().len(), 2);

    let registry = Registry::with_database(&config, pool.clone());
    let report = registry.resume().await.unwrap();
    assert_eq!((report.resumed, report.failed.len()), (1, 0), "{report:?}");
    let handle = registry.handle(game).unwrap();
    let after = handle.status().await.unwrap();
    assert_eq!(
        (after.events, after.state_hash),
        (before.events, before.state_hash)
    );

    for (seat, token) in tokens.iter().enumerate() {
        let access = registry.authenticate(game, token).unwrap();
        bots[seat].rejoin(&access.handle).await;
    }
    assert!(drive(&mut bots, &handle, usize::MAX).await.is_some());
    let record = GameRecords::new(pool).get(game).await.unwrap().unwrap();
    assert_eq!(record.status, RecordStatus::Finished);
}

/// The game of `started`, stopped by a restart that found its cards changed by `source`: it fails
/// with the deck named, no event is replayed, and nobody can reach it.
async fn fails_with_the_deck_changed(pool: PgPool, seed: u64, source: Option<CardSource>) {
    let config = config(seed);
    let registry = Registry::with_database(&config, pool.clone());
    let (game, tokens, _bots) = started(&registry, seed, 40).await;
    let control = started(&registry, seed + 1, 20).await.0;
    drop(registry);
    let events = event_count(&pool, game).await;
    assert!(events > 10);

    let registry = match source {
        Some(source) => Registry::with_card_source(&config, pool.clone(), source),
        None => Registry::with_database(&config, pool.clone()),
    };
    let report = registry.resume().await.unwrap();
    assert_eq!(
        report.failed.len(),
        2,
        "both games name the changed cards: {report:?}"
    );
    assert!(report.failed.contains(&game) && report.failed.contains(&control));
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
    assert_eq!(
        event_count(&pool, game).await,
        events,
        "the log is left as it was"
    );
    assert!(registry.handle(game).is_none());
    for token in &tokens {
        assert!(registry.authenticate(game, token).is_none());
    }
}

async fn event_count(pool: &PgPool, game: GameId) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM game_events WHERE game_id = $1")
        .bind(game.uuid())
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test(migrations = false)]
async fn a_corrected_card_record_fails_the_game_at_boot(pool: PgPool) {
    seeded(&pool).await;
    let registry_pool = pool.clone();
    // Created under the old record, resumed under the corrected one.
    let config = config(41);
    let registry = Registry::with_database(&config, registry_pool);
    let (game, ..) = started(&registry, 41, 30).await;
    drop(registry);
    change_bears(&pool).await;
    let registry = Registry::with_database(&config, pool.clone());
    let report = registry.resume().await.unwrap();
    assert_eq!(report.failed, [game], "{report:?}");
    let record = GameRecords::new(pool).get(game).await.unwrap().unwrap();
    let reason = record.failure.unwrap();
    assert!(
        reason.starts_with("the deck changed: seat 0 (deck of 60 cards by oracle id)"),
        "{reason}"
    );
}

#[sqlx::test(migrations = false)]
async fn a_changed_rules_file_fails_the_game_at_boot(pool: PgPool) {
    seeded(&pool).await;
    fails_with_the_deck_changed(
        pool.clone(),
        42,
        Some(CardSource::new(pool).with_rules(elves_make_red())),
    )
    .await;
}

#[sqlx::test(migrations = false)]
async fn a_change_in_what_the_engine_runs_fails_the_game_at_boot(pool: PgPool) {
    seeded(&pool).await;
    // The engine of the next deploy runs the elf's ability differently from this one: here, not at all.
    fails_with_the_deck_changed(
        pool.clone(),
        43,
        Some(CardSource::new(pool).with_caps(Caps::none())),
    )
    .await;
}

#[sqlx::test(migrations = false)]
async fn a_game_that_names_a_card_the_database_lost_fails_at_boot_with_its_id(pool: PgPool) {
    seeded(&pool).await;
    let config = config(44);
    let registry = Registry::with_database(&config, pool.clone());
    let (game, ..) = started(&registry, 44, 10).await;
    drop(registry);
    let gone = Uuid::parse_str(&id_of("Fireball")).unwrap();
    sqlx::query("DELETE FROM cards WHERE oracle_id = $1")
        .bind(gone)
        .execute(&pool)
        .await
        .unwrap();
    let registry = Registry::with_database(&config, pool.clone());
    let report = registry.resume().await.unwrap();
    assert_eq!(report.failed, [game]);
    let reason = GameRecords::new(pool)
        .get(game)
        .await
        .unwrap()
        .unwrap()
        .failure
        .unwrap();
    assert!(reason.contains(&id_of("Fireball")), "{reason}");
    assert!(reason.contains("no card for the oracle id"), "{reason}");
}

#[sqlx::test(migrations = false)]
async fn creating_a_game_with_an_unknown_card_names_it(pool: PgPool) {
    seeded(&pool).await;
    let registry = Registry::with_database(&config(45), pool);
    let mut ids = deck_ids();
    ids[7] = "00000000-0000-4000-8000-0000000000aa".to_owned();
    let recipe = GameRecipe::normal(45, [oracle(ids.clone()), oracle(deck_ids())]);
    let error = registry
        .create_from_recipe(recipe, vec![], None)
        .await
        .unwrap_err();
    assert_eq!(
        error,
        CreateError::Cards(CatalogError::NotFound {
            ids: vec![ids[7].clone()]
        })
    );
    assert!(registry.is_empty(), "nothing was created");
}

#[tokio::test]
async fn a_registry_without_a_database_has_no_cards_for_a_deck_of_oracle_ids() {
    let registry = Registry::new(&config(46));
    let recipe = GameRecipe::normal(46, [oracle(deck_ids()), oracle(deck_ids())]);
    let error = registry
        .create_from_recipe(recipe, vec![], None)
        .await
        .unwrap_err();
    assert!(
        matches!(
            error,
            CreateError::Recipe(RecipeError::UnknownCards { seat: 0, .. })
        ),
        "{error}"
    );
}
