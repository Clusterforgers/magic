//! Maps each text-based deck format to its parsing switches.

use super::blocks::BlankLines;
use super::line::LineOptions;
use super::text::TextOptions;
use super::types::DeckFormat;

/// Moxfield's `#tag` suffixes, the only switch its dialects share.
const HASH_TAGS: LineOptions = LineOptions {
    bracket_sets: false,
    categories: false,
    hash_tags: true,
};

/// Text parsing switches for `format`, or `None` for formats that are not line-oriented text.
pub fn text_options(format: DeckFormat) -> Option<TextOptions> {
    let options = match format {
        DeckFormat::PlainText => TextOptions {
            line: LineOptions {
                bracket_sets: true,
                ..LineOptions::default()
            },
            blank_lines: BlankLines::SideboardNext,
            ..TextOptions::default()
        },
        DeckFormat::Arena => TextOptions {
            line: HASH_TAGS,
            about_block: true,
            ..TextOptions::default()
        },
        DeckFormat::Moxfield => TextOptions {
            line: HASH_TAGS,
            ..TextOptions::default()
        },
        DeckFormat::MtgoText => TextOptions {
            blank_lines: BlankLines::CommandZoneLast,
            ..TextOptions::default()
        },
        DeckFormat::Archidekt => TextOptions {
            line: LineOptions {
                categories: true,
                ..LineOptions::default()
            },
            category_headers: true,
            ..TextOptions::default()
        },
        DeckFormat::MtgoDek => return None,
    };
    Some(options)
}

#[cfg(test)]
mod tests {
    //! Goal: each format's dialect enables only the quirks that belong to it, so formats cannot silently bleed into each other.
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(DeckFormat::PlainText, "bracket_sets sideboard_next")]
    #[case(DeckFormat::Arena, "hash_tags about_block")]
    #[case(DeckFormat::Moxfield, "hash_tags")]
    #[case(DeckFormat::MtgoText, "command_zone_last")]
    #[case(DeckFormat::Archidekt, "categories category_headers")]
    fn dialect_switches(#[case] format: DeckFormat, #[case] enabled: &str) {
        let options = text_options(format).unwrap();
        let switches = [
            ("bracket_sets", options.line.bracket_sets),
            ("categories", options.line.categories),
            ("hash_tags", options.line.hash_tags),
            (
                "sideboard_next",
                options.blank_lines == BlankLines::SideboardNext,
            ),
            ("about_block", options.about_block),
            (
                "command_zone_last",
                options.blank_lines == BlankLines::CommandZoneLast,
            ),
            ("category_headers", options.category_headers),
        ];
        let on: Vec<&str> = switches
            .iter()
            .filter(|(_, value)| *value)
            .map(|(name, _)| *name)
            .collect();
        assert_eq!(on.join(" "), enabled);
    }

    #[test]
    fn dek_is_not_text() {
        assert!(text_options(DeckFormat::MtgoDek).is_none());
    }
}
