//! Maps each text-based deck format to its parsing switches.

use super::line::LineOptions;
use super::text::TextOptions;
use super::types::DeckFormat;

/// Text parsing switches for `format`, or `None` for formats that are not line-oriented text.
pub fn text_options(format: DeckFormat) -> Option<TextOptions> {
    let text = |bracket_sets, categories, blank_line_sideboard, about_block| TextOptions {
        line: LineOptions {
            bracket_sets,
            categories,
        },
        blank_line_sideboard,
        about_block,
    };
    match format {
        DeckFormat::PlainText => Some(text(true, false, true, false)),
        DeckFormat::Arena => Some(text(false, false, false, true)),
        DeckFormat::Moxfield => Some(text(false, false, false, false)),
        DeckFormat::Archidekt => Some(text(false, true, false, false)),
        DeckFormat::MtgoDek => None,
    }
}

#[cfg(test)]
mod tests {
    //! Goal: each format's dialect enables only the quirks that belong to it, so formats cannot silently bleed into each other.
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(DeckFormat::PlainText, true, false, true, false)]
    #[case(DeckFormat::Arena, false, false, false, true)]
    #[case(DeckFormat::Moxfield, false, false, false, false)]
    #[case(DeckFormat::Archidekt, false, true, false, false)]
    fn dialect_switches(
        #[case] format: DeckFormat,
        #[case] bracket_sets: bool,
        #[case] categories: bool,
        #[case] blank_line_sideboard: bool,
        #[case] about_block: bool,
    ) {
        let options = text_options(format).unwrap();
        assert_eq!(options.line.bracket_sets, bracket_sets);
        assert_eq!(options.line.categories, categories);
        assert_eq!(options.blank_line_sideboard, blank_line_sideboard);
        assert_eq!(options.about_block, about_block);
    }

    #[test]
    fn dek_is_not_text() {
        assert!(text_options(DeckFormat::MtgoDek).is_none());
    }
}
