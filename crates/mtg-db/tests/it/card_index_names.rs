//! Goal: the index built from the database resolves every line like the index built from the same Scryfall records: a meld piece is found by the name of both halves, a flavor or digital name selects the printing that carries it, and every export of the corpus ties to the same cards and printings through both.

use crate::common;

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::BufReader;
use std::path::PathBuf;

use flate2::read::GzDecoder;
use mtg_db::Database;
use mtg_import::decklist::{DeckFormat, parse_as};
use mtg_import::ids::{OracleId, PrintingId};
use mtg_import::resolve::{CardIndex, MatchKind, Resolution, resolve_deck};
use mtg_import::scryfall::{CardRecord, ScryfallError, cards_jsonl};
use sqlx::PgPool;
use uuid::Uuid;

use common::{alias_records, corpus, db, fixture_records, meld_records, record};

fn stream(records: &[CardRecord]) -> Vec<Result<CardRecord, ScryfallError>> {
    records.iter().cloned().map(Ok).collect()
}

/// Stores `cards` and then `printings` as the CLI does (Oracle Cards, then Default Cards) and builds the index of the database and the index of the records.
async fn both_indexes(
    database: &Database,
    cards: &[CardRecord],
    printings: &[CardRecord],
) -> (CardIndex, CardIndex) {
    let importer = database.importer();
    importer
        .import_cards(stream(cards))
        .await
        .unwrap_or_else(|e| panic!("cards import: {e}"));
    importer
        .import_printings(stream(printings))
        .await
        .unwrap_or_else(|e| panic!("printings import: {e}"));
    let stored = database
        .cards()
        .card_index()
        .await
        .unwrap_or_else(|e| panic!("index loads: {e}"));
    let all: Vec<CardRecord> = cards.iter().chain(printings).cloned().collect();
    (stored, CardIndex::from_records(&all))
}

/// A German printing of a card with a name no line may use.
fn german_printing() -> CardRecord {
    let mut german = common::another_printing(&record("Lightning Bolt"), 90, "lea", None);
    german.lang = "de".into();
    german.printed_name = Some("Blitzschlag".into());
    german
}

/// The cards (the fixture, the meld pair and the cards of the printings that carry other names) and the printings of the other names and the German one.
fn name_records() -> (Vec<CardRecord>, Vec<CardRecord>) {
    let cards = fixture_records()
        .into_iter()
        .chain(meld_records())
        .chain(alias_records())
        .collect();
    let mut printings = alias_records();
    printings.push(german_printing());
    (cards, printings)
}

/// What a line came to: the card, how it matched, the set and the number of the entry.
type Outcome = (String, MatchKind, Option<String>, Option<String>);

fn outcomes(resolution: &Resolution) -> Vec<Outcome> {
    let entry = |entry: &mtg_import::resolve::ResolvedEntry| {
        (
            entry.name.clone(),
            entry.matched,
            entry.set.clone(),
            entry.collector_number.clone(),
        )
    };
    resolution.resolved.iter().map(entry).collect()
}

fn expected(card: &str, matched: MatchKind, set: Option<&str>, number: Option<&str>) -> Outcome {
    let owned = |text: Option<&str>| text.map(str::to_owned);
    (card.to_owned(), matched, owned(set), owned(number))
}

#[sqlx::test]
async fn the_names_of_cards_and_printings_resolve_the_same_through_both_indexes(pool: PgPool) {
    let database = db(pool);
    let (cards, printings) = name_records();
    let (stored, in_memory) = both_indexes(&database, &cards, &printings).await;
    let (name, other, printing) = (MatchKind::Name, MatchKind::OtherName, MatchKind::Printing);
    let none = (None, None);

    // (what the line names, the line, the card it ties to, how it matched, the set and number of the entry)
    let table = [
        (
            "front face",
            "1 Delver of Secrets",
            "Delver of Secrets // Insectile Aberration",
            name,
            none,
        ),
        (
            "full name",
            "1 Delver of Secrets // Insectile Aberration",
            "Delver of Secrets // Insectile Aberration",
            name,
            none,
        ),
        ("split card", "1 Fire // Ice", "Fire // Ice", name, none),
        (
            "split card by its front",
            "1 Fire",
            "Fire // Ice",
            name,
            none,
        ),
        (
            "flavor name",
            "1 The Ball",
            "Fellwar Stone",
            other,
            (Some("sld"), Some("1040")),
        ),
        (
            "another flavor name",
            "1 Unstable Harmonics",
            "Rhystic Study",
            other,
            (Some("sld"), Some("478")),
        ),
        (
            "flavor name of three printings",
            "1 Helm's Deep",
            "Shinka, the Bloodsoaked Keep",
            other,
            none,
        ),
        (
            "digital name",
            "1 Skittering Kitten",
            "Masked Meower",
            other,
            (Some("om1"), Some("96")),
        ),
        (
            "another digital name",
            "1 Phenomena Recorder",
            "Peter Parker's Camera",
            other,
            (Some("om1"), Some("171")),
        ),
        (
            "flavor name beside its printing",
            "1 The Ball (SLD) 1040",
            "Fellwar Stone",
            printing,
            (Some("SLD"), Some("1040")),
        ),
        (
            "flavor name beside a set",
            "1 The Ball (SLD)",
            "Fellwar Stone",
            other,
            (Some("SLD"), None),
        ),
        (
            "meld piece",
            "1 Gisela, the Broken Blade",
            "Gisela, the Broken Blade",
            name,
            none,
        ),
        (
            "other meld piece",
            "1 Bruna, the Fading Light",
            "Bruna, the Fading Light",
            name,
            none,
        ),
        (
            "meld result",
            "1 Brisela, Voice of Nightmares",
            "Brisela, Voice of Nightmares",
            name,
            none,
        ),
        (
            "meld name",
            "1 Gisela, the Broken Blade // Brisela, Voice of Nightmares",
            "Gisela, the Broken Blade",
            other,
            none,
        ),
        (
            "meld name of the other piece",
            "1 Bruna, the Fading Light // Brisela, Voice of Nightmares",
            "Bruna, the Fading Light",
            other,
            none,
        ),
    ];
    for (what, line, card, matched, (set, number)) in table {
        let deck = parse_as(DeckFormat::PlainText, line);
        let from_db = resolve_deck(&stored, &deck);
        assert_eq!(from_db, resolve_deck(&in_memory, &deck), "{what}: {line}");
        assert!(from_db.unresolved.is_empty(), "{what}: {from_db:?}");
        assert_eq!(
            outcomes(&from_db),
            [expected(card, matched, set, number)],
            "{what}: {line}"
        );
    }

    let unresolved = [
        ("half a meld name", "1 Brisela"),
        (
            "the halves in the wrong order",
            "1 Brisela, Voice of Nightmares // Gisela, the Broken Blade",
        ),
        ("back face", "1 Insectile Aberration"),
        ("printed name of a foreign printing", "1 Blitzschlag"),
    ];
    for (what, line) in unresolved {
        let deck = parse_as(DeckFormat::PlainText, line);
        let from_db = resolve_deck(&stored, &deck);
        assert_eq!(from_db, resolve_deck(&in_memory, &deck), "{what}: {line}");
        assert!(from_db.resolved.is_empty(), "{what}: {from_db:?}");
    }
}

/// Records of the printings the sites list for the corpus decks (ids derived from the names) and of the printings that carry other names.
fn corpus_records() -> (Vec<CardRecord>, Vec<CardRecord>) {
    let template = record("Lightning Bolt");
    let listed = corpus::listed();
    let other_names = alias_records();
    let names: BTreeSet<String> = listed
        .iter()
        .map(|printing| printing.name.clone())
        .chain(other_names.iter().map(|record| record.name.clone()))
        .collect();
    let oracle: BTreeMap<String, OracleId> = names
        .into_iter()
        .zip(1000_u128..)
        .map(|(name, n)| (name, OracleId(Uuid::from_u128(n).to_string())))
        .collect();

    let printings = listed
        .iter()
        .zip(1_000_000_u128..)
        .map(|(printing, n)| {
            let mut card = template.clone();
            card.oracle_id = oracle[&printing.name].clone();
            card.printing_id = PrintingId(Uuid::from_u128(n).to_string());
            card.name.clone_from(&printing.name);
            card.front.name.clone_from(&printing.name);
            card.set.clone_from(&printing.set);
            card.collector_number.clone_from(&printing.number);
            card
        })
        .collect();
    let other_names = other_names
        .into_iter()
        .map(|mut card| {
            card.oracle_id = oracle[&card.name].clone();
            card
        })
        .collect();
    (printings, other_names)
}

/// How the lines of the exports were matched.
#[derive(Debug, Default, PartialEq)]
struct Tally {
    exports: usize,
    printing: usize,
    name: usize,
    other_name: usize,
    selecting_a_printing: usize,
}

/// Resolves every export of the corpus through both indexes, which must agree on every line and leave none unresolved.
fn corpus_tally(stored: &CardIndex, in_memory: &CardIndex) -> Tally {
    let mut tally = Tally::default();
    for export in corpus::exports() {
        let deck = parse_as(export.format, &export.text);
        let from_db = resolve_deck(stored, &deck);
        assert_eq!(from_db, resolve_deck(in_memory, &deck), "{}", export.label);
        let unresolved = &from_db.unresolved;
        assert!(unresolved.is_empty(), "{}: {unresolved:?}", export.label);
        tally.exports += 1;
        for entry in &from_db.resolved {
            match entry.matched {
                MatchKind::Printing => tally.printing += 1,
                MatchKind::Name => tally.name += 1,
                MatchKind::OtherName => {
                    tally.other_name += 1;
                    tally.selecting_a_printing += usize::from(entry.set.is_some());
                }
            }
        }
    }
    tally
}

#[sqlx::test]
async fn every_export_of_the_corpus_resolves_the_same_through_both_indexes(pool: PgPool) {
    let database = db(pool);
    let (printings, other_names) = corpus_records();
    let all_printings: Vec<CardRecord> = printings.iter().chain(&other_names).cloned().collect();
    let (stored, in_memory) = both_indexes(&database, &printings, &all_printings).await;

    let tally = corpus_tally(&stored, &in_memory);

    assert_eq!(tally.exports, 183, "{tally:?}");
    assert_eq!(tally.other_name, 24, "{tally:?}");
    assert_eq!(tally.selecting_a_printing, 23, "{tally:?}");
    assert!(tally.printing + tally.name > 17_000, "{tally:?}");
}

/// The Oracle Cards file of the mtg-cli cache, or of the directory `MTG_REAL_DATA_DIR` names.
fn oracle_cards() -> Vec<CardRecord> {
    let dir = std::env::var("MTG_REAL_DATA_DIR").map_or_else(
        |_| PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".cache/mtg-cli"),
        PathBuf::from,
    );
    let mut files: Vec<PathBuf> = ["", "scryfall"]
        .into_iter()
        .filter_map(|sub| fs::read_dir(dir.join(sub)).ok())
        .flatten()
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
            name.is_some_and(|n| n.starts_with("oracle-cards-") && n.ends_with(".jsonl.gz"))
        })
        .collect();
    files.sort();
    let path = files
        .pop()
        .unwrap_or_else(|| panic!("no oracle cards file in {}", dir.display()));
    let file = File::open(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    cards_jsonl(GzDecoder::new(BufReader::new(file)))
        .map(|record| record.unwrap_or_else(|e| panic!("real card failed to parse: {e}")))
        .collect()
}

/// Needs the Oracle Cards file that `mtg-cli fetch scryfall` caches.
#[sqlx::test]
#[ignore = "needs the cached Oracle Cards file"]
async fn every_export_resolves_the_same_through_both_indexes_over_the_real_cards(pool: PgPool) {
    let database = db(pool);
    let cards = oracle_cards();
    assert!(cards.len() > 25_000, "only {} cards", cards.len());
    let mut printings = alias_records();
    printings.push(german_printing());
    let (stored, in_memory) = both_indexes(&database, &cards, &printings).await;

    let tally = corpus_tally(&stored, &in_memory);

    assert_eq!(tally.exports, 183, "{tally:?}");
    assert_eq!(tally.other_name, 24, "{tally:?}");

    // Every name of every real card: its own, its front face and the name of a meld piece.
    let mut melds = 0;
    for card in cards.iter().filter(|card| card.layout.is_deck_card()) {
        let front = card.name.split(" // ").next().unwrap_or(&card.name);
        let meld = card
            .melds_into
            .as_ref()
            .map(|result| format!("{} // {result}", card.name));
        melds += usize::from(meld.is_some());
        for query in [Some(card.name.as_str()), Some(front), meld.as_deref()]
            .into_iter()
            .flatten()
        {
            assert_eq!(stored.find(query), in_memory.find(query), "{query}");
            assert_eq!(
                stored.find_alias(query),
                in_memory.find_alias(query),
                "{query}"
            );
        }
    }
    assert_eq!(melds, 14, "the meld pieces of the real cards");
}

#[sqlx::test]
async fn the_meld_name_is_stored_on_the_pieces_and_follows_a_new_import(pool: PgPool) {
    let database = db(pool.clone());
    let meld = meld_records();
    let importer = database.importer();
    importer.import_cards(stream(&meld)).await.unwrap();
    let melding = || async {
        sqlx::query_as::<_, (String, String)>(
            "SELECT name, melds_into FROM cards WHERE melds_into IS NOT NULL ORDER BY name",
        )
        .fetch_all(&pool)
        .await
        .unwrap()
    };
    let result = "Brisela, Voice of Nightmares".to_owned();
    assert_eq!(
        melding().await,
        [
            ("Bruna, the Fading Light".to_owned(), result.clone()),
            ("Gisela, the Broken Blade".to_owned(), result),
        ]
    );
    let before = database.cards().index_version().await.unwrap();

    let again = importer.import_cards(stream(&meld)).await.unwrap();
    assert_eq!(
        (again.cards.updated, again.cards.unchanged),
        (0, 3),
        "{again:?}"
    );
    assert_eq!(database.cards().index_version().await.unwrap(), before);

    let without: Vec<CardRecord> = meld
        .into_iter()
        .map(|mut card| {
            card.melds_into = None;
            card
        })
        .collect();
    let report = importer.import_cards(stream(&without)).await.unwrap();
    assert_eq!(report.cards.updated, 2, "{report:?}");
    assert_ne!(database.cards().index_version().await.unwrap(), before);
    assert_eq!(melding().await, []);
}
