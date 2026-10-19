//! Deck-list parsing (plain text, Arena, MTGO text and `.dek`, Moxfield, Archidekt) and export as text, Arena or MTGO text.

mod affix;
mod blocks;
mod commander;
mod dek;
mod detect;
mod dialect;
mod export;
mod line;
mod section;
mod text;
mod types;
mod writers;

pub use commander::promote_implied_commander;
pub use detect::detect;
pub use export::{ExportError, export};
pub use types::{
    DeckEntry, DeckFormat, IgnoredSection, LineError, MAX_COUNT, MAX_NAME_CHARS, ParsedDeck,
    Section,
};
pub use writers::{ExportFormat, export_as};

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
