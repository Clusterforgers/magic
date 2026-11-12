//! Suffixes of the shared noun-phrase reader and writer for the r30-1b pack, consulted after the others: "banded with ~"
//! (CR 702.22) and "whose controller controls an Island".

use super::selectors::{Form, phrase, phrase_text};
use super::tokens::Reader;
use super::triggers::article;
use crate::refs::{Pred, Ref, Selector};
use crate::value::{Amount, Cmp, Condition};

/// "banded with ~": a creature in a band with the card itself.
pub(super) fn r301b_suffix(r: &mut Reader<'_>, form: Form) -> Option<Pred> {
    r.attempt(|r| {
        r.need("banded with")?;
        r.source().then_some(Pred::BandedWith(Ref::This))
    })
    .or_else(|| {
        (!form.plural())
            .then(|| r.attempt(controller_controls))
            .flatten()
    })
}

/// "whose controller controls an Island": the object's controller controls a permanent of the kind.
fn controller_controls(r: &mut Reader<'_>) -> Option<Pred> {
    r.need("whose controller controls")?;
    (r.word("a") || r.word("an")).then_some(())?;
    let mut kind = phrase(r, Form::Subject)?;
    kind.0.push(Pred::ControlledBy(Ref::You));
    Some(controls(kind))
}

/// The predicate of "whose controller controls" a permanent the selector picks, written for "you".
fn controls(kind: Selector) -> Pred {
    Pred::ControlledBy(Ref::PlayersWhere {
        who: Box::new(Ref::EachPlayer),
        test: Box::new(Condition::Compare {
            left: Amount::Count(kind),
            op: Cmp::Ge,
            right: Amount::Fixed(1),
        }),
    })
}

/// The suffix of this module for a predicate.
pub(super) fn r301b_suffix_text(pred: &Pred, form: Form) -> Option<String> {
    if *pred == Pred::BandedWith(Ref::This) {
        return Some("banded with ~".to_owned());
    }
    let Pred::ControlledBy(Ref::PlayersWhere { who, test }) = pred else {
        return None;
    };
    let Condition::Compare {
        left: Amount::Count(kind),
        op: Cmp::Ge,
        right: Amount::Fixed(1),
    } = &**test
    else {
        return None;
    };
    let mine = Pred::ControlledBy(Ref::You);
    let rest: Vec<Pred> = kind
        .0
        .iter()
        .filter(|held| **held != mine)
        .cloned()
        .collect();
    (**who == Ref::EachPlayer && !form.plural() && rest.len() + 1 == kind.0.len()).then_some(())?;
    let text = phrase_text(&Selector(rest), Form::Subject)?;
    Some(format!(
        "whose controller controls {} {text}",
        article(&text)
    ))
}
