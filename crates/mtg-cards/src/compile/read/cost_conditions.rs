//! What decides a cost change besides the spell it names: the condition of "This spell costs {2} less to cast if you
//! control a Dragon." and "If there are ten or more creature cards total in all graveyards, this spell costs {6} less
//! to cast.", and the spells of two kinds in "Merfolk spells and Wizard spells you cast cost {1} less to cast." Read
//! from the wording and written back the same way.

use super::as_long_as;
use super::facts::{quantity, quantity_texts};
use super::selectors::{Form, phrase, phrase_in, phrase_text, subtype, type_name};
use super::tokens::Reader;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::value::{Amount, Cmp, Condition};
use mtg_core::card::Subtype;

/// "If ..., " before the spell: "If there are ten or more creature cards total in all graveyards, ".
pub(super) fn leading(r: &mut Reader<'_>) -> Option<Condition> {
    r.need("if")?;
    let held = condition(r)?;
    r.punct(',').then_some(held)
}

/// " if ..." after the cost: " if you control a Dragon".
pub(super) fn trailing(r: &mut Reader<'_>) -> Option<Condition> {
    r.need("if")?;
    condition(r)
}

/// The facts a cost change can depend on.
fn condition(r: &mut Reader<'_>) -> Option<Condition> {
    r.attempt(counted_on_the_battlefield)
        .or_else(|| r.attempt(counted_in_all_graveyards))
        .or_else(|| r.attempt(opponent_lacks))
        .or_else(|| as_long_as::body(r, None))
}

/// A count of objects compared with a number.
fn at_least(selector: Selector, number: u32) -> Option<Condition> {
    Some(Condition::Compare {
        left: Amount::Count(selector),
        op: Cmp::Ge,
        right: Amount::Fixed(i32::try_from(number).ok()?),
    })
}

/// "there are ten or more nonland permanents on the battlefield".
fn counted_on_the_battlefield(r: &mut Reader<'_>) -> Option<Condition> {
    r.need("there are")?;
    let number = quantity(r)?;
    r.need("or more")?;
    let mut preds = phrase(r, Form::All)?.0;
    let placed = preds.contains(&Pred::In(ZoneKind::Battlefield));
    preds.retain(|pred| *pred != Pred::In(ZoneKind::Battlefield));
    if preds
        .iter()
        .any(|pred| matches!(pred, Pred::In(_) | Pred::ControlledBy(_) | Pred::OwnedBy(_)))
    {
        return None;
    }
    if !placed {
        r.need("on the battlefield")?;
    }
    at_least(Selector(preds), number)
}

/// "there are ten or more creature cards total in all graveyards".
fn counted_in_all_graveyards(r: &mut Reader<'_>) -> Option<Condition> {
    r.need("there are")?;
    let number = quantity(r)?;
    r.need("or more")?;
    let mut preds = phrase_in(r, Form::All, true)?.0;
    if preds
        .iter()
        .any(|pred| matches!(pred, Pred::In(_) | Pred::ControlledBy(_) | Pred::OwnedBy(_)))
    {
        return None;
    }
    r.need("total in all graveyards")?;
    preds.push(Pred::In(ZoneKind::Graveyard));
    at_least(Selector(preds), number)
}

/// "an opponent controls no basic lands": no opponent controls one.
fn opponent_lacks(r: &mut Reader<'_>) -> Option<Condition> {
    r.need("an opponent controls no")?;
    let mut preds = phrase(r, Form::All)?.0;
    if preds
        .iter()
        .any(|pred| matches!(pred, Pred::In(_) | Pred::ControlledBy(_) | Pred::OwnedBy(_)))
    {
        return None;
    }
    preds.push(Pred::ControlledBy(Ref::Opponents));
    Some(Condition::Compare {
        left: Amount::Count(Selector(preds)),
        op: Cmp::Eq,
        right: Amount::Fixed(0),
    })
}

/// What `condition` reads, written back after "if", one text for each wording.
pub(super) fn texts(held: &Condition) -> Vec<String> {
    let mut written = counted_texts(held);
    written.extend(as_long_as::texts(held));
    written
}

/// The wordings of the counts this module reads.
fn counted_texts(held: &Condition) -> Vec<String> {
    let Condition::Compare {
        left: Amount::Count(selector),
        op,
        right: Amount::Fixed(number),
    } = held
    else {
        return Vec::new();
    };
    let Ok(number) = u32::try_from(*number) else {
        return Vec::new();
    };
    let kind = |text: &str, tail: &str| -> Vec<String> {
        quantity_texts(number)
            .into_iter()
            .map(|words| format!("there are {words} or more {text}{tail}"))
            .collect()
    };
    match (op, selector.0.as_slice()) {
        (Cmp::Ge, [Pred::Type(card_type), Pred::In(ZoneKind::Graveyard)]) => kind(
            &format!("{} cards", type_name(*card_type)),
            " total in all graveyards",
        ),
        (Cmp::Ge, preds) if allowed_on_the_battlefield(preds) => phrase_text(selector, Form::All)
            .map(|group| kind(&group, " on the battlefield"))
            .unwrap_or_default(),
        (Cmp::Eq, preds) if number == 0 => {
            let rest: Vec<Pred> = preds
                .iter()
                .filter(|pred| **pred != Pred::ControlledBy(Ref::Opponents))
                .cloned()
                .collect();
            if rest.len() + 1 != preds.len() {
                return Vec::new();
            }
            phrase_text(&Selector(rest), Form::All)
                .map(|group| vec![format!("an opponent controls no {group}")])
                .unwrap_or_default()
        }
        _ => Vec::new(),
    }
}

/// Whether a group names no zone and no player: "nonland permanents".
fn allowed_on_the_battlefield(preds: &[Pred]) -> bool {
    !preds.is_empty()
        && preds
            .iter()
            .all(|pred| !matches!(pred, Pred::In(_) | Pred::ControlledBy(_) | Pred::OwnedBy(_)))
}

/// "Merfolk spells and Wizard spells": spells of one kind each, joined by "and"; the first is already read.
pub(super) fn more_spells(r: &mut Reader<'_>, first: Selector) -> Option<Selector> {
    let [Pred::Spell, kind] = first.0.as_slice() else {
        return Some(first);
    };
    let mut kinds = vec![kind.clone()];
    while r.word("and") {
        let next = phrase(r, Form::All)?;
        let [Pred::Spell, kind] = next.0.as_slice() else {
            return None;
        };
        kinds.push(kind.clone());
    }
    Some(Selector(if kinds.len() > 1 {
        vec![Pred::Spell, Pred::AnyOf(kinds)]
    } else {
        first.0
    }))
}

/// The spells of an `AnyOf` of kinds, written as "Merfolk spells and Wizard spells".
pub(super) fn spells_text(kinds: &[Pred]) -> Option<String> {
    let parts: Option<Vec<String>> = kinds
        .iter()
        .map(|kind| phrase_text(&Selector(vec![Pred::Spell, kind.clone()]), Form::All))
        .collect();
    Some(parts?.join(" and "))
}

/// "Aura and Equipment spells": spells of several subtypes named before the noun they share.
pub(super) fn shared_spells(r: &mut Reader<'_>) -> Option<Selector> {
    let mut kinds = vec![Pred::Subtype(Subtype(subtype(r)?))];
    while r.word("and") {
        kinds.push(Pred::Subtype(Subtype(subtype(r)?)));
    }
    r.need("spells")?;
    (kinds.len() > 1).then(|| Selector(vec![Pred::Spell, Pred::AnyOf(kinds)]))
}

/// "Creature spells with power 4 or greater you cast" worded as "Creature spells you cast with power 4 or greater".
pub(super) fn cast_before_number(subject: &str) -> Option<String> {
    let (head, number) = subject.split_once(" with ")?;
    let (number, cast) = number
        .strip_suffix(" you cast")
        .map(|number| (number, " you cast"))
        .or_else(|| {
            number
                .strip_suffix(" your opponents cast")
                .map(|number| (number, " your opponents cast"))
        })?;
    Some(format!("{head}{cast} with {number}"))
}

/// "Aura and Equipment spells you cast": the subtypes of an `AnyOf` of kinds share the noun after them.
pub(super) fn shared_subject_text(applies_to: &Selector) -> Option<String> {
    let [Pred::Spell, Pred::AnyOf(kinds), rest @ ..] = applies_to.0.as_slice() else {
        return None;
    };
    if kinds.len() < 2 || !kinds.iter().all(|kind| matches!(kind, Pred::Subtype(_))) {
        return None;
    }
    let who = match rest {
        [] => "",
        [Pred::ControlledBy(Ref::You)] => " you cast",
        _ => return None,
    };
    let names: Option<Vec<String>> = kinds
        .iter()
        .map(|kind| {
            let spells = phrase_text(&Selector(vec![Pred::Spell, kind.clone()]), Form::All)?;
            spells.strip_suffix(" spells").map(str::to_owned)
        })
        .collect();
    Some(format!("{} spells{who}", names?.join(" and ")))
}
