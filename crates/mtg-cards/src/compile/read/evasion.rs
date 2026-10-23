//! Evasion statics: creatures that can't be blocked except by some creatures, creatures that can't block some
//! creatures (by power, relative to the source), and the wordings of both written back.

use mtg_core::card::CardType;

use super::selectors::{Form, phrase, phrase_text};
use super::statics::{plural, statics, subjects};
use super::tokens::Reader;
use crate::ability::Ability;
use crate::modify::Modification;
use crate::refs::{Pred, Ref, Selector};
use crate::value::Condition;

/// The creature type.
fn creature() -> Pred {
    Pred::Type(CardType::Creature)
}

/// "Creatures with power less than this creature's power can't block it.", "Creatures with power less than this
/// creature's power can't block creatures you control.": the group that can't block, and whom it can't block.
pub(super) fn cannot_block(r: &mut Reader<'_>) -> Option<Ability> {
    let blockers = phrase(r, Form::All)?;
    r.need("can't block")?;
    let affected = if r.word("it") {
        Ref::This
    } else {
        plural(r)?
    };
    Some(statics(
        affected,
        vec![Modification::CantBeBlockedBy(blockers)],
        None,
    ))
}

/// The blockers a creature can't be blocked except by, as a selector of the blockers it can't be blocked by: not one
/// quality, or not all of several. Every blocker is a creature, so the type says nothing.
pub(super) fn except(allowed: Selector) -> Option<Selector> {
    let mut qualities: Vec<Pred> = allowed
        .0
        .into_iter()
        .filter(|pred| *pred != creature())
        .collect();
    match qualities.len() {
        0 => None,
        1 => qualities.pop().map(|only| Selector(vec![negated(only)])),
        _ => Some(Selector(vec![Pred::AnyOf(
            qualities.into_iter().map(negated).collect(),
        )])),
    }
}

/// The predicate that fails where `pred` holds.
fn negated(pred: Pred) -> Pred {
    Pred::Not(Box::new(pred))
}

/// What `except` was given, if the blockers were made that way.
fn allowed(blockers: &Selector) -> Option<Vec<Pred>> {
    match blockers.0.as_slice() {
        [Pred::Not(only)] => Some(vec![(**only).clone()]),
        [Pred::AnyOf(nots)] => nots
            .iter()
            .map(|pred| match pred {
                Pred::Not(inner) => Some((**inner).clone()),
                _ => None,
            })
            .collect(),
        _ => None,
    }
}

/// The wordings of blockers a creature can't be blocked except by: "creatures with flying", "Slivers".
fn allowed_texts(blockers: &Selector) -> Vec<String> {
    let Some(qualities) = allowed(blockers) else {
        return Vec::new();
    };
    // The card type that qualifies the noun comes first: "artifact creatures".
    let mut typed = qualities.clone();
    typed.push(creature());
    let mut texts: Vec<String> = phrase_text(&Selector(typed), Form::All)
        .into_iter()
        .collect();
    // A subtype or a card type names the creatures by itself: "Slivers".
    if qualities
        .iter()
        .any(|pred| matches!(pred, Pred::Subtype(_)))
    {
        texts.extend(phrase_text(&Selector(qualities), Form::All));
    }
    texts
}

/// The sentences of `affected` that can't be blocked by `blockers` or that `blockers` can't block, in each wording.
pub(super) fn texts(
    affected: &Ref,
    mods: &[Modification],
    condition: Option<&Condition>,
) -> Vec<String> {
    let ([Modification::CantBeBlockedBy(blockers)], None) = (mods, condition) else {
        return Vec::new();
    };
    let mut predicates: Vec<String> = allowed_texts(blockers)
        .into_iter()
        .map(|group| format!("can't be blocked except by {group}"))
        .collect();
    let blocked_by = phrase_text(blockers, Form::All);
    predicates.extend(blocked_by.iter().map(|group| format!("can't be blocked by {group}")));
    let mut written: Vec<String> = subjects(affected, true)
        .into_iter()
        .flat_map(|(subject, _)| {
            predicates
                .iter()
                .map(move |predicate| format!("{subject} {predicate}"))
        })
        .collect();
    // "Creatures with power less than this creature's power can't block it / creatures you control."
    if let Some(group) = blocked_by {
        let target = match affected {
            Ref::This => Some("it".to_owned()),
            Ref::Each(selector) => phrase_text(selector, Form::All),
            _ => None,
        };
        written.extend(target.map(|target| format!("{group} can't block {target}")));
    }
    // "~ can't be blocked by creatures with greater power."
    let greater: Vec<String> = written
        .iter()
        .filter(|text| text.contains("with power greater than ~'s power"))
        .map(|text| text.replace("with power greater than ~'s power", "with greater power"))
        .collect();
    written.extend(greater);
    written
}
