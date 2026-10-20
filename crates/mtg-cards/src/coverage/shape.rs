//! The shape of a clause: its kind and its text with the numbers, the mana and the card's name abstracted away, so wordings that differ only in those count as one.

use crate::compile::ClauseKind;

/// The spelled-out numbers a shape treats as numbers.
const NUMBER_WORDS: [&str; 10] = [
    "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
];

/// How a clause kind is named in a shape.
pub(super) const fn kind_name(kind: ClauseKind) -> &'static str {
    match kind {
        ClauseKind::Keyword => "keyword",
        ClauseKind::Spell => "spell",
        ClauseKind::Triggered => "triggered",
        ClauseKind::Activated => "activated",
        ClauseKind::Loyalty => "loyalty",
        ClauseKind::Modal => "modal",
        ClauseKind::Static => "static",
    }
}

/// The shape of a clause the compiler listed: `kind: text`, where `text` is the compiler's clause text
/// (reminder text removed, the card's name as `~`) with digits and the words one to ten as `N`, a run
/// of mana symbols as `{M}`, and lines and spaces joined into single spaces.
pub(super) fn shape(kind: ClauseKind, text: &str) -> String {
    format!("{}: {}", kind_name(kind), abstracted(text))
}

fn abstracted(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    while let Some(&c) = chars.get(at) {
        if let Some(mut end) = mana_symbol_end(&chars, at) {
            while let Some(next) = mana_symbol_end(&chars, end) {
                end = next;
            }
            out.push_str("{M}");
            at = end;
        } else if c.is_ascii_digit() {
            while chars.get(at).is_some_and(char::is_ascii_digit) {
                at += 1;
            }
            out.push('N');
        } else if c.is_alphabetic() {
            let start = at;
            while chars.get(at).is_some_and(|c| c.is_alphabetic()) {
                at += 1;
            }
            let word: String = chars[start..at].iter().collect();
            if NUMBER_WORDS.contains(&word.to_lowercase().as_str()) {
                out.push('N');
            } else {
                out.push_str(&word);
            }
        } else {
            if c == '\n' {
                out.push_str(" | ");
            } else {
                out.push(c);
            }
            at += 1;
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Where the mana symbol that starts at `at` ends (one past its `}`), or `None` when none starts there.
///
/// A mana symbol is a generic amount, a color, `C`, `S` or `X`, or a hybrid or Phyrexian mix of them;
/// the tap and untap symbols, energy and the like are not mana.
fn mana_symbol_end(chars: &[char], at: usize) -> Option<usize> {
    if chars.get(at) != Some(&'{') {
        return None;
    }
    let close = chars.get(at..)?.iter().position(|&c| c == '}')?;
    let inside: String = chars[at + 1..at + close].iter().collect();
    let part_is_mana = |part: &str| {
        matches!(part, "W" | "U" | "B" | "R" | "G" | "C" | "S" | "X" | "P")
            || (!part.is_empty() && part.len() <= 2 && part.chars().all(|c| c.is_ascii_digit()))
    };
    (!inside.is_empty() && inside.split('/').all(part_is_mana)).then_some(at + close + 1)
}

#[cfg(test)]
mod tests {
    //! Goal: wordings that differ only in their numbers, mana or spelled-out amounts share a shape, and symbols that are not mana, the card's name and the words around them survive.
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::digits("Draw 3 cards.", "Draw N cards.")]
    #[case::words("Draw two cards.", "Draw N cards.")]
    #[case::capitalized_word("One or more creatures", "N or more creatures")]
    #[case::inside_a_word("Someone attacks. Money", "Someone attacks. Money")]
    #[case::pump(
        "Target creature gets +2/-1 until end of turn.",
        "Target creature gets +N/-N until end of turn."
    )]
    #[case::minus_sign(
        "Creatures get \u{2212}1/\u{2212}1.",
        "Creatures get \u{2212}N/\u{2212}N."
    )]
    #[case::mana_run("Pay {2}{G}{G}.", "Pay {M}.")]
    #[case::hybrid_and_phyrexian("{G/U}{2/W}{B/P}: Scry 1.", "{M}: Scry N.")]
    #[case::x_is_mana("{X}{R}: Draw X cards.", "{M}: Draw X cards.")]
    #[case::two_runs("{1}, {T}: Add {C}{C}.", "{M}, {T}: Add {M}.")]
    #[case::tap_is_not_mana("{T}, Sacrifice ~: Add {B}.", "{T}, Sacrifice ~: Add {M}.")]
    #[case::other_symbols("You get {E}{E}. {Q}", "You get {E}{E}. {Q}")]
    #[case::lines(
        "Choose one \u{2014}\n\u{2022} Draw a card.\n\u{2022} Scry 2.",
        "Choose N \u{2014} | \u{2022} Draw a card. | \u{2022} Scry N."
    )]
    #[case::spacing("  a   b \t c ", "a b c")]
    #[case::open_brace("{ 3", "{ N")]
    #[case::unclosed("{2", "{N")]
    fn the_text_is_abstracted(#[case] text: &str, #[case] expected: &str) {
        assert_eq!(abstracted(text), expected);
    }

    #[rstest]
    #[case(ClauseKind::Keyword, "keyword")]
    #[case(ClauseKind::Spell, "spell")]
    #[case(ClauseKind::Triggered, "triggered")]
    #[case(ClauseKind::Activated, "activated")]
    #[case(ClauseKind::Loyalty, "loyalty")]
    #[case(ClauseKind::Modal, "modal")]
    #[case(ClauseKind::Static, "static")]
    fn a_shape_starts_with_its_kind(#[case] kind: ClauseKind, #[case] word: &str) {
        assert_eq!(
            shape(kind, "~ gets +1/+1."),
            format!("{word}: ~ gets +N/+N.")
        );
    }
}
