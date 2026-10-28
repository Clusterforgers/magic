//! Counted amounts of what the resolving effect did: "for each creature destroyed this way", "the number of creatures
//! you controlled that were destroyed this way".

use super::selectors::{Form, phrase, phrase_text};
use super::tokens::Reader;
use crate::refs::{Pred, Ref, Selector};
use crate::value::Amount;

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
