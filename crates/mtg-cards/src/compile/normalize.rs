//! Stage 1: reminder text removed, the card's own name replaced by `~`, and a map from every
//! remaining character back to the original oracle text.

use std::ops::Range;

use crate::card::Span;

/// One character of the normalized text and the original characters it stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Piece {
    ch: char,
    start: usize,
    end: usize,
}

/// The oracle text of one face without reminder text and with its own name as `~`.
///
/// Offsets into it count characters; [`Normalized::span`] maps a range back to the original text.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Normalized {
    pieces: Vec<Piece>,
    /// The original offset of every line break, then the original length.
    line_ends: Vec<usize>,
}

impl Normalized {
    /// The characters of a range, or nothing when it lies outside the text.
    pub fn slice(&self, range: Range<usize>) -> String {
        self.pieces
            .get(range)
            .into_iter()
            .flatten()
            .map(|piece| piece.ch)
            .collect()
    }

    /// The non-empty lines, as character ranges.
    pub fn lines(&self) -> Vec<Range<usize>> {
        let mut lines = Vec::new();
        let mut start = 0;
        for (at, piece) in self.pieces.iter().enumerate() {
            if piece.ch == '\n' {
                if at > start {
                    lines.push(start..at);
                }
                start = at + 1;
            }
        }
        if self.pieces.len() > start {
            lines.push(start..self.pieces.len());
        }
        lines
    }

    /// Where a range of the normalized text lies in the original, in Unicode scalar values, end
    /// exclusive. With `to_line_end` the end moves to the end of its original line, so reminder
    /// text after the clause belongs to it. `None` for an empty range or one outside the text.
    pub fn span(&self, range: Range<usize>, to_line_end: bool) -> Option<Span> {
        let first = self.pieces.get(range.start)?;
        let last = self.pieces.get(range.end.checked_sub(1)?)?;
        if last.end <= first.start {
            return None;
        }
        let mut end = last.end;
        if to_line_end {
            end = self
                .line_ends
                .iter()
                .copied()
                .find(|line_end| *line_end >= end)
                .unwrap_or(end);
        }
        Some(Span {
            start: u32::try_from(first.start).unwrap_or(u32::MAX),
            end: u32::try_from(end).unwrap_or(u32::MAX),
        })
    }
}

/// Removes reminder text (balanced parentheses) and replaces each of `names` by `~`.
///
/// An unclosed parenthesis and a stray closing one stay text, as `has_rules_text` counts them.
pub fn normalize(oracle_text: &str, names: &[String]) -> Normalized {
    let original: Vec<char> = oracle_text.chars().collect();
    let removed = reminder_ranges(&original);
    let mut removed = removed.iter().peekable();
    let mut pieces: Vec<Piece> = Vec::with_capacity(original.len());
    let mut at = 0;
    while let Some(&ch) = original.get(at) {
        if let Some(range) = removed.next_if(|range| range.start == at) {
            while pieces
                .last()
                .is_some_and(|piece| matches!(piece.ch, ' ' | '\t'))
            {
                pieces.pop();
            }
            at = range.end;
        } else {
            pieces.push(Piece {
                ch,
                start: at,
                end: at + 1,
            });
            at += 1;
        }
    }
    let mut line_ends: Vec<usize> = original
        .iter()
        .enumerate()
        .filter(|(_, ch)| **ch == '\n')
        .map(|(at, _)| at)
        .collect();
    line_ends.push(original.len());
    Normalized {
        pieces: replace_names(&trim_lines(pieces), names),
        line_ends,
    }
}

/// The outermost balanced parenthesised groups, parentheses included.
fn reminder_ranges(chars: &[char]) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let (mut depth, mut start) = (0usize, 0usize);
    for (at, ch) in chars.iter().enumerate() {
        match ch {
            '(' => {
                if depth == 0 {
                    start = at;
                }
                depth += 1;
            }
            ')' if depth > 0 => {
                depth -= 1;
                if depth == 0 {
                    ranges.push(start..at + 1);
                }
            }
            _ => {}
        }
    }
    ranges
}

/// Whether reminder text in `text` speaks of a counter: it then explains a counter the clause puts
/// (a shield or hone counter), which the reading would drop with the reminder.
pub fn reminder_mentions_counter(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    reminder_ranges(&chars).iter().any(|range| {
        let reminder: String = chars[range.clone()].iter().collect();
        let reminder = reminder.to_lowercase();
        reminder.contains(" counter") || reminder.contains("counters")
    })
}

/// Drops the whitespace at both ends of every line.
fn trim_lines(pieces: Vec<Piece>) -> Vec<Piece> {
    fn trim_end(out: &mut Vec<Piece>, line_start: usize) {
        while out.len() > line_start && out.last().is_some_and(|piece| piece.ch.is_whitespace()) {
            out.pop();
        }
    }
    let mut out: Vec<Piece> = Vec::with_capacity(pieces.len());
    let mut line_start = 0;
    for piece in pieces {
        if piece.ch == '\n' {
            trim_end(&mut out, line_start);
            out.push(piece);
            line_start = out.len();
        } else if !(piece.ch.is_whitespace() && out.len() == line_start) {
            out.push(piece);
        }
    }
    trim_end(&mut out, line_start);
    out
}

/// Replaces whole-word occurrences of the names, the longest first, by one `~` each.
fn replace_names(pieces: &[Piece], names: &[String]) -> Vec<Piece> {
    let mut names: Vec<Vec<char>> = names
        .iter()
        .filter(|name| !name.is_empty())
        .map(|name| name.chars().collect())
        .collect();
    names.sort_by_key(|name| std::cmp::Reverse(name.len()));
    let mut out = Vec::with_capacity(pieces.len());
    let mut at = 0;
    while let Some(piece) = pieces.get(at) {
        let hit = names.iter().find(|name| name_at(pieces, at, name));
        if let Some(name) = hit {
            let last = pieces.get(at + name.len() - 1).unwrap_or(piece);
            out.push(Piece {
                ch: '~',
                start: piece.start,
                end: last.end,
            });
            at += name.len();
        } else {
            out.push(*piece);
            at += 1;
        }
    }
    out
}

/// Whether `name` stands at `at` as a whole word.
fn name_at(pieces: &[Piece], at: usize, name: &[char]) -> bool {
    let Some(window) = pieces.get(at..at + name.len()) else {
        return false;
    };
    let word_char = |piece: Option<&Piece>| piece.is_some_and(|piece| piece.ch.is_alphanumeric());
    window.iter().zip(name).all(|(piece, ch)| piece.ch == *ch)
        && !word_char(at.checked_sub(1).and_then(|before| pieces.get(before)))
        && !word_char(pieces.get(at + name.len()))
}

#[cfg(test)]
mod tests {
    //! Goal: reminder text and the card's own name leave the text however they are written, and every span maps back to the original characters.
    use super::*;
    use rstest::rstest;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(ToString::to_string).collect()
    }

    fn text_of(normalized: &Normalized) -> String {
        normalized.slice(0..normalized.pieces.len())
    }

    #[rstest]
    #[case::keyword("Flying (This creature can fly.)", "Flying")]
    #[case::two_lines(
        "Haste (This creature can attack and {T} as soon as it comes under your control.)\nCascade (When you cast this spell, exile cards.)",
        "Haste\nCascade"
    )]
    #[case::convoke_and_a_sentence(
        "Convoke (Your creatures can help cast this spell. Each creature you tap while casting this spell pays for {1} or one mana of that creature's color.)\nStoke the Flames deals 4 damage to any target.",
        "Convoke\nStoke the Flames deals 4 damage to any target."
    )]
    #[case::delve(
        "Delve (Each card you exile from your graveyard while casting this spell pays for {1}.)\nDraw three cards.",
        "Delve\nDraw three cards."
    )]
    #[case::only_reminder("({T}: Add {G}.)", "")]
    #[case::before_a_sentence(
        "Scry 1. (Look at the top card.)\nDraw a card.",
        "Scry 1.\nDraw a card."
    )]
    #[case::quotes_inside(
        "Create a Treasure. (It's an artifact with \"{T}, Sacrifice this token: Add one mana.\")",
        "Create a Treasure."
    )]
    #[case::nested("Foo (a (b) c) bar", "Foo bar")]
    #[case::in_the_middle("Choose one (reminder) \u{2014} draw", "Choose one \u{2014} draw")]
    #[case::before_punctuation("Draw a card (reminder).", "Draw a card.")]
    #[case::no_space_before("Draw(reminder) a card", "Draw a card")]
    #[case::two_groups("A (x) B (y)", "A B")]
    #[case::unclosed_stays_text("Foo (bar", "Foo (bar")]
    #[case::stray_close_stays_text("Foo) bar", "Foo) bar")]
    #[case::unclosed_outer_keeps_inner("(a (b) c", "(a (b) c")]
    #[case::whitespace_trimmed("  Flying  \n\n  Haste (x)  ", "Flying\n\nHaste")]
    fn reminder_text_is_removed(#[case] original: &str, #[case] expected: &str) {
        assert_eq!(text_of(&normalize(original, &[])), expected);
    }

    #[rstest]
    #[case::plain("Lightning Bolt deals 3 damage.", &["Lightning Bolt"], "~ deals 3 damage.")]
    #[case::possessive("Questing Beast's power", &["Questing Beast"], "~'s power")]
    #[case::twice("Foo deals 1 damage to Foo.", &["Foo"], "~ deals 1 damage to ~.")]
    #[case::inside_a_word("Wildfire and Firebolt", &["Fire"], "Wildfire and Firebolt")]
    #[case::face_of_a_split_card("Fire deals 2 damage.", &["Fire"], "~ deals 2 damage.")]
    #[case::legendary_short_name(
        "Atraxa has flying. Atraxa, Praetors' Voice gets +1/+1.",
        &["Atraxa", "Atraxa, Praetors' Voice"],
        "~ has flying. ~ gets +1/+1."
    )]
    #[case::absent("Draw a card.", &["Foo"], "Draw a card.")]
    #[case::empty_name_is_ignored("Draw a card.", &[""], "Draw a card.")]
    #[case::name_in_reminder_text("Foo (Foo can fly.) Foo draws.", &["Foo"], "~ ~ draws.")]
    #[case::case_matters("foo deals", &["Foo"], "foo deals")]
    fn the_card_name_becomes_a_tilde(
        #[case] original: &str,
        #[case] own: &[&str],
        #[case] expected: &str,
    ) {
        assert_eq!(text_of(&normalize(original, &names(own))), expected);
    }

    #[test]
    fn spans_slice_the_original_text_in_scalar_values() {
        let original =
            "Flying (This can fly.)\n\u{2212}1: Questing Beast deals 1 damage \u{2022} now.";
        let normalized = normalize(original, &names(&["Questing Beast"]));
        let lines = normalized.lines();
        assert_eq!(lines.len(), 2);
        let first = normalized.span(lines[0].clone(), false).unwrap();
        assert_eq!(first.slice(original), Some("Flying"));
        let with_reminder = normalized.span(lines[0].clone(), true).unwrap();
        assert_eq!(
            with_reminder.slice(original),
            Some("Flying (This can fly.)")
        );
        let second = normalized.span(lines[1].clone(), true).unwrap();
        assert_eq!(
            second.slice(original),
            Some("\u{2212}1: Questing Beast deals 1 damage \u{2022} now.")
        );
        // The replaced name is one normalized character and covers the whole name.
        let tilde = lines[1].start + 4;
        let name = normalized.span(tilde..tilde + 1, false).unwrap();
        assert_eq!(name.slice(original), Some("Questing Beast"));
        assert_eq!(
            normalized.slice(lines[1].clone()),
            "\u{2212}1: ~ deals 1 damage \u{2022} now."
        );
    }

    #[rstest]
    #[case::empty(0..0)]
    #[case::backwards(Range { start: 3, end: 2 })]
    #[case::past_the_end(4..99)]
    fn empty_and_outside_ranges_have_no_span(#[case] range: Range<usize>) {
        let normalized = normalize("Flying", &[]);
        assert_eq!(normalized.span(range, false), None);
    }

    mod generated {
        //! Goal: over generated texts, nothing is left exactly when the text has no rules text, and every line maps to an in-bounds, ordered stretch of the original.
        use super::*;
        use crate::support::has_rules_text;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn nothing_is_left_exactly_when_there_is_no_rules_text(
                original in "[a-c(). \n\u{2014}~\"]{0,24}"
            ) {
                let normalized = normalize(&original, &[]);
                prop_assert_eq!(
                    !normalized.slice(0..normalized.pieces.len()).trim().is_empty(),
                    has_rules_text(&original),
                    "{:?}", original
                );
            }

            #[test]
            fn spans_are_in_bounds_and_ordered(
                original in "[a-c(). \n\u{2014}F]{0,24}",
                to_line_end in any::<bool>()
            ) {
                let normalized = normalize(&original, &names(&["Fa"]));
                let length = u32::try_from(original.chars().count()).unwrap();
                let mut previous_end = 0;
                for line in normalized.lines() {
                    let span = normalized.span(line, to_line_end).unwrap();
                    prop_assert!(span.start < span.end && span.end <= length, "{:?}", original);
                    prop_assert!(span.start >= previous_end, "{:?}", original);
                    previous_end = span.end;
                }
            }
        }
    }
}
