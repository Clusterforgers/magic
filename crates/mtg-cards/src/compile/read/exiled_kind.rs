//! What an exile does when the object it took was of a kind: "Exile target card from a graveyard. If it was a creature
//! card, put a +1/+1 counter on this creature and you gain 1 life." (an `Exile` that binds what it took, then an `If` of
//! `Matches` over the binding), read after the sentence of the exile and written back the same way.

use super::conditional;
use super::library::{filter, filter_text};
use super::tokens::Reader;
use super::triggers::article;
use crate::effect::Effect;
use crate::refs::{Ref, Selector};
use crate::value::Condition;

/// The name the exiled object is bound to.
pub const EXILED: &str = "exiled";

/// ". If it was a creature card, S." after a sentence that ends in the exile of the one target: the exile binds what it
/// took and the sentence after it follows as an `If` over the kind of the object.
pub(super) fn tail(r: &mut Reader<'_>, effects: &mut Vec<Effect>) -> bool {
    let Some(Effect::Exile {
        what: Ref::Target(0),
        bind: bind @ None,
        duration: None,
    }) = effects.last_mut()
    else {
        return false;
    };
    let read = r.attempt(|r| {
        r.need("if it was")?;
        (r.word("a") || r.word("an")).then_some(())?;
        let kind = filter(r, false)?;
        r.punct(',').then_some(())?;
        let condition = Condition::Matches {
            subject: Ref::Bound(EXILED.to_owned()),
            selector: kind,
        };
        let then = conditional::consequence(r, &condition)?;
        r.sentence_end().then_some(())?;
        Some(Effect::If {
            condition,
            then,
            otherwise: Vec::new(),
        })
    });
    let Some(follow) = read else {
        return false;
    };
    *bind = Some(EXILED.to_owned());
    effects.push(follow);
    true
}

/// The exile that binds what it took and the `If` over its kind, as the plain exile and the kind: `None` for any other
/// pair of effects.
pub(super) fn is_exile_kind(effects: &[Effect]) -> Option<(Effect, &Selector, &[Effect])> {
    let [
        Effect::Exile {
            what: what @ Ref::Target(0),
            bind: Some(named),
            duration: None,
        },
        Effect::If {
            condition:
                Condition::Matches {
                    subject: Ref::Bound(subject),
                    selector,
                },
            then,
            otherwise,
        },
        ..,
    ] = effects
    else {
        return None;
    };
    if named != EXILED || subject != EXILED || !otherwise.is_empty() {
        return None;
    }
    let plain = Effect::Exile {
        what: what.clone(),
        bind: None,
        duration: None,
    };
    Some((plain, selector, then))
}

/// "if it was a creature card": the kind of the object the exile took.
pub(super) fn condition_text(kind: &Selector) -> Option<String> {
    let text = filter_text(kind, false)?;
    Some(format!("if it was {} {text}", article(&text)))
}
