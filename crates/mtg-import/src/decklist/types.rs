//! Format-independent deck-list types.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Largest copy count accepted for one line; guards against overflow and typos like `1996`.
pub const MAX_COUNT: u32 = 999;

/// Longest card name accepted, in characters.
pub const MAX_NAME_CHARS: usize = 256;

/// Part of a deck a card belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Section {
    /// Main deck.
    Main,
    /// Sideboard.
    Sideboard,
    /// Commander(s) in the command zone.
    Commander,
    /// Companion.
    Companion,
    /// Cards considered but not in the deck.
    Maybeboard,
}

/// One deck-list line: a card name with a copy count and optional printing hint.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeckEntry {
    /// Number of copies.
    pub count: u32,
    /// Card name as written.
    pub name: String,
    /// Set code, uppercase.
    pub set: Option<String>,
    /// Collector number within the set; only meaningful together with `set`.
    pub collector_number: Option<String>,
    /// Section the line belongs to.
    pub section: Section,
}

impl DeckEntry {
    /// An entry without printing information.
    pub fn new(count: u32, name: impl Into<String>, section: Section) -> Self {
        Self {
            count,
            name: name.into(),
            set: None,
            collector_number: None,
            section,
        }
    }

    /// Adds a printing hint (set code is uppercased).
    #[must_use]
    pub fn with_printing(mut self, set: &str, collector_number: Option<&str>) -> Self {
        self.set = Some(set.to_ascii_uppercase());
        self.collector_number = collector_number.map(str::to_owned);
        self
    }
}

/// A line that could not be read as a card entry.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[error("line {line}: {message}")]
pub struct LineError {
    /// One-based line number.
    pub line: usize,
    /// What is wrong with the line.
    pub message: String,
}

/// A parsed deck list: the entries that were readable plus per-line errors.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedDeck {
    /// Entries in input order.
    pub entries: Vec<DeckEntry>,
    /// Lines that were skipped, in input order.
    pub errors: Vec<LineError>,
}

impl ParsedDeck {
    /// Total number of cards (sum of counts) in `section`.
    pub fn card_count(&self, section: Section) -> u64 {
        self.entries
            .iter()
            .filter(|e| e.section == section)
            .map(|e| u64::from(e.count))
            .sum()
    }
}

/// A recognized deck-list format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeckFormat {
    /// `4 Lightning Bolt` style text, optionally with sideboard/commander sections.
    PlainText,
    /// Magic Arena export with `Deck`/`Commander`/`Sideboard` headers.
    Arena,
    /// Magic Online `.dek` XML.
    MtgoDek,
    /// Moxfield text export.
    Moxfield,
    /// Archidekt text export with `[Category]` suffixes.
    Archidekt,
}
