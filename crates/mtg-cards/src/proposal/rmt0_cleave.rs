//! Cleave read back (CR 702.148): the keyword line is "Cleave" and its cost, and the spell's text is its full wording
//! with the words its cleaved spell leaves out in square brackets ("Destroy target [attacking] creature.").

use crate::ability::Ability;
use crate::compile::{render_cost, written_variants};
use crate::keyword::Keyword;

/// The most wordings of the whole spell and of the cleaved spell that are set against each other.
const WORDINGS: (usize, usize) = (400, 40);

/// The keyword line and the cleaved spell of a cleave keyword: "Cleave {1}{U}" and its `Spell` ability.
pub(super) fn cleave(ability: &Ability) -> Option<(String, Ability)> {
    let Ability::Keyword(Keyword::Cleave {
        cost,
        targets,
        effects,
    }) = ability
    else {
        return None;
    };
    let words = render_cost(cost)?;
    Some((
        format!("Cleave {words}"),
        Ability::Spell {
            targets: targets.clone(),
            effects: effects.clone(),
        },
    ))
}

/// The wordings of a spell whose cleaved spell is `cleaved`: each wording of the whole spell with the words a wording of
/// the cleaved spell lacks in square brackets.
pub(super) fn bracketed_variants(spell: &Ability, cleaved: &Ability) -> Vec<String> {
    let full = written_variants(spell);
    let short = written_variants(cleaved);
    let mut out = Vec::new();
    for whole in full.iter().take(WORDINGS.0) {
        for less in short.iter().take(WORDINGS.1) {
            for text in bracketed(whole, less) {
                if !out.contains(&text) {
                    out.push(text);
                }
            }
        }
    }
    out
}

/// The words of a text with each mark of punctuation a word of its own, so "spell." and "spell" share the word "spell".
fn words(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for word in text.split_whitespace() {
        let body = word.trim_end_matches([',', '.', ';', ':']);
        if !body.is_empty() {
            out.push(body.to_owned());
        }
        out.extend(word[body.len()..].chars().map(String::from));
    }
    out
}

/// `whole` with the words `less` leaves out in square brackets, matching the words of `less` in order from the front
/// and, as a second reading, from the back; nothing when `less` is not a part of `whole` in order or leaves nothing out.
fn bracketed(whole: &str, less: &str) -> Vec<String> {
    let all = words(whole);
    let kept = words(less);
    let mut out = Vec::new();
    for from_back in [false, true] {
        let Some(marks) = matched(&all, &kept, from_back) else {
            continue;
        };
        if marks.iter().all(|kept| *kept) {
            continue;
        }
        let text = joined(&all, &marks);
        if !out.contains(&text) {
            out.push(text);
        }
    }
    out
}

/// Which words of `all` the words of `kept` take, in order from the front or from the back.
fn matched(all: &[String], kept: &[String], from_back: bool) -> Option<Vec<bool>> {
    let mut marks = vec![false; all.len()];
    let order: Vec<usize> = if from_back {
        (0..all.len()).rev().collect()
    } else {
        (0..all.len()).collect()
    };
    let wanted: Vec<&String> = if from_back {
        kept.iter().rev().collect()
    } else {
        kept.iter().collect()
    };
    let mut next = 0;
    for at in order {
        if next < wanted.len() && all[at] == *wanted[next] {
            marks[at] = true;
            next += 1;
        }
    }
    (next == wanted.len()).then_some(marks)
}

/// The words joined by spaces, a mark of punctuation joined to the word before it, and each run of words not kept in
/// square brackets.
fn joined(all: &[String], kept: &[bool]) -> String {
    let mut text = String::new();
    for (at, word) in all.iter().enumerate() {
        let opens = !kept[at] && (at == 0 || kept[at - 1]);
        let closes = !kept[at] && (at + 1 == all.len() || kept[at + 1]);
        let punctuation =
            word.len() == 1 && word.chars().all(|c| matches!(c, ',' | '.' | ';' | ':'));
        if !text.is_empty() && !punctuation {
            text.push(' ');
        }
        if opens {
            text.push('[');
        }
        text.push_str(word);
        if closes {
            text.push(']');
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::bracketed;
    use rstest::rstest;

    #[rstest]
    #[case::a_word(
        "Destroy target attacking creature.",
        "Destroy target creature.",
        "Destroy target [attacking] creature."
    )]
    #[case::before_the_period(
        "Counter target spell that wasn't cast from its owner's hand.",
        "Counter target spell.",
        "Counter target spell [that wasn't cast from its owner's hand]."
    )]
    #[case::a_sentence(
        "Draw three cards. Your maximum hand size is reduced by three.",
        "Draw three cards.",
        "Draw three cards. [Your maximum hand size is reduced by three.]"
    )]
    #[case::two_runs(
        "Search your library for a basic land card, reveal it, put it into your hand, then shuffle.",
        "Search your library for a card, put it into your hand, then shuffle.",
        "Search your library for a [basic land] card, [reveal it,] put it into your hand, then shuffle."
    )]
    fn the_words_the_cleaved_spell_leaves_out_are_bracketed(
        #[case] whole: &str,
        #[case] less: &str,
        #[case] expected: &str,
    ) {
        assert!(bracketed(whole, less).contains(&expected.to_owned()));
    }

    #[rstest]
    #[case::the_same_text("Destroy target creature.", "Destroy target creature.")]
    #[case::another_word("Destroy target attacking creature.", "Exile target creature.")]
    #[case::out_of_order("Destroy target attacking creature.", "Destroy creature target.")]
    fn a_cleaved_spell_that_is_no_part_of_the_spell_brackets_nothing(
        #[case] whole: &str,
        #[case] less: &str,
    ) {
        assert!(bracketed(whole, less).is_empty());
    }
}
