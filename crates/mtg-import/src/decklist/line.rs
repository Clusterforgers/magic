//! Grammar of a single card line shared by the text-based deck formats.

use thiserror::Error;

use super::affix::{hash_tags_start, split_printing, split_trailing_group, strip_foil_marker};
use super::types::{MAX_COUNT, MAX_NAME_CHARS};

/// Dialect switches for entry-line parsing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LineOptions {
    /// Accept `[SET]` as a set code (plain text only; clashes with Archidekt categories).
    pub bracket_sets: bool,
    /// Read Archidekt `[Category]` and `^tag^` suffixes.
    pub categories: bool,
    /// Drop Moxfield `#tag` suffixes: the first `#` after whitespace opens tags, which may hold spaces, parentheses and `!`.
    pub hash_tags: bool,
}

/// A card line split into its parts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawEntry {
    /// Number of copies.
    pub count: u32,
    /// Card name.
    pub name: String,
    /// Uppercase set code.
    pub set: Option<String>,
    /// Collector number, only when a set is present.
    pub collector_number: Option<String>,
    /// Archidekt category text, without brackets.
    pub category: Option<String>,
    /// Whether the line carried a foil/etched marker (not tracked by [`super::DeckEntry`]).
    pub foil: bool,
    /// Whether the line carried Archidekt `^tag^` markers.
    pub tagged: bool,
}

/// Why a line is not a valid card entry.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EntryError {
    /// The line does not start with a number.
    #[error("expected a card count before the name")]
    MissingCount,
    /// The count is not followed by whitespace.
    #[error("expected a space between the count and the card name")]
    NoSpaceAfterCount,
    /// The count is zero.
    #[error("card count must be at least 1")]
    ZeroCount,
    /// The count exceeds [`MAX_COUNT`].
    #[error("card count is too large (maximum {MAX_COUNT})")]
    CountTooLarge,
    /// Nothing follows the count.
    #[error("missing card name")]
    MissingName,
    /// The name exceeds [`MAX_NAME_CHARS`].
    #[error("card name is longer than {MAX_NAME_CHARS} characters")]
    NameTooLong,
    /// The line contains a control character.
    #[error("line contains a control character")]
    ControlCharacter,
}

/// Validates a copy count written as ASCII digits.
pub fn check_count(digits: &str) -> Result<u32, EntryError> {
    match digits.parse::<u32>() {
        Ok(0) => Err(EntryError::ZeroCount),
        Ok(n) if n <= MAX_COUNT => Ok(n),
        _ => Err(EntryError::CountTooLarge),
    }
}

/// Trims a card name and checks it is non-empty, short enough and free of control characters.
pub fn check_name(name: &str) -> Result<&str, EntryError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(EntryError::MissingName);
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(EntryError::NameTooLong);
    }
    if name.chars().any(char::is_control) {
        return Err(EntryError::ControlCharacter);
    }
    Ok(name)
}

/// Parses `COUNT[x] NAME [(SET) [NUMBER]] [markers]`.
pub fn parse_entry(text: &str, options: LineOptions) -> Result<RawEntry, EntryError> {
    let text = text.trim();
    if text.chars().any(|c| c.is_control() && c != '\t') {
        return Err(EntryError::ControlCharacter);
    }

    let digits_end = text
        .bytes()
        .position(|b| !b.is_ascii_digit())
        .unwrap_or(text.len());
    if digits_end == 0 {
        return Err(EntryError::MissingCount);
    }
    let count = check_count(&text[..digits_end])?;

    let mut rest = &text[digits_end..];
    if let Some(after_x) = rest.strip_prefix(['x', 'X'])
        && (after_x.is_empty() || after_x.starts_with(char::is_whitespace))
    {
        rest = after_x;
    }
    if rest.is_empty() {
        return Err(EntryError::MissingName);
    }
    if !rest.starts_with(char::is_whitespace) {
        return Err(EntryError::NoSpaceAfterCount);
    }

    let mut body = rest.trim();
    if options.hash_tags
        && let Some(at) = hash_tags_start(body)
    {
        body = body[..at].trim_end();
    }
    let (mut foil, mut tagged, mut category) = (false, false, None);
    loop {
        if let Some(head) = strip_foil_marker(body) {
            foil = true;
            body = head;
        } else if options.categories
            && let Some((head, inner)) = split_trailing_group(body, '[', ']')
        {
            category.get_or_insert_with(|| inner.to_owned());
            body = head;
        } else if options.categories
            && let Some((head, _)) = split_trailing_group(body, '^', '^')
        {
            tagged = true;
            body = head;
        } else {
            break;
        }
    }

    let (name, set, collector_number) = split_printing(body, options.bracket_sets);
    let name = check_name(name)?;
    Ok(RawEntry {
        count,
        name: name.to_owned(),
        set,
        collector_number,
        category,
        foil,
        tagged,
    })
}

#[cfg(test)]
mod tests {
    //! Goal: the entry-line grammar extracts count, name and printing correctly across every notation we accept, and rejects the rest with a specific reason.
    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;

    const PLAIN: LineOptions = LineOptions {
        bracket_sets: true,
        categories: false,
        hash_tags: false,
    };
    const ARCHIDEKT: LineOptions = LineOptions {
        bracket_sets: false,
        categories: true,
        hash_tags: false,
    };
    const MOXFIELD: LineOptions = LineOptions {
        bracket_sets: false,
        categories: false,
        hash_tags: true,
    };

    type Parsed<'a> = (u32, &'a str, Option<&'a str>, Option<&'a str>);

    fn summary(raw: &RawEntry) -> (u32, &str, Option<&str>, Option<&str>) {
        (
            raw.count,
            raw.name.as_str(),
            raw.set.as_deref(),
            raw.collector_number.as_deref(),
        )
    }

    #[rstest]
    #[case::bare("4 Lightning Bolt", (4, "Lightning Bolt", None, None))]
    #[case::x_suffix("4x Lightning Bolt", (4, "Lightning Bolt", None, None))]
    #[case::capital_x("4X Lightning Bolt", (4, "Lightning Bolt", None, None))]
    #[case::tab_separator("4\tLightning Bolt", (4, "Lightning Bolt", None, None))]
    #[case::extra_spaces("  4   Lightning Bolt  ", (4, "Lightning Bolt", None, None))]
    #[case::leading_zeros("004 Lightning Bolt", (4, "Lightning Bolt", None, None))]
    #[case::max_count("999 Relentless Rats", (999, "Relentless Rats", None, None))]
    #[case::set_and_number("4 Lightning Bolt (M10) 146", (4, "Lightning Bolt", Some("M10"), Some("146")))]
    #[case::lowercase_set("1x Sol Ring (cmm) 400", (1, "Sol Ring", Some("CMM"), Some("400")))]
    #[case::set_only("4 Lightning Bolt (M10)", (4, "Lightning Bolt", Some("M10"), None))]
    #[case::bracket_set("4 Lightning Bolt [M10]", (4, "Lightning Bolt", Some("M10"), None))]
    #[case::letter_number("1 Wear // Tear (DGM) 135a", (1, "Wear // Tear", Some("DGM"), Some("135a")))]
    #[case::star_number("1 Sol Ring (PLST) 12★", (1, "Sol Ring", Some("PLST"), Some("12★")))]
    #[case::big_star_number("1 Reconnaissance (SLD) 1575★", (1, "Reconnaissance", Some("SLD"), Some("1575★")))]
    #[case::prefixed_number("1 Ancient Tomb (PLST) TMP-315", (1, "Ancient Tomb", Some("PLST"), Some("TMP-315")))]
    #[case::year_dash_number("1 Culling the Weak (PMEI) 2026-2", (1, "Culling the Weak", Some("PMEI"), Some("2026-2")))]
    #[case::promo_letter_number("1 Temur Ascendancy (PKTK) 271p", (1, "Temur Ascendancy", Some("PKTK"), Some("271p")))]
    #[case::year_number("6 Abrupt Decay (WMC) 2016", (6, "Abrupt Decay", Some("WMC"), Some("2016")))]
    #[case::letters_before_digits("1 Island (TVDL) tvdl345", (1, "Island", Some("TVDL"), Some("tvdl345")))]
    #[case::underscore_name("1 _____ Goblin (UNF) 107", (1, "_____ Goblin", Some("UNF"), Some("107")))]
    #[case::slash_between_faces("1 Agadeem's Awakening / Agadeem, the Undercrypt (ZNR) 336 *F*", (1, "Agadeem's Awakening / Agadeem, the Undercrypt", Some("ZNR"), Some("336")))]
    #[case::split_name("1 Fire // Ice", (1, "Fire // Ice", None, None))]
    #[case::split_name_with_set("1 Fire // Ice (MH2) 290", (1, "Fire // Ice", Some("MH2"), Some("290")))]
    #[case::apostrophe_comma("1 Atraxa, Praetors' Voice (C16) 28", (1, "Atraxa, Praetors' Voice", Some("C16"), Some("28")))]
    #[case::digit_led_name("1 1996 World Champion", (1, "1996 World Champion", None, None))]
    #[case::paren_with_spaces_stays_in_name("1 B.F.M. (Big Furry Monster)", (1, "B.F.M. (Big Furry Monster)", None, None))]
    #[case::unicode_name("1 Lim-Dûl's Vault (ALL) 108", (1, "Lim-Dûl's Vault", Some("ALL"), Some("108")))]
    #[case::ligature("1 Æther Vial", (1, "Æther Vial", None, None))]
    #[case::too_long_set_is_name("1 Foo (TOOLONGSET)", (1, "Foo (TOOLONGSET)", None, None))]
    #[case::one_char_set_is_name("1 Foo (X)", (1, "Foo (X)", None, None))]
    #[case::foil_marker("1 Sol Ring (CMM) 400 *F*", (1, "Sol Ring", Some("CMM"), Some("400")))]
    #[case::etched_marker("1 Sol Ring *E*", (1, "Sol Ring", None, None))]
    #[case::number_without_digit_not_printing("1 Foo (M10) Bar", (1, "Foo (M10) Bar", None, None))]
    fn parses_plain_entries(#[case] line: &str, #[case] expected: Parsed<'_>) {
        let raw = parse_entry(line, PLAIN).unwrap();
        assert_eq!(summary(&raw), expected);
    }

    #[rstest]
    #[case::tag("1 Ancient Tomb (V12) 1 #Lands", (1, "Ancient Tomb", Some("V12"), Some("1")))]
    #[case::bang_tag("1 Ancient Tomb (V12) 1 *F* #!Lands", (1, "Ancient Tomb", Some("V12"), Some("1")))]
    #[case::tag_with_spaces_and_parentheses("1 Sol Ring (BRC) 160 #Mana (LOCKED)", (1, "Sol Ring", Some("BRC"), Some("160")))]
    #[case::several_tags("3 Beseech the Mirror (WOE) 82 #Card Advantage #Payoff #!Wheel", (3, "Beseech the Mirror", Some("WOE"), Some("82")))]
    #[case::tag_without_printing("1 Ancient Tomb #!Lands", (1, "Ancient Tomb", None, None))]
    #[case::custom_card("1 My Homebrew #CustomCard", (1, "My Homebrew", None, None))]
    #[case::tag_after_etched_flag("1 Talisman of Indulgence (SLD) 1054 *E* #Mana", (1, "Talisman of Indulgence", Some("SLD"), Some("1054")))]
    #[case::hash_inside_a_word_stays("1 Mountain#2 (ABC) 3", (1, "Mountain#2", Some("ABC"), Some("3")))]
    fn moxfield_hash_tags_are_dropped(#[case] line: &str, #[case] expected: Parsed<'_>) {
        let raw = parse_entry(line, MOXFIELD).unwrap();
        assert_eq!(summary(&raw), expected);
    }

    #[test]
    fn hash_tags_are_part_of_the_name_unless_the_dialect_reads_them() {
        let raw = parse_entry("1 Mountain #2", PLAIN).unwrap();
        assert_eq!(raw.name, "Mountain #2");
        assert_eq!(
            parse_entry("1 #Lands", MOXFIELD).unwrap_err(),
            EntryError::MissingName
        );
    }

    #[test]
    fn foil_markers_are_flagged() {
        assert!(parse_entry("1 Sol Ring *F*", PLAIN).unwrap().foil);
        assert!(!parse_entry("1 Sol Ring", PLAIN).unwrap().foil);
    }

    #[rstest]
    #[case::category(
        "1x Sol Ring (cmm) 400 [Ramp]",
        "Sol Ring",
        Some("CMM"),
        Some("400"),
        Some("Ramp"),
        false
    )]
    #[case::commander_flag(
        "1x Atraxa (c16) 28 [Commander{top}]",
        "Atraxa",
        Some("C16"),
        Some("28"),
        Some("Commander{top}"),
        false
    )]
    #[case::foil_then_category(
        "1x Counterspell (cmm) 81 *F* [Control]",
        "Counterspell",
        Some("CMM"),
        Some("81"),
        Some("Control"),
        false
    )]
    #[case::category_then_foil(
        "1x Counterspell (cmm) 81 [Control] *F*",
        "Counterspell",
        Some("CMM"),
        Some("81"),
        Some("Control"),
        false
    )]
    #[case::tags(
        "1x Sol Ring (cmm) 400 [Ramp] ^Have,#37db9e^",
        "Sol Ring",
        Some("CMM"),
        Some("400"),
        Some("Ramp"),
        true
    )]
    #[case::no_markers(
        "1x Sol Ring (cmm) 400",
        "Sol Ring",
        Some("CMM"),
        Some("400"),
        None,
        false
    )]
    fn parses_archidekt_suffixes(
        #[case] line: &str,
        #[case] name: &str,
        #[case] set: Option<&str>,
        #[case] number: Option<&str>,
        #[case] category: Option<&str>,
        #[case] tagged: bool,
    ) {
        let raw = parse_entry(line, ARCHIDEKT).unwrap();
        assert_eq!(raw.name, name);
        assert_eq!(raw.set.as_deref(), set);
        assert_eq!(raw.collector_number.as_deref(), number);
        assert_eq!(raw.category.as_deref(), category);
        assert_eq!(raw.tagged, tagged);
    }

    #[rstest]
    #[case::no_count("Lightning Bolt", EntryError::MissingCount)]
    #[case::empty("", EntryError::MissingCount)]
    #[case::only_spaces("    ", EntryError::MissingCount)]
    #[case::only_count("4", EntryError::MissingName)]
    #[case::count_and_x("4x", EntryError::MissingName)]
    #[case::count_and_spaces("4   ", EntryError::MissingName)]
    #[case::zero("0 Lightning Bolt", EntryError::ZeroCount)]
    #[case::zero_x("0x Lightning Bolt", EntryError::ZeroCount)]
    #[case::too_large("1000 Lightning Bolt", EntryError::CountTooLarge)]
    #[case::year_like("1996 World Champion", EntryError::CountTooLarge)]
    #[case::overflow("99999999999999999999999 Lightning Bolt", EntryError::CountTooLarge)]
    #[case::glued("4Lightning Bolt", EntryError::NoSpaceAfterCount)]
    #[case::negative("-4 Lightning Bolt", EntryError::MissingCount)]
    #[case::nul("4 Light\0ning Bolt", EntryError::ControlCharacter)]
    #[case::escape("4 Lightning\u{1b}[0m Bolt", EntryError::ControlCharacter)]
    #[case::only_printing("4 (M10) 146", EntryError::MissingName)]
    fn rejects_bad_entries(#[case] line: &str, #[case] expected: EntryError) {
        assert_eq!(parse_entry(line, PLAIN).unwrap_err(), expected);
    }

    #[test]
    fn overlong_names_are_rejected() {
        let line = format!("1 {}", "a".repeat(MAX_NAME_CHARS + 1));
        assert_eq!(
            parse_entry(&line, PLAIN).unwrap_err(),
            EntryError::NameTooLong
        );
        let ok = format!("1 {}", "a".repeat(MAX_NAME_CHARS));
        assert!(parse_entry(&ok, PLAIN).is_ok());
    }

    proptest! {
        #[test]
        fn arbitrary_text_never_panics(line in "\\PC{0,80}") {
            let _ = parse_entry(&line, PLAIN);
            let _ = parse_entry(&line, ARCHIDEKT);
        }

        #[test]
        fn parsed_counts_stay_in_range(count in 0u64..5000, name in "[A-Za-z][A-Za-z ,']{0,20}[A-Za-z]") {
            let result = parse_entry(&format!("{count} {name}"), PLAIN);
            match result {
                Ok(raw) => prop_assert!((1..=u64::from(MAX_COUNT)).contains(&count) && u64::from(raw.count) == count),
                Err(e) => prop_assert!(count == 0 && e == EntryError::ZeroCount || count > u64::from(MAX_COUNT) && e == EntryError::CountTooLarge),
            }
        }
    }
}
