//! Goal: precon deck entries resolve by the strongest identifier available, fall back to names only when unambiguous, and stay visible, unresolved, when nothing matches.

mod common;

use mtg_db::DeckBoard;
use mtg_db::import::UnresolvedReason;
use mtg_import::ids::{OracleId, PrintingId};
use mtg_import::mtgjson::PreconCard;
use sqlx::PgPool;

use common::precon::{deck, entry, ingest};
use common::{another_card, db, precon_deck, record, seeded};

fn resolution_cases() -> Vec<(
    &'static str,
    PreconCard,
    Result<&'static str, UnresolvedReason>,
)> {
    let with_ids = |name: &str, oracle: Option<OracleId>, printing: Option<PrintingId>| {
        let mut card = entry(name, 1);
        card.oracle_id = oracle;
        card.scryfall_id = printing;
        card
    };
    let shock = record("Shock");
    vec![
        (
            "exact name",
            entry("Lightning Bolt", 1),
            Ok("Lightning Bolt"),
        ),
        (
            "sloppy name",
            entry("  lightning   BOLT", 1),
            Ok("Lightning Bolt"),
        ),
        ("accents", entry("Aether Vial", 1), Ok("Æther Vial")),
        (
            "split card, slash variant",
            entry("Fire/Ice", 1),
            Ok("Fire // Ice"),
        ),
        (
            "front face of a double-faced card",
            entry("Delver of Secrets", 1),
            Ok("Delver of Secrets // Insectile Aberration"),
        ),
        (
            "oracle id beats a wrong name",
            with_ids("Not Shock", Some(shock.oracle_id.clone()), None),
            Ok("Shock"),
        ),
        (
            "printing id beats a wrong name",
            with_ids("Not Shock", None, Some(shock.printing_id.clone())),
            Ok("Shock"),
        ),
        (
            "unknown oracle id falls back to the name",
            with_ids(
                "Sol Ring",
                Some(OracleId(uuid::Uuid::from_u128(5).to_string())),
                None,
            ),
            Ok("Sol Ring"),
        ),
        (
            "malformed ids fall back to the name",
            with_ids(
                "Sol Ring",
                Some(OracleId("junk".into())),
                Some(PrintingId("junk".into())),
            ),
            Ok("Sol Ring"),
        ),
        (
            "unknown card",
            entry("Black Lotus", 1),
            Err(UnresolvedReason::NotFound),
        ),
        (
            "token names are not deck cards",
            entry("Goblin", 1),
            Err(UnresolvedReason::NotFound),
        ),
        (
            "back face alone",
            entry("Insectile Aberration", 1),
            Err(UnresolvedReason::NotFound),
        ),
        (
            "typo",
            entry("Lightnin Bolt", 1),
            Err(UnresolvedReason::NotFound),
        ),
        (
            "ambiguous name",
            entry("Twin", 1),
            Err(UnresolvedReason::Ambiguous),
        ),
    ]
}

#[sqlx::test]
async fn entries_resolve_by_id_then_name_and_the_rest_are_reported(pool: PgPool) {
    let database = seeded(pool).await;
    let base = record("Shock");
    database
        .importer()
        .import_cards(
            [
                another_card(&base, 90, "Twin"),
                another_card(&base, 91, "Twin"),
            ]
            .into_iter()
            .map(Ok),
        )
        .await
        .unwrap();
    let cases = resolution_cases();
    let entries: Vec<PreconCard> = cases.iter().map(|(_, card, _)| card.clone()).collect();

    let report = ingest(&database, vec![deck("All Cases", "TST", None, entries)]).await;

    let ingested = &report.decks[0];
    let loaded = database
        .precons()
        .get(ingested.deck_id)
        .await
        .unwrap()
        .unwrap();
    let mut failures = Vec::new();
    for ((label, _, expected), line) in cases.iter().zip(&loaded.entries) {
        let actual = line.card.as_ref().map(|card| card.name.as_str());
        match (expected, actual) {
            (Ok(name), Some(found)) if *name == found => {}
            (Err(_), None) => {}
            _ => failures.push(format!("{label}: expected {expected:?}, got {actual:?}")),
        }
        let reported = ingested
            .unresolved
            .iter()
            .find(|u| u.name == line.card_name);
        match (expected, reported) {
            (Ok(_), None) => {}
            (Err(reason), Some(found)) if found.reason == *reason => {}
            _ => failures.push(format!("{label}: report says {reported:?}")),
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
    assert_eq!(
        loaded.entries.len(),
        cases.len(),
        "unresolved entries are kept"
    );
    let unresolved = cases.iter().filter(|(_, _, r)| r.is_err()).count() as u64;
    assert_eq!(loaded.summary.unresolved_count, unresolved);
}

#[sqlx::test]
async fn a_deck_ingested_before_any_cards_keeps_every_line_unresolved(pool: PgPool) {
    let database = db(pool);

    let report = ingest(&database, vec![precon_deck()]).await;

    let ingested = &report.decks[0];
    assert_eq!(ingested.unresolved.len(), 5);
    assert!(
        ingested
            .unresolved
            .iter()
            .all(|u| u.reason == UnresolvedReason::NotFound)
    );
    let commander = &ingested.unresolved[0];
    assert_eq!(
        (commander.board, commander.count),
        (DeckBoard::Commander, 1)
    );
    let loaded = database
        .precons()
        .get(ingested.deck_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(loaded.summary.card_count, 40);
    assert_eq!(loaded.summary.unresolved_count, 40);
    assert!(loaded.entries.iter().all(|e| e.card.is_none()));
}

#[sqlx::test]
async fn printings_the_card_data_lacks_are_remembered_anyway(pool: PgPool) {
    let database = seeded(pool).await;
    let mut card = entry("Shock", 4);
    card.scryfall_id = Some(PrintingId(uuid::Uuid::from_u128(424_242).to_string()));
    card.set_code = Some("zzz".into());
    card.number = Some("99".into());

    let report = ingest(
        &database,
        vec![deck("Odd Print", "ODD", None, vec![card.clone()])],
    )
    .await;

    let loaded = database
        .precons()
        .get(report.decks[0].deck_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(loaded.entries[0].printing_id, card.scryfall_id);
    assert_eq!(loaded.entries[0].card.as_ref().unwrap().name, "Shock");
}
