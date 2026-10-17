//! Ground truth (`truth.json`) of the export corpus and the loader for decks and their export files.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// The site that produced a deck's exports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Site {
    /// Moxfield: byte-exact output of the site's export endpoints.
    Moxfield,
    /// Archidekt: rendered from the deck JSON by a port of the site's exporter.
    Archidekt,
}

impl fmt::Display for Site {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Site::Moxfield => "moxfield",
            Site::Archidekt => "archidekt",
        })
    }
}

/// One card row of a board as the site itself lists it.
#[derive(Clone, Debug, Deserialize)]
pub struct Row {
    /// Oracle name, `A // B` for faces.
    pub name: String,
    /// Copies.
    pub qty: u32,
    /// Lower-case set code.
    pub set: String,
    /// Collector number, free text.
    pub cn: String,
}

/// The boards of a deck; token rows have another shape and are deliberately not modelled.
#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Boards {
    /// Commander board.
    pub commanders: Vec<Row>,
    /// Oathbreaker signature spells.
    pub signature_spells: Vec<Row>,
    /// Main deck.
    pub mainboard: Vec<Row>,
    /// Sideboard.
    pub sideboard: Vec<Row>,
    /// Maybeboard.
    pub maybeboard: Vec<Row>,
    /// Moxfield stickers.
    pub stickers: Vec<Row>,
}

/// How a variant's text marks the commanders (`variants.<name>.commanders`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Marking {
    /// Archidekt `{top}` flag or `Commander` header.
    Marked,
    /// Arena `Commander` section.
    Section,
    /// Moxfield `full`: the first lines, unmarked.
    FirstLines,
    /// Moxfield `mtgo`: a trailing block without header.
    LastBlock,
    /// Not told apart: every card is in the main list.
    None,
}

/// What one export variant carries (`variants.<name>`).
#[derive(Debug, Deserialize)]
pub struct Variant {
    /// Boards the text carries.
    pub boards: Vec<String>,
    /// How commanders are marked.
    pub commanders: Marking,
    /// Whether cards without an Arena id are missing, so the text is a subset.
    pub lossy: bool,
}

/// A deck's `truth.json`.
#[derive(Debug, Deserialize)]
pub struct Truth {
    /// What the site says the deck contains.
    pub boards: Boards,
    /// Export variants by file stem.
    pub variants: BTreeMap<String, Variant>,
}

/// A corpus deck: its folder and ground truth.
#[derive(Debug)]
pub struct Deck {
    /// Source site.
    pub site: Site,
    /// Folder name.
    pub name: String,
    /// Folder holding `truth.json` and the export files.
    pub dir: PathBuf,
    /// Parsed `truth.json`.
    pub truth: Truth,
}

impl Deck {
    /// Loads the deck whose `truth.json` is at `path` (absolute or relative to the crate root).
    pub fn load(path: &Path) -> Deck {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
        Deck::from_dir(path.parent().expect("truth.json has a folder").to_owned())
    }

    fn from_dir(dir: PathBuf) -> Deck {
        let name = file_name(&dir);
        let site = match file_name(dir.parent().expect("deck folder has a site folder")).as_str() {
            "moxfield" => Site::Moxfield,
            "archidekt" => Site::Archidekt,
            other => panic!("unknown corpus site {other:?}"),
        };
        let json = fs::read_to_string(dir.join("truth.json")).expect("truth.json is readable");
        let truth = serde_json::from_str(&json).unwrap_or_else(|e| panic!("{name}: {e}"));
        Deck {
            site,
            name,
            dir,
            truth,
        }
    }

    /// Every deck of the corpus, sorted by site and folder.
    pub fn all() -> Vec<Deck> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/corpus");
        let mut decks = Vec::new();
        for site in ["moxfield", "archidekt"] {
            let mut dirs: Vec<PathBuf> = fs::read_dir(root.join(site))
                .expect("corpus site folder")
                .map(|entry| entry.expect("readable entry").path())
                .filter(|path| path.is_dir())
                .collect();
            dirs.sort();
            decks.extend(dirs.into_iter().map(Deck::from_dir));
        }
        decks
    }

    /// `site/deck`, for failure messages.
    pub fn label(&self) -> String {
        format!("{}/{}", self.site, self.name)
    }

    /// Text of the export `variant`, if the deck has that file.
    pub fn try_text(&self, variant: &str) -> Option<String> {
        fs::read_to_string(self.dir.join(format!("{variant}.txt"))).ok()
    }

    /// Text of the export `variant`; the file must exist.
    pub fn text(&self, variant: &str) -> String {
        self.try_text(variant)
            .unwrap_or_else(|| panic!("{}: {variant}.txt is missing", self.label()))
    }

    /// Names of the `*.txt` files in the deck's folder, sorted, without extension.
    pub fn export_files(&self) -> Vec<String> {
        let mut stems: Vec<String> = fs::read_dir(&self.dir)
            .expect("deck folder")
            .filter_map(|file| {
                let path = file.ok()?.path();
                (path.extension()? == "txt").then(|| file_name(&path.with_extension("")))
            })
            .collect();
        stems.sort();
        stems
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .expect("UTF-8 file name")
        .to_owned()
}
