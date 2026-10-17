//! Line-oriented text deck parsing shared by the plain, Arena, MTGO text, Moxfield and Archidekt dialects.

use super::blocks::{BlankLines, Blocks, Place};
use super::line::{EntryError, LineOptions, parse_entry};
use super::section::{Header, header, section_from_category, strip_sideboard_prefix};
use super::types::{DeckEntry, LineError, ParsedDeck, Section};

/// Per-dialect switches for text parsing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TextOptions {
    /// Entry-line grammar switches.
    pub line: LineOptions,
    /// What a blank line means.
    pub blank_lines: BlankLines,
    /// Whether Arena's `About` / `Name` preamble is skipped.
    pub about_block: bool,
    /// Whether a line that cannot be an entry, alone above a block of entries, is a category or card-type header (Archidekt).
    pub category_headers: bool,
}

/// Splits on `\n`, `\r\n` and lone `\r`, yielding one-based line numbers.
pub fn numbered_lines(input: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut rest = Some(input);
    let mut number = 0;
    std::iter::from_fn(move || {
        let text = rest?;
        number += 1;
        if let Some(end) = text.find(['\n', '\r']) {
            let skip = if text[end..].starts_with("\r\n") {
                2
            } else {
                1
            };
            rest = Some(&text[end + skip..]);
            Some((number, &text[..end]))
        } else {
            rest = None;
            (!text.is_empty()).then_some((number, text))
        }
    })
}

enum Line<'a> {
    Blank,
    Comment,
    Preamble,
    Header(Header),
    Label,
    Entry(&'a str),
}

fn classify<'a>(raw: &'a str, in_preamble: &mut bool, options: TextOptions) -> Line<'a> {
    let text = raw.trim();
    if text.is_empty() {
        *in_preamble = false;
        return Line::Blank;
    }
    if text.starts_with("//") || text.starts_with('#') {
        return Line::Comment;
    }
    if *in_preamble {
        return Line::Preamble;
    }
    if options.about_block && text.eq_ignore_ascii_case("about") {
        *in_preamble = true;
        return Line::Preamble;
    }
    match header(text) {
        Some(header) => Line::Header(header),
        None => Line::Entry(text),
    }
}

/// Finds the category and card-type headers: lines no entry could be, standing alone above a block of entries.
fn mark_labels(lines: &mut [(usize, Line<'_>)], options: LineOptions) {
    for at in 0..lines.len() {
        let Line::Entry(text) = lines[at].1 else {
            continue;
        };
        let after_gap = at == 0 || matches!(lines[at - 1].1, Line::Blank);
        let above_entries = matches!(lines.get(at + 1), Some((_, Line::Entry(_))));
        if after_gap
            && above_entries
            && matches!(
                parse_entry(text, options),
                Err(EntryError::MissingCount | EntryError::NoSpaceAfterCount)
            )
        {
            lines[at].1 = Line::Label;
        }
    }
}

fn parse_entry_line(
    text: &str,
    number: usize,
    options: TextOptions,
    blocks: &mut Blocks,
    deck: &mut ParsedDeck,
) {
    let (text, prefixed) = match strip_sideboard_prefix(text) {
        Some(rest) => (rest, Some(Section::Sideboard)),
        None => (text, None),
    };
    match parse_entry(text, options.line) {
        Ok(raw) => match blocks.place {
            Place::Ignored(at) => {
                if let Some(ignored) = deck.ignored.get_mut(at) {
                    ignored.count += u64::from(raw.count);
                }
                blocks.counted();
            }
            Place::In(current) => {
                let from_category = raw.category.as_deref().and_then(section_from_category);
                let explicit = prefixed.or(from_category);
                if explicit.is_none() {
                    blocks.counted();
                }
                deck.entries.push(DeckEntry {
                    count: raw.count,
                    name: raw.name,
                    set: raw.set,
                    collector_number: raw.collector_number,
                    section: explicit.unwrap_or(current),
                });
            }
        },
        Err(error) => deck.errors.push(LineError {
            line: number,
            message: error.to_string(),
        }),
    }
}

/// Parses a text deck list under `options`; bad lines become [`LineError`]s.
pub fn parse_text(input: &str, options: TextOptions) -> ParsedDeck {
    let mut in_preamble = false;
    let mut lines: Vec<(usize, Line<'_>)> = numbered_lines(input.trim_start_matches('\u{feff}'))
        .map(|(number, raw)| (number, classify(raw, &mut in_preamble, options)))
        .collect();
    if options.category_headers {
        mark_labels(&mut lines, options.line);
    }
    let mut blocks = Blocks::new(
        lines
            .iter()
            .any(|(_, l)| matches!(l, Line::Header(_) | Line::Label)),
        options.blank_lines,
    );

    let mut deck = ParsedDeck::default();
    for (number, line) in lines {
        match line {
            Line::Blank => blocks.on_blank(),
            Line::Comment | Line::Preamble => {}
            Line::Header(header) => blocks.on_header(header, &mut deck),
            Line::Label => blocks.on_label(),
            Line::Entry(text) => parse_entry_line(text, number, options, &mut blocks, &mut deck),
        }
    }
    deck.ignored.retain(|section| section.count > 0);
    deck
}

#[cfg(test)]
mod dialect_tests;
#[cfg(test)]
mod tests;
