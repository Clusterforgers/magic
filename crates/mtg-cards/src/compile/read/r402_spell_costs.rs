//! r40-2 cost changes that count for the spell being cast, and their writers: "for each artifact its controller
//! controls" (`ControlledBy(ControllerOf(Affected))`), "for each card with the same name as that spell in your
//! graveyard" (`SharesName(Affected)`) and "for each creature it targets" (`Amount::CastTargetsOf`); a subject of one
//! spell, "Each artifact spell costs", is the twin of "Artifact spells cost".

use super::selectors::{Form, phrase, phrase_text};
use super::tokens::{Reader, tokenize};
use crate::refs::{Pred, Ref, Selector};
use crate::value::Amount;

/// The controller of the spell being cast: "its controller".
fn its_controller() -> Ref {
    Ref::ControllerOf(Box::new(Ref::Affected))
}

/// "That spell" of a cost change: the spell being cast.
fn same_name() -> Pred {
    Pred::SharesName(Ref::Affected)
}

/// The selector of a phrase read alone, `None` when the reader leaves words.
fn phrase_of(text: &str, form: Form) -> Option<Selector> {
    let tokens = tokenize(text);
    let mut r = Reader::new(&tokens, false);
    let selector = phrase(&mut r, form)?;
    r.end().then_some(selector)
}

/// The cards "card in your graveyard" names.
fn your_graveyard_cards() -> Option<Selector> {
    phrase_of("card in your graveyard", Form::Count)
}

/// The count after "for each" of a cost change of other spells that reads the spell being cast; `None` for any other.
pub(super) fn r402_for_each(r: &mut Reader<'_>) -> Option<Amount> {
    r.need("for each")?;
    if r.words("card with the same name as that spell in your graveyard") {
        let mut cards = your_graveyard_cards()?;
        cards.0.push(same_name());
        return Some(Amount::Count(cards));
    }
    if let Some(counted) = r.attempt(|r| {
        let mut counted = phrase(r, Form::Count)?;
        r.need("its controller controls")?;
        counted.0.push(Pred::ControlledBy(its_controller()));
        Some(Amount::Count(counted))
    }) {
        return Some(counted);
    }
    r.attempt(|r| {
        let counted = phrase(r, Form::Count)?;
        r.need("it targets")?;
        Some(Amount::CastTargetsOf(counted))
    })
}

/// The words after "for each" of a count [`r402_for_each`] reads.
pub(super) fn r402_for_each_text(per: &Amount) -> Option<String> {
    match per {
        Amount::CastTargetsOf(counted) => {
            Some(format!("{} it targets", phrase_text(counted, Form::Count)?))
        }
        Amount::Count(counted) => {
            if let Some((last, rest)) = counted.0.split_last()
                && *last == same_name()
                && Some(Selector(rest.to_vec())) == your_graveyard_cards()
            {
                return Some("card with the same name as that spell in your graveyard".to_owned());
            }
            let (last, rest) = counted.0.split_last()?;
            (*last == Pred::ControlledBy(its_controller())).then_some(())?;
            let rest = Selector(rest.to_vec());
            Some(format!(
                "{} its controller controls",
                phrase_text(&rest, Form::Count)?
            ))
        }
        _ => None,
    }
}

/// "Artifact spells cost {1} more ..." as "Each artifact spell costs {1} more ...": the subject of one spell; `None`
/// when the text does not open with a plural subject of spells before "cost".
pub(super) fn r402_each_spell(text: &str) -> Option<String> {
    let (subject, rest) = text.split_once(" cost ")?;
    let (kinds, after) = subject.split_once("spells")?;
    (!kinds.contains("spell") && !after.contains("spell")).then_some(())?;
    let mut lowered = kinds.chars();
    let first = lowered.next()?.to_lowercase();
    Some(format!(
        "Each {first}{}spell{after} costs {rest}",
        lowered.as_str()
    ))
}

/// "Each artifact spell costs {1} more ..." as "Artifact spells cost {1} more ...", the twin
/// [`r402_each_spell`] writes back; `None` for any other text.
pub(super) fn r402_spells_twin(text: &str) -> Option<String> {
    let rest = text.strip_prefix("Each ")?;
    let (subject, tail) = rest.split_once(" costs ")?;
    let (kinds, after) = subject.split_once("spell")?;
    (after.is_empty() || after.starts_with(' ')).then_some(())?;
    (!after.contains("spell") && !after.contains(" a player")).then_some(())?;
    let mut chars = kinds.chars();
    let first = chars.next()?.to_uppercase();
    Some(format!(
        "{first}{}spells{after} cost {tail}",
        chars.as_str()
    ))
}

#[cfg(test)]
mod tests {
    //! Goal: the subject of one spell and the counts of the spell being cast read into the shapes a cost modifier holds
    //! and back, and a near miss (a subject of a player, "that creature", another zone) reads nothing.
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::artifact(
        "Each artifact spell costs {1} more to cast for each artifact its controller controls.",
        Some("Artifact spells cost {1} more to cast for each artifact its controller controls.")
    )]
    #[case::you_cast(
        "Each instant and sorcery spell you cast costs {1} less to cast for each creature it targets.",
        Some(
            "Instant and sorcery spells you cast cost {1} less to cast for each creature it targets."
        )
    )]
    #[case::a_player_casts(
        "Each spell a player casts costs {1} more to cast for each other spell that player has cast this turn.",
        None
    )]
    #[case::no_each("Artifact spells cost {1} more to cast.", None)]
    fn r402_one_spell_is_the_twin_of_spells(#[case] text: &str, #[case] twin: Option<&str>) {
        let found = r402_spells_twin(text);
        assert_eq!(found.as_deref(), twin);
        if let Some(twin) = found {
            assert_eq!(r402_each_spell(&twin).as_deref(), Some(text));
        }
    }

    #[rstest]
    #[case::its_controller("for each artifact its controller controls", true)]
    #[case::same_name(
        "for each card with the same name as that spell in your graveyard",
        true
    )]
    #[case::it_targets("for each creature it targets", true)]
    #[case::that_creature(
        "for each card with the same name as that creature in your graveyard",
        false
    )]
    #[case::your_hand("for each card with the same name as that spell in your hand", false)]
    #[case::you_control("for each artifact you control", false)]
    fn r402_counts_of_the_spell_read_back(#[case] words: &str, #[case] reads: bool) {
        let tokens = tokenize(words);
        let mut r = Reader::new(&tokens, false);
        let found = r402_for_each(&mut r).filter(|_| r.end());
        assert_eq!(found.is_some(), reads, "{words}: {found:?}");
        if let Some(amount) = found {
            let text = r402_for_each_text(&amount).unwrap();
            assert_eq!(format!("for each {text}"), words);
        }
    }
}
