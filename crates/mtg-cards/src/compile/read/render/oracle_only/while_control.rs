//! An attack trigger that works while the player controls no other creature of a kind: "Whenever this creature attacks
//! while you don't control another Dinosaur, put a stun counter on it." The renderers write the condition after the
//! event as an intervening "if".

use super::super::super::subtypes::plural_subtype;
use crate::ability::Ability;
use crate::refs::{Pred, Ref};
use crate::trigger::TriggerPattern;
use crate::value::{Amount, Cmp, Condition};

/// Adds the wording that puts "while you don't control another X" in place of "if you control no other Xs".
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when: TriggerPattern::Attacks { .. },
        intervening_if:
            Some(Condition::Compare {
                left: Amount::Count(selector),
                op: Cmp::Eq,
                right: Amount::Fixed(0),
            }),
        ..
    } = ability
    else {
        return;
    };
    let [
        Pred::Subtype(kind),
        Pred::Other,
        Pred::ControlledBy(Ref::You),
    ] = selector.0.as_slice()
    else {
        return;
    };
    let (from, to) = (
        format!(", if you control no other {}, ", plural_subtype(&kind.0)),
        format!(" while you don't control another {}, ", kind.0),
    );
    let found: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(&from))
        .map(|text| text.replacen(&from, &to, 1))
        .collect();
    texts.extend(found);
}
