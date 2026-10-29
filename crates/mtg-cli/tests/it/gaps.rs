//! Goal: `check-rules --gaps` writes the ledger of what holds stored proposals back (with `--marks`, ranked by the fix marks of players and with a marks column): equal reasons of several cards are one row, a card with several proposals counts once by its best one, the rows rank by the popularity the cached Oracle Cards file gives the cards they alone hold back, the TSV and the printed top are stable, and a missing cache file or an unwritable path is an error that says what to do.

#![allow(clippy::unwrap_used)]

use crate::common;

use std::path::{Path, PathBuf};

use mtg_cli::args::{CheckRulesArgs, Cli, Command};
use mtg_cli::cache::Cache;
use mtg_cli::check_rules::check_rules;
use mtg_cli::gaps::{Marks, write_ledger};
use mtg_cli::ingest::{IngestOptions, ingest};
use mtg_cli::popularity::Popularity;
use mtg_cli::run;
use mtg_db::Database;
use mtg_import::ids::OracleId;
use sqlx::PgPool;

use common::{fixture_cache, progress, put_ranks};

const BOLT: &str = r#"{"faces":[{"abilities":[{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":3}}]}}]}]}"#;
const BOLT_OF_FOUR: &str = r#"{"faces":[{"abilities":[{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":4}}]}}]}]}"#;
/// Valid, and nothing contradicts the text of Shock, but no writer writes a Connive of a spell.
const SHOCK_CONNIVES: &str = r#"{"faces":[{"abilities":[{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"Connive":{"what":{"Target":0}}}]}}]}]}"#;
/// Invalid: the spell has no target specification.
const NO_TARGET: &str = r#"{"faces":[{"abilities":[{"Spell":{"targets":[],"effects":[{"DealDamage":{"to":{"Target":0},"amount":3}}]}}]}]}"#;
const NO_TARGET_ONE: &str = r#"{"faces":[{"abilities":[{"Spell":{"targets":[],"effects":[{"DealDamage":{"to":{"Target":1},"amount":3}}]}}]}]}"#;
/// Two abilities that read back as other text than the two clauses of Aether Vial.
const VIAL_CONNIVES: &str = r#"{"faces":[{"abilities":[{"Triggered":{"when":{"StepBegins":{"step":"Upkeep","whose":"You"}},"effects":[{"Connive":{"what":"This"}}]}},{"Activated":{"cost":["Tap"],"effects":[{"Connive":{"what":"This"}}]}}]}]}"#;

const HEADER: &str = "kind\treason\tblocked\tfreed\tweight\texamples\n";
const NO_SUCH_TARGET: &str = "target N does not exist: this ability has N target specification(s)";

/// A database with the card fixtures and a cache whose Oracle Cards file ranks Sol Ring 1, Aether Vial 3, Lightning Bolt 6,
/// Shock 10 and Island 50, and Mountain not at all.
async fn ranked(pool: PgPool, dir: &Path) -> (Database, Cache) {
    let cache = fixture_cache(dir);
    put_ranks(
        &cache,
        &[
            ("Sol Ring", Some(1)),
            ("Æther Vial", Some(3)),
            ("Lightning Bolt", Some(6)),
            ("Shock", Some(10)),
            ("Island", Some(50)),
        ],
    );
    let db = Database::from_pool(pool);
    let (progress, _) = progress();
    let options = IngestOptions {
        force: false,
        batch_size: 500,
    };
    ingest(&cache, &db, &options, &progress)
        .await
        .unwrap_or_else(|e| panic!("ingest runs: {e}"));
    (db, cache)
}

/// A bundle file of the proposals, by card name.
fn bundle(dir: &Path, file: &str, entries: &[(&str, &str)]) -> PathBuf {
    let body: Vec<String> = entries
        .iter()
        .map(|(name, rules)| format!("{}:{rules}", serde_json::json!(name)))
        .collect();
    let path = dir.join(file);
    std::fs::write(&path, format!("{{{}}}", body.join(","))).unwrap();
    path
}

fn args(files: &[&Path], gaps: Option<&Path>) -> CheckRulesArgs {
    CheckRulesArgs {
        card: None,
        verbose: false,
        bundle: true,
        write: None,
        gaps: gaps.map(Path::to_path_buf),
        marks: false,
        files: files.iter().map(|file| file.to_path_buf()).collect(),
    }
}

/// The two bundles: one with a card of each gap and a `Verified` Lightning Bolt, one with a Lightning Bolt that differs.
fn bundles(dir: &Path) -> (PathBuf, PathBuf) {
    let main = bundle(
        dir,
        "main.json",
        &[
            ("Lightning Bolt", BOLT),
            ("Shock", SHOCK_CONNIVES),
            ("Sol Ring", NO_TARGET_ONE),
            ("Island", NO_TARGET),
            ("Mountain", NO_TARGET),
            ("Æther Vial", VIAL_CONNIVES),
        ],
    );
    let extra = bundle(dir, "extra.json", &[("Lightning Bolt", BOLT_OF_FOUR)]);
    (main, extra)
}

fn vial_rows() -> String {
    "mismatch\twrote \"At the beginning of your upkeep, ~ connives.\" for \"At the beginning of your upkeep, you may put a charge counter on ~.\"\t1\t0\t0.000000\tÆther Vial\n\
     mismatch\twrote \"{T}: ~ connives.\" for \"{T}: You may put a creature card with mana value equal to the number of charge counters on ~ from your hand onto the battlefield.\"\t1\t0\t0.000000\tÆther Vial\n".to_owned()
}

#[sqlx::test]
async fn the_ledger_groups_the_reasons_ranks_them_by_the_popularity_freed_and_counts_a_card_once(
    pool: PgPool,
) {
    ledger_of_the_bundles(pool, false).await;
}

#[sqlx::test]
async fn the_order_of_the_files_does_not_change_the_ledger(pool: PgPool) {
    ledger_of_the_bundles(pool, true).await;
}

/// Checks the two bundles, in the order of the files or swapped, and compares the ledger with the expected one.
async fn ledger_of_the_bundles(pool: PgPool, swapped: bool) {
    let dir = tempfile::tempdir().unwrap();
    let (db, cache) = ranked(pool, dir.path()).await;
    let (main, extra) = bundles(dir.path());
    let gaps = dir.path().join("gaps.tsv");
    let files: [&Path; 2] = if swapped {
        [&extra, &main]
    } else {
        [&main, &extra]
    };

    let checked = check_rules(&db, &args(&files, Some(&gaps))).await.unwrap();
    let ranks = Popularity::from_cache(&cache).unwrap();
    let text = write_ledger(&checked, &ranks, None, &gaps).await.unwrap();

    // Sol Ring (1), Island (50) and the unranked Mountain share one invalid row; Shock (10) alone has a missing writer; the
    // two clauses of Aether Vial are two rows that free nothing, since each card has two blockers; Lightning Bolt is
    // verified by its better proposal, so its mismatch is not a gap.
    let expected = format!(
        "{HEADER}invalid\t{NO_SUCH_TARGET}\t3\t3\t0.016568\tSol Ring; Island; Mountain\n\
         missing-writer\tSpell: effect Connive\t1\t1\t0.009091\tShock\n{}",
        vial_rows()
    );
    assert_eq!(std::fs::read_to_string(&gaps).unwrap(), expected);
    assert!(
        text.starts_with(
            "7 cards: 1 ACCEPTED Verified, 3 INVALID, 2 MISMATCH, 1 UNCHECKED Verified\n5 cards held back by 4 gaps\ngaps (top 4 of 4;"
        ),
        "{text}"
    );
    let first = text.lines().find(|line| line.starts_with("  1  ")).unwrap();
    assert!(
        first.contains("invalid") && first.contains(" 0.01657  "),
        "{first}"
    );
    assert!(first.ends_with("(Sol Ring; Island; Mountain)"), "{first}");
    assert!(
        text.ends_with(&format!("gaps written to {}", gaps.display())),
        "{text}"
    );
}

/// Marks the card `name` for `accounts` accounts.
async fn mark(db: &Database, name: &str, accounts: u128) {
    let oracle: String = sqlx::query_scalar("SELECT oracle_id::text FROM cards WHERE name = $1")
        .bind(name)
        .fetch_one(db.pool())
        .await
        .unwrap();
    for account in 1..=accounts {
        db.fix_marks()
            .mark(
                sqlx::types::uuid::Uuid::from_u128(account),
                &OracleId(oracle.clone()),
                None,
            )
            .await
            .unwrap()
            .unwrap();
    }
}

#[sqlx::test]
async fn the_marks_of_players_rank_the_gaps_first_and_add_a_column(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let (db, cache) = ranked(pool, dir.path()).await;
    let (main, extra) = bundles(dir.path());
    let gaps = dir.path().join("gaps.tsv");
    // Shock (3 marks) is less played than Sol Ring, yet its gap comes first; Mountain's single mark lifts the
    // invalid row over the two rows that free nothing.
    mark(&db, "Shock", 3).await;
    mark(&db, "Mountain", 1).await;
    mark(&db, "Æther Vial", 5).await;
    let marks: Marks = db
        .fix_marks()
        .ranked(None)
        .await
        .unwrap()
        .into_iter()
        .map(|mark| (mark.oracle_id.to_string(), mark.marks))
        .collect();

    let checked = check_rules(&db, &args(&[&main, &extra], Some(&gaps)))
        .await
        .unwrap();
    let ranks = Popularity::from_cache(&cache).unwrap();
    let text = write_ledger(&checked, &ranks, Some(&marks), &gaps)
        .await
        .unwrap();

    // Aether Vial's two gaps free nothing (two blockers each), so its five marks weigh on no row.
    let expected = format!(
        "kind\treason\tblocked\tfreed\tmarks\tweight\texamples\n\
         missing-writer\tSpell: effect Connive\t1\t1\t3\t0.009091\tShock\n\
         invalid\t{NO_SUCH_TARGET}\t3\t3\t1\t0.016568\tSol Ring; Island; Mountain\n{}",
        vial_rows().replace("\t1\t0\t0.000000\t", "\t1\t0\t0\t0.000000\t")
    );
    assert_eq!(std::fs::read_to_string(&gaps).unwrap(), expected);
    let header = text
        .lines()
        .find(|line| line.contains("reason (examples)"))
        .unwrap();
    assert!(header.contains("marks"), "{header}");
    let first = text.lines().find(|line| line.starts_with("  1  ")).unwrap();
    assert!(
        first.contains("missing-writer") && first.ends_with("(Shock)"),
        "{first}"
    );
}

#[sqlx::test]
async fn without_marks_the_ledger_is_as_it_was_even_when_cards_are_marked(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let (db, cache) = ranked(pool, dir.path()).await;
    let (main, extra) = bundles(dir.path());
    let gaps = dir.path().join("gaps.tsv");
    mark(&db, "Shock", 3).await;

    let checked = check_rules(&db, &args(&[&main, &extra], Some(&gaps)))
        .await
        .unwrap();
    write_ledger(
        &checked,
        &Popularity::from_cache(&cache).unwrap(),
        None,
        &gaps,
    )
    .await
    .unwrap();

    let file = std::fs::read_to_string(&gaps).unwrap();
    assert!(file.starts_with(HEADER), "{file}");
    assert!(
        file.lines().nth(1).unwrap().starts_with("invalid\t"),
        "{file}"
    );
}

#[sqlx::test]
async fn a_card_with_a_verified_proposal_leaves_the_ledger_empty(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let (db, cache) = ranked(pool, dir.path()).await;
    let file = bundle(dir.path(), "bolts.json", &[("Lightning Bolt", BOLT)]);
    let gaps = dir.path().join("gaps.tsv");

    let checked = check_rules(&db, &args(&[&file], Some(&gaps)))
        .await
        .unwrap();
    let text = write_ledger(
        &checked,
        &Popularity::from_cache(&cache).unwrap(),
        None,
        &gaps,
    )
    .await
    .unwrap();

    assert_eq!(std::fs::read_to_string(&gaps).unwrap(), HEADER);
    assert!(text.contains("0 cards held back by 0 gaps"), "{text}");
}

#[sqlx::test]
async fn a_path_that_cannot_be_written_is_an_error_that_names_it(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let (db, cache) = ranked(pool, dir.path()).await;
    let file = bundle(dir.path(), "bolts.json", &[("Lightning Bolt", BOLT)]);
    let gaps = dir.path().join("missing").join("gaps.tsv");

    let checked = check_rules(&db, &args(&[&file], Some(&gaps)))
        .await
        .unwrap();
    let error = write_ledger(
        &checked,
        &Popularity::from_cache(&cache).unwrap(),
        None,
        &gaps,
    )
    .await
    .unwrap_err();

    assert!(error.to_string().contains("gaps.tsv"), "{error}");
}

#[tokio::test]
async fn gaps_need_the_oracle_cards_file_of_the_cache() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("rules.json");
    std::fs::write(&file, "{}").unwrap();
    let cli = Cli {
        cache: Some(dir.path().join("empty-cache")),
        database_url: None,
        command: Command::CheckRules(args(&[&file], Some(&dir.path().join("gaps.tsv")))),
    };
    let (progress, _) = progress();

    let error = run(cli, progress).await.unwrap_err();

    assert!(error.to_string().contains("Oracle Cards"), "{error}");
}
