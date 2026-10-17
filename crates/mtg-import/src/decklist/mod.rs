//! Deck-list parsing (plain text, Arena, MTGO `.dek`, Moxfield, Archidekt) and plain-text export.

mod dek;
mod detect;
mod dialect;
mod export;
mod line;
mod section;
mod text;
mod types;

pub use detect::detect;
pub use export::{ExportError, export};
pub use types::{DeckEntry, DeckFormat, LineError, MAX_COUNT, MAX_NAME_CHARS, ParsedDeck, Section};

/// Parses `input` as `format`; bad lines become entries in [`ParsedDeck::errors`].
pub fn parse_as(format: DeckFormat, input: &str) -> ParsedDeck {
    match dialect::text_options(format) {
        Some(options) => text::parse_text(input, options),
        None => dek::parse_dek(input),
    }
}

/// Detects the format of `input` and parses it accordingly.
pub fn parse(input: &str) -> ParsedDeck {
    parse_as(detect(input), input)
}
