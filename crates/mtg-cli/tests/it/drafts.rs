//! Goal: every draft under `drafts/` is a whole rules file the checker marks `Unchecked`, in canonical form, named after
//! its card, with one record of the card's Scryfall data (equal to the cached Oracle Cards file where there is one),
//! written for the text of that record, so its read hash is the one `compile` stores, and no rules file or `Verified`
//! compiled row outranks it; the embedded set is the directory.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use mtg_cards::builtin;
use mtg_cards::card::Card;
use mtg_cards::load::{canonical, slug, unwhole};
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::support::read_hash;
use mtg_cards::{Caps, PrintedCard};
use mtg_cli::cache::{Cache, default_dir};
use mtg_cli::drafts::{self, FIXTURES, Misfit, misfits};
use mtg_cli::w2a_promote::fixture::w2a_oracle_cards;
use mtg_import::scryfall::{CardRecord, cards, cards_jsonl};
use mtg_testkit::corpus::printed_card;

/// The drafts directory of this crate.
pub fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("drafts")
}

/// One draft with the record of its card.
pub struct Item {
    /// The file name without `.json`.
    pub slug: String,
    /// The text of the draft file.
    pub text: String,
    /// The draft.
    pub card: Card,
    /// The text of the record file.
    pub record: String,
    /// The card as the importer reads its record.
    pub printed: PrintedCard,
}

fn read_record(text: &str) -> Result<CardRecord, String> {
    cards(format!("[{text}]").as_bytes())
        .next()
        .ok_or_else(|| "the file holds no record".to_owned())?
        .map_err(|error| error.to_string())
}

/// The slugs of the `.json` files of a directory, sorted.
fn slugs_in(dir: &Path) -> Vec<String> {
    let mut slugs: Vec<String> = fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.is_file() && path.extension().is_some_and(|e| e == "json"))
                .filter_map(|path| Some(path.file_stem()?.to_str()?.to_owned()))
                .collect()
        })
        .unwrap_or_default();
    slugs.sort();
    slugs
}

/// Every draft of the directory with its record; a file that cannot be read is reported with the others' problems.
pub fn items() -> (Vec<Item>, Vec<String>) {
    let mut found = Vec::new();
    let mut problems = Vec::new();
    for slug in slugs_in(&dir()) {
        let text = fs::read_to_string(dir().join(format!("{slug}.json"))).unwrap();
        let card = match serde_json::from_str::<Card>(&text) {
            Ok(card) => card,
            Err(error) => {
                problems.push(format!("{slug}: the draft does not parse: {error}"));
                continue;
            }
        };
        let record_file = dir().join(FIXTURES).join(format!("{slug}.json"));
        let Ok(record) = fs::read_to_string(&record_file) else {
            problems.push(format!("{slug}: no record at {}", record_file.display()));
            continue;
        };
        match read_record(&record) {
            Ok(parsed) => found.push(Item {
                slug,
                text,
                card,
                record,
                printed: printed_card(&parsed),
            }),
            Err(error) => problems.push(format!("{slug}: the record does not import: {error}")),
        }
    }
    (found, problems)
}

fn names_of(items: &[Item]) -> BTreeSet<String> {
    let library = builtin::library().unwrap();
    let mut names = mtg_cli::w2a_promote::w2a_corpus_names(library);
    for item in items {
        names.insert(item.printed.name.to_lowercase());
        names.extend(item.printed.faces.iter().map(|f| f.name.to_lowercase()));
    }
    names
}

#[test]
fn the_embedded_drafts_are_the_files_of_the_directory() {
    let embedded: Vec<(String, String)> = drafts::embedded()
        .iter()
        .map(|(slug, text)| ((*slug).to_owned(), (*text).to_owned()))
        .collect();
    let files: Vec<(String, String)> = slugs_in(&dir())
        .into_iter()
        .map(|slug| {
            let text = fs::read_to_string(dir().join(format!("{slug}.json"))).unwrap();
            (slug, text)
        })
        .collect();
    assert_eq!(
        embedded.len(),
        files.len(),
        "the build embeds {} drafts, the directory holds {}: build again",
        embedded.len(),
        files.len()
    );
    assert!(
        embedded == files,
        "an embedded draft differs from its file: build again"
    );
}

#[test]
fn every_draft_has_one_record_and_every_record_one_draft() {
    let (items, mut problems) = items();
    let drafts: BTreeSet<String> = slugs_in(&dir()).into_iter().collect();
    let records: BTreeSet<String> = slugs_in(&dir().join(FIXTURES)).into_iter().collect();
    problems.extend(
        records
            .difference(&drafts)
            .map(|slug| format!("{slug}: a record without a draft")),
    );
    let mut ids: BTreeMap<&str, &str> = BTreeMap::new();
    for item in &items {
        if slug(&item.printed.name) != item.slug {
            problems.push(format!(
                "{}: not the slug of the card's name ({})",
                item.slug, item.printed.name
            ));
        }
        if item.card.oracle_id != item.printed.oracle_id {
            problems.push(format!(
                "{}: the draft is for oracle id {}, its record for {}",
                item.slug, item.card.oracle_id, item.printed.oracle_id
            ));
        }
        if let Some(other) = ids.insert(&item.printed.oracle_id, &item.slug) {
            problems.push(format!(
                "{}: {other} is a draft of the same card",
                item.slug
            ));
        }
        if !item.record.ends_with('\n') || item.record.ends_with("\n\n") {
            problems.push(format!(
                "{}: the record does not end in one newline",
                item.slug
            ));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn every_draft_is_a_whole_unchecked_card_in_canonical_form_that_fits_its_record() {
    let (items, mut problems) = items();
    let library = builtin::library().unwrap();
    let names = names_of(&items);
    let caps: Caps = mtg_engine::caps();
    let pairs: Vec<(&PrintedCard, &Card)> = items.iter().map(|i| (&i.printed, &i.card)).collect();
    let fits = misfits(library, &caps, &pairs);
    let mut hashes: BTreeMap<String, &str> = BTreeMap::new();
    for (item, fit) in items.iter().zip(fits) {
        let slug = &item.slug;
        if item.text != canonical(&item.card) {
            problems.push(format!("{slug}: not in canonical form (`cargo run -p mtg-cli -- promote --draft` writes it)"));
        }
        if let Some(why) = unwhole(&item.card) {
            problems.push(format!("{slug}: {why}"));
        }
        match fit {
            None => {}
            Some(Misfit::Outranked) => problems.push(format!(
                "{slug}: the card is Verified without the draft: delete the draft and its record"
            )),
            Some(other) => problems.push(format!("{slug}: {}", other.reason())),
        }
        let report = check(&item.text, &item.printed, library, &names, &caps);
        match report.verdict() {
            Verdict::Unchecked => {}
            Verdict::Accepted => problems.push(format!(
                "{slug}: the checker accepts it now: promote it as a corpus card and delete the draft"
            )),
            other => problems.push(format!("{slug}: the checker finds it {}", other.word())),
        }
        if report.rules.as_ref() != Some(&item.card) {
            problems.push(format!("{slug}: the checker completes it differently"));
        }
        let hash = read_hash(&item.printed, &item.card);
        if hash.len() != 32 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            problems.push(format!("{slug}: read hash {hash} is not 32 hex characters"));
        }
        if let Some(other) = hashes.insert(hash, slug) {
            problems.push(format!("{slug}: the same read hash as {other}"));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// The newest Oracle Cards file of `MTG_REAL_DATA_DIR` or the `mtg-cli` cache; `None` where nothing was fetched.
fn oracle_cards() -> Option<PathBuf> {
    let cache = std::env::var_os("MTG_REAL_DATA_DIR")
        .map(PathBuf::from)
        .or_else(|| default_dir(|name| std::env::var_os(name)))?;
    w2a_oracle_cards(&Cache::new(cache)).ok()
}

/// Runs on the Oracle Cards file of the cache and says so when there is none.
#[test]
fn every_record_equals_the_cached_oracle_card() {
    let Some(file) = oracle_cards() else {
        eprintln!(
            "skipped: no Oracle Cards file; set MTG_REAL_DATA_DIR or run `mtg-cli fetch scryfall`"
        );
        return;
    };
    let (items, mut problems) = items();
    let wanted: BTreeSet<&str> = items.iter().map(|i| i.printed.oracle_id.as_str()).collect();
    let reader = BufReader::new(flate2::read::GzDecoder::new(BufReader::new(
        fs::File::open(&file).unwrap(),
    )));
    let real: BTreeMap<String, PrintedCard> = cards_jsonl(reader)
        .filter_map(Result::ok)
        .filter(|record| wanted.contains(record.oracle_id.as_str()))
        .map(|record| (record.oracle_id.to_string(), printed_card(&record)))
        .collect();
    for item in &items {
        match real.get(&item.printed.oracle_id) {
            None => problems.push(format!("{}: not in {}", item.slug, file.display())),
            Some(real) if *real != item.printed => problems.push(format!(
                "{}: the record differs from {}: run `promote --draft` again",
                item.slug,
                file.display()
            )),
            Some(_) => {}
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
