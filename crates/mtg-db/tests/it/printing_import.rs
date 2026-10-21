//! Goal: the all-printings import stores flavor and printed names on printings, never writes a card, counts printings of unknown cards as orphaned and repeats without a change; the index built from the stored names resolves lines like one built from the records.

use crate::common;

use mtg_import::decklist::parse;
use mtg_import::resolve::{CardIndex, Lookup, resolve_deck};
use mtg_import::scryfall::{CardRecord, Layout};
use sqlx::PgPool;

use common::{
    alias_records, another_printing, db, fixture_records, record, row_counts, seeded, snapshot,
};

type Names = (Option<String>, Option<String>);

/// The records as an Oracle Cards file has them: the printings without their other names.
fn without_names(records: &[CardRecord]) -> Vec<CardRecord> {
    let mut plain = records.to_vec();
    for record in &mut plain {
        record.flavor_name = None;
        record.printed_name = None;
    }
    plain
}

fn stream(records: &[CardRecord]) -> Vec<Result<CardRecord, mtg_import::scryfall::ScryfallError>> {
    records.iter().cloned().map(Ok).collect()
}

async fn names_of(pool: &PgPool, set: &str, number: &str) -> Names {
    sqlx::query_as(
        "SELECT flavor_name, printed_name FROM printings WHERE set_code = $1 AND collector_number = $2",
    )
    .bind(set)
    .bind(number)
    .fetch_one(pool)
    .await
    .unwrap_or_else(|e| panic!("names of {set} {number}: {e}"))
}

async fn stores_the_names(printings_import: bool, pool: PgPool) {
    let database = db(pool.clone());
    let records = alias_records();
    let loaded = if printings_import {
        let plain = database
            .importer()
            .import_cards(stream(&without_names(&records)))
            .await;
        assert!(plain.is_ok());
        let report = database
            .importer()
            .import_printings(stream(&records))
            .await
            .unwrap_or_else(|e| panic!("printings import: {e}"));
        assert_eq!(report.printings.updated, records.len() as u64);
        true
    } else {
        database
            .importer()
            .import_cards(stream(&records))
            .await
            .is_ok()
    };
    assert!(loaded);

    assert_eq!(
        names_of(&pool, "sld", "1040").await,
        (Some("The Ball".into()), None)
    );
    assert_eq!(
        names_of(&pool, "om1", "171").await,
        (None, Some("Phenomena Recorder".into()))
    );
    let named = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM printings WHERE flavor_name IS NOT NULL OR printed_name IS NOT NULL",
    )
    .fetch_one(&pool)
    .await
    .unwrap_or_else(|e| panic!("count of named printings: {e}"));
    assert_eq!(usize::try_from(named).ok(), Some(records.len()));
}

#[sqlx::test]
async fn the_card_import_stores_the_names_of_a_printing(pool: PgPool) {
    stores_the_names(false, pool).await;
}

#[sqlx::test]
async fn the_printing_import_stores_the_names_of_a_printing(pool: PgPool) {
    stores_the_names(true, pool).await;
}

#[sqlx::test]
async fn importing_the_printings_again_changes_nothing(pool: PgPool) {
    let database = db(pool.clone());
    let records = alias_records();
    database
        .importer()
        .import_cards(stream(&records))
        .await
        .unwrap();
    let before = snapshot(&pool, "printings", "printing_id", true).await;

    let report = database
        .importer()
        .import_printings(stream(&records))
        .await
        .unwrap();

    let all = records.len() as u64;
    assert_eq!(
        (
            report.printings.inserted,
            report.printings.updated,
            report.printings.unchanged
        ),
        (0, 0, all)
    );
    assert_eq!((report.orphaned, report.skipped.count), (0, 0));
    assert_eq!(
        snapshot(&pool, "printings", "printing_id", true).await,
        before
    );
}

#[sqlx::test]
async fn a_printing_whose_card_is_not_stored_is_counted_and_left_out(pool: PgPool) {
    let database = db(pool.clone());
    let records = alias_records();

    let report = database
        .importer()
        .import_printings(stream(&records))
        .await
        .unwrap();

    assert_eq!(report.orphaned, records.len() as u64);
    assert_eq!(report.printings.inserted, 0);
    assert_eq!(row_counts(&pool).await, (0, 0, 0));
    let run = database
        .import_runs()
        .get(report.run_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(run.counts["orphaned"], records.len() as u64);
}

#[sqlx::test]
async fn a_printing_of_a_stored_card_arrives_without_touching_the_card(pool: PgPool) {
    let database = seeded(pool.clone()).await;
    let cards_before = snapshot(&pool, "cards", "oracle_id", true).await;
    let (_, printings_before, _) = row_counts(&pool).await;
    // A reversible card names itself after both faces and has the layout of no card.
    let mut reversible = another_printing(&record("Lightning Bolt"), 40, "sld", Some("2024-01-01"));
    reversible.layout = Layout::ReversibleCard;
    reversible.name = "Lightning Bolt // Lightning Bolt".into();
    reversible.mana_cost = "{9}".into();

    let report = database
        .importer()
        .import_printings(stream(&[reversible]))
        .await
        .unwrap();

    assert_eq!(report.printings.inserted, 1);
    assert_eq!(
        snapshot(&pool, "cards", "oracle_id", true).await,
        cards_before
    );
    assert_eq!(row_counts(&pool).await.1, printings_before + 1);
}

#[sqlx::test]
async fn unusable_records_are_skipped_and_the_rest_arrive(pool: PgPool) {
    let database = seeded(pool.clone()).await;
    let mut bad_id = another_printing(&record("Lightning Bolt"), 41, "sld", None);
    bad_id.printing_id.0 = "not-a-uuid".into();
    let mut bad_date = another_printing(&record("Lightning Bolt"), 42, "sld", Some("2024-13-40"));
    bad_date.collector_number = "42".into();
    let fine = another_printing(&record("Lightning Bolt"), 43, "sld", Some("2024-01-01"));

    let report = database
        .importer()
        .import_printings(stream(&[bad_id, bad_date, fine]))
        .await
        .unwrap();

    assert_eq!(report.printings.inserted, 1);
    assert_eq!(report.skipped.count, 2);
}

/// Every record of the card fixture and the aliased printings, and one printing in German with a localized name.
fn all_records() -> Vec<CardRecord> {
    let mut records = fixture_records();
    records.extend(alias_records());
    let mut german = another_printing(&record("Lightning Bolt"), 90, "lea", None);
    german.lang = "de".into();
    german.printed_name = Some("Blitzschlag".into());
    records.push(german);
    records
}

#[sqlx::test]
async fn the_stored_index_resolves_other_names_like_the_index_of_the_records(pool: PgPool) {
    let database = db(pool);
    let records = all_records();
    database
        .importer()
        .import_cards(stream(&without_names(&records)))
        .await
        .unwrap();
    database
        .importer()
        .import_printings(stream(&records))
        .await
        .unwrap();
    let stored = database.cards().card_index().await.unwrap();
    let in_memory = CardIndex::from_records(&records);

    let mut aliases = 0;
    for record in records.iter().filter(|r| r.lang == "en") {
        for name in [&record.flavor_name, &record.printed_name]
            .into_iter()
            .flatten()
        {
            let found = stored.find_alias(name);
            assert!(
                matches!(&found, Lookup::Found(card) if card.oracle_id == record.oracle_id),
                "{name}: {found:?}"
            );
            assert_eq!(found, in_memory.find_alias(name), "{name}");
            aliases += 1;
        }
    }
    assert_eq!(aliases, alias_records().len());
    assert_eq!(stored.find_alias("Blitzschlag"), Lookup::Missing);

    let deck = parse(
        "1 The Ball\n1 Phenomena Recorder\n1 Unstable Harmonics (SLD) 478\n1 Helm's Deep\n\
         1 Skittering Kitten\n1 Lightning Bolt\n1 Blitzschlag\n",
    );
    let resolution = resolve_deck(&stored, &deck);
    assert_eq!(resolution, resolve_deck(&in_memory, &deck));
    assert_eq!(resolution.resolved.len(), 6);
    assert_eq!(resolution.unresolved.len(), 1);
}
