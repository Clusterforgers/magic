//! Damage that goes both ways: "~ deals damage equal to its power to target creature. That creature deals damage equal
//! to its power to ~." The renderers have no wording for the return damage.

use super::super::super::where_x::equal_to_wordings;
use super::bare::with_effects;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::Ref;
use crate::value::{Amount, StatKind};

/// The power of the object as an amount.
fn power(of: Ref) -> Amount {
    Amount::Stat {
        of,
        stat: StatKind::Power,
    }
}

/// Adds the wordings of damage equal to the power of the source to a target and the same damage back.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let [
        Effect::DealDamage {
            source: first,
            to: Ref::Target(0),
            amount: out,
        },
        Effect::DealDamage {
            source: Some(Ref::Target(0)),
            to: Ref::This,
            amount: back,
        },
    ] = ability.effects()
    else {
        return;
    };
    if !matches!(first, None | Some(Ref::This))
        || *out != power(Ref::This)
        || *back != power(Ref::Target(0))
    {
        return;
    }
    let one = Effect::DealDamage {
        source: None,
        to: Ref::Target(0),
        amount: out.clone(),
    };
    let Some(plain) = with_effects(ability, vec![one]) else {
        return;
    };
    for text in equal_to_wordings(&plain) {
        if let Some(sentence) = text.strip_suffix('.') {
            texts.push(format!(
                "{sentence}. That creature deals damage equal to its power to ~."
            ));
        }
    }
}
