//! Goal: `compile` keeps the drafts of the build in `card_drafts`: a draft that fits its stored card becomes the card's
//! current draft with the read hash the library computes, a rerun changes nothing, an edited draft retires the old one
//! (which stays stored), a retired draft of the same hash returns, a draft the build no longer holds is retired, and a
//! draft for a card that is not stored, written for another text, outranked by a rules file or not a whole loadable draft is
//! counted and never stored; the report says it all.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use mtg_cards::builtin;
use mtg_cards::card::{Card, ImplStatus};
use mtg_cards::load::{Library, Source, canonical};
use mtg_cards::support::{oracle_fingerprint, read_hash};
use mtg_cards::{Caps, PrintedCard};
use mtg_cli::audit::stored_cards;
use mtg_cli::cache::Cache;
use mtg_cli::drafts::{DraftCounts, store_files};
use mtg_cli::ingest::{IngestOptions, ingest};
use mtg_db::{Database, DraftKey};
use mtg_import::ids::OracleId;
use sqlx::PgPool;

use common::{fixture_cache, progress};

const FIRE_ICE: &str = "Fire // Ice";

async fn loaded(pool: PgPool, cache_dir: &std::path::Path) -> (Database, PrintedCard, Card) {
    let cache: Cache = fixture_cache(cache_dir);
    let db = Database::from_pool(pool);
    let (progress, _) = progress();
    let options = IngestOptions {
        force: false,
        batch_size: 500,
    };
    ingest(&cache, &db, &options, &progress).await.unwrap();
    let printed = stored_cards(&db)
        .await
        .unwrap()
        .into_iter()
        .find(|card| card.name == FIRE_ICE)
        .unwrap();
    // The rules of the corpus card, written for the text of the stored record.
    let mut card = builtin::library()
        .unwrap()
        .card_by_name(FIRE_ICE)
        .unwrap()
        .clone();
    card.oracle_id.clone_from(&printed.oracle_id);
    for (face, shown) in card.faces.iter_mut().zip(&printed.faces) {
        face.oracle_fingerprint = oracle_fingerprint(&shown.oracle_text);
    }
    (db, printed, card)
}

/// Stores these drafts (the cards' rules as files) against the stored cards; no rules file outranks them.
async fn store(db: &Database, drafts: &[&Card]) -> DraftCounts {
    let texts: Vec<(String, String)> = drafts
        .iter()
        .enumerate()
        .map(|(at, card)| (format!("draft_{at}"), canonical(card)))
        .collect();
    store_texts(db, &texts, &Library::default()).await
}

async fn store_texts(db: &Database, texts: &[(String, String)], library: &Library) -> DraftCounts {
    let files: Vec<(&str, &str)> = texts
        .iter()
        .map(|(slug, text)| (slug.as_str(), text.as_str()))
        .collect();
    let cards = stored_cards(db).await.unwrap();
    let caps: Caps = mtg_engine::caps();
    store_files(db, &files, library, &caps, &cards)
        .await
        .unwrap()
}

fn key(printed: &PrintedCard, card: &Card) -> DraftKey {
    DraftKey {
        oracle_id: OracleId(printed.oracle_id.clone()),
        read_hash: read_hash(printed, card),
    }
}

/// The card with its first face's last ability removed: other rules, so another read hash.
fn edited(card: &Card) -> Card {
    let mut edited = card.clone();
    edited.faces[0].abilities.pop();
    edited
}

fn counted(counts: &DraftCounts) -> (usize, usize, usize, usize) {
    (counts.new, counts.revived, counts.unchanged, counts.retired)
}

#[sqlx::test]
async fn a_draft_that_fits_becomes_the_current_draft_with_its_read_hash(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let (db, printed, card) = loaded(pool, dir.path()).await;

    let counts = store(&db, &[&card]).await;

    assert_eq!(counted(&counts), (1, 0, 0, 0));
    assert_eq!(counts.embedded, 1);
    let current = db.drafts().current().await.unwrap();
    assert_eq!(current.len(), 1);
    assert_eq!(current[0].key(), key(&printed, &card));
    assert_eq!(current[0].rules, serde_json::to_value(&card).unwrap());
}

#[sqlx::test]
async fn a_second_run_changes_nothing(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let (db, _, card) = loaded(pool, dir.path()).await;
    store(&db, &[&card]).await;
    let version = db.drafts().version().await.unwrap();

    let counts = store(&db, &[&card]).await;

    assert_eq!(counted(&counts), (0, 0, 1, 0));
    assert_eq!(db.drafts().version().await.unwrap(), version);
}

#[sqlx::test]
async fn an_edited_draft_retires_the_old_one_which_stays_stored(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let (db, printed, card) = loaded(pool, dir.path()).await;
    let newer = edited(&card);
    store(&db, &[&card]).await;

    let counts = store(&db, &[&newer]).await;

    assert_eq!(counted(&counts), (1, 0, 0, 1));
    let current = db.drafts().current().await.unwrap();
    assert_eq!(
        current
            .iter()
            .map(mtg_db::StoredDraft::key)
            .collect::<Vec<_>>(),
        [key(&printed, &newer)]
    );
    let old = db.drafts().get(&[key(&printed, &card)]).await.unwrap();
    assert_eq!(old.len(), 1);
    assert!(old[0].retired);
}

#[sqlx::test]
async fn a_retired_draft_returns_when_the_build_holds_it_again(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let (db, printed, card) = loaded(pool, dir.path()).await;
    let newer = edited(&card);
    store(&db, &[&card]).await;
    store(&db, &[&newer]).await;

    let counts = store(&db, &[&card]).await;

    assert_eq!(counted(&counts), (0, 1, 0, 1));
    let current = db.drafts().current().await.unwrap();
    assert_eq!(
        current
            .iter()
            .map(mtg_db::StoredDraft::key)
            .collect::<Vec<_>>(),
        [key(&printed, &card)]
    );
}

#[sqlx::test]
async fn a_draft_the_build_no_longer_holds_is_retired_not_deleted(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let (db, printed, card) = loaded(pool, dir.path()).await;
    store(&db, &[&card]).await;

    let counts = store(&db, &[]).await;

    assert_eq!(counted(&counts), (0, 0, 0, 1));
    assert_eq!(db.drafts().current().await.unwrap().len(), 0);
    let kept = db.drafts().get(&[key(&printed, &card)]).await.unwrap();
    assert_eq!(kept.len(), 1);
    assert!(kept[0].retired);
}

/// How a draft that does not fit goes wrong, and the count that says so.
#[derive(Debug, Clone, Copy)]
enum Fault {
    NoStoredCard,
    OtherText,
    RulesFile,
    NotWhole,
    NotJson,
}

#[sqlx::test]
async fn a_draft_that_does_not_fit_is_counted_and_never_stored(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let (db, _, card) = loaded(pool, dir.path()).await;
    for fault in [
        Fault::NoStoredCard,
        Fault::OtherText,
        Fault::RulesFile,
        Fault::NotWhole,
        Fault::NotJson,
    ] {
        let mut draft = card.clone();
        let mut library = Library::default();
        let mut text = String::new();
        match fault {
            Fault::NoStoredCard => {
                draft.oracle_id = "00000000-0000-4000-8000-000000000001".into();
            }
            Fault::OtherText => draft.faces[0].oracle_fingerprint = "0".repeat(32),
            Fault::RulesFile => {
                let source = Source {
                    path: format!("cards/{}.json", mtg_cards::load::slug(&card.name)),
                    text: canonical(&card),
                };
                library = Library::from_sources(&[source]).unwrap();
            }
            Fault::NotWhole => draft.impl_status = ImplStatus::Partial,
            Fault::NotJson => text = "not json".into(),
        }
        if text.is_empty() {
            text = canonical(&draft);
        }

        let counts = store_texts(&db, &[("draft".into(), text)], &library).await;

        let found = (
            counts.no_card,
            counts.stale,
            counts.outranked,
            counts.invalid.len(),
        );
        let expected = match fault {
            Fault::NoStoredCard => (1, 0, 0, 0),
            Fault::OtherText => (0, 1, 0, 0),
            Fault::RulesFile => (0, 0, 1, 0),
            Fault::NotWhole | Fault::NotJson => (0, 0, 0, 1),
        };
        assert_eq!(found, expected, "{fault:?}: {counts:?}");
        assert_eq!(counted(&counts), (0, 0, 0, 0), "{fault:?}");
        assert!(db.drafts().current().await.unwrap().is_empty(), "{fault:?}");
    }
}

#[sqlx::test]
async fn compile_reports_the_drafts_of_the_build(pool: PgPool) {
    use mtg_cli::args::CompileArgs;
    use mtg_cli::compile::compile;

    let dir = tempfile::tempdir().unwrap();
    let (db, _, _) = loaded(pool, dir.path()).await;

    let report = compile(
        &db,
        &CompileArgs {
            all: false,
            card: None,
        },
    )
    .await
    .unwrap();

    assert_eq!(report.drafts.embedded, mtg_cli::drafts::embedded().len());
    assert!(report.render().contains("drafts: "), "{}", report.render());
}
