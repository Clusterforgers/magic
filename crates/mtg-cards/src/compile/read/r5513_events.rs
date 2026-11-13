//! Events and conditions of the r55-13 pack: "they were attacked this turn by an Assassin you controlled", "it
//! exploited that creature".

use super::tokens::Reader;
use crate::refs::{Pred, Ref, Selector};
use crate::value::Condition;
use mtg_core::card::{CardType, Subtype};

/// The subject of an attack condition and its verb: "they were", "you were".
const SUBJECTS: [(Ref, &str); 2] = [(Ref::Triggering, "they were"), (Ref::You, "you were")];

/// After "if": "they were attacked this turn by an Assassin you controlled", "you were attacked this turn by a creature".
pub(super) fn r5513_condition(r: &mut Reader<'_>) -> Option<Condition> {
    if r.words("it exploited that creature") {
        return Some(Condition::Exploited {
            by: Ref::This,
            what: Ref::Triggering,
        });
    }
    let player = SUBJECTS
        .iter()
        .find_map(|(player, words)| r.words(words).then(|| player.clone()))?;
    r.need("attacked this turn by")?;
    let article = r.take_word()?;
    let noun = if r.word("creature") {
        None
    } else {
        Some(r.name()?.to_owned())
    };
    let by = attacker(article, noun, r.words("you controlled"))?;
    Some(Condition::AttackedThisTurnBy { player, by })
}

/// The attacker of [`r5513_condition`] from its article, its subtype (none for "creature") and whether "you controlled"
/// follows.
fn attacker(article: &str, noun: Option<String>, yours: bool) -> Option<Selector> {
    let mut by = match noun {
        None => (article == "a").then(|| vec![Pred::Type(CardType::Creature)])?,
        Some(name) => {
            let wanted = if starts_with_vowel(&name) { "an" } else { "a" };
            (article == wanted).then(|| vec![Pred::Subtype(Subtype(name))])?
        }
    };
    if yours {
        by.push(Pred::ControlledBy(Ref::You));
    }
    Some(Selector(by))
}

/// Whether the word takes "an".
fn starts_with_vowel(word: &str) -> bool {
    word.chars()
        .next()
        .is_some_and(|first| "AEIOUaeiou".contains(first))
}

/// The words of [`r5513_condition`], after "if".
pub(super) fn r5513_condition_text(condition: &Condition) -> Option<String> {
    let (player, by) = match condition {
        Condition::AttackedThisTurnBy { player, by } => (player, by),
        Condition::Exploited {
            by: Ref::This,
            what: Ref::Triggering,
        } => return Some("it exploited that creature".to_owned()),
        _ => return None,
    };
    let subject = SUBJECTS
        .iter()
        .find_map(|(named, words)| (named == player).then_some(*words))?;
    let (noun, yours) = match by.0.as_slice() {
        [Pred::Type(CardType::Creature)] => ("a creature".to_owned(), false),
        [Pred::Type(CardType::Creature), Pred::ControlledBy(Ref::You)] => {
            ("a creature".to_owned(), true)
        }
        [Pred::Subtype(Subtype(name))] => (with_article(name), false),
        [Pred::Subtype(Subtype(name)), Pred::ControlledBy(Ref::You)] => (with_article(name), true),
        _ => return None,
    };
    let owner = if yours { " you controlled" } else { "" };
    Some(format!("{subject} attacked this turn by {noun}{owner}"))
}

/// "an Assassin", "a Ninja".
fn with_article(name: &str) -> String {
    let article = if starts_with_vowel(name) { "an" } else { "a" };
    format!("{article} {name}")
}

#[cfg(test)]
mod tests {
    //! Goal: an attack condition reads back as it is written, for each subject and attacker, and a near miss is not read.
    use super::*;
    use crate::compile::read::tokens::tokenize;
    use rstest::rstest;

    #[rstest]
    #[case::assassin("they were attacked this turn by an Assassin you controlled")]
    #[case::ninja("they were attacked this turn by a Ninja you controlled")]
    #[case::any_creature("you were attacked this turn by a creature")]
    #[case::your_creature("they were attacked this turn by a creature you controlled")]
    #[case::exploited("it exploited that creature")]
    fn r5513_attack_condition_reads_back(#[case] text: &str) {
        let tokens = tokenize(text);
        let mut reader = Reader::new(&tokens, false);
        let condition = r5513_condition(&mut reader).unwrap();
        assert!(reader.end());
        assert_eq!(r5513_condition_text(&condition).as_deref(), Some(text));
    }

    #[rstest]
    #[case::wrong_article("they were attacked this turn by a Assassin you controlled")]
    #[case::no_noun("they were attacked this turn")]
    #[case::lowercase_noun("they were attacked this turn by an assassin")]
    fn r5513_a_near_miss_is_not_read(#[case] text: &str) {
        let tokens = tokenize(text);
        let mut reader = Reader::new(&tokens, false);
        let read = r5513_condition(&mut reader);
        assert!(read.is_none() || !reader.end(), "{text} read as {read:?}");
    }
}
