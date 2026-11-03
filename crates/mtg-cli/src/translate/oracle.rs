//! Whether Forge's `Oracle:` line says what the printed Oracle text says, after the spelling differences of the two
//! sources are removed: a rebalanced or Alchemy card differs, and a translation of its script is not the printed card.

/// The words that stand for the card itself after "this".
const SELF_WORDS: [&str; 14] = [
    "creature",
    "permanent",
    "artifact",
    "land",
    "enchantment",
    "planeswalker",
    "spell",
    "card",
    "equipment",
    "aura",
    "vehicle",
    "saga",
    "battle",
    "class",
];

/// The text with the spelling differences removed: lowercase, straight quotes and hyphens, no reminder text, one space
/// between words and one line break between abilities.
pub fn normalise(text: &str, name: &str) -> String {
    let mut plain = String::new();
    let mut depth = 0u32;
    for c in text.replace("\\n", "\n").chars() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if depth > 0 => {}
            '\u{2019}' | '\u{2018}' => plain.push('\''),
            '\u{201c}' | '\u{201d}' => plain.push('"'),
            '\u{2013}' | '\u{2014}' | '\u{2212}' => plain.push('-'),
            _ => plain.extend(c.to_lowercase()),
        }
    }
    // The card's name, its faces' names and the name before a comma stand for the card, as "this creature" does.
    let mut names: Vec<String> = name.split(" // ").map(str::to_lowercase).collect();
    names.push(name.to_lowercase());
    let shorts: Vec<String> = names
        .iter()
        .filter_map(|n| n.split_once(',').map(|(first, _)| first.to_owned()))
        .collect();
    names.extend(shorts);
    names.sort_by_key(|n| std::cmp::Reverse(n.len()));
    for n in names.iter().filter(|n| !n.is_empty()) {
        plain = plain.replace(n.as_str(), "~");
    }
    plain = plain.replace(", or ", " or ").replace(", and ", " and ");
    for kind in SELF_WORDS {
        plain = plain.replace(&format!("this {kind}"), "~");
    }
    plain
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Whether the Forge text of a card (its faces' `Oracle:` lines) equals the printed text of the faces.
pub fn matches<'a>(
    name: &str,
    forge: impl IntoIterator<Item = &'a str>,
    printed: impl IntoIterator<Item = &'a str>,
) -> bool {
    let join = |faces: Vec<&str>| normalise(&faces.join("\n"), name);
    join(forge.into_iter().collect()) == join(printed.into_iter().collect())
}

#[cfg(test)]
mod tests {
    //! Goal: the comparison ignores the spelling differences of Forge and Scryfall and sees a real change of the text.

    use rstest::rstest;

    use super::matches;

    #[rstest]
    #[case::same("Flying\\nDraw a card.", "Flying\nDraw a card.", true)]
    #[case::reminder("Flying (This creature can't be blocked.)", "Flying", true)]
    #[case::quotes("Don\u{2019}t \u{2014} go", "don't - go", true)]
    #[case::spaces("Draw  a card.\n\n", "Draw a card.", true)]
    #[case::oxford("A, B, or C", "A, B or C", true)]
    #[case::rebalanced("Draw two cards.", "Draw a card.", false)]
    fn compares_text(#[case] forge: &str, #[case] printed: &str, #[case] same: bool) {
        assert_eq!(matches("Quartz Spark", [forge], [printed]), same);
    }

    #[test]
    fn faces_are_joined() {
        assert!(matches("X", ["A", "B"], ["A\nB"]));
    }

    #[rstest]
    #[case::name("When Quartz Spark enters, draw.", "When this creature enters, draw.")]
    #[case::comma("Grima attacks.", "This creature attacks.")]
    fn the_card_is_its_name_or_this_type(#[case] forge: &str, #[case] printed: &str) {
        let name = if forge.starts_with("Grima") {
            "Grima, Footman"
        } else {
            "Quartz Spark"
        };
        assert!(matches(name, [forge], [printed]));
    }
}
