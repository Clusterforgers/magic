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
        .map(spell_line)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// One line with the spelling differences of its punctuation removed: Forge's `[+1]:` for Scryfall's `+1:`, no space
/// after a colon or before a comma, `, and/or` for `and/or`, and a line of keywords in any order.
fn spell_line(line: &str) -> String {
    let mut line = line.trim().to_owned();
    if let Some(rest) = line.strip_prefix('[')
        && let Some((cost, rest)) = rest.split_once("]:")
        && !cost.is_empty()
        && cost
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-')
    {
        line = format!("{cost}:{rest}");
    }
    let mut spaced = String::new();
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        spaced.push(c);
        if c == ':' && chars.peek().is_some_and(|next| !next.is_whitespace()) {
            spaced.push(' ');
        }
    }
    let words = spaced
        .replace(", and/or ", " and/or ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace(" ,", ",");
    sorted_keywords(&words)
}

/// A keyword with its parameters: at most three words and none that makes a sentence.
fn is_keyword(part: &str) -> bool {
    const SENTENCE: [&str; 9] = [
        "a", "an", "the", "target", "you", "your", "each", "card", "cards",
    ];
    part.split(' ').count() <= 3 && !part.split(' ').any(|w| SENTENCE.contains(&w))
}

/// A line of comma-separated keywords (no sentence, cost or quotation) in alphabetical order.
fn sorted_keywords(line: &str) -> String {
    let plain = !line.contains(['.', ':', '"', '\u{2022}']);
    let parts: Vec<&str> = line.split(", ").collect();
    if !plain || parts.len() < 2 || parts.iter().any(|p| !is_keyword(p)) {
        return line.to_owned();
    }
    let mut parts = parts;
    parts.sort_unstable();
    parts.join(", ")
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

    #[rstest]
    #[case::loyalty_brackets(
        "[+1]: Draw a card.\n[-2]: Scry 2.\n[0]: Mill 1.",
        "+1: Draw a card.\n\u{2212}2: Scry 2.\n0: Mill 1.",
        true
    )]
    #[case::loyalty_rebalanced("[+1]: Draw two cards.", "+1: Draw a card.", false)]
    #[case::loyalty_cost_changed("[-3]: Draw a card.", "\u{2212}2: Draw a card.", false)]
    #[case::colon_without_space("{T}:Draw a card.", "{T}: Draw a card.", true)]
    #[case::space_before_comma("You get {E} , then draw.", "You get {E}, then draw.", true)]
    #[case::and_or(
        "Search your hand, and/or library.",
        "Search your hand and/or library.",
        true
    )]
    #[case::keyword_order("Vigilance, reach, trample", "Reach, vigilance, trample", true)]
    #[case::keyword_rebalanced("Vigilance, reach", "Reach, vigilance, trample", false)]
    #[case::sentence_order_matters(
        "Draw a card, then discard a card",
        "Then discard a card, draw a card",
        false
    )]
    fn punctuation_and_loyalty_spelling(
        #[case] forge: &str,
        #[case] printed: &str,
        #[case] same: bool,
    ) {
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
