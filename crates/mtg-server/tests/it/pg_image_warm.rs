//! Goal: when a game starts, the pictures of the cards its seats play are fetched in the background, once per card and face and two at a time; the game never waits for them, a CDN that fails changes nothing for the table, and a game that is still in its lobby fetches nothing.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::cdn::{Behavior, Cdn};
use crate::common::{Bot, roomy};

use std::collections::BTreeSet;
use std::time::Duration;

use mtg_cards::Caps;
use mtg_db::Database;
use mtg_import::scryfall::cards;
use mtg_protocol::ClientMessage;
use mtg_server::{
    CardSource, DeckRef, GameRecipe, ImageConfig, Images, Registry, ServerConfig, migrate,
};
use serde_json::{Value, json};
use sqlx::PgPool;

/// The cards of the deck and how many times each is in it.
const DECK: [(&str, usize); 6] = [
    ("Forest", 40),
    ("Grizzly Bears", 4),
    ("Llanowar Elves", 4),
    ("Giant Growth", 4),
    ("Sol Ring", 4),
    ("Fireball", 4),
];

/// The corpus Scryfall objects with a `normal` picture on each, named after the card.
fn scryfall() -> Vec<Value> {
    let mut all: Vec<Value> =
        serde_json::from_str(&mtg_testkit::corpus::scryfall_array().unwrap()).unwrap();
    for card in &mut all {
        let tag = card["id"].as_str().unwrap().to_owned();
        card["image_uris"] = json!({
            "normal": format!("https://cards.scryfall.io/normal/front/w/a/{tag}.jpg?1")
        });
    }
    all
}

fn oracle_of(all: &[Value], name: &str) -> String {
    all.iter().find(|card| card["name"] == name).unwrap()["oracle_id"]
        .as_str()
        .unwrap()
        .to_owned()
}

struct Table {
    registry: Registry,
    cdn: Cdn,
    _dir: tempfile::TempDir,
    pictures: BTreeSet<String>,
    recipe: GameRecipe,
}

/// A registry over the corpus whose pictures come from a fake CDN that behaves as `behavior`.
async fn table(pool: &PgPool, behavior: Behavior) -> Table {
    migrate(pool).await.unwrap();
    let db = Database::from_pool(pool.clone());
    db.migrate().await.unwrap();
    let all = scryfall();
    let text = serde_json::to_string(&all).unwrap();
    db.importer()
        .import_cards(cards(text.as_bytes()))
        .await
        .unwrap();
    let cdn = Cdn::start().await;
    cdn.set(behavior);
    let dir = tempfile::tempdir().unwrap();
    let images = Images::new(&ImageConfig {
        cache_dir: dir.path().join("cache"),
        upstream_base: Some(cdn.base()),
        min_gap: Duration::ZERO,
        backoff: Duration::from_millis(5),
        ..ImageConfig::default()
    });
    let source = CardSource::new(pool.clone())
        .with_caps(Caps::everything())
        .with_images(images);
    let config = ServerConfig {
        fixed_seed: Some(5),
        game: roomy(),
        ..ServerConfig::default()
    };
    let registry = Registry::with_card_source(&config, pool.clone(), source);
    let ids: Vec<String> = DECK
        .iter()
        .flat_map(|(name, copies)| std::iter::repeat_n(oracle_of(&all, name), *copies))
        .collect();
    let pictures = DECK
        .iter()
        .map(|(name, _)| {
            let card = all.iter().find(|card| card["name"] == *name).unwrap();
            format!("/normal/front/w/a/{}.jpg?1", card["id"].as_str().unwrap())
        })
        .collect();
    let recipe = GameRecipe::normal(5, [DeckRef::oracle(ids.clone()), DeckRef::oracle(ids)]);
    Table {
        registry,
        cdn,
        _dir: dir,
        pictures,
        recipe,
    }
}

async fn seat_and_ready(table: &Table) -> mtg_server::GameHandle {
    let names = vec!["Bot 0".to_owned(), "Bot 1".to_owned()];
    let created = table
        .registry
        .create_from_recipe(table.recipe.clone(), names, None)
        .await
        .unwrap();
    let handle = table.registry.handle(created.id).unwrap();
    let bots = [
        Bot::join(&handle, 0, 5).await,
        Bot::join(&handle, 1, 5).await,
    ];
    // The lobby alone fetches nothing.
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(table.cdn.count(), 0, "nothing is fetched before the start");
    for bot in &bots {
        handle.send(bot.seat, ClientMessage::Ready).await.unwrap();
    }
    handle
}

async fn until(what: &str, mut done: impl FnMut() -> bool) {
    for _ in 0..200 {
        if done() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("gave up waiting for {what}");
}

#[sqlx::test(migrations = false)]
async fn starting_a_game_fetches_each_card_picture_once(pool: PgPool) {
    let table = table(&pool, Behavior::Serve).await;
    let handle = seat_and_ready(&table).await;
    until("the pictures", || table.cdn.count() >= table.pictures.len()).await;
    // Give a stray duplicate the time to show up.
    tokio::time::sleep(Duration::from_millis(300)).await;
    let paths = table.cdn.paths();
    assert_eq!(paths.len(), DECK.len(), "{paths:?}");
    assert_eq!(
        paths.iter().cloned().collect::<BTreeSet<_>>(),
        table.pictures
    );
    assert!(handle.status().await.unwrap().started);
}

#[sqlx::test(migrations = false)]
async fn the_game_never_waits_for_the_pictures_and_they_come_two_at_a_time(pool: PgPool) {
    let table = table(&pool, Behavior::Delay(Duration::from_secs(2))).await;
    let handle = seat_and_ready(&table).await;
    let started = tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if handle.status().await.unwrap().started {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    assert!(
        started.is_ok(),
        "the game started while the pictures were still coming"
    );
    tokio::time::sleep(Duration::from_millis(300)).await;
    let asked = table.cdn.count();
    assert!(
        (1..=2).contains(&asked),
        "{asked} requests in flight, two at most"
    );
}

#[sqlx::test(migrations = false)]
async fn a_failing_cdn_changes_nothing_for_the_table(pool: PgPool) {
    let table = table(&pool, Behavior::Fail(500)).await;
    let handle = seat_and_ready(&table).await;
    until("the CDN to be asked", || table.cdn.count() > 0).await;
    let status = handle.status().await.unwrap();
    assert!(status.started && status.events > 0);
}
