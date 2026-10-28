//! Counted amounts left over from the other readers: what the resolving effect destroyed ("for each creature destroyed
//! this way", "the number of creatures you controlled that were destroyed this way") and different values ("the number
//! of different powers among creatures you control", "three or more creatures with different powers").

use super::defining;
use super::facts;
use super::selectors::{Form, phrase, phrase_text};
use super::tokens::Reader;
use crate::refs::{Pred, Ref, Selector};
use crate::value::{Amount, Cmp, Condition, StatKind};

/// The characteristics a card compares for "different", singular and plural.
const W2C_STATS: [(&str, &str, StatKind); 2] = [
    ("power", "powers", StatKind::Power),
    ("mana value", "mana values", StatKind::ManaValue),
];

/// "different powers among creatures you control" after "the number of", or "different power among ..." after "for
/// each" (`plural` false).
pub(super) fn w2c_distinct(r: &mut Reader<'_>, plural: bool) -> Option<Amount> {
    r.attempt(|r| {
        r.need("different")?;
        let stat = W2C_STATS
            .iter()
            .find(|(one, many, _)| r.words(if plural { many } else { one }))
            .map(|(_, _, stat)| *stat)?;
        r.need("among")?;
        Some(Amount::Distinct {
            of: defining::counted(r)?,
            stat,
        })
    })
}

/// What `w2c_distinct` reads, written back.
pub(super) fn w2c_distinct_text(amount: &Amount, plural: bool) -> Option<String> {
    let Amount::Distinct { of, stat } = amount else {
        return None;
    };
    let (one, many, _) = W2C_STATS.iter().find(|(_, _, kind)| kind == stat)?;
    let name = if plural { many } else { one };
    Some(format!(
        "different {name} among {}",
        defining::counted_text(of)?
    ))
}

/// "you control three or more creatures with different powers" (coven): how many different powers there are among the
/// creatures you control, at least the number.
pub(super) fn w2c_coven(r: &mut Reader<'_>) -> Option<Condition> {
    r.need("you control")?;
    let number = facts::quantity(r)?;
    r.need("or more")?;
    let Selector(mut preds) = phrase(r, Form::All)?;
    r.need("with different")?;
    let stat = W2C_STATS
        .iter()
        .find(|(_, many, _)| r.words(many))
        .map(|(_, _, stat)| *stat)?;
    preds.push(Pred::ControlledBy(Ref::You));
    Some(Condition::Compare {
        left: Amount::Distinct {
            of: Selector(preds),
            stat,
        },
        op: Cmp::Ge,
        right: Amount::Fixed(i32::try_from(number).ok()?),
    })
}

/// What `w2c_coven` reads, written back in each wording of the number.
pub(super) fn w2c_coven_texts(condition: &Condition) -> Vec<String> {
    let Condition::Compare {
        left: Amount::Distinct { of, stat },
        op: Cmp::Ge,
        right: Amount::Fixed(number),
    } = condition
    else {
        return Vec::new();
    };
    let Some((Pred::ControlledBy(Ref::You), preds)) = of.0.split_last() else {
        return Vec::new();
    };
    let (Some((_, many, _)), Ok(number)) = (
        W2C_STATS.iter().find(|(_, _, kind)| kind == stat),
        u32::try_from(*number),
    ) else {
        return Vec::new();
    };
    let Some(group) = phrase_text(&Selector(preds.to_vec()), Form::All) else {
        return Vec::new();
    };
    facts::quantity_texts(number)
        .iter()
        .map(|words| format!("you control {words} or more {group} with different {many}"))
        .collect()
}

/// "creature destroyed this way" after "for each", "nontoken creature you controlled that was destroyed this way".
pub(super) fn w2c_destroyed_each(r: &mut Reader<'_>) -> Option<Amount> {
    r.attempt(|r| w2c_destroyed(r, Form::Count, "was"))
}

/// "creatures destroyed this way" after "the number of", "creatures you controlled that were destroyed this way".
pub(super) fn w2c_destroyed_all(r: &mut Reader<'_>) -> Option<Amount> {
    r.attempt(|r| w2c_destroyed(r, Form::All, "were"))
}

fn w2c_destroyed(r: &mut Reader<'_>, form: Form, verb: &str) -> Option<Amount> {
    let Selector(mut preds) = phrase(r, form)?;
    // The phrase names what it was, not who holds it now.
    if preds
        .iter()
        .any(|pred| matches!(pred, Pred::ControlledBy(_) | Pred::OwnedBy(_) | Pred::In(_)))
    {
        return None;
    }
    if r.words("you controlled that") {
        r.need(verb)?;
        preds.push(Pred::ControlledBy(Ref::You));
    }
    r.need("destroyed this way")?;
    Some(Amount::Destroyed(Selector(preds)))
}

/// What `w2c_destroyed_each` reads, written back; `None` for any other amount.
pub(super) fn w2c_destroyed_each_text(amount: &Amount) -> Option<String> {
    w2c_destroyed_text(amount, Form::Count, "was")
}

/// What `w2c_destroyed_all` reads, written back; `None` for any other amount.
pub(super) fn w2c_destroyed_all_text(amount: &Amount) -> Option<String> {
    w2c_destroyed_text(amount, Form::All, "were")
}

fn w2c_destroyed_text(amount: &Amount, form: Form, verb: &str) -> Option<String> {
    let Amount::Destroyed(Selector(preds)) = amount else {
        return None;
    };
    let yours = Pred::ControlledBy(Ref::You);
    let kinds: Vec<Pred> = preds
        .iter()
        .filter(|pred| **pred != yours)
        .cloned()
        .collect();
    if kinds
        .iter()
        .any(|pred| matches!(pred, Pred::ControlledBy(_) | Pred::OwnedBy(_) | Pred::In(_)))
    {
        return None;
    }
    let phrase = phrase_text(&Selector(kinds.clone()), form)?;
    Some(if kinds.len() == preds.len() {
        format!("{phrase} destroyed this way")
    } else {
        format!("{phrase} you controlled that {verb} destroyed this way")
    })
}
