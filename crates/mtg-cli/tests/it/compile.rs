//! Goal: `compile` stores the rules the compiler reads for the stored cards as `card_rules` rows (the cards of the precons by default, every card with `--all`, one with `--card`), keeps only rules that validate, leaves hand-written cards to their files, writes again a row whose card text or compiler changed, leaves the others alone, removes the rows that no longer belong, and says what it did; a database without cards, without precons or without the card asked for says what to do.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use mtg_cards::generated::COMPILER_VERSION;
use mtg_cli::args::CompileArgs;
use mtg_cli::cache::Cache;
use mtg_cli::compile::compile;
use mtg_cli::error::CliError;
use mtg_cli::ingest::{IngestOptions, ingest};
use mtg_db::{Database, ImplStatus, NewRules};
use serde_json::{Value, json};
use sqlx::PgPool;

use common::{fixture_cache, progress};

const OLD: &str = "0123456789abcdef0123456789abcdef";

async fn loaded(pool: PgPool, cache_dir: &std::path::Path) -> Database {
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

fn args(all: bool, card: Option<&str>) -> CompileArgs {
    CompileArgs {
        all,
        card: card.map(str::to_owned),
    }
}

/// The names of the cards that have a row, in name order.
async fn named_rows(pool: &PgPool) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT c.name FROM card_rules r JOIN cards c USING (oracle_id) ORDER BY c.name_normalized",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

async fn status_of(pool: &PgPool, name: &str) -> String {
    sqlx::query_scalar(
        "SELECT r.status::text FROM card_rules r JOIN cards c USING (oracle_id) WHERE c.name = $1",
    )
    .bind(name)
    .fetch_one(pool)
    .await
    .unwrap_or_else(|e| panic!("{name} has a row: {e}"))
}

async fn rules_of(pool: &PgPool, name: &str) -> Value {
    sqlx::query_scalar(
        "SELECT r.rules FROM card_rules r JOIN cards c USING (oracle_id) WHERE c.name = $1",
    )
    .bind(name)
    .fetch_one(pool)
    .await
    .unwrap_or_else(|e| panic!("{name} has a row: {e}"))
}

async fn run(db: &Database, all: bool, card: Option<&str>) -> mtg_cli::compile::CompileReport {
    compile(db, &args(all, card))
        .await
        .unwrap_or_else(|e| panic!("compile runs: {e}"))
}

#[sqlx::test]
async fn the_default_run_compiles_the_cards_of_the_precons_and_no_others(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let db = loaded(pool.clone(), dir.path()).await;

    let report = run(&db, false, None).await;

    // The precon holds Arcane Signet, Atraxa, Fire // Ice, Forest and Sol Ring; the compiler reads all but Forest and Fire // Ice.
    assert_eq!(
        named_rows(&pool).await,
        ["Arcane Signet", "Atraxa, Praetors' Voice", "Sol Ring"]
    );
    assert_eq!((report.looked_at, report.written, report.full), (5, 3, 3));
    assert_eq!((report.nothing, report.invalid.len()), (1, 1));
    assert_eq!(report.invalid[0].card, "Fire // Ice");
    assert!(report.invalid[0].problems[0].contains("Fuse"), "{report:?}");
    assert!(report.scope.contains("precons"), "{report:?}");
}

#[sqlx::test]
async fn every_card_is_compiled_with_all_and_a_hand_written_card_is_left_to_its_file(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let db = loaded(pool.clone(), dir.path()).await;

    let report = run(&db, true, None).await;

    let named = named_rows(&pool).await;
    assert_eq!(
        named,
        [
            "Æther Vial",
            "Arcane Signet",
            "Atraxa, Praetors' Voice",
            "Bonecrusher Giant // Stomp",
            "Delver of Secrets // Insectile Aberration",
            "Hazmat Suit (Used)",
            "Shock",
            "Sol Ring",
            "Valakut Awakening // Valakut Stoneforge",
        ]
    );
    assert!(!named.contains(&"Lightning Bolt".to_owned()));
    assert_eq!(report.hand_written, 1, "Lightning Bolt has its file");
    assert_eq!(
        (report.written, report.replaced, report.unchanged),
        (9, 0, 0)
    );
    assert_eq!((report.full, report.partial), (6, 3));
    assert_eq!(status_of(&pool, "Shock").await, "full");
    assert_eq!(status_of(&pool, "Æther Vial").await, "partial");
    // What is stored is what the loader reads back as the card's rules.
    let shock = rules_of(&pool, "Shock").await;
    assert_eq!(shock["name"], "Shock");
    assert_eq!(shock["impl_status"], "Full");
}

#[sqlx::test]
async fn one_card_is_compiled_by_its_name_in_any_case_or_by_a_face_name(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let db = loaded(pool.clone(), dir.path()).await;

    let report = run(&db, false, Some("sHoCk")).await;
    assert_eq!((report.looked_at, report.written), (1, 1));
    assert_eq!(named_rows(&pool).await, ["Shock"]);

    run(&db, false, Some("Valakut Stoneforge")).await;
    assert_eq!(
        named_rows(&pool).await,
        ["Shock", "Valakut Awakening // Valakut Stoneforge"]
    );

    let error = compile(&db, &args(false, Some("Black Lotus")))
        .await
        .unwrap_err();
    assert!(matches!(error, CliError::Config(_)), "{error}");
    assert!(error.to_string().contains("Black Lotus"), "{error}");
}

#[sqlx::test]
async fn a_second_run_changes_nothing(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let db = loaded(pool.clone(), dir.path()).await;
    run(&db, true, None).await;
    let before = db.card_rules().version().await.unwrap();
    let rows_before = db.card_rules().all().await.unwrap();

    let report = run(&db, true, None).await;

    assert_eq!(
        (report.written, report.replaced, report.unchanged),
        (0, 0, 9)
    );
    assert_eq!(
        (report.full, report.partial),
        (6, 3),
        "the unchanged count too"
    );
    assert_eq!(db.card_rules().version().await.unwrap(), before);
    assert_eq!(db.card_rules().all().await.unwrap(), rows_before);
}

#[sqlx::test]
async fn a_row_of_another_compiler_or_another_text_is_written_again(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let db = loaded(pool.clone(), dir.path()).await;
    run(&db, true, None).await;
    sqlx::query("UPDATE card_rules SET compiler_version = 'old' WHERE oracle_id = (SELECT oracle_id FROM cards WHERE name = 'Sol Ring')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE card_rules SET fingerprint = $1 WHERE oracle_id = (SELECT oracle_id FROM cards WHERE name = 'Atraxa, Praetors'' Voice')")
        .bind(OLD)
        .execute(&pool)
        .await
        .unwrap();
    // Scryfall corrects the text of Shock.
    sqlx::query("UPDATE cards SET oracle_text = replace(oracle_text, '2 damage', '3 damage'), faces = replace(faces::text, '2 damage', '3 damage')::jsonb WHERE name = 'Shock'")
        .execute(&pool)
        .await
        .unwrap();

    let report = run(&db, true, None).await;

    assert_eq!(
        (report.replaced, report.unchanged, report.written),
        (3, 6, 0)
    );
    let shock = rules_of(&pool, "Shock").await;
    assert_eq!(
        shock["faces"][0]["abilities"][0]["Spell"]["effects"][0]["DealDamage"]["amount"],
        json!(3)
    );
    let stamps = db.card_rules().stamps().await.unwrap();
    assert!(
        stamps
            .iter()
            .all(|s| s.compiler_version == COMPILER_VERSION && s.fingerprint != OLD)
    );
}

#[sqlx::test]
async fn a_selection_leaves_the_rows_of_the_other_cards_alone(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let db = loaded(pool.clone(), dir.path()).await;
    run(&db, true, None).await;

    let report = run(&db, false, Some("Shock")).await;

    assert_eq!((report.looked_at, report.unchanged), (1, 1));
    assert_eq!(named_rows(&pool).await.len(), 9);
}

#[sqlx::test]
async fn a_row_of_a_hand_written_card_is_removed(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let db = loaded(pool.clone(), dir.path()).await;
    let bolt: sqlx::types::Uuid =
        sqlx::query_scalar("SELECT oracle_id FROM cards WHERE name = 'Lightning Bolt'")
            .fetch_one(&pool)
            .await
            .unwrap();
    db.card_rules()
        .put(&[NewRules {
            oracle_id: mtg_import::ids::OracleId(bolt.to_string()),
            rules: json!({"name": "Lightning Bolt"}),
            status: ImplStatus::Full,
            fingerprint: OLD.into(),
            compiler_version: "old".into(),
        }])
        .await
        .unwrap();

    let report = run(&db, false, Some("Shock")).await;

    assert_eq!(report.removed_hand_written, 1);
    assert!(
        !named_rows(&pool)
            .await
            .contains(&"Lightning Bolt".to_owned())
    );
}

#[sqlx::test]
async fn a_row_is_removed_when_its_card_is_no_longer_a_deck_card(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let db = loaded(pool.clone(), dir.path()).await;
    run(&db, true, None).await;
    sqlx::query("UPDATE cards SET is_deck_card = false WHERE name = 'Shock'")
        .execute(&pool)
        .await
        .unwrap();

    let report = run(&db, false, Some("Sol Ring")).await;

    assert_eq!(report.removed_gone, 1);
    assert!(!named_rows(&pool).await.contains(&"Shock".to_owned()));
    assert_eq!(named_rows(&pool).await.len(), 8);
}

#[sqlx::test]
async fn a_row_is_removed_when_the_compiler_no_longer_reads_the_card(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let db = loaded(pool.clone(), dir.path()).await;
    run(&db, true, None).await;
    sqlx::query("UPDATE cards SET oracle_text = '', faces = replace(faces::text, 'Shock deals 2 damage to any target.', '')::jsonb WHERE name = 'Shock'")
        .execute(&pool)
        .await
        .unwrap();

    let report = run(&db, true, None).await;

    assert_eq!(report.removed_unread, 1);
    assert_eq!(
        report.nothing, 10,
        "Shock joined the cards with nothing to read"
    );
    assert!(!named_rows(&pool).await.contains(&"Shock".to_owned()));
}

#[sqlx::test]
async fn rules_that_fail_a_check_are_reported_and_never_stored(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let db = loaded(pool.clone(), dir.path()).await;
    // A row an older compiler wrote for a card the current one gets wrong.
    let fire_ice: sqlx::types::Uuid =
        sqlx::query_scalar("SELECT oracle_id FROM cards WHERE name = 'Fire // Ice'")
            .fetch_one(&pool)
            .await
            .unwrap();
    db.card_rules()
        .put(&[NewRules {
            oracle_id: mtg_import::ids::OracleId(fire_ice.to_string()),
            rules: json!({"name": "Fire // Ice"}),
            status: ImplStatus::Partial,
            fingerprint: OLD.into(),
            compiler_version: "old".into(),
        }])
        .await
        .unwrap();

    let report = run(&db, true, None).await;

    assert_eq!(report.invalid.len(), 1);
    assert_eq!(report.invalid[0].card, "Fire // Ice");
    assert_eq!(report.removed_unread, 1);
    assert!(!named_rows(&pool).await.contains(&"Fire // Ice".to_owned()));
    let text = report.render();
    assert!(text.contains("1 invalid"), "{text}");
    assert!(
        text.contains("Fire // Ice:\n      cards/fire_ice.json: faces:"),
        "{text}"
    );
}

#[sqlx::test]
async fn the_report_says_what_was_done(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let db = loaded(pool.clone(), dir.path()).await;

    let text = run(&db, true, None).await.render();

    assert!(
        text.starts_with(
            "compile: every stored card, 20 cards\n  rows: 9 written, 0 replaced, 0 unchanged, 0 removed\n  by state: 6 full, 3 partial\n  not compiled: 1 hand-written, 9 nothing to read, 1 invalid\n"
        ),
        "{text}"
    );
}

#[sqlx::test]
async fn a_database_without_cards_says_how_to_load_them(pool: PgPool) {
    let error = compile(&Database::from_pool(pool), &args(true, None))
        .await
        .unwrap_err();

    assert!(matches!(error, CliError::Config(_)), "{error}");
    assert!(error.to_string().contains("mtg-cli ingest"), "{error}");
}

#[sqlx::test]
async fn a_database_without_precons_points_to_all(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let db = loaded(pool.clone(), dir.path()).await;
    sqlx::query("DELETE FROM precon_decks")
        .execute(&pool)
        .await
        .unwrap();

    let error = compile(&db, &args(false, None)).await.unwrap_err();

    assert!(matches!(error, CliError::Config(_)), "{error}");
    assert!(error.to_string().contains("--all"), "{error}");
    assert!(compile(&db, &args(true, None)).await.is_ok());
}

#[sqlx::test(migrations = false)]
async fn a_database_without_a_schema_says_how_to_apply_it(pool: PgPool) {
    let error = compile(&Database::from_pool(pool), &args(true, None))
        .await
        .unwrap_err();

    assert!(matches!(error, CliError::Config(_)), "{error}");
    assert!(
        error.to_string().contains("schema is not applied"),
        "{error}"
    );
}
