//! Guesses which deck-list format a piece of text is written in.

use super::line::{LineOptions, parse_entry};
use super::section::{header_section, is_shouting_header};
use super::text::numbered_lines;
use super::types::{DeckFormat, Section};

/// Dialect markers collected from one pass over the text.
#[derive(Default)]
struct Evidence {
    /// Format named by a marker on an entry line: Archidekt tags and categories outrank Moxfield foil flags.
    entry_marker: Option<DeckFormat>,
    /// An all-caps `NAME:` header, written by Moxfield and by the MTGO text export.
    shouting_header: bool,
    /// Whether some line names a set; the MTGO text export strips every printing.
    printing: bool,
    /// Whether the first meaningful line opens like an Arena export; unset until a line is seen.
    arena_start: Option<bool>,
}

impl Evidence {
    fn observe(&mut self, line: &str) {
        self.arena_start.get_or_insert_with(|| {
            let arena_header = matches!(
                header_section(line),
                Some(Section::Main | Section::Commander | Section::Companion)
            ) && !line.ends_with(':');
            arena_header || line.eq_ignore_ascii_case("about")
        });
        if is_shouting_header(line) {
            self.shouting_header = true;
            return;
        }
        let options = LineOptions {
            bracket_sets: false,
            categories: true,
        };
        if let Ok(entry) = parse_entry(line, options) {
            // A bare `[M10]` is a plain-text set code; categories only count after a `(SET)` printing.
            if entry.tagged || (entry.category.is_some() && entry.set.is_some()) {
                self.entry_marker = Some(DeckFormat::Archidekt);
            } else if entry.foil {
                self.entry_marker.get_or_insert(DeckFormat::Moxfield);
            }
            self.printing |= entry.set.is_some();
        }
    }

    fn format(&self) -> DeckFormat {
        match (self.entry_marker, self.shouting_header, self.printing) {
            (Some(format), _, _) => format,
            (None, true, true) => DeckFormat::Moxfield,
            (None, true, false) => DeckFormat::MtgoText,
            (None, false, _) if self.arena_start == Some(true) => DeckFormat::Arena,
            (None, false, _) => DeckFormat::PlainText,
        }
    }
}

/// Detects the format of `input`; anything unrecognized is plain text.
pub fn detect(input: &str) -> DeckFormat {
    let text = input.trim_start_matches('\u{feff}');
    if text.trim_start().starts_with('<') {
        return DeckFormat::MtgoDek;
    }
    let mut evidence = Evidence::default();
    for (_, raw) in numbered_lines(text) {
        let line = raw.trim();
        if line.is_empty() || line.starts_with("//") || line.starts_with('#') {
            continue;
        }
        evidence.observe(line);
    }
    evidence.format()
}

#[cfg(test)]
mod tests {
    //! Goal: format auto-detection picks the right dialect for realistic exports and never mistakes ordinary text for a richer format.
    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;

    use DeckFormat::{Archidekt, Arena, Moxfield, MtgoDek, MtgoText, PlainText};

    #[rstest]
    #[case::plain_simple("4 Lightning Bolt\n2 Shock\n", PlainText)]
    #[case::plain_x_counts("4x Lightning Bolt\n", PlainText)]
    #[case::plain_with_sets("4 Lightning Bolt (M10) 146\n", PlainText)]
    #[case::plain_bracket_sets("4 Lightning Bolt [M10]\n", PlainText)]
    #[case::plain_sb_prefix("4 Bolt\nSB: 2 Shock\n", PlainText)]
    #[case::plain_blank_sideboard("4 Bolt\n\n2 Shock\n", PlainText)]
    #[case::plain_sideboard_header("4 Bolt\nSideboard\n2 Shock\n", PlainText)]
    #[case::plain_first_line_sideboard("Sideboard\n2 Shock\n", PlainText)]
    #[case::plain_comments_first("// my deck\n4 Bolt\n", PlainText)]
    #[case::plain_crlf("4 Bolt\r\n2 Shock\r\n", PlainText)]
    #[case::plain_bom("\u{feff}4 Bolt\n", PlainText)]
    #[case::empty("", PlainText)]
    #[case::whitespace("  \n \n", PlainText)]
    #[case::garbage("lorem ipsum dolor", PlainText)]
    #[case::arena_deck_header("Deck\n4 Lightning Bolt (M10) 146\n", Arena)]
    #[case::arena_lowercase_header("deck\n4 Lightning Bolt\n", Arena)]
    #[case::arena_commander_first(
        "Commander\n1 Atraxa, Praetors' Voice (C16) 28\n\nDeck\n1 Sol Ring (CMM) 400\n",
        Arena
    )]
    #[case::arena_companion_first(
        "Companion\n1 Lurrus of the Dream-Den (IKO) 226\n\nDeck\n4 Bolt\n",
        Arena
    )]
    #[case::arena_about("About\nName Mono Red\n\nDeck\n4 Bolt\n", Arena)]
    #[case::arena_comment_before_header("// exported\nDeck\n4 Bolt\n", Arena)]
    #[case::arena_crlf("Deck\r\n4 Bolt\r\n\r\nSideboard\r\n2 Shock\r\n", Arena)]
    #[case::moxfield_foil("1 Sol Ring (CMM) 400 *F*\n1 Arcane Signet (CMM) 297\n", Moxfield)]
    #[case::moxfield_etched("1 Sol Ring (CMM) 400 *E*\n", Moxfield)]
    #[case::moxfield_shouting_header(
        "1 Sol Ring (CMM) 400\n\nSIDEBOARD:\n1 Fury (MH2) 126\n",
        Moxfield
    )]
    #[case::moxfield_commander_header(
        "COMMANDER:\n1 Atraxa (C16) 28\n\n1 Sol Ring (CMM) 400\n",
        Moxfield
    )]
    #[case::moxfield_one_printing_is_enough(
        "1 Sol Ring\n\nSIDEBOARD:\n1 Fury (MH2) 126\n",
        Moxfield
    )]
    #[case::moxfield_foil_without_printings("1 Sol Ring *F*\n\nSIDEBOARD:\n1 Fury\n", Moxfield)]
    #[case::mtgo_text_commander_last(
        "1 Sol Ring\n\nSIDEBOARD:\n1 Fury\n\n1 Atraxa, Praetors' Voice\n",
        MtgoText
    )]
    #[case::mtgo_text_without_commander("4 Sol Ring\n\nSIDEBOARD:\n2 Fury\n", MtgoText)]
    #[case::mtgo_text_crlf("1 Sol Ring\r\n\r\nSIDEBOARD:\r\n1 Fury\r\n", MtgoText)]
    #[case::mtgo_text_bom("\u{feff}1 Sol Ring\n\nSIDEBOARD:\n1 Fury\n", MtgoText)]
    #[case::archidekt_category("1x Sol Ring (cmm) 400 [Ramp]\n", Archidekt)]
    #[case::archidekt_commander(
        "1x Atraxa (c16) 28 [Commander{top}]\n1x Sol Ring (cmm) 400 [Ramp]\n",
        Archidekt
    )]
    #[case::archidekt_tags("1x Sol Ring (cmm) 400 ^Have,#37db9e^\n", Archidekt)]
    #[case::archidekt_beats_moxfield("1x Sol Ring (cmm) 400 *F* [Ramp]\n", Archidekt)]
    #[case::dek_with_prolog("<?xml version=\"1.0\"?>\n<Deck></Deck>", MtgoDek)]
    #[case::dek_no_prolog("<Deck><Cards Quantity=\"1\" Name=\"A\"/></Deck>", MtgoDek)]
    #[case::dek_leading_space("  \n<Deck/>", MtgoDek)]
    #[case::dek_bom("\u{feff}<?xml version=\"1.0\"?><Deck/>", MtgoDek)]
    fn detects_formats(#[case] input: &str, #[case] expected: DeckFormat) {
        assert_eq!(detect(input), expected);
    }

    proptest! {
        #[test]
        fn detection_never_panics(input in "\\PC{0,200}") {
            let _ = detect(&input);
        }

        #[test]
        fn plain_count_name_lines_are_plain_text(
            lines in prop::collection::vec((1u32..=99, "[A-Z][a-z]{2,10}( [A-Z][a-z]{2,8}){0,2}"), 1..12)
        ) {
            let rows: Vec<String> = lines.iter().map(|(c, n)| format!("{c} {n}\n")).collect();
            let text = rows.concat();
            prop_assert_eq!(detect(&text), PlainText);
        }
    }
}
