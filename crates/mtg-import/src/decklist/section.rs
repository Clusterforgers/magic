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

/// What a header line stands for: a part of the deck, or cards the format lists apart from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Header {
    /// A part of the deck.
    Section(Section),
    /// Listed cards that are no deck cards, such as Moxfield's stickers; holds the name as written.
    Ignored(String),
}

/// Sections some formats list next to the deck that hold no deck cards (case-insensitive).
const IGNORED_WORDS: [&str; 1] = ["stickers"];

/// The words of a header line without its colon and `(15)` count.
fn header_text(line: &str) -> &str {
    let mut text = line.trim().trim_end_matches(':').trim_end();
    if let Some(open) = text.rfind('(')
        && let Some(inner) = text[open + 1..].strip_suffix(')')
        && !inner.is_empty()
        && inner.bytes().all(|b| b.is_ascii_digit())
    {
        text = text[..open].trim_end().trim_end_matches(':').trim_end();
    }
    text
}

/// Reads a header line such as `Sideboard`, `SIDEBOARD:`, `Sideboard (15)` or `STICKERS:`.
pub fn header(line: &str) -> Option<Header> {
    let text = header_text(line);
    if let Some(section) = section_from_word(text) {
        return Some(Header::Section(section));
    }
    IGNORED_WORDS
        .iter()
        .any(|word| text.eq_ignore_ascii_case(word))
        .then(|| Header::Ignored(text.to_owned()))
}

/// Reads a header line that names a part of the deck.
pub fn header_section(line: &str) -> Option<Section> {
    section_from_word(header_text(line))
}

/// Whether a header uses Moxfield's all-caps `NAME:` style.
pub fn is_shouting_header(line: &str) -> bool {
    let text = line.trim();
    text.ends_with(':')
        && text.chars().any(char::is_alphabetic)
        && !text.chars().any(char::is_lowercase)
        && header(text).is_some()
}

/// Splits an `SB:` prefix (any case, optional space) off an entry line.
pub fn strip_sideboard_prefix(line: &str) -> Option<&str> {
    let prefix = line.get(..3)?;
    prefix
        .eq_ignore_ascii_case("sb:")
        .then(|| line[3..].trim_start())
}

/// Maps an Archidekt category list such as `Ramp,Maybeboard{noDeck}` to a section; only the first category counts.
///
/// The `{top}` flag makes the command zone, a category named Sideboard the sideboard (even when it is excluded
/// from the deck), any other `{noDeck}` category the maybeboard; anything else says nothing about the board.
pub fn section_from_category(category: &str) -> Option<Section> {
    let first = category.split(',').next().unwrap_or_default();
    let has_flag = |flag: &str| first.contains(&format!("{{{flag}}}"));
    if has_flag("top") {
        return Some(Section::Commander);
    }
    let name = first.split('{').next().unwrap_or(first).trim();
    match section_from_word(name) {
        Some(Section::Main) | None => has_flag("noDeck").then_some(Section::Maybeboard),
        other => other,
    }
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
    #[case::moxfield("STICKERS:", Some(Header::Ignored("STICKERS".to_owned())))]
    #[case::any_case("Stickers", Some(Header::Ignored("Stickers".to_owned())))]
    #[case::with_count("Stickers (10)", Some(Header::Ignored("Stickers".to_owned())))]
    #[case::a_deck_section("SIDEBOARD:", Some(Header::Section(Section::Sideboard)))]
    #[case::a_card_named_so("Stickers of Doom", None)]
    #[case::unknown_shouting_header("LANDS:", None)]
    fn headers_name_a_deck_section_or_one_to_leave_out(
        #[case] line: &str,
        #[case] expected: Option<Header>,
    ) {
        assert_eq!(header(line), expected);
        assert_eq!(
            header_section(line).is_some(),
            matches!(expected, Some(Header::Section(_)))
        );
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
    #[case::top_flag("Commander{top}", Some(Section::Commander))]
    #[case::top_flag_on_any_name("Partners{top},Ramp", Some(Section::Commander))]
    #[case::later_categories_never_change_it("Ramp,Commander", None)]
    #[case::later_top_flag("Ramp,Commander{top}", None)]
    #[case::later_exclusion("Ramp,Maybeboard{noDeck}{noPrice}", None)]
    #[case::sideboard("Sideboard", Some(Section::Sideboard))]
    #[case::sideboard_then_top_flag("Sideboard,Commander{top}", Some(Section::Sideboard))]
    #[case::sideboard_even_when_excluded("Sideboard{noDeck}{noPrice}", Some(Section::Sideboard))]
    #[case::maybeboard("Maybeboard{noDeck}{noPrice}", Some(Section::Maybeboard))]
    #[case::maybeboard_with_a_second_category(
        "Maybeboard{noDeck}{noPrice},Creature",
        Some(Section::Maybeboard)
    )]
    #[case::excluded_custom_category(
        "Last Minute Cuts{noDeck}{noPrice}",
        Some(Section::Maybeboard)
    )]
    #[case::name_holding_sideboard_is_just_excluded(
        "1v1 Sideboard{noDeck}{noPrice}",
        Some(Section::Maybeboard)
    )]
    #[case::ordinary_category("Ramp", None)]
    #[case::price_flag_alone("Ramp{noPrice}", None)]
    #[case::deck_word("Deck", None)]
    #[case::empty("", None)]
    fn maps_archidekt_categories(#[case] category: &str, #[case] expected: Option<Section>) {
        assert_eq!(section_from_category(category), expected);
    }

    #[rstest]
    #[case("SIDEBOARD:", true)]
    #[case("COMMANDER:", true)]
    #[case("STICKERS:", true)]
    #[case("STICKERS", false)]
    #[case("Sideboard:", false)]
    #[case("SIDEBOARD", false)]
    #[case("RAMP:", false)]
    #[case(":", false)]
    fn detects_shouting_headers(#[case] line: &str, #[case] expected: bool) {
        assert_eq!(is_shouting_header(line), expected);
    }
}
