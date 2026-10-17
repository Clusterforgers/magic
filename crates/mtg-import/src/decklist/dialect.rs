//! Maps each text-based deck format to its parsing switches.

use super::line::LineOptions;
use super::text::TextOptions;
use super::types::DeckFormat;

/// Text parsing switches for `format`, or `None` for formats that are not line-oriented text.
pub fn text_options(format: DeckFormat) -> Option<TextOptions> {
    let options = match format {
        DeckFormat::PlainText => TextOptions {
            line: LineOptions {
                bracket_sets: true,
                categories: false,
            },
            blank_line_sideboard: true,
            ..TextOptions::default()
        },
        DeckFormat::Arena => TextOptions {
            about_block: true,
            ..TextOptions::default()
        },
        DeckFormat::Moxfield => TextOptions::default(),
        DeckFormat::MtgoText => TextOptions {
            commander_after_sideboard: true,
            ..TextOptions::default()
        },
        DeckFormat::Archidekt => TextOptions {
            line: LineOptions {
                bracket_sets: false,
                categories: true,
            },
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
    #[case(DeckFormat::PlainText, "bracket_sets blank_line_sideboard")]
    #[case(DeckFormat::Arena, "about_block")]
    #[case(DeckFormat::Moxfield, "")]
    #[case(DeckFormat::MtgoText, "commander_after_sideboard")]
    #[case(DeckFormat::Archidekt, "categories")]
    fn dialect_switches(#[case] format: DeckFormat, #[case] enabled: &str) {
        let options = text_options(format).unwrap();
        let switches = [
            ("bracket_sets", options.line.bracket_sets),
            ("categories", options.line.categories),
            ("blank_line_sideboard", options.blank_line_sideboard),
            ("about_block", options.about_block),
            (
                "commander_after_sideboard",
                options.commander_after_sideboard,
            ),
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
