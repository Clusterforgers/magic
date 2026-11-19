//! "As a historic permanent you control enters, it becomes a 7/7 Dinosaur creature in addition to its other types.": a
//! replacement over the entering of each permanent a group names, cards and tokens alike, that gives it one form
//! (`ReplaceBy::BecomesChoice` of a single form; CR 614.1c, 614.12), read and written back.

use mtg_core::card::Subtype;

use super::r303_forms::form_text;
use super::selectors::{Form, card_type, phrase, phrase_text};
use super::tokens::Reader;
use super::triggers::article;
use crate::ability::Ability;
use crate::modify::Modification;
use crate::refs::{Pred, Selector, ZoneKind};
use crate::trigger::{ReplaceBy, ReplaceWhen};
use crate::value::Amount;

/// Whether the group names permanents other than the card itself and in no zone.
fn r109_group(what: &Selector) -> bool {
    !what.0.is_empty()
        && !what
            .0
            .iter()
            .any(|pred| matches!(pred, Pred::Spell | Pred::This | Pred::In(_)))
}

/// One form after its article: "7/7 Dinosaur creature", the power and toughness, then subtypes, then card types.
fn r109_form(r: &mut Reader<'_>) -> Option<Vec<Modification>> {
    let power = i32::try_from(r.number()?).ok()?;
    r.punct('/').then_some(())?;
    let toughness = i32::try_from(r.number()?).ok()?;
    let mut form = vec![Modification::SetPT {
        power: Amount::Fixed(power),
        toughness: Amount::Fixed(toughness),
    }];
    // A subtype is capitalized, so it is a name among the tokens.
    while let Some(name) = r.attempt(|r| {
        let name = r.name()?;
        r.is_subtype(name).then(|| name.to_owned())
    }) {
        form.push(Modification::AddSubtype(Subtype(name)));
    }
    let mut typed = false;
    while let Some(kind) = r.peek_word().and_then(card_type) {
        form.push(Modification::AddType(kind));
        typed = true;
        r.skip();
    }
    typed.then_some(form)
}

/// "As a historic permanent you control enters, it becomes a 7/7 Dinosaur creature in addition to its other types."
pub(super) fn r109_group_becomes(r: &mut Reader<'_>) -> Option<Ability> {
    r.need("as")?;
    (r.word("a") || r.word("an")).then_some(())?;
    let what = phrase(r, Form::Subject)?;
    r109_group(&what).then_some(())?;
    r.need("enters")?;
    r.punct(',').then_some(())?;
    r.need("it becomes")?;
    (r.word("a") || r.word("an")).then_some(())?;
    let form = r109_form(r)?;
    r.need("in addition to its other types")?;
    Some(Ability::Replacement {
        when: ReplaceWhen::Enters { what },
        by: ReplaceBy::BecomesChoice(vec![form]),
        condition: None,
        from: ZoneKind::Battlefield,
    })
}

/// What `r109_group_becomes` reads, written back.
pub(super) fn r109_wordings(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Replacement {
        when: ReplaceWhen::Enters { what },
        by: ReplaceBy::BecomesChoice(forms),
        condition: None,
        from: ZoneKind::Battlefield,
    } = ability
    else {
        return;
    };
    let ([form], true) = (forms.as_slice(), r109_group(what)) else {
        return;
    };
    let (Some(group), Some(form)) = (phrase_text(what, Form::Subject), form_text(form)) else {
        return;
    };
    texts.push(format!(
        "As {} {group} enters, it becomes {form} in addition to its other types",
        article(&group)
    ));
}

#[cfg(test)]
mod tests {
    //! Goal: a group's entering that becomes one form reads into one replacement and writes back the same words, and near misses (no "in addition", a choice of forms, the card itself, a form with no card type) read as nothing or other words.
    use rstest::rstest;

    use super::*;
    use crate::compile::read::tokens::tokenize;

    fn read(text: &str) -> Option<Ability> {
        let tokens = tokenize(text);
        let mut r = Reader::new(&tokens, false);
        r109_group_becomes(&mut r).filter(|_| r.sentence_end() && r.end())
    }

    #[rstest]
    #[case::historic(
        "As a historic permanent you control enters, it becomes a 7/7 Dinosaur creature in addition to its other types."
    )]
    #[case::artifact(
        "As an artifact you control enters, it becomes a 3/3 Golem creature in addition to its other types."
    )]
    fn it_reads_and_writes_back(#[case] text: &str) {
        let ability = read(text).expect("the wording reads");
        let mut texts = Vec::new();
        r109_wordings(&ability, &mut texts);
        assert_eq!(texts, vec![text.trim_end_matches('.').to_owned()]);
    }

    #[test]
    fn the_static_reader_reads_it() {
        let text = "As a historic permanent you control enters, it becomes a 7/7 Dinosaur creature in addition to its other types.";
        let found =
            crate::compile::read::clause(text, crate::compile::read::ClauseKind::Static, false);
        assert!(found.is_some());
        assert_eq!(found, read(text));
    }

    #[rstest]
    #[case::no_addition(
        "As a historic permanent you control enters, it becomes a 7/7 Dinosaur creature."
    )]
    #[case::no_type(
        "As a historic permanent you control enters, it becomes a 7/7 Dinosaur in addition to its other types."
    )]
    #[case::itself(
        "As this creature enters, it becomes a 7/7 Dinosaur creature in addition to its other types."
    )]
    fn near_misses_read_nothing(#[case] text: &str) {
        assert_eq!(read(text), None);
    }

    #[test]
    fn a_choice_of_two_forms_is_not_written_here() {
        let form = vec![
            Modification::SetPT {
                power: Amount::Fixed(1),
                toughness: Amount::Fixed(1),
            },
            Modification::AddType(mtg_core::card::CardType::Creature),
        ];
        let ability = Ability::Replacement {
            when: ReplaceWhen::Enters {
                what: Selector(vec![Pred::Type(mtg_core::card::CardType::Artifact)]),
            },
            by: ReplaceBy::BecomesChoice(vec![form.clone(), form]),
            condition: None,
            from: ZoneKind::Battlefield,
        };
        let mut texts = Vec::new();
        r109_wordings(&ability, &mut texts);
        assert_eq!(texts, Vec::<String>::new());
    }
}
