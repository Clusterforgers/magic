//! Goal: the daily refresh records each run: a first load is marked as such, later runs name the new cards and precons, a run with nothing new loads nothing and stores no new snapshot, a running game defers loading, and every run takes a coverage snapshot.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use mtg_cli::args::RefreshArgs;
use mtg_cli::cache::Cache;
use mtg_cli::refresh::{Fetched, after_fetch};
use mtg_db::{Database, RefreshStatus};
use serde_json::Value;
use sqlx::PgPool;

use common::{fixture, fixture_cache, gzip, progress, put_deck, put_scryfall};

fn args(release: &str) -> RefreshArgs {
    RefreshArgs {
        release: Some(release.to_owned()),
        even_if_games: false,
        force: false,
    }
}

fn fetched(changed: bool) -> Fetched {
    Fetched {
        changed,
        steps: vec!["fetch: test".to_owned()],
    }
}

/// The oracle fixture without the cards named in `left_out`, as the cached Oracle Cards file.
fn put_oracle(cache: &Cache, left_out: &[&str]) {
    let mut cards: Value = serde_json::from_slice(&fixture("scryfall_oracle_cards.json")).unwrap();
    cards
        .as_array_mut()
        .unwrap()
        .retain(|card| !left_out.iter().any(|name| card["name"] == *name));
    let bytes = serde_json::to_vec(&cards).unwrap();
    let file = format!("oracle-cards-{}.json.gz", left_out.len());
    put_scryfall(cache, "oracle_cards", &file, &gzip(&bytes));
}

/// The commander deck fixture under another name and set code.
fn put_other_deck(cache: &Cache) {
    let mut deck: Value = serde_json::from_slice(&fixture("mtgjson_deck_commander.json")).unwrap();
    deck["data"]["name"] = "Second Deck".into();
    deck["data"]["code"] = "TST".into();
    put_deck(cache, "SecondDeck_TST", &serde_json::to_vec(&deck).unwrap());
}

async fn run(db: &Database, cache: &Cache, changed: bool, release: &str) -> String {
    let (progress, _captured) = progress();
    after_fetch(db, cache, &fetched(changed), &args(release), &progress)
        .await
        .unwrap()
}

#[sqlx::test(migrations = false)]
async fn each_run_records_what_it_added_or_why_it_loaded_nothing(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let cache = fixture_cache(dir.path());
    put_oracle(&cache, &["Lightning Bolt"]);
    let db = Database::from_pool(pool.clone());
    let feed = db.card_feed();

    let text = run(&db, &cache, true, "v1").await;
    assert!(text.starts_with("refresh: done"), "{text}");
    assert!(text.contains("(first load)"), "{text}");
    let first = feed.refreshes(1).await.unwrap().remove(0);
    assert_eq!(first.status, RefreshStatus::Succeeded);
    assert!(first.report.first_load);
    assert!(first.report.new_card_count > 0);
    assert_eq!(first.report.new_precons.len(), 1);
    assert_eq!(first.report.new_precons[0].code, "C16");
    assert_eq!(
        first.report.scryfall_updated_at.as_deref(),
        Some("2026-10-18T21:02:00.000+00:00")
    );
    assert_eq!(feed.snapshots(9).await.unwrap().len(), 1);

    put_oracle(&cache, &[]);
    put_other_deck(&cache);
    run(&db, &cache, true, "v1").await;
    let second = feed.refreshes(1).await.unwrap().remove(0);
    assert!(!second.report.first_load);
    assert_eq!(second.report.new_cards, vec!["Lightning Bolt".to_owned()]);
    assert_eq!(second.report.new_card_count, 1);
    let precons: Vec<(&str, &str)> = second
        .report
        .new_precons
        .iter()
        .map(|p| (p.code.as_str(), p.name.as_str()))
        .collect();
    assert_eq!(precons, vec![("TST", "Second Deck")]);
    let snapshots = feed.snapshots(9).await.unwrap().len();

    let text = run(&db, &cache, false, "v1").await;
    assert!(
        text.contains("load: nothing changed since the last run"),
        "{text}"
    );
    let quiet = feed.refreshes(1).await.unwrap().remove(0);
    assert_eq!(quiet.status, RefreshStatus::Succeeded);
    assert_eq!(quiet.report.new_card_count, 0);
    assert!(
        quiet
            .report
            .steps
            .iter()
            .any(|s| s.contains("unchanged since")),
        "{:?}",
        quiet.report.steps
    );
    assert_eq!(feed.snapshots(9).await.unwrap().len(), snapshots);

    let text = run(&db, &cache, false, "v2").await;
    assert!(
        !text.contains("nothing changed"),
        "a new release loads: {text}"
    );
    let latest = feed.snapshots(1).await.unwrap().remove(0);
    assert_eq!(latest.release.as_deref(), Some("v2"));
}

#[sqlx::test(migrations = false)]
async fn a_running_game_defers_loading(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let cache = fixture_cache(dir.path());
    let db = Database::from_pool(pool.clone());
    run(&db, &cache, true, "v1").await;
    sqlx::query("CREATE TABLE games (status text NOT NULL)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO games VALUES ('running')")
        .execute(&pool)
        .await
        .unwrap();
    put_other_deck(&cache);
    let text = run(&db, &cache, true, "v1").await;
    assert!(text.starts_with("refresh: deferred"), "{text}");
    assert!(text.contains("1 game(s) running"), "{text}");
    let run_row = db.card_feed().refreshes(1).await.unwrap().remove(0);
    assert_eq!(run_row.status, RefreshStatus::Deferred);
    assert_eq!(run_row.report.new_precons.len(), 0);
    let stored: i64 = sqlx::query_scalar("SELECT count(*) FROM precon_decks")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored, 1, "the new deck waits for a run without games");
}
