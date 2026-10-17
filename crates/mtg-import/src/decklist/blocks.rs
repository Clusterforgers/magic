//! Tracks which part of the deck the next entry of a text list belongs to, as headers and blank lines go by.

use super::section::Header;
use super::types::{IgnoredSection, ParsedDeck, Section};

/// Where the entries being read go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// Into this section of the deck.
    In(Section),
    /// Into the ignored section at this index of [`ParsedDeck::ignored`].
    Ignored(usize),
}

/// What a blank line means in a dialect.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BlankLines {
    /// Nothing: sections come from headers and categories.
    #[default]
    Nothing,
    /// In a list without headers, a blank line after the main deck starts the sideboard.
    SideboardNext,
    /// The block after the main deck, a sideboard or an ignored section is the command zone (MTGO text lists commanders last).
    CommandZoneLast,
}

/// The reading position of one list: the current block and the dialect rules that move it.
pub struct Blocks {
    /// Where the next entry goes.
    pub place: Place,
    entries_in_block: usize,
    file_has_headers: bool,
    blank_lines: BlankLines,
}

impl Blocks {
    /// Starts in the main deck; `file_has_headers` says whether the list has any header or label.
    pub fn new(file_has_headers: bool, blank_lines: BlankLines) -> Self {
        Self {
            place: Place::In(Section::Main),
            entries_in_block: 0,
            file_has_headers,
            blank_lines,
        }
    }

    /// A header starts a new block in its section; an ignored section is reported once, however often it recurs.
    pub fn on_header(&mut self, header: Header, deck: &mut ParsedDeck) {
        self.place = match header {
            Header::Section(section) => Place::In(section),
            Header::Ignored(name) => {
                let at = deck
                    .ignored
                    .iter()
                    .position(|s| s.name.eq_ignore_ascii_case(&name))
                    .unwrap_or_else(|| {
                        deck.ignored.push(IgnoredSection { name, count: 0 });
                        deck.ignored.len() - 1
                    });
                Place::Ignored(at)
            }
        };
        self.entries_in_block = 0;
    }

    /// A header that says nothing about the board (an Archidekt category or card type) puts the block in the main deck.
    pub fn on_label(&mut self) {
        self.place = Place::In(Section::Main);
        self.entries_in_block = 0;
    }

    /// Notes an entry that did not name its own section.
    pub fn counted(&mut self) {
        self.entries_in_block += 1;
    }

    /// A blank line ends a command-zone block and, as the dialect says, moves on from the main deck or a sideboard.
    pub fn on_blank(&mut self) {
        if self.entries_in_block == 0 {
            return;
        }
        let next = match (self.place, self.blank_lines) {
            (Place::In(Section::Commander | Section::Companion), _) => Section::Main,
            (Place::In(Section::Main), BlankLines::SideboardNext) if !self.file_has_headers => {
                Section::Sideboard
            }
            (
                Place::In(Section::Main | Section::Sideboard) | Place::Ignored(_),
                BlankLines::CommandZoneLast,
            ) => Section::Commander,
            _ => return,
        };
        self.place = Place::In(next);
        self.entries_in_block = 0;
    }
}
