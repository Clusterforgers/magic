//! Goal: a start serves the precon judgements the server kept in the database at once, without a build, while they
//! match the cards, precons, stored rules and server build; when any of them changed it serves the kept list marked
//! as judging while the current one is built, then the current one, and keeps that in place of the old.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::{Duration, Instant};

use mtg_db::{Database, NewDraft, SOURCE_SWEEP};
use mtg_import::mtgjson::{PreconCard, PreconDeck};
use mtg_import::scryfall::cards;
use sqlx::PgPool;

use super::*;
use crate::cards::tests::BuildGate;

/// How long a served list may take when no build is waited for: the reads of the stamp and of the kept row.
const PROMPT: Duration = Duration::from_secs(2);

fn deck(name: &str, cards: &[(&str, u32)]) -> PreconDeck {
    PreconDeck {
        name: name.to_owned(),
        code: "TST".to_owned(),
        deck_type: Some("Starter Deck".to_owned()),
        release_date: Some("2024-05-01".to_owned()),
        main_board: cards
            .iter()
            .map(|(name, count)| PreconCard {
                count: *count,
                uuid: format!("uuid-{name}"),
                name: (*name).to_owned(),
                set_code: None,
                number: None,
                is_foil: false,
                scryfall_id: None,
                oracle_id: None,
            })
            .collect(),
        side_board: Vec::new(),
        commander: Vec::new(),
        display_commander: Vec::new(),
    }
}

/// Both schemas, the corpus and two precons, and a source with the build's rules.
async fn seeded(pool: PgPool) -> CardSource {
    crate::migrate(&pool).await.unwrap();
    let db = Database::from_pool(pool.clone());
    db.migrate().await.unwrap();
    db.importer()
        .import_cards(cards(
            mtg_testkit::corpus::scryfall_array().unwrap().as_bytes(),
        ))
        .await
        .unwrap();
    db.importer()
        .import_precons(
            [
                deck("Evergreen Pack", &[("Forest", 60)]),
                deck("Explorers", &[("Forest", 56), ("Explore", 4)]),
            ]
            .into_iter()
            .map(Ok),
        )
        .await
        .unwrap();
    CardSource::new(pool).with_build("first")
}

/// `future`'s output; a hang fails the test instead of stalling it.
async fn within<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(30), future)
        .await
        .expect("the request hung")
}

/// The fingerprint of the kept row, once one is written that differs from `before`.
async fn kept_after(source: &CardSource, before: Option<&str>) -> String {
    within(async {
        loop {
            let row = sqlx::query_scalar::<_, String>("SELECT fingerprint FROM precon_judgements")
                .fetch_optional(source.database().pool())
                .await
                .unwrap();
            if let Some(row) = row
                && Some(row.as_str()) != before
            {
                break row;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
}

fn codes(index: &PreconIndex) -> Vec<String> {
    index.items().map(|item| item.code.clone()).collect()
}

/// The source a restart makes: nothing in memory, its precon builds held at a closed gate.
fn restarted(source: &CardSource, build: &str) -> (CardSource, Arc<BuildGate>) {
    let gate = Arc::new(BuildGate::default());
    let source = source
        .clone()
        .with_build(build)
        .with_precon_gate(gate.clone());
    (source, gate)
}

#[sqlx::test(migrations = false)]
async fn a_start_serves_the_kept_judgements_without_a_build(pool: PgPool) {
    let source = seeded(pool).await;
    let built = within(source.precon_index()).await.unwrap();
    kept_after(&source, None).await;

    let (restarted, gate) = restarted(&source, "first");
    let started = Instant::now();
    let served = within(restarted.precon_served()).await.unwrap();
    assert!(
        started.elapsed() < PROMPT,
        "served in {:?}",
        started.elapsed()
    );
    assert!(!served.judging);
    assert_eq!(codes(&served.index), codes(&built));
    let explorers = |index: &PreconIndex| {
        index
            .items()
            .find(|item| item.name == "Explorers")
            .cloned()
            .unwrap()
    };
    assert_eq!(explorers(&served.index), explorers(&built));
    assert!(served.index.find(&explorers(&built).code).is_some());
    assert_eq!(
        gate.passes.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "nothing was judged again"
    );
}

/// What changes between two starts.
#[derive(Debug, Clone, Copy)]
enum Change {
    /// A new server build.
    Release,
    /// A card is written again.
    Card,
    /// A draft is stored, so the stored rules differ.
    Draft,
}

/// Makes `change` to what `source` reads; the new build, for a release.
async fn make(change: Change, source: &CardSource, round: usize) -> Option<String> {
    let pool = source.database().pool();
    match change {
        Change::Release => return Some(format!("release-{round}")),
        Change::Card => {
            sqlx::query("UPDATE cards SET updated_at = now() + make_interval(secs => $1) WHERE name = 'Forest'")
                .bind(f64::from(u32::try_from(round).unwrap()))
            .execute(pool)
            .await
            .unwrap();
        }
        Change::Draft => {
            let rules = mtg_cards::builtin::library()
                .unwrap()
                .card_by_name("Explore")
                .unwrap()
                .clone();
            source
                .database()
                .drafts()
                .insert(&[NewDraft {
                    oracle_id: mtg_import::ids::OracleId(rules.oracle_id.clone()),
                    read_hash: format!("{round:032x}"),
                    rules: serde_json::to_value(&rules).unwrap(),
                    source: SOURCE_SWEEP.to_owned(),
                    oracle_matches: false,
                    differs: false,
                    proof: None,
                }])
                .await
                .unwrap();
        }
    }
    None
}

#[sqlx::test(migrations = false)]
async fn a_change_serves_the_kept_list_as_judging_until_the_new_one_is_kept(pool: PgPool) {
    let mut source = seeded(pool).await.with_build("release-0");
    let built = within(source.precon_index()).await.unwrap();
    let mut kept = kept_after(&source, None).await;
    let mut build = "release-0".to_owned();
    for (round, change) in [Change::Release, Change::Card, Change::Draft]
        .into_iter()
        .enumerate()
    {
        if let Some(next) = make(change, &source, round + 1).await {
            build = next;
        }
        let (next, gate) = restarted(&source, &build);
        let started = Instant::now();
        let served = within(next.precon_served()).await.unwrap();
        assert!(
            started.elapsed() < FRESH_WAIT + PROMPT,
            "{change:?}: served in {:?}",
            started.elapsed()
        );
        assert!(served.judging, "{change:?}: the kept list is from before");
        assert_eq!(codes(&served.index), codes(&built));

        gate.open();
        let fresh = within(async {
            loop {
                let served = next.precon_served().await.unwrap();
                if !served.judging {
                    break served.index;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await;
        assert_eq!(codes(&fresh), codes(&built), "{change:?}");
        assert_eq!(gate.passes.load(std::sync::atomic::Ordering::SeqCst), 1);
        kept = kept_after(&next, Some(&kept)).await;

        // The next start of the same build and data serves the new judgements without a build.
        let (again, gate) = restarted(&next, &build);
        let served = within(again.precon_served()).await.unwrap();
        assert!(!served.judging, "{change:?}");
        assert_eq!(gate.passes.load(std::sync::atomic::Ordering::SeqCst), 0);
        source = again;
    }
}

#[sqlx::test(migrations = false)]
async fn other_rules_files_keep_no_judgements(pool: PgPool) {
    let source = seeded(pool).await;
    let custom = source.clone().with_rules(
        mtg_cards::load::Library::from_sources(&mtg_cards::builtin::sources()).unwrap(),
    );
    within(custom.precon_index()).await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    let rows = sqlx::query_scalar::<_, i64>("SELECT count(*) FROM precon_judgements")
        .fetch_one(source.database().pool())
        .await
        .unwrap();
    assert_eq!(rows, 0);
}
