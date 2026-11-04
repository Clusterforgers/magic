//! Goal: a card that conjures a named card plays in a game the server builds: the name is resolved from the card database (a card without rules arrives as its printed side), the recipe pins the conjured card's fingerprint, bots conjure in whole games that replay to the same state, a game created through the registry stores the pin, a changed conjured card fails the build with its name, and a name the database lacks conjures nothing without stopping the game.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::roomy;
use mtg_cards::builtin;
use mtg_cards::load::{Library, Source};
use mtg_db::Database;
use mtg_engine::events::Event;
use mtg_import::scryfall::{CardRecord, cards};
use mtg_server::{
    CardSource, DeckFingerprint, DeckRef, GameRecipe, GameRecords, RecipeError, Registry,
    ServerConfig, migrate,
};
use mtg_testkit::play_game;
use sqlx::PgPool;

const GROWTH_ID: &str = "5748ebf1-24e3-499d-ab7c-c2cebd462a24";

fn records() -> Vec<CardRecord> {
    let json = mtg_testkit::corpus::scryfall_array().unwrap();
    cards(json.as_bytes()).map(Result::unwrap).collect()
}

fn id_of(name: &str) -> String {
    let found = records().into_iter().find(|record| record.name == name);
    found.unwrap().oracle_id.to_string()
}

async fn seeded(pool: &PgPool) -> Database {
    migrate(pool).await.unwrap();
    let db = Database::from_pool(pool.clone());
    db.migrate().await.unwrap();
    let json = mtg_testkit::corpus::scryfall_array().unwrap();
    db.importer()
        .import_cards(cards(json.as_bytes()))
        .await
        .unwrap();
    db
}

/// The embedded rules with Giant Growth replaced by a spell that conjures `name` into its caster's hand.
fn conjuring(name: &str, zone: &str) -> Library {
    let text = format!(
        r#"{{"oracle_id":"{GROWTH_ID}","name":"Giant Growth","impl_status":"Full","faces":[{{"name":"Giant Growth",
        "oracle_fingerprint":"94ca5acb80503f883be9d13a1c72ea46",
        "abilities":[{{"Spell":{{"effects":[{{"Conjure":{{"name":"{name}","zone":"{zone}"}}}}]}}}}]}}]}}"#
    );
    let mut sources: Vec<Source> = builtin::sources()
        .into_iter()
        .filter(|source| source.path != "cards/giant_growth.json")
        .collect();
    sources.push(Source {
        path: "cards/giant_growth.json".to_owned(),
        text,
    });
    Library::from_sources(&sources).unwrap_or_else(|errors| panic!("{errors}"))
}

fn ids() -> Vec<String> {
    let mut ids = vec![id_of("Forest"); 56];
    ids.extend(vec![GROWTH_ID.to_owned(); 4]);
    ids
}

fn recipe(seed: u64) -> GameRecipe {
    GameRecipe::normal(seed, [DeckRef::oracle(ids()), DeckRef::oracle(ids())])
}

/// Every conjure of a game played by bots from seeds 0 to 30, with the seed and the outcome.
fn conjures(catalog: &mtg_server::Catalog) -> Vec<(u64, mtg_testkit::GameOutcome)> {
    (0..30)
        .filter_map(|seed| {
            let recipe = recipe(seed).with_fingerprints_from(catalog).unwrap();
            let setup = recipe.build_with(catalog).unwrap();
            let outcome = play_game(setup, seed).unwrap_or_else(|failure| panic!("{failure}"));
            outcome
                .events
                .iter()
                .any(|event| matches!(event, Event::CardConjured(_)))
                .then_some((seed, outcome))
        })
        .collect()
}

#[sqlx::test(migrations = false)]
async fn a_conjured_name_resolves_from_the_database_and_bots_conjure_in_replayable_games(
    pool: PgPool,
) {
    seeded(&pool).await;
    let source = CardSource::new(pool).with_rules(conjuring("Grizzly Bears", "Hand"));
    let catalog = source.catalog(&ids()).await.unwrap();
    let bears = catalog.conjured().get("Grizzly Bears").expect("resolved");
    assert_eq!(bears.name, "Grizzly Bears");
    let played = conjures(&catalog);
    assert!(!played.is_empty(), "a bot casts the spell in some game");
    for (seed, outcome) in played {
        let setup = recipe(seed)
            .with_fingerprints_from(&catalog)
            .unwrap()
            .build_with(&catalog)
            .unwrap();
        let replayed = mtg_engine::replay(&setup, &outcome.events).unwrap();
        assert_eq!(replayed.state_hash(), outcome.final_hash, "seed {seed}");
    }
}

#[sqlx::test(migrations = false)]
async fn the_recipe_pins_the_conjured_card_and_refuses_a_changed_one(pool: PgPool) {
    seeded(&pool).await;
    let source = CardSource::new(pool).with_rules(conjuring("Grizzly Bears", "Graveyard"));
    let catalog = source.catalog(&ids()).await.unwrap();
    let mut recipe = recipe(1).with_fingerprints_from(&catalog).unwrap();
    assert_eq!(
        recipe.conjured.keys().collect::<Vec<_>>(),
        ["Grizzly Bears"],
        "the pin names the card"
    );
    let json = serde_json::to_value(&recipe).unwrap();
    assert!(json["conjured"]["Grizzly Bears"].is_string());
    let again: GameRecipe = serde_json::from_value(json).unwrap();
    assert_eq!(again, recipe, "the pin survives its stored form");
    assert!(recipe.build_with(&catalog).is_ok());
    let other = DeckFingerprint::of(&[]).unwrap();
    recipe.conjured.insert("Grizzly Bears".to_owned(), other);
    let error = recipe.build_with(&catalog).unwrap_err();
    assert!(
        matches!(&error, RecipeError::ConjuredChanged { name, .. } if name == "Grizzly Bears"),
        "{error}"
    );
}

#[sqlx::test(migrations = false)]
async fn a_name_the_database_lacks_conjures_nothing_and_the_game_goes_on(pool: PgPool) {
    seeded(&pool).await;
    let source = CardSource::new(pool).with_rules(conjuring("No Such Card", "Hand"));
    let catalog = source.catalog(&ids()).await.unwrap();
    assert!(catalog.conjured().is_empty());
    assert!(conjures(&catalog).is_empty(), "nothing is conjured");
}

#[sqlx::test(migrations = false)]
async fn a_game_created_through_the_registry_stores_the_pin(pool: PgPool) {
    seeded(&pool).await;
    let config = ServerConfig {
        fixed_seed: Some(5),
        game: roomy(),
        ..ServerConfig::default()
    };
    let source = CardSource::new(pool.clone()).with_rules(conjuring("Grizzly Bears", "Hand"));
    let registry = Registry::with_card_source(&config, pool.clone(), source);
    let names = vec!["Bot 0".to_owned(), "Bot 1".to_owned()];
    let created = registry
        .create_from_recipe(recipe(5), names, None)
        .await
        .unwrap();
    let record = GameRecords::new(pool).get(created.id).await.unwrap();
    let stored = record.unwrap().recipe;
    assert!(stored["conjured"]["Grizzly Bears"].is_string(), "{stored}");
}
