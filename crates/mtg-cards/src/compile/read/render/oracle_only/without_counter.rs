//! A bounce of each object that holds no counter of a kind: "Return each creature without a +1/+1 counter on it to its
//! owner's hand." The renderers word a bounce of a group only after "all" and without a negated counter.

use super::super::super::counters::kind_text;
use super::super::super::selectors::{Form, phrase_text};
use crate::ability::Ability;
use crate::effect::{Destination, Effect};
use crate::refs::{Pred, Ref, Selector};

/// Adds the wording of a spell that returns each object without a counter to its owner's hand.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Spell { targets, effects } = ability else {
        return;
    };
    let [
        Effect::Move {
            what: Ref::Each(selector),
            to: Destination::Hand,
        },
    ] = effects.as_slice()
    else {
        return;
    };
    if !targets.is_empty() {
        return;
    }
    let (kept, lacking): (Vec<&Pred>, Vec<&Pred>) = selector
        .0
        .iter()
        .partition(|pred| !matches!(pred, Pred::Not(inner) if matches!(**inner, Pred::HasCounter(_))));
    let [Pred::Not(inner)] = lacking.as_slice() else {
        return;
    };
    let Pred::HasCounter(kind) = &**inner else {
        return;
    };
    let rest = Selector(kept.into_iter().cloned().collect());
    if let (Some(each), Some(kind)) = (phrase_text(&rest, Form::Each), kind_text(kind)) {
        texts.push(format!(
            "Return each {each} without a {kind} counter on it to its owner's hand."
        ));
    }
}
