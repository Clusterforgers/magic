//! Goal: the checks of a corpus (a card directory and its Scryfall fixture) that stand for every count a table once held: files pair with records, fingerprints are current, the compiler's status follows its clauses, and the compiled sources load (this is not a test module itself).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use mtg_cards::card::{Card, ImplStatus, TokenFile};
use mtg_cards::compile::compile;
use mtg_cards::load::{Library, Source, canonical, parse_json, slug};
use mtg_cards::support::{PrintedCard, has_rules_text, oracle_fingerprint};
use mtg_import::scryfall::{CardRecord, cards};

/// The command that rewrites the fingerprints of the card files and adds the reproduced cards to the ratchet list.
pub const UPDATE: &str = "MTG_UPDATE_CARDS=1 cargo test -p mtg-cards corpus::";

/// Where the steps for a new corpus card are written down.
pub const ADDING: &str = "docs/cards-dsl.md, \"Adding a corpus card\"";

/// A card directory and the text of the Scryfall fixture it is checked against: the repository's own, or a copy a test changes.
pub struct Corpus {
    /// The directory of rules files (`cards/`), with `tokens/` inside.
    pub dir: PathBuf,
    /// The fixture: a JSON array of Scryfall objects.
    pub fixture: String,
}

/// The files with this extension directly in `dir`, in path order.
fn json_in(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("a readable directory")
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && path.extension().is_some_and(|e| e == "json"))
        .collect();
    files.sort();
    files
}

/// Every `.json` file below `dir`, in path order.
fn json_below(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(dir)
        .expect("a readable directory")
        .flatten()
    {
        let path = entry.path();
        if path.is_dir() {
            found.extend(json_below(&path));
        } else if path.extension().is_some_and(|e| e == "json") {
            found.push(path);
        }
    }
    found.sort();
    found
}

/// With `MTG_UPDATE_CARDS` set, rewrites every file of the repository's directory in canonical form with the
/// fingerprints of the fixture, once per run. Whoever loads the library first does it and the others wait, so the
/// run does not fail on a file it is about to mend.
pub fn update_repo_once() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        if std::env::var_os("MTG_UPDATE_CARDS").is_none() {
            return;
        }
        for (path, text, wanted) in Corpus::repo().rewrites(true) {
            if text != wanted {
                std::fs::write(path, wanted).expect("a writable rules file");
            }
        }
    });
}

/// The compiled card as the file the loader would read.
pub fn source_of(card: &Card) -> Source {
    Source {
        path: format!("cards/{}.json", slug(&card.name)),
        text: canonical(card),
    }
}

impl Corpus {
    /// The corpus of this repository.
    pub fn repo() -> Corpus {
        Corpus {
            dir: super::cards_dir(),
            fixture: super::FIXTURE.to_owned(),
        }
    }

    /// The card files (`cards/*.json`, the tokens aside), in path order.
    pub fn card_files(&self) -> Vec<PathBuf> {
        json_in(&self.dir)
    }

    /// The token files (`cards/tokens/*.json`), in path order.
    pub fn token_files(&self) -> Vec<PathBuf> {
        json_in(&self.dir.join("tokens"))
    }

    /// The predefined tokens as sources, named like the embedded files.
    pub fn token_sources(&self) -> Vec<Source> {
        self.token_files()
            .into_iter()
            .map(|path| Source {
                path: format!(
                    "cards/tokens/{}",
                    path.file_name().expect("a file name").to_string_lossy()
                ),
                text: std::fs::read_to_string(&path).expect("a token file"),
            })
            .collect()
    }

    /// The imported records of the fixture.
    pub fn records(&self) -> Vec<CardRecord> {
        cards(self.fixture.as_bytes())
            .map(|record| record.expect("the fixture holds only valid records"))
            .collect()
    }

    /// The imported cards of the fixture.
    pub fn printed(&self) -> Vec<PrintedCard> {
        self.records().iter().map(super::printed_of).collect()
    }

    /// Every file of the directory (the tokens too) with its text and the canonical text it should have; with
    /// `fingerprints`, a `Full` or `Partial` card also carries the fingerprints of the fixture's text.
    fn rewrites(&self, fingerprints: bool) -> Vec<(PathBuf, String, String)> {
        let printed = self.printed();
        let mut rewrites = Vec::new();
        for path in json_below(&self.dir) {
            let name = path.file_name().expect("a file name").to_string_lossy();
            let text = std::fs::read_to_string(&path).expect("a rules file");
            let wanted = if path.parent().is_some_and(|dir| dir.ends_with("tokens")) {
                let token: TokenFile =
                    parse_json(&name, &text).unwrap_or_else(|e| panic!("{name}: {e:?}"));
                canonical(&token)
            } else {
                let mut card: Card =
                    parse_json(&name, &text).unwrap_or_else(|e| panic!("{name}: {e:?}"));
                let shown = printed.iter().find(|p| p.oracle_id == card.oracle_id);
                if let Some(shown) = shown.filter(|_| {
                    fingerprints
                        && matches!(card.impl_status, ImplStatus::Full | ImplStatus::Partial)
                }) {
                    for (face, shown) in card.faces.iter_mut().zip(&shown.faces) {
                        face.oracle_fingerprint = oracle_fingerprint(&shown.oracle_text);
                    }
                }
                canonical(&card)
            };
            rewrites.push((path, text, wanted));
        }
        rewrites
    }

    /// The files that are not in canonical form.
    pub fn non_canonical(&self) -> Vec<String> {
        self.rewrites(false)
            .into_iter()
            .filter(|(_, text, wanted)| text != wanted)
            .map(|(path, ..)| {
                path.file_name()
                    .expect("a file name")
                    .to_string_lossy()
                    .into_owned()
            })
            .collect()
    }

    /// The library of the directory, validated; a file the loader refuses stops the test with the commands that mend it.
    pub fn library(&self) -> Library {
        if self.dir == super::cards_dir() {
            update_repo_once();
        }
        Library::load_dir(&self.dir).unwrap_or_else(|errors| {
            panic!("{errors}\n(a missing or stale fingerprint: run `{UPDATE}`; see {ADDING})")
        })
    }

    /// Every card file without exactly one fixture record, and every fixture record without exactly one card file.
    pub fn pairing_problems(&self) -> Vec<String> {
        let mut files: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for path in self.card_files() {
            let file = path.file_name().expect("a file name").to_string_lossy();
            let text = std::fs::read_to_string(&path).expect("a card file");
            let card: Card = parse_json(&file, &text).unwrap_or_else(|e| panic!("{file}: {e:?}"));
            files
                .entry(card.oracle_id)
                .or_default()
                .push(format!("{} ({file})", card.name));
        }
        let mut records: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for record in self.records() {
            records
                .entry(record.oracle_id.to_string())
                .or_default()
                .push(record.name);
        }
        let mut problems = Vec::new();
        for (id, named) in &files {
            if named.len() > 1 {
                problems.push(format!("{named:?} share the oracle id {id}"));
            }
            if !records.contains_key(id) {
                problems.push(format!(
                    "{}: no record in the fixture has its oracle id; add the Scryfall object to \
                     tests/fixtures/corpus_scryfall.json ({ADDING})",
                    named.join(", ")
                ));
            }
        }
        for (id, named) in &records {
            if named.len() > 1 {
                problems.push(format!("the fixture holds {named:?} more than once ({id})"));
            }
            if !files.contains_key(id) {
                problems.push(format!(
                    "{}: the fixture has a record but cards/ has no file with its oracle id; add \
                     cards/<slug>.json ({ADDING})",
                    named.join(", ")
                ));
            }
        }
        problems
    }

    /// Every `Full` or `Partial` face whose fingerprint is not that of the imported text, with the card.
    pub fn fingerprint_problems(&self) -> Vec<String> {
        if self.dir == super::cards_dir() {
            update_repo_once();
        }
        let printed = self.printed();
        let mut problems = Vec::new();
        // The files are read as data: the loader refuses a missing fingerprint without saying which command writes it.
        for path in self.card_files() {
            let file = path.file_name().expect("a file name").to_string_lossy();
            let text = std::fs::read_to_string(&path).expect("a card file");
            let card: Card = parse_json(&file, &text).unwrap_or_else(|e| panic!("{file}: {e:?}"));
            if !matches!(card.impl_status, ImplStatus::Full | ImplStatus::Partial) {
                continue;
            }
            let Some(shown) = printed.iter().find(|p| p.oracle_id == card.oracle_id) else {
                continue;
            };
            if shown.faces.len() != card.faces.len() {
                problems.push(format!(
                    "{}: the file has {} faces and the fixture {}",
                    card.name,
                    card.faces.len(),
                    shown.faces.len()
                ));
                continue;
            }
            for (index, (face, shown)) in card.faces.iter().zip(&shown.faces).enumerate() {
                let wanted = oracle_fingerprint(&shown.oracle_text);
                let state = match face.oracle_fingerprint.as_str() {
                    "" => "missing",
                    held if held != wanted => "stale",
                    _ => continue,
                };
                problems.push(format!(
                    "{}: the fingerprint of face {index} is {state}; run `{UPDATE}` ({ADDING})",
                    card.name
                ));
            }
        }
        problems
    }

    /// Every compiled card whose status is not the one its clauses earn: `Manual` when none is read, `Full` when all are,
    /// `Partial` between; and every card that compiles to nothing though it has rules text, or the reverse.
    pub fn status_problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        for shown in self.printed() {
            let compiled = compile(&shown);
            let has_text = shown
                .faces
                .iter()
                .any(|face| has_rules_text(&face.oracle_text));
            let Some(card) = &compiled.card else {
                if has_text {
                    problems.push(format!("{}: rules text but no compiled card", shown.name));
                }
                continue;
            };
            if !has_text {
                problems.push(format!(
                    "{}: a compiled card without rules text",
                    shown.name
                ));
            }
            let total = compiled.clauses.len();
            let read = compiled.clauses.iter().filter(|clause| clause.read).count();
            let wanted = if read == 0 {
                ImplStatus::Manual
            } else if read == total {
                ImplStatus::Full
            } else {
                ImplStatus::Partial
            };
            if card.impl_status != wanted {
                problems.push(format!(
                    "{}: {:?} with {read} of {total} clauses read, wanted {wanted:?}",
                    shown.name, card.impl_status
                ));
            }
        }
        problems
    }

    /// The compiled cards, with the tokens, as sources that do not load, or whose cards differ from the imported text;
    /// one source for each card that has rules text.
    pub fn compiled_problems(&self) -> Vec<String> {
        let printed = self.printed();
        let mut sources: Vec<Source> = printed
            .iter()
            .filter_map(|shown| compile(shown).card)
            .map(|card| source_of(&card))
            .collect();
        let with_text = printed
            .iter()
            .filter(|shown| {
                shown
                    .faces
                    .iter()
                    .any(|face| has_rules_text(&face.oracle_text))
            })
            .count();
        let mut problems = Vec::new();
        if sources.len() != with_text {
            problems.push(format!(
                "{} compiled sources for {with_text} cards with rules text",
                sources.len()
            ));
        }
        sources.extend(self.token_sources());
        match Library::from_sources(&sources) {
            Ok(library) => problems.extend(
                library
                    .check_printed(&printed)
                    .iter()
                    .map(ToString::to_string),
            ),
            Err(errors) => problems.push(errors.to_string()),
        }
        problems
    }
}
