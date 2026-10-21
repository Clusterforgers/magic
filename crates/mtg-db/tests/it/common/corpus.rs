//! Goal: the export corpus of mtg-import as test data: every export text with the format a user would name, and the cards and printings the sites list for the decks.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use mtg_import::decklist::DeckFormat;
use serde_json::Value;

use super::fixture_path;

/// One export file of a corpus deck.
pub struct Export {
    /// `site/deck/variant`, for failure messages.
    pub label: String,
    /// The format a user would name for the file.
    pub format: DeckFormat,
    /// The text.
    pub text: String,
}

/// A printing a site lists for a deck: the oracle name (`A // B` for faces), the set and the collector number.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Listed {
    /// Oracle name.
    pub name: String,
    /// Lower-case set code.
    pub set: String,
    /// Collector number.
    pub number: String,
}

/// The boards of `truth.json` that hold deck cards.
const BOARDS: [&str; 6] = [
    "commanders",
    "signatureSpells",
    "mainboard",
    "sideboard",
    "maybeboard",
    "stickers",
];

fn deck_dirs() -> Vec<(String, PathBuf)> {
    let mut dirs = Vec::new();
    for site in ["moxfield", "archidekt"] {
        let root = fixture_path("corpus").join(site);
        let mut found: Vec<PathBuf> = fs::read_dir(&root)
            .unwrap_or_else(|e| panic!("corpus folder {}: {e}", root.display()))
            .map(|entry| entry.unwrap_or_else(|e| panic!("corpus entry: {e}")).path())
            .filter(|path| path.is_dir())
            .collect();
        found.sort();
        dirs.extend(found.into_iter().map(|dir| (site.to_owned(), dir)));
    }
    dirs
}

/// The format a user would name for an export variant of a site (the class table of the corpus README).
fn format_of(site: &str, variant: &str) -> DeckFormat {
    match (site, variant) {
        ("moxfield", "mtgo") => DeckFormat::MtgoText,
        ("moxfield", v) if v.starts_with("arena") => DeckFormat::Arena,
        ("moxfield", v) if v.starts_with("full") => DeckFormat::Moxfield,
        ("archidekt", _) => DeckFormat::Archidekt,
        _ => panic!("variant {variant:?} of {site} has no format"),
    }
}

/// Every export file of the corpus, sorted by site, deck and variant.
pub fn exports() -> Vec<Export> {
    let mut exports = Vec::new();
    for (site, dir) in deck_dirs() {
        let deck = dir.file_name().map(|n| n.to_string_lossy().into_owned());
        let mut files: Vec<PathBuf> = fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("deck folder {}: {e}", dir.display()))
            .map(|entry| entry.unwrap_or_else(|e| panic!("deck entry: {e}")).path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "txt"))
            .collect();
        files.sort();
        for file in files {
            let variant = file
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let text =
                fs::read_to_string(&file).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
            exports.push(Export {
                label: format!("{site}/{}/{variant}", deck.clone().unwrap_or_default()),
                format: format_of(&site, &variant),
                text,
            });
        }
    }
    exports
}

/// Writes the faces of a multi-face card the oracle way (`A // B`).
fn canonical(name: &str) -> String {
    name.split('/')
        .map(str::trim)
        .filter(|face| !face.is_empty())
        .collect::<Vec<_>>()
        .join(" // ")
}

/// Every distinct printing the sites list for the corpus decks.
pub fn listed() -> BTreeSet<Listed> {
    let mut listed = BTreeSet::new();
    for (_, dir) in deck_dirs() {
        let path = dir.join("truth.json");
        let json = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let truth: Value = serde_json::from_str(&json)
            .unwrap_or_else(|e| panic!("{} parses: {e}", path.display()));
        for board in BOARDS {
            let rows = truth["boards"][board].as_array().into_iter().flatten();
            for row in rows {
                let text = |key: &str| row[key].as_str().unwrap_or_default().to_owned();
                listed.insert(Listed {
                    name: canonical(&text("name")),
                    set: text("set"),
                    number: text("cn"),
                });
            }
        }
    }
    listed
}
