//! Cleave (CR 702.148): the keyword carries the spell the face's text is without its words in square brackets. The spell
//! line is read twice, its brackets dropped for the spell and its bracketed words dropped for the keyword; a face whose
//! line does not read both ways leaves the keyword and the line to the table.

use super::{ClauseKind, Pending, classify, read};
use crate::ability::Ability;
use crate::keyword::Keyword;

/// The text with its brackets dropped (`cut` false) or with the words in them dropped (`cut` true).
pub(super) fn without_brackets(text: &str, cut: bool) -> String {
    let mut kept = String::new();
    let mut inside = false;
    for c in text.chars() {
        match c {
            '[' => inside = true,
            ']' => inside = false,
            c if !(inside && cut) => kept.push(c),
            _ => {}
        }
    }
    let mut tidy = kept.replace("  ", " ");
    for mark in [".", ",", ";"] {
        tidy = tidy.replace(&format!(" {mark}"), mark);
    }
    tidy.trim().to_owned()
}

/// Gives a cleave keyword the spell of the face's one bracketed spell line and gives the line the whole spell.
pub(super) fn give_cleave(pending: &mut [Pending], x_free: bool, own: &read::Own) {
    let Some(keyword) = pending.iter().position(|clause| {
        matches!(
            clause.ability,
            Some(Ability::Keyword(Keyword::Cleave { .. }))
        )
    }) else {
        return;
    };
    let spells: Vec<usize> = (0..pending.len())
        .filter(|&at| pending[at].kind == ClauseKind::Spell)
        .collect();
    let read_both = match spells.as_slice() {
        [line] if pending[*line].text.contains('[') => {
            let text = pending[*line].text.as_str();
            let text = text
                .get(classify::ability_word_len(text).unwrap_or(0)..)
                .unwrap_or(text);
            let whole = read::clause_with(
                &without_brackets(text, false),
                ClauseKind::Spell,
                x_free,
                own,
            );
            let cleaved = read::clause_with(
                &without_brackets(text, true),
                ClauseKind::Spell,
                x_free,
                own,
            );
            whole
                .zip(cleaved)
                .map(|(whole, cleaved)| (*line, whole, cleaved))
        }
        _ => None,
    };
    let Some((line, whole, Ability::Spell { targets, effects })) = read_both else {
        pending[keyword].ability = None;
        pending[keyword].read = false;
        return;
    };
    if let Some(Ability::Keyword(Keyword::Cleave {
        targets: given,
        effects: done,
        ..
    })) = &mut pending[keyword].ability
    {
        *given = targets;
        *done = effects;
    }
    pending[line].ability = Some(whole);
    pending[line].read = true;
    pending[line].held = false;
}

#[cfg(test)]
mod tests {
    use super::without_brackets;
    use rstest::rstest;

    #[rstest]
    #[case::kept(
        "Destroy target [attacking] creature.",
        false,
        "Destroy target attacking creature."
    )]
    #[case::cut(
        "Destroy target [attacking] creature.",
        true,
        "Destroy target creature."
    )]
    #[case::cut_before_the_period(
        "Counter target spell [that wasn't cast from its owner's hand].",
        true,
        "Counter target spell."
    )]
    #[case::cut_two_runs(
        "Search your library for a [basic land] card, [reveal it,] put it into your hand, then shuffle.",
        true,
        "Search your library for a card, put it into your hand, then shuffle."
    )]
    fn the_brackets_or_their_words_are_dropped(
        #[case] text: &str,
        #[case] cut: bool,
        #[case] expected: &str,
    ) {
        assert_eq!(without_brackets(text, cut), expected);
    }
}
