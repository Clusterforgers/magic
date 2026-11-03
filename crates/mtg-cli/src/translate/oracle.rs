//! Whether Forge's `Oracle:` line says what the printed Oracle text says, after the spelling differences of the two
//! sources are removed: a rebalanced or Alchemy card differs, and a translation of its script is not the printed card.

/// The text with the spelling differences removed: lowercase, straight quotes and hyphens, no reminder text, one space
/// between words and one line break between abilities.
pub fn normalise(text: &str) -> String {
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
    plain
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Whether the Forge text of a card (its faces' `Oracle:` lines) equals the printed text of the faces.
pub fn matches<'a>(
    forge: impl IntoIterator<Item = &'a str>,
    printed: impl IntoIterator<Item = &'a str>,
) -> bool {
    let join = |faces: Vec<&str>| normalise(&faces.join("\n"));
    join(forge.into_iter().collect()) == join(printed.into_iter().collect())
}
