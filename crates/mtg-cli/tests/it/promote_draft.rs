//! Goal: `promote --draft` turns a proposal the checker marks `Unchecked` into a draft (`drafts/<slug>.json` in canonical
//! form and the card's record from the cache) once the smoke gate passes, promotes an accepted one to the corpus as without
//! the switch and removes the draft of that card, keeps the first of several drafts of a card, writes nothing in a dry run
//! and without the switch an `Unchecked` proposal stays rejected.

#![allow(clippy::unwrap_used)]

use crate::common;

use std::fs;
use std::path::{Path, PathBuf};

use mtg_cards::card::Card;
use mtg_cards::load::{Library, canonical};
use mtg_cards::proposal::Verdict;
use mtg_cli::args::PromoteArgs;
use mtg_cli::cache::Cache;
use mtg_cli::ingest::{IngestOptions, ingest};
use mtg_cli::w2a_promote::{W2aPromoted, W2aSkip, w2a_promote_against};
use mtg_db::Database;
use sqlx::PgPool;

use common::{fixture, gzip, progress, put_scryfall};

const PYRE: &str = "cathartic_pyre";
const CATAPULT: &str = "acorn_catapult";
const RESERVOIR: &str = "aetherflux_reservoir";

fn crate_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn cards_dir() -> PathBuf {
    crate_dir().join("../mtg-cards")
}

/// The record of a card: the real Oracle Cards object of this crate's fixtures or of the corpus.
fn record(slug: &str) -> String {
    let own = crate_dir().join(format!("tests/fixtures/draft/{slug}.json"));
    let path = if own.is_file() {
        own
    } else {
        cards_dir().join(format!("tests/fixtures/corpus/{slug}.json"))
    };
    fs::read_to_string(path).unwrap()
}

/// A stored proposal of the sweeps' fixtures.
fn stored_proposal(slug: &str) -> String {
    let dir = cards_dir().join("tests/fixtures/proposals/r2ca");
    fs::read_to_string(dir.join(format!("{slug}.json"))).unwrap()
}

async fn loaded(pool: PgPool, dir: &Path) -> (Database, Cache) {
    let cache = Cache::new(dir.join("cache"));
    let records: Vec<String> = [PYRE, CATAPULT, RESERVOIR].map(record).to_vec();
    let body = format!("[{}]", records.join(","));
    put_scryfall(
        &cache,
        "oracle_cards",
        "oracle-cards-draft.json.gz",
        &gzip(body.as_bytes()),
    );
    put_scryfall(
        &cache,
        "rulings",
        "rulings-draft.json",
        &fixture("scryfall_rulings.json"),
    );
    let db = Database::from_pool(pool);
    let (progress, _) = progress();
    let options = IngestOptions {
        force: false,
        batch_size: 500,
    };
    ingest(&cache, &db, &options, &progress).await.unwrap();
    (db, cache)
}

/// The two directories a run writes to, below `dir`.
fn dirs(dir: &Path) -> (PathBuf, PathBuf) {
    let out = dir.join("crate");
    fs::create_dir_all(out.join("cards")).unwrap();
    fs::create_dir_all(out.join("tests/fixtures/corpus")).unwrap();
    (out, dir.join("drafts"))
}

fn proposal(dir: &Path, folder: &str, slug: &str, text: &str) -> PathBuf {
    let path = dir
        .join("proposals")
        .join(folder)
        .join(format!("{slug}.json"));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, text).unwrap();
    path
}

/// What a run is asked: the switches and the files.
struct Run {
    draft: bool,
    dry: bool,
    no_smoke: bool,
    files: Vec<PathBuf>,
}

async fn promote(
    (db, cache): &(Database, Cache),
    (out, drafts): &(PathBuf, PathBuf),
    run: Run,
) -> W2aPromoted {
    let args = PromoteArgs {
        dry_run: run.dry,
        partial: false,
        out: Some(out.clone()),
        draft: run.draft,
        no_smoke: run.no_smoke,
        drafts: run.draft.then(|| drafts.clone()),
        files: run.files,
    };
    w2a_promote_against(db, cache, &args, &Library::default())
        .await
        .unwrap()
}

fn drafting(files: Vec<PathBuf>) -> Run {
    Run {
        draft: true,
        dry: false,
        no_smoke: false,
        files,
    }
}

fn outcomes(promoted: &W2aPromoted) -> Vec<(String, Option<W2aSkip>)> {
    promoted
        .decisions
        .iter()
        .map(|d| (d.card.clone(), d.outcome.as_ref().err().copied()))
        .collect()
}

#[sqlx::test]
async fn an_unchecked_whole_card_becomes_a_draft_with_its_record(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let stored = loaded(pool, dir.path()).await;
    let places = dirs(dir.path());
    let file = proposal(dir.path(), "a", PYRE, &stored_proposal(PYRE));

    let promoted = promote(&stored, &places, drafting(vec![file])).await;

    assert_eq!(outcomes(&promoted), [("Cathartic Pyre".to_owned(), None)]);
    assert_eq!(promoted.written, [places.1.join("cathartic_pyre.json")]);
    let text = fs::read_to_string(places.1.join("cathartic_pyre.json")).unwrap();
    let card: Card = serde_json::from_str(&text).unwrap();
    assert_eq!(text, canonical(&card));
    assert_eq!(
        fs::read_to_string(places.1.join("fixtures/cathartic_pyre.json")).unwrap(),
        record(PYRE)
    );
    assert!(
        fs::read_dir(places.0.join("cards"))
            .unwrap()
            .next()
            .is_none(),
        "a draft is no corpus card"
    );
    let rendered = promoted.render();
    assert!(
        rendered.contains("drafted Cathartic Pyre: drafts/cathartic_pyre.json"),
        "{rendered}"
    );
    assert!(
        rendered.ends_with("0 promoted, 1 drafted, 0 skipped"),
        "{rendered}"
    );
}

#[sqlx::test]
async fn without_the_switch_an_unchecked_proposal_stays_rejected(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let stored = loaded(pool, dir.path()).await;
    let places = dirs(dir.path());
    let file = proposal(dir.path(), "a", PYRE, &stored_proposal(PYRE));
    let run = Run {
        draft: false,
        ..drafting(vec![file])
    };

    let promoted = promote(&stored, &places, run).await;

    assert_eq!(
        outcomes(&promoted),
        [(
            "Cathartic Pyre".to_owned(),
            Some(W2aSkip::Rejected(Verdict::Unchecked))
        )]
    );
    assert!(!places.1.exists());
}

#[sqlx::test]
async fn a_dry_run_writes_nothing_and_no_smoke_skips_the_gate(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let stored = loaded(pool, dir.path()).await;
    let places = dirs(dir.path());
    let file = proposal(dir.path(), "a", PYRE, &stored_proposal(PYRE));
    let run = Run {
        dry: true,
        no_smoke: true,
        ..drafting(vec![file])
    };

    let promoted = promote(&stored, &places, run).await;

    assert_eq!(outcomes(&promoted), [("Cathartic Pyre".to_owned(), None)]);
    assert!(promoted.written.is_empty() && !places.1.exists());
    assert!(
        promoted
            .render()
            .starts_with("would draft Cathartic Pyre: drafts/cathartic_pyre.json"),
        "{}",
        promoted.render()
    );
}

#[sqlx::test]
async fn of_several_drafts_of_a_card_the_first_stays_and_an_accepted_one_wins(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let stored = loaded(pool, dir.path()).await;
    let places = dirs(dir.path());
    let d = dir.path();
    // A draft of the reservoir from an earlier run: the corpus card replaces it.
    fs::create_dir_all(places.1.join("fixtures")).unwrap();
    fs::write(places.1.join("aetherflux_reservoir.json"), "{}").unwrap();
    fs::write(places.1.join("fixtures/aetherflux_reservoir.json"), "{}").unwrap();
    let files = vec![
        proposal(d, "a", PYRE, &stored_proposal(PYRE)),
        proposal(d, "b", PYRE, &stored_proposal(PYRE)),
        proposal(d, "a", CATAPULT, &stored_proposal(CATAPULT)),
        proposal(d, "a", RESERVOIR, &stored_proposal(RESERVOIR)),
        proposal(d, "b", RESERVOIR, "not json"),
    ];

    let promoted = promote(&stored, &places, drafting(files)).await;

    assert_eq!(
        outcomes(&promoted),
        [
            ("Cathartic Pyre".to_owned(), None),
            ("Cathartic Pyre".to_owned(), Some(W2aSkip::Repeated)),
            ("Acorn Catapult".to_owned(), None),
            ("Aetherflux Reservoir".to_owned(), None),
            ("Aetherflux Reservoir".to_owned(), Some(W2aSkip::Repeated)),
        ]
    );
    assert!(places.0.join("cards/aetherflux_reservoir.json").is_file());
    assert!(!places.1.join("aetherflux_reservoir.json").exists());
    assert!(!places.1.join("fixtures/aetherflux_reservoir.json").exists());
    assert_eq!(promoted.superseded, ["aetherflux_reservoir"]);
    let rendered = promoted.render();
    assert!(
        rendered.ends_with("1 promoted, 2 drafted, 2 skipped: 2 an earlier file promotes it"),
        "{rendered}"
    );
}
