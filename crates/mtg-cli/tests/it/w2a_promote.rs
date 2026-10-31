//! Goal: `promote` writes a card the checker accepts as `Verified` with its record copied from the cache, a dry run
//! writes nothing and `--partial` admits a `Partial` card, and every other file is skipped with its reason: no card, a
//! corpus file, a card the compiler makes `Verified`, an invalid or mismatching proposal, a `Partial` one, a repeated
//! card and a card the cached Oracle Cards file lacks.

#![allow(clippy::unwrap_used)]

use crate::common;

use std::fs;
use std::path::{Path, PathBuf};

use mtg_cards::load::Library;
use mtg_cards::proposal::Verdict;
use mtg_cards::support::Support;
use mtg_cli::args::PromoteArgs;
use mtg_cli::cache::Cache;
use mtg_cli::ingest::{IngestOptions, ingest};
use mtg_cli::w2a_promote::{W2aPromoted, W2aSkip, w2a_promote_against};
use mtg_db::Database;
use sqlx::PgPool;

use common::{fixture, gzip, progress, put_scryfall};

const CARDS: [&str; 4] = [
    "aetherflux_reservoir",
    "eureka_moment",
    "lightning_bolt",
    "trynn_champion_of_freedom",
];

const BOLT: &str = r#"{"faces":[{"abilities":[{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":3}}]}}]}]}"#;
const INVALID: &str = r#"{"faces":[{"abilities":[{"Nonsense":{}}]}]}"#;
const RESERVOIR_TRIGGER: &str = r#"{"faces":[{"abilities":[{"Triggered":{"when":{"SpellCast":{"by":"You"}},"effects":[{"GainLife":{"amount":{"Tallied":{"who":"You","what":"SpellsCast"}}}}]}}]}]}"#;

fn mtg_cards_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../mtg-cards")
}

fn corpus_fixture(slug: &str) -> String {
    let path = mtg_cards_dir().join(format!("tests/fixtures/corpus/{slug}.json"));
    fs::read_to_string(path).unwrap()
}

fn stored_proposal(sample: &str, slug: &str) -> String {
    let path = mtg_cards_dir().join(format!("tests/fixtures/proposals/{sample}/{slug}.json"));
    fs::read_to_string(path).unwrap()
}

/// Puts the real records of these corpus cards into the cache as the Oracle Cards file.
fn put_oracle_cards(cache: &Cache, slugs: &[&str], name: &str) {
    let records: Vec<String> = slugs.iter().map(|slug| corpus_fixture(slug)).collect();
    let body = format!("[{}]", records.join(","));
    put_scryfall(cache, "oracle_cards", name, &gzip(body.as_bytes()));
}

async fn loaded(pool: PgPool, dir: &Path) -> (Database, Cache) {
    let cache = Cache::new(dir.join("cache"));
    put_oracle_cards(&cache, &CARDS, "oracle-cards-w2a.json.gz");
    put_scryfall(
        &cache,
        "rulings",
        "rulings-w2a.json",
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

/// An empty `mtg-cards` crate directory.
fn out_dir(dir: &Path, name: &str) -> PathBuf {
    let out = dir.join(name);
    fs::create_dir_all(out.join("cards")).unwrap();
    fs::create_dir_all(out.join("tests/fixtures/corpus")).unwrap();
    out
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

async fn promote(
    db: &Database,
    cache: &Cache,
    out: &Path,
    files: Vec<PathBuf>,
    (dry_run, partial): (bool, bool),
) -> W2aPromoted {
    let args = PromoteArgs {
        dry_run,
        partial,
        out: Some(out.to_path_buf()),
        draft: false,
        no_smoke: false,
        drafts: None,
        files,
    };
    w2a_promote_against(db, cache, &args, &Library::default())
        .await
        .unwrap()
}

fn outcomes(promoted: &W2aPromoted) -> Vec<(String, Option<W2aSkip>)> {
    promoted
        .decisions
        .iter()
        .map(|d| (d.card.clone(), d.outcome.as_ref().err().copied()))
        .collect()
}

#[sqlx::test]
async fn each_file_is_promoted_or_skipped_with_its_reason(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let (db, cache) = loaded(pool, dir.path()).await;
    let out = out_dir(dir.path(), "crate");
    fs::write(out.join("cards/trynn_champion_of_freedom.json"), "{}").unwrap();
    let reservoir = stored_proposal("r2ca", "aetherflux_reservoir");
    let d = dir.path();
    let forty = reservoir.replace("\"amount\":50", "\"amount\":40");
    assert_ne!(forty, reservoir);
    let files = vec![
        proposal(d, "a", "aetherflux_reservoir", RESERVOIR_TRIGGER),
        proposal(d, "b", "aetherflux_reservoir", INVALID),
        proposal(d, "c", "aetherflux_reservoir", &forty),
        proposal(d, "d", "aetherflux_reservoir", &reservoir),
        proposal(d, "e", "aetherflux_reservoir", &reservoir),
        proposal(d, "a", "lightning_bolt", BOLT),
        proposal(
            d,
            "a",
            "eureka_moment",
            &stored_proposal("r2g", "eureka_moment"),
        ),
        proposal(d, "a", "trynn_champion_of_freedom", &reservoir),
        proposal(d, "a", "no_card_by_this_name", BOLT),
    ];
    let promoted = promote(&db, &cache, &out, files, (false, false)).await;

    let name = |n: &str| n.to_owned();
    let reservoir_name = || name("Aetherflux Reservoir");
    assert_eq!(
        outcomes(&promoted),
        vec![
            (reservoir_name(), Some(W2aSkip::State(Support::Partial))),
            (reservoir_name(), Some(W2aSkip::Rejected(Verdict::Invalid))),
            (reservoir_name(), Some(W2aSkip::Rejected(Verdict::Mismatch))),
            (reservoir_name(), None),
            (reservoir_name(), Some(W2aSkip::Repeated)),
            (name("Lightning Bolt"), Some(W2aSkip::Compiled)),
            (name("Eureka Moment"), Some(W2aSkip::Compiled)),
            (
                name("Trynn, Champion of Freedom"),
                Some(W2aSkip::CorpusFile)
            ),
            (name("no_card_by_this_name"), Some(W2aSkip::NoCard)),
        ]
    );
    assert_eq!(
        promoted.written,
        [out.join("cards/aetherflux_reservoir.json")]
    );
    let fixture_text =
        fs::read_to_string(out.join("tests/fixtures/corpus/aetherflux_reservoir.json")).unwrap();
    assert_eq!(fixture_text, corpus_fixture("aetherflux_reservoir"));
    let rules = fs::read_to_string(out.join("cards/aetherflux_reservoir.json")).unwrap();
    assert!(
        rules.contains("\"impl_status\": \"Full\"") && rules.contains("oracle_fingerprint"),
        "{rules}"
    );
    assert_eq!(
        fs::read_to_string(out.join("cards/trynn_champion_of_freedom.json")).unwrap(),
        "{}"
    );
    let text = promoted.render();
    assert!(
        text.contains("promoted Aetherflux Reservoir (Verified): cards/aetherflux_reservoir.json"),
        "{text}"
    );
    assert!(text.ends_with("1 promoted, 8 skipped: 1 INVALID, 1 MISMATCH, 1 accepted Partial, 1 already a corpus card, 1 an earlier file promotes it, 1 no stored card is named after the file, 2 the compiler makes it Verified"), "{text}");
}

#[sqlx::test]
async fn a_dry_run_writes_nothing_partial_admits_partial_and_a_missing_record_is_skipped(
    pool: PgPool,
) {
    let dir = tempfile::tempdir().unwrap();
    let (db, cache) = loaded(pool, dir.path()).await;
    let out = out_dir(dir.path(), "crate");
    let files = vec![proposal(
        dir.path(),
        "a",
        "aetherflux_reservoir",
        RESERVOIR_TRIGGER,
    )];

    let dry = promote(&db, &cache, &out, files.clone(), (true, true)).await;
    assert_eq!(
        outcomes(&dry),
        [(String::from("Aetherflux Reservoir"), None)]
    );
    assert!(dry.written.is_empty() && fs::read_dir(out.join("cards")).unwrap().next().is_none());
    assert!(
        dry.render()
            .starts_with("would promote Aetherflux Reservoir (Partial)"),
        "{}",
        dry.render()
    );

    put_oracle_cards(&cache, &["eureka_moment"], "oracle-cards-later.json.gz");
    let later = promote(&db, &cache, &out, files, (false, true)).await;
    assert_eq!(
        outcomes(&later),
        [(
            String::from("Aetherflux Reservoir"),
            Some(W2aSkip::NoRecord)
        )]
    );
    assert_eq!(later.written, Vec::<PathBuf>::new());
}

#[sqlx::test]
async fn a_folder_that_is_not_the_cards_crate_is_refused(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let (db, cache) = loaded(pool, dir.path()).await;
    let args = PromoteArgs {
        dry_run: true,
        partial: false,
        out: Some(dir.path().join("elsewhere")),
        draft: false,
        no_smoke: false,
        drafts: None,
        files: vec![proposal(dir.path(), "a", "lightning_bolt", BOLT)],
    };
    let error = w2a_promote_against(&db, &cache, &args, &Library::default())
        .await
        .unwrap_err();
    assert!(
        error.to_string().contains("not the mtg-cards crate"),
        "{error}"
    );
}
