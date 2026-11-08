//! Goal: the precon index is built off the runtime, so the health check and other requests answer while it is built, one build serves every request of its stamp, and after a verdict the held index is served while the new one is built, which decides again only the card the verdict changed.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

use mtg_cards::builtin;
use mtg_cards::load::{Library, Source, slug};
use mtg_cards::support::read_hash;
use mtg_db::{Database, DraftKey, NewDraft, SOURCE_SWEEP, Verdict};
use mtg_import::ids::OracleId;
use mtg_import::mtgjson::{PreconCard, PreconDeck};
use mtg_import::scryfall::cards;
use sqlx::PgPool;

use super::*;
use crate::cards::tests::BuildGate;

/// A card the corpus writes by hand; its file is kept out and it is read as a draft instead.
const DRAFTED: &str = "Explore";

fn without_drafted() -> Library {
    let skipped = format!("cards/{}.json", slug(DRAFTED));
    let sources: Vec<Source> = builtin::sources()
        .into_iter()
        .filter(|source| source.path != skipped)
        .collect();
    Library::from_sources(&sources).unwrap()
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

fn deck(name: &str, main: &[(&str, u32)]) -> PreconDeck {
    PreconDeck {
        name: name.to_owned(),
        code: "TST".to_owned(),
        deck_type: Some("Starter Deck".to_owned()),
        release_date: Some("2024-05-01".to_owned()),
        main_board: main
            .iter()
            .map(|(name, count)| entry(name, *count))
            .collect(),
        side_board: Vec::new(),
        commander: Vec::new(),
        display_commander: Vec::new(),
    }
}

/// Both schemas, the corpus, two precons and a pending draft of [`DRAFTED`]; the source and the draft's key.
async fn seeded(pool: PgPool) -> (CardSource, DraftKey) {
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
                deck("Explorers", &[("Forest", 56), (DRAFTED, 4)]),
            ]
            .into_iter()
            .map(Ok),
        )
        .await
        .unwrap();
    let rules = builtin::library()
        .unwrap()
        .card_by_name(DRAFTED)
        .unwrap()
        .clone();
    let stored = db
        .cards()
        .get(&OracleId(rules.oracle_id.clone()))
        .await
        .unwrap()
        .unwrap();
    let key = DraftKey {
        oracle_id: stored.oracle_id.clone(),
        read_hash: read_hash(&crate::cards::printed_card(&stored), &rules),
    };
    db.drafts()
        .insert(&[NewDraft {
            oracle_id: key.oracle_id.clone(),
            read_hash: key.read_hash.clone(),
            rules: serde_json::to_value(&rules).unwrap(),
            source: SOURCE_SWEEP.to_owned(),
            oracle_matches: false,
            differs: false,
            proof: None,
        }])
        .await
        .unwrap();
    let source = CardSource::new(pool)
        .with_rules(without_drafted())
        .with_caps(Caps::everything());
    (source, key)
}

/// `future`'s output; a hang fails the test instead of stalling it.
async fn within<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(30), future)
        .await
        .expect("the request hung")
}

fn to_read(index: &PreconIndex) -> u32 {
    index
        .items()
        .find(|item| item.name == "Explorers")
        .unwrap()
        .summary
        .to_read
}

/// The test runtime has one worker: the health check answers while the index is built only if the build is off it.
#[sqlx::test(migrations = false)]
async fn the_runtime_serves_while_the_index_is_built_and_one_build_serves_all(pool: PgPool) {
    let (source, _) = seeded(pool).await;
    within(source.rules()).await.unwrap();
    let gate = Arc::new(BuildGate::default());
    let source = source.with_precon_gate(gate.clone());
    let first = tokio::spawn({
        let source = source.clone();
        async move { source.precon_index().await }
    });
    within(gate.reached.notified()).await;
    let waiting = tokio::spawn({
        let source = source.clone();
        async move { source.precon_index().await }
    });
    assert_eq!(
        within(tokio::spawn(crate::api::healthz())).await.unwrap(),
        "ok"
    );
    within(source.rules()).await.unwrap();
    assert!(!first.is_finished());
    gate.open();
    let first = within(first).await.unwrap().unwrap();
    let waiting = within(waiting).await.unwrap().unwrap();
    assert!(
        !gate.timed_out.load(std::sync::atomic::Ordering::SeqCst),
        "the build held the worker"
    );
    assert_eq!(gate.passes.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert!(Arc::ptr_eq(&first, &waiting));
    assert_eq!(first.items().count(), 2);
}

#[sqlx::test(migrations = false)]
async fn a_verdict_serves_the_held_index_and_decides_again_only_its_card(pool: PgPool) {
    let (source, key) = seeded(pool).await;
    let held = within(source.precon_index()).await.unwrap();
    assert_eq!(to_read(&held), 1);
    let decided = source.rules().await.unwrap().library().decided();
    assert!(decided >= 2, "the build decided the precons' cards");

    source
        .database()
        .reads()
        .append(None, &key, Verdict::Wrong, Some("it explores twice"))
        .await
        .unwrap()
        .unwrap();
    let gate = Arc::new(BuildGate::default());
    let gated = source.clone().with_precon_gate(gate.clone());
    let started = std::time::Instant::now();
    let served = within(gated.precon_index()).await.unwrap();
    assert!(
        Arc::ptr_eq(&served, &held),
        "the held index is served while the new one is built"
    );
    assert!(started.elapsed() < FRESH_WAIT + Duration::from_secs(2));
    within(gate.reached.notified()).await;
    // The library of the verdict took over every decision but the drafted card's.
    let library = gated.rules().await.unwrap();
    assert_eq!(library.library().decided(), decided - 1);

    gate.open();
    let fresh = within(async {
        loop {
            let index = gated.precon_index().await.unwrap();
            if !Arc::ptr_eq(&index, &held) {
                break index;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    assert_eq!(to_read(&fresh), 0);
    assert_eq!(gate.passes.load(std::sync::atomic::Ordering::SeqCst), 1);
}
