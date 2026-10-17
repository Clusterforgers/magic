//! Recognizes section headers, sideboard prefixes and Archidekt categories.

use super::types::Section;

/// Maps a section word (case-insensitive) to its section.
fn section_from_word(word: &str) -> Option<Section> {
    let word = word.to_ascii_lowercase();
    match word.as_str() {
        "deck" | "main" | "mainboard" | "main deck" | "maindeck" => Some(Section::Main),
        "sideboard" | "side" | "side board" => Some(Section::Sideboard),
        "commander" | "commanders" | "command zone" => Some(Section::Commander),
        "companion" => Some(Section::Companion),
        "maybeboard" | "maybe" | "maybe board" => Some(Section::Maybeboard),
        _ => None,
    }
}

/// The header word the exporter writes for `section`.
pub fn header_word(section: Section) -> &'static str {
    match section {
        Section::Main => "Deck",
        Section::Sideboard => "Sideboard",
        Section::Commander => "Commander",
        Section::Companion => "Companion",
        Section::Maybeboard => "Maybeboard",
    }
}

/// Reads a header line such as `Sideboard`, `SIDEBOARD:` or `Sideboard (15)`.
pub fn header_section(line: &str) -> Option<Section> {
    let mut text = line.trim().trim_end_matches(':').trim_end();
    if let Some(open) = text.rfind('(')
        && let Some(inner) = text[open + 1..].strip_suffix(')')
        && !inner.is_empty()
        && inner.bytes().all(|b| b.is_ascii_digit())
    {
        text = text[..open].trim_end().trim_end_matches(':').trim_end();
    }
    section_from_word(text)
}

/// Whether a header uses Moxfield's all-caps `NAME:` style.
pub fn is_shouting_header(line: &str) -> bool {
    let text = line.trim();
    text.ends_with(':')
        && text.chars().any(char::is_alphabetic)
        && !text.chars().any(char::is_lowercase)
        && header_section(text).is_some()
}

/// Splits an `SB:` prefix (any case, optional space) off an entry line.
pub fn strip_sideboard_prefix(line: &str) -> Option<&str> {
    let prefix = line.get(..3)?;
    prefix
        .eq_ignore_ascii_case("sb:")
        .then(|| line[3..].trim_start())
}

/// Maps an Archidekt category list such as `Commander{top},Ramp` to a section, if any item names one.
pub fn section_from_category(category: &str) -> Option<Section> {
    category.split(',').find_map(|item| {
        let name = item.split('{').next().unwrap_or(item).trim();
        match section_from_word(name) {
            Some(Section::Main) | None => None,
            other => other,
        }
    })
}

#[cfg(test)]
mod tests {
    //! Goal: section vocabulary is recognized in every spelling exporters use, without mistaking card names for headers.
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("Deck", Some(Section::Main))]
    #[case("deck", Some(Section::Main))]
    #[case("Main", Some(Section::Main))]
    #[case("Mainboard", Some(Section::Main))]
    #[case("Main Deck", Some(Section::Main))]
    #[case("Sideboard", Some(Section::Sideboard))]
    #[case("SIDEBOARD:", Some(Section::Sideboard))]
    #[case("Sideboard:", Some(Section::Sideboard))]
    #[case("  Sideboard (15)  ", Some(Section::Sideboard))]
    #[case("Side", Some(Section::Sideboard))]
    #[case("Commander", Some(Section::Commander))]
    #[case("COMMANDER:", Some(Section::Commander))]
    #[case("Commanders", Some(Section::Commander))]
    #[case("Companion", Some(Section::Companion))]
    #[case("Maybeboard", Some(Section::Maybeboard))]
    #[case("MAYBEBOARD:", Some(Section::Maybeboard))]
    #[case("Sideboards", None)]
    #[case("Lightning Bolt", None)]
    #[case("4 Sideboard", None)]
    #[case("Sideboard (x)", None)]
    #[case("Sideboard ()", None)]
    #[case("", None)]
    #[case(":", None)]
    fn recognizes_headers(#[case] line: &str, #[case] expected: Option<Section>) {
        assert_eq!(header_section(line), expected);
    }

    #[rstest]
    #[case(Section::Main)]
    #[case(Section::Sideboard)]
    #[case(Section::Commander)]
    #[case(Section::Companion)]
    #[case(Section::Maybeboard)]
    fn exported_headers_read_back(#[case] section: Section) {
        assert_eq!(header_section(header_word(section)), Some(section));
    }

    #[rstest]
    #[case("SB: 2 Pyroblast", Some("2 Pyroblast"))]
    #[case("sb:2 Pyroblast", Some("2 Pyroblast"))]
    #[case("Sb:   2 Pyroblast", Some("2 Pyroblast"))]
    #[case("SB 2 Pyroblast", None)]
    #[case("2 Pyroblast", None)]
    #[case("SB", None)]
    #[case("é: 2 Pyroblast", None)]
    #[case("", None)]
    fn strips_sideboard_prefix(#[case] line: &str, #[case] expected: Option<&str>) {
        assert_eq!(strip_sideboard_prefix(line), expected);
    }

    #[rstest]
    #[case("Commander{top}", Some(Section::Commander))]
    #[case("Ramp,Commander", Some(Section::Commander))]
    #[case("Sideboard", Some(Section::Sideboard))]
    #[case("Maybeboard{noDeck}{noPrice}", Some(Section::Maybeboard))]
    #[case("Ramp", None)]
    #[case("Deck", None)]
    #[case("", None)]
    fn maps_archidekt_categories(#[case] category: &str, #[case] expected: Option<Section>) {
        assert_eq!(section_from_category(category), expected);
    }

    #[rstest]
    #[case("SIDEBOARD:", true)]
    #[case("COMMANDER:", true)]
    #[case("Sideboard:", false)]
    #[case("SIDEBOARD", false)]
    #[case("RAMP:", false)]
    #[case(":", false)]
    fn detects_shouting_headers(#[case] line: &str, #[case] expected: bool) {
        assert_eq!(is_shouting_header(line), expected);
    }
}
