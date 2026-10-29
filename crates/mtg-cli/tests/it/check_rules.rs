//! Goal: `check-rules` judges proposed rules files against the stored cards: a file named after its card, a named card and a bundle of cards are checked, the accepted rules are written completed, a mismatch is shown beside the card's text, and an unknown card or a file that names none is an error that says what to do.

use crate::common;

use std::path::Path;

use mtg_cli::args::CheckRulesArgs;
use mtg_cli::cache::Cache;
use mtg_cli::check_rules::check_rules;
use mtg_cli::ingest::{IngestOptions, ingest};
use mtg_db::Database;
use sqlx::PgPool;

use common::{fixture_cache, progress};

const BOLT: &str = r#"{"faces":[{"abilities":[{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":3}}]}}]}]}"#;
const BOLT_OF_FOUR: &str = r#"{"faces":[{"abilities":[{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":4}}]}}]}]}"#;

async fn loaded(pool: PgPool, cache_dir: &Path) -> Database {
    let cache: Cache = fixture_cache(cache_dir);
    let db = Database::from_pool(pool);
    let (progress, _) = progress();
    let options = IngestOptions {
        force: false,
        batch_size: 500,
    };
    ingest(&cache, &db, &options, &progress)
        .await
        .unwrap_or_else(|e| panic!("ingest runs: {e}"));
    db
}

fn args(files: &[&Path]) -> CheckRulesArgs {
    CheckRulesArgs {
        card: None,
        verbose: false,
        bundle: false,
        write: None,
        gaps: None,
        marks: false,
        files: files.iter().map(|file| file.to_path_buf()).collect(),
    }
}

#[sqlx::test]
async fn a_file_named_after_its_card_is_checked_and_the_completed_rules_are_written(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let db = loaded(pool, dir.path()).await;
    let file = dir.path().join("lightning_bolt.json");
    std::fs::write(&file, BOLT).unwrap();
    let out = dir.path().join("out");
    std::fs::create_dir(&out).unwrap();

    let checked = check_rules(
        &db,
        &CheckRulesArgs {
            write: Some(out.clone()),
            ..args(&[&file])
        },
    )
    .await
    .unwrap();

    let text = checked.render(false);
    assert!(
        text.starts_with("Lightning Bolt: ACCEPTED Verified"),
        "{text}"
    );
    let written = std::fs::read_to_string(out.join("lightning_bolt.json")).unwrap();
    assert!(written.contains("\"impl_status\": \"Full\""), "{written}");
    assert!(written.contains("oracle_fingerprint"), "{written}");
}

#[sqlx::test]
async fn a_bundle_checks_each_card_and_ends_with_the_counts(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let db = loaded(pool, dir.path()).await;
    let file = dir.path().join("bundle.json");
    let bundle = format!(r#"{{"Lightning Bolt":{BOLT},"lightning bolt":{BOLT_OF_FOUR}}}"#);
    std::fs::write(&file, bundle).unwrap();

    let checked = check_rules(
        &db,
        &CheckRulesArgs {
            bundle: true,
            ..args(&[&file])
        },
    )
    .await
    .unwrap();

    let text = checked.render(false);
    assert!(
        text.ends_with("2 cards: 1 ACCEPTED Verified, 1 MISMATCH"),
        "{text}"
    );
}

#[sqlx::test]
async fn a_mismatch_is_shown_beside_the_text_of_the_card(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let db = loaded(pool, dir.path()).await;
    let file = dir.path().join("anything.json");
    std::fs::write(&file, BOLT_OF_FOUR).unwrap();

    let checked = check_rules(
        &db,
        &CheckRulesArgs {
            card: Some("LIGHTNING BOLT".into()),
            ..args(&[&file])
        },
    )
    .await
    .unwrap();

    let text = checked.render(false);
    assert!(text.starts_with("Lightning Bolt: MISMATCH"), "{text}");
    assert!(text.contains("~ deals 4 damage to any target."), "{text}");
    assert!(
        text.contains("clause: ~ deals 3 damage to any target."),
        "{text}"
    );
    assert_eq!(checked.written.len(), 0);
}

#[sqlx::test]
async fn an_unknown_card_or_a_file_that_names_none_is_an_error_that_says_what_to_do(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let db = loaded(pool, dir.path()).await;
    let file = dir.path().join("not_a_card.json");
    std::fs::write(&file, BOLT).unwrap();

    let unnamed = check_rules(&db, &args(&[&file])).await.unwrap_err();
    let unknown = check_rules(
        &db,
        &CheckRulesArgs {
            card: Some("No Such Card".into()),
            ..args(&[&file])
        },
    )
    .await
    .unwrap_err();
    let two = check_rules(
        &db,
        &CheckRulesArgs {
            card: Some("Lightning Bolt".into()),
            ..args(&[&file, &file])
        },
    )
    .await
    .unwrap_err();

    assert!(unnamed.to_string().contains("--card"), "{unnamed}");
    assert!(unknown.to_string().contains("No Such Card"), "{unknown}");
    assert!(two.to_string().contains("exactly one file"), "{two}");
}
