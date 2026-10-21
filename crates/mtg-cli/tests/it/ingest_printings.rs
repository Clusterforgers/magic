//! Goal: ingest reads the Default Cards file as a step of its own after the cards: every printing arrives with its flavor and printed names, no card is written or changed by it, a new cards file brings it back in, and without the file nothing changes.

use crate::common;

use mtg_cli::ingest::{IngestOptions, IngestReport, Step, dry_run, ingest};
use mtg_db::Database;
use serde_json::{Value, json};
use sqlx::PgPool;

use common::{count, fixture, fixture_cache, gzip, progress, put_scryfall};

const BOLT: &str = "Lightning Bolt";

fn options() -> IngestOptions {
    IngestOptions {
        force: false,
        batch_size: 3,
    }
}

/// The fixture cards as Default Cards has them (the same printings) plus a printing with a flavor
/// name and a reversible card, whose own name and layout are no card's.
fn default_cards() -> Vec<u8> {
    let mut records: Vec<Value> = serde_json::from_slice(&fixture("scryfall_oracle_cards.json"))
        .unwrap_or_else(|e| panic!("the fixture parses: {e}"));
    let bolt = records
        .iter()
        .find(|record| record["name"] == BOLT)
        .cloned()
        .unwrap_or_else(|| panic!("the fixture has {BOLT}"));

    let mut flavored = bolt.clone();
    flavored["id"] = json!("00000000-0000-4000-8000-0000000000a1");
    flavored["set"] = json!("sld");
    flavored["collector_number"] = json!("999");
    flavored["flavor_name"] = json!("Zap");

    let mut reversible = bolt.clone();
    reversible["id"] = json!("00000000-0000-4000-8000-0000000000a2");
    reversible["set"] = json!("sld");
    reversible["collector_number"] = json!("998");
    reversible["layout"] = json!("reversible_card");
    reversible["name"] = json!("Lightning Bolt // Lightning Bolt");
    reversible["mana_cost"] = json!("{9}");
    let oracle_id = reversible["oracle_id"].take();
    reversible["card_faces"] = json!([
        {"name": BOLT, "oracle_id": oracle_id},
        {"name": BOLT, "oracle_id": oracle_id}
    ]);

    records.extend([flavored, reversible]);
    serde_json::to_vec(&records).unwrap_or_default()
}

fn with_printings(dir: &std::path::Path) -> mtg_cli::cache::Cache {
    let cache = fixture_cache(dir);
    put_scryfall(
        &cache,
        "default_cards",
        "default-cards-fixture.json.gz",
        &gzip(&default_cards()),
    );
    cache
}

async fn run(cache: &mtg_cli::cache::Cache, pool: &PgPool) -> (IngestReport, String) {
    let (progress, log) = progress();
    let db = Database::from_pool(pool.clone());
    let report = ingest(cache, &db, &options(), &progress)
        .await
        .unwrap_or_else(|e| panic!("ingest runs: {e}"));
    (report, log.text())
}

fn expect<R>(step: Step<R>) -> R {
    match step {
        Step::Imported(report) => report,
        _ => panic!("expected an import"),
    }
}

async fn text_of(pool: &PgPool, sql: &'static str) -> Vec<String> {
    sqlx::query_scalar::<_, String>(sql)
        .fetch_all(pool)
        .await
        .unwrap_or_else(|e| panic!("{sql}: {e}"))
}

#[sqlx::test]
async fn printings_arrive_with_their_names_and_no_card_is_written_by_them(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let cache = with_printings(dir.path());

    let (report, log) = run(&cache, &pool).await;

    let cards = expect(report.cards);
    let printings = expect(report.printings);
    assert_eq!(printings.printings.inserted, 2);
    assert_eq!(printings.printings.unchanged, cards.printings.inserted);
    assert_eq!((printings.orphaned, printings.skipped.count), (0, 1));
    assert!(log.contains("printings: 2 new"), "{log}");
    assert_eq!(
        count(&pool, "cards").await,
        i64::try_from(cards.cards.inserted).unwrap()
    );
    assert_eq!(
        count(&pool, "printings").await,
        i64::try_from(cards.printings.inserted).unwrap() + 2
    );
    assert_eq!(
        text_of(
            &pool,
            "SELECT name || ' / ' || layout || ' / ' || mana_cost FROM cards WHERE name = 'Lightning Bolt' OR (layout = 'reversible_card' AND name LIKE 'Lightning%')"
        )
        .await,
        ["Lightning Bolt / normal / {R}"]
    );
    assert_eq!(
        text_of(
            &pool,
            "SELECT set_code || ' ' || collector_number || ' ' || flavor_name FROM printings WHERE flavor_name IS NOT NULL"
        )
        .await,
        ["sld 999 Zap"]
    );
}

#[sqlx::test]
async fn a_second_ingest_skips_the_printings_and_a_new_cards_file_brings_them_back(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let cache = with_printings(dir.path());
    run(&cache, &pool).await;

    let (again, _) = run(&cache, &pool).await;
    assert!(matches!(again.printings, Step::Unchanged));

    let original = String::from_utf8(fixture("scryfall_oracle_cards.json")).unwrap();
    put_scryfall(
        &cache,
        "oracle_cards",
        "oracle-cards-fixture.json.gz",
        &gzip(
            original
                .replace(
                    "deals 3 damage to any target.",
                    "deals 3 damage to any target, twice.",
                )
                .as_bytes(),
        ),
    );
    let (changed, _) = run(&cache, &pool).await;
    expect(changed.cards);
    let printings = expect(changed.printings);
    assert_eq!(printings.printings.inserted, 0);
}

#[sqlx::test]
async fn without_the_file_the_step_is_missing_and_says_how_to_fetch_it(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let cache = fixture_cache(dir.path());

    let (report, log) = run(&cache, &pool).await;

    assert!(matches!(report.printings, Step::Missing));
    assert!(log.contains("fetch scryfall --printings"), "{log}");
    assert!(
        report
            .summary(std::time::Duration::ZERO)
            .contains("printings not cached")
    );
}

#[test]
fn a_dry_run_counts_the_printings_without_a_database() {
    let dir = tempfile::tempdir().unwrap();
    let cache = with_printings(dir.path());
    let (progress, _) = progress();

    let report = dry_run(&cache, &progress).unwrap_or_else(|e| panic!("dry run: {e}"));

    let Step::Checked(check) = report.printings else {
        panic!("expected a check");
    };
    let Step::Checked(cards) = report.cards else {
        panic!("expected a check");
    };
    assert_eq!((check.usable, check.skipped.count), (cards.usable + 2, 1));
}
