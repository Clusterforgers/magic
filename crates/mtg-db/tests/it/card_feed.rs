//! Goal: refresh runs record what they saw and end once; a snapshot is stored only when it differs from the newest, the newest two keep their missing cards, lists are capped, and the helpers for the diff read the stored cards, precons and running games.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use mtg_db::{
    MAX_LISTED, MissingCard, NewPrecon, NewSnapshot, RankBucket, RefreshReport, RefreshStatus,
    StateCounts, Stored,
};
use rstest::rstest;
use sqlx::PgPool;

fn missing(name: &str) -> MissingCard {
    MissingCard {
        oracle_id: format!("id-{name}"),
        name: name.to_owned(),
        state: "partial".to_owned(),
        blocker: "A triggered ability is not read yet".to_owned(),
        detail: "triggered: When ~ dies, N".to_owned(),
        blockers: 1,
        rank: Some(10),
    }
}

fn snapshot(release: &str, automatic: u32, missing_names: &[&str]) -> NewSnapshot {
    NewSnapshot {
        release: Some(release.to_owned()),
        cards: 100,
        automatic,
        states: StateCounts {
            verified: automatic,
            partial: 100 - automatic,
            ..StateCounts::default()
        },
        buckets: vec![RankBucket {
            label: "top 100".to_owned(),
            cards: 100,
            verified: automatic,
            automatic,
        }],
        missing: missing_names.iter().map(|name| missing(name)).collect(),
        newly_supported_count: 0,
        newly_supported: Vec::new(),
    }
}

#[rstest]
#[case::succeeded(RefreshStatus::Succeeded, RefreshStatus::Succeeded)]
#[case::deferred(RefreshStatus::Deferred, RefreshStatus::Deferred)]
#[case::failed(RefreshStatus::Failed, RefreshStatus::Failed)]
#[case::running_is_no_end(RefreshStatus::Running, RefreshStatus::Failed)]
#[sqlx::test]
async fn a_refresh_run_ends_once_with_what_it_found(
    #[case] given: RefreshStatus,
    #[case] stored: RefreshStatus,
    #[ignore] pool: PgPool,
) {
    let db = common::db(pool);
    let feed = db.card_feed();
    let id = feed.start_refresh(Some("v0.2.1")).await.unwrap();
    let running = feed.refreshes(5).await.unwrap();
    assert_eq!(running[0].status, RefreshStatus::Running);
    assert_eq!(running[0].finished_at, None);
    let names: Vec<String> = (0..MAX_LISTED + 5).map(|n| format!("Card {n}")).collect();
    let report = RefreshReport {
        scryfall_updated_at: Some("2026-11-12T10:02:02.801+00:00".to_owned()),
        mtgjson_build: Some("5.3.0+20261022".to_owned()),
        new_card_count: u32::try_from(names.len()).unwrap(),
        new_cards: names,
        new_precons: vec![NewPrecon {
            code: "STC".to_owned(),
            name: "Final Frontier".to_owned(),
            set_name: Some("Star Trek Commander".to_owned()),
        }],
        first_load: false,
        steps: vec!["fetch scryfall: up to date".to_owned()],
        failures: vec!["ingest: boom".to_owned()],
    };
    feed.finish_refresh(id, given, &report).await.unwrap();
    feed.finish_refresh(id, RefreshStatus::Succeeded, &RefreshReport::default())
        .await
        .unwrap();
    let run = feed.refreshes(5).await.unwrap().remove(0);
    assert_eq!(run.id, id);
    assert_eq!(run.status, stored);
    assert_eq!(run.release.as_deref(), Some("v0.2.1"));
    assert!(run.finished_at.is_some());
    assert_eq!(run.report.new_cards.len(), MAX_LISTED);
    assert_eq!(run.report.new_card_count as usize, MAX_LISTED + 5);
    assert_eq!(run.report.new_precons, report.new_precons);
    assert_eq!(run.report.failures, report.failures);
    assert_eq!(run.report.mtgjson_build, report.mtgjson_build);
}

#[sqlx::test]
async fn refresh_runs_list_the_newest_first(pool: PgPool) {
    let feed = common::db(pool).card_feed();
    let first = feed.start_refresh(None).await.unwrap();
    let second = feed.start_refresh(Some("")).await.unwrap();
    let runs = feed.refreshes(1).await.unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].id, second);
    assert_eq!(runs[0].release, None, "an empty release is none");
    assert_eq!(feed.refreshes(9).await.unwrap()[1].id, first);
}

#[sqlx::test]
async fn a_snapshot_is_stored_only_when_it_changed(pool: PgPool) {
    let feed = common::db(pool).card_feed();
    let first = feed
        .store_snapshot(&snapshot("v1", 90, &["A", "B"]))
        .await
        .unwrap();
    let Stored::New(first) = first else {
        panic!("the first snapshot is new")
    };
    let again = feed
        .store_snapshot(&snapshot("v1", 90, &["A", "B"]))
        .await
        .unwrap();
    assert_eq!(again, Stored::Unchanged(first));
    for changed in [
        snapshot("v2", 90, &["A", "B"]),
        snapshot("v2", 91, &["A"]),
        snapshot("v2", 91, &["B"]),
    ] {
        assert!(matches!(
            feed.store_snapshot(&changed).await.unwrap(),
            Stored::New(_)
        ));
    }
    let all = feed.snapshots(10).await.unwrap();
    assert_eq!(all.len(), 4);
    assert_eq!(all[0].missing_count, 1);
    assert_eq!(all[3].id, first);
    assert_eq!(all[0].release.as_deref(), Some("v2"));
    let (id, cards) = feed.latest_missing().await.unwrap().unwrap();
    assert_eq!((id, cards), (all[0].id, vec![missing("B")]));
}

#[sqlx::test]
async fn only_the_newest_two_snapshots_keep_their_missing_cards(pool: PgPool) {
    let feed = common::db(pool.clone()).card_feed();
    for automatic in 80..85 {
        feed.store_snapshot(&snapshot("v1", automatic, &["A"]))
            .await
            .unwrap();
    }
    let kept: Vec<i32> = sqlx::query_scalar(
        "SELECT automatic FROM coverage_snapshots WHERE missing IS NOT NULL ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(kept, vec![83, 84]);
}

#[sqlx::test]
async fn newly_supported_names_are_capped_and_counted(pool: PgPool) {
    let feed = common::db(pool).card_feed();
    let mut new = snapshot("v1", 90, &[]);
    new.newly_supported = (0..MAX_LISTED + 1).map(|n| format!("Card {n}")).collect();
    new.newly_supported_count = u32::try_from(MAX_LISTED + 1).unwrap();
    feed.store_snapshot(&new).await.unwrap();
    let stored = feed.snapshots(1).await.unwrap().remove(0);
    assert_eq!(stored.newly_supported.len(), MAX_LISTED);
    assert_eq!(stored.newly_supported_count as usize, MAX_LISTED + 1);
    assert_eq!(stored.states, new.states);
    assert_eq!(stored.buckets, new.buckets);
}

#[sqlx::test]
async fn nothing_is_stored_before_the_first_snapshot(pool: PgPool) {
    let feed = common::db(pool).card_feed();
    assert_eq!(feed.latest_missing().await.unwrap(), None);
    assert!(feed.snapshots(5).await.unwrap().is_empty());
    assert!(feed.refreshes(5).await.unwrap().is_empty());
}

#[sqlx::test]
async fn the_diff_helpers_read_cards_precons_and_games(pool: PgPool) {
    let db = common::seeded(pool.clone()).await;
    let feed = db.card_feed();
    let cards = feed.deck_cards().await.unwrap();
    assert!(cards.iter().any(|(_, name)| name == "Lightning Bolt"));
    assert!(feed.precon_names().await.unwrap().is_empty());
    assert_eq!(feed.set_name("NOPE").await.unwrap(), None);
    assert_eq!(feed.running_games().await.unwrap(), None, "no game table");
    sqlx::query("CREATE TABLE games (status text NOT NULL)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO games VALUES ('running'), ('running'), ('finished')")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(feed.running_games().await.unwrap(), Some(2));
}
