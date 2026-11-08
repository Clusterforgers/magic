//! Sequence of w16: the actions of one trigger or one spell run in the order they are said, whether the text joins them
//! with "and", a comma or a period. "Sacrifice ~ and target opponent discards their hand" and "Sacrifice ~. Target
//! opponent discards their hand" are one text; a condition, a choice or a "may" in the text keeps the joins apart.

use super::w16b_labels::{ENDS, VERBS};

/// The words of a text that make a join carry a meaning: the second action depends on the first or on a choice.
const BLOCKERS: [&str; 9] = [
    "if you do",
    "unless",
    " may ",
    "otherwise",
    "instead",
    "as long as",
    "for each",
    "each time",
    "choose",
];

/// A text of triggers: its condition covers every sentence of the ability.
const TRIGGER_STARTS: [&str; 3] = ["when ", "at the ", "whenever "];

/// Whether the word is an order ("draw") and not a verb with a subject ("draws").
fn order(word: &str) -> bool {
    let word = word.trim_end_matches(ENDS).to_ascii_lowercase();
    VERBS.contains(&word.as_str()) && !word.ends_with('s')
}

/// Whether the words start a clause with a player as its subject ("target opponent discards").
fn player_clause(words: &[&str]) -> bool {
    matches!(words, ["target", who, ..] if who.trim_end_matches(ENDS) == "player" || who.trim_end_matches(ENDS) == "opponent")
}

/// The text with each "and" or "," between two actions written as a period, and each sentence opening in lower case.
pub(super) fn normal(text: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    if BLOCKERS.iter().any(|word| lower.contains(word)) {
        return None;
    }
    let trigger = TRIGGER_STARTS.iter().any(|start| lower.starts_with(start));
    if (lower.contains(" if ") || lower.starts_with("if ")) && !trigger {
        return None;
    }
    let words: Vec<&str> = text.split(' ').collect();
    let mut out: Vec<String> = Vec::with_capacity(words.len());
    // Whether the clause since the last boundary opened with an order.
    let mut ordered = false;
    let mut clause_start = true;
    let mut at = 0;
    while let Some(word) = words.get(at) {
        let after_comma = at > 0 && words[at - 1].ends_with(',');
        let and_split = *word == "and"
            && !clause_start
            && words
                .get(at + 1)
                .is_some_and(|next| order(next) || player_clause(&words[at + 1..]));
        let comma_split = after_comma && ordered && order(word);
        if and_split || comma_split {
            if let Some(last) = out.last_mut() {
                *last = last.trim_end_matches(',').to_owned();
                last.push('.');
            }
            if and_split {
                at += 1;
            }
            clause_start = true;
        }
        let Some(word) = words.get(at) else { break };
        if clause_start || after_comma {
            ordered = order(word);
        }
        let sentence_start = out.last().is_some_and(|last| last.ends_with('.'));
        let word = if sentence_start {
            lowered(word)
        } else {
            (*word).to_owned()
        };
        clause_start = word.ends_with('.');
        out.push(word);
        at += 1;
    }
    let changed = out.join(" ");
    (changed != text).then_some(changed)
}

/// The word with its first letter in lower case.
fn lowered(word: &str) -> String {
    let mut chars = word.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_lowercase().chain(chars).collect()
    })
}
