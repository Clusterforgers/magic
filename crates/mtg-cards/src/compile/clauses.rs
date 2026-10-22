//! Stage 2: the text of a face as clauses. A line is a clause, a keyword line is one clause per
//! keyword, and the lines of a mode list or a level block belong to the clause they follow.

use std::ops::Range;

use super::keywords;
use super::normalize::Normalized;

/// A clause before it is classified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawClause {
    /// Its characters in the normalized text.
    pub range: Range<usize>,
    /// Whether it ends its line, so reminder text after it belongs to it.
    pub ends_line: bool,
}

/// Splits a face's normalized text into clauses, in order.
pub fn split(text: &Normalized) -> Vec<RawClause> {
    let mut clauses: Vec<RawClause> = Vec::new();
    let mut in_level_block = false;
    for line in text.lines() {
        let line_text = text.slice(line.clone());
        let header = is_level_header(&line_text);
        let joins = if in_level_block {
            !header
        } else {
            is_bullet(&line_text)
        };
        if joins && let Some(last) = clauses.last_mut() {
            last.range.end = line.end;
            continue;
        }
        in_level_block |= header;
        match keywords::split_items(&line_text) {
            Some(items) => {
                let count = items.len();
                for (at, item) in items.into_iter().enumerate() {
                    clauses.push(RawClause {
                        range: line.start + item.start..line.start + item.end,
                        ends_line: at + 1 == count,
                    });
                }
            }
            None => match entering_split(&line_text) {
                Some(first) => {
                    clauses.push(RawClause {
                        range: line.start..line.start + first,
                        ends_line: false,
                    });
                    clauses.push(RawClause {
                        range: line.start + first + 1..line.end,
                        ends_line: true,
                    });
                }
                None => clauses.push(RawClause {
                    range: line,
                    ends_line: true,
                }),
            },
        }
    }
    clauses
}

/// Where the first sentence of "This land enters tapped. As it enters, choose a color." ends, in characters from the
/// start of the line: entering tapped and the choice made as it enters are two abilities on one line.
fn entering_split(line: &str) -> Option<usize> {
    let (first, rest) = line.split_once(". ")?;
    let tapped =
        first.ends_with(" enters tapped") || first.ends_with(" enters the battlefield tapped");
    (tapped && rest.starts_with("As ")).then(|| first.chars().count() + 1)
}

/// A mode of a modal spell or a cost of a Spree spell.
fn is_bullet(line: &str) -> bool {
    line.starts_with('\u{2022}') || line.starts_with("+ {")
}

/// `LEVEL 2-3`, the line that opens a level block.
fn is_level_header(line: &str) -> bool {
    line.strip_prefix("LEVEL ")
        .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    //! Goal: a face's text splits into the clauses its lines make, whatever mixes of keyword lists, mode lists, ability words, quotes and level blocks it holds, and each clause maps back to the original text.
    use super::*;
    use crate::compile::normalize::normalize;
    use rstest::rstest;

    fn clause_texts(original: &str) -> Vec<String> {
        let text = normalize(original, &[]);
        split(&text)
            .into_iter()
            .map(|clause| text.slice(clause.range))
            .collect()
    }

    #[rstest]
    #[case::one_line("Draw a card.", &["Draw a card."])]
    #[case::one_per_line("Flying\nWhen ~ enters, draw a card.\n{T}: Add {G}.", &["Flying", "When ~ enters, draw a card.", "{T}: Add {G}."])]
    #[case::blank_lines_dropped("Flying\n\n\nHaste", &["Flying", "Haste"])]
    #[case::entering_tapped_then_a_choice("This land enters tapped. As it enters, choose a color other than red.\n{T}: Add {R}.", &["This land enters tapped.", "As it enters, choose a color other than red.", "{T}: Add {R}."])]
    #[case::entering_tapped_alone("This land enters tapped.\nAs this land enters, choose a color.", &["This land enters tapped.", "As this land enters, choose a color."])]
    #[case::entering_tapped_then_a_rule("This land enters tapped. Draw a card.", &["This land enters tapped. Draw a card."])]
    #[case::a_condition_before_the_choice("This land enters tapped unless you control a Forest. As it enters, choose a color.", &["This land enters tapped unless you control a Forest. As it enters, choose a color."])]
    #[case::keyword_list("Vigilance, deathtouch, haste", &["Vigilance", "deathtouch", "haste"])]
    #[case::keyword_list_between_lines("Flying\nFirst strike, lifelink\nWhenever ~ attacks, draw.", &["Flying", "First strike", "lifelink", "Whenever ~ attacks, draw."])]
    #[case::partner_name_keeps_its_comma("Partner with Pir, Imaginative Rascal", &["Partner with Pir, Imaginative Rascal"])]
    #[case::cost_with_commas("Ward\u{2014}{2}, Pay 2 life.", &["Ward\u{2014}{2}, Pay 2 life."])]
    #[case::keyword_as_a_subject("Flying creatures you control get +1/+1, and more.", &["Flying creatures you control get +1/+1, and more."])]
    #[case::ability_word("Landfall \u{2014} Whenever a land you control enters, draw a card.", &["Landfall \u{2014} Whenever a land you control enters, draw a card."])]
    #[case::quoted_ability("Enchanted creature has \"{T}: Add {G}.\"\nFlying", &["Enchanted creature has \"{T}: Add {G}.\"", "Flying"])]
    #[case::quote_with_a_comma("Creatures you control have \"Flying, haste\"", &["Creatures you control have \"Flying, haste\""])]
    #[case::modal(
        "Choose one \u{2014}\n\u{2022} Counter target spell.\n\u{2022} Draw a card.\n\u{2022} Gain 2 life.",
        &["Choose one \u{2014}\n\u{2022} Counter target spell.\n\u{2022} Draw a card.\n\u{2022} Gain 2 life."]
    )]
    #[case::modal_then_more(
        "Choose one or both \u{2014}\n\u{2022} A.\n\u{2022} B.\nDraw a card.",
        &["Choose one or both \u{2014}\n\u{2022} A.\n\u{2022} B.", "Draw a card."]
    )]
    #[case::two_modal_lists(
        "Flying\nChoose one \u{2014}\n\u{2022} A.\n\u{2022} B.\nWhen ~ enters, choose one \u{2014}\n\u{2022} C.\n\u{2022} D.",
        &["Flying", "Choose one \u{2014}\n\u{2022} A.\n\u{2022} B.", "When ~ enters, choose one \u{2014}\n\u{2022} C.\n\u{2022} D."]
    )]
    #[case::spree(
        "Spree\n+ {1} \u{2014} Draw a card.\n+ {2}{R} \u{2014} ~ deals 3 damage.",
        &["Spree\n+ {1} \u{2014} Draw a card.\n+ {2}{R} \u{2014} ~ deals 3 damage."]
    )]
    #[case::loyalty_with_modes(
        "\u{2212}2: Choose one \u{2014}\n\u{2022} A.\n\u{2022} B.\n+1: Draw a card.",
        &["\u{2212}2: Choose one \u{2014}\n\u{2022} A.\n\u{2022} B.", "+1: Draw a card."]
    )]
    #[case::loyalty_lines("+2: Look.\n0: Draw.\n\u{2212}1: Return.\n\u{2212}12: Exile.", &["+2: Look.", "0: Draw.", "\u{2212}1: Return.", "\u{2212}12: Exile."])]
    #[case::level_up(
        "Level up {1}{U}\nLEVEL 2-3\n2/4\n{U}{U}, {T}: Copy target spell.\nLEVEL 4+\n2/5\n{U}{U}, {T}: Copy it twice.",
        &["Level up {1}{U}", "LEVEL 2-3\n2/4\n{U}{U}, {T}: Copy target spell.", "LEVEL 4+\n2/5\n{U}{U}, {T}: Copy it twice."]
    )]
    #[case::only_reminder("({T}: Add {G}.)", &[])]
    #[case::reminder_after_keywords("Flying (It can fly.)\nHaste (It is fast.)", &["Flying", "Haste"])]
    #[case::empty("", &[])]
    fn text_splits_into_clauses(#[case] original: &str, #[case] expected: &[&str]) {
        assert_eq!(clause_texts(original), expected);
    }

    #[test]
    fn only_the_last_keyword_of_a_line_takes_the_reminder_text_after_it() {
        let original = "Vigilance, deathtouch, haste (Reminders.)\nFlying (It flies.)";
        let text = normalize(original, &[]);
        let clauses = split(&text);
        let spans: Vec<_> = clauses
            .iter()
            .map(|clause| {
                text.span(clause.range.clone(), clause.ends_line)
                    .unwrap()
                    .slice(original)
                    .unwrap()
            })
            .collect();
        assert_eq!(
            spans,
            [
                "Vigilance",
                "deathtouch",
                "haste (Reminders.)",
                "Flying (It flies.)"
            ]
        );
    }

    #[rstest]
    #[case::keywords("Flying, first strike\nWhenever ~ attacks, draw.\n{T}: Add {G}.")]
    #[case::modes("Choose one \u{2014}\n\u{2022} A (x).\n\u{2022} B.\nFlying (y)")]
    #[case::levels("Level up {1}\nLEVEL 1-2\n1/1\nFlying\nLEVEL 3+\n2/2\nHaste")]
    #[case::unicode("\u{2212}1: Draw \u{2022} a card (z).\n+2: Look.")]
    fn clause_spans_are_in_bounds_and_disjoint(#[case] original: &str) {
        let text = normalize(original, &[]);
        let length = u32::try_from(original.chars().count()).unwrap();
        let spans: Vec<_> = split(&text)
            .into_iter()
            .map(|clause| text.span(clause.range, clause.ends_line).unwrap())
            .collect();
        assert!(!spans.is_empty(), "{original}");
        for (at, span) in spans.iter().enumerate() {
            assert!(span.start < span.end && span.end <= length, "{span:?}");
            assert!(span.slice(original).is_some());
            assert!(spans[..at].iter().all(|earlier| !earlier.overlaps(span)));
        }
    }
}
