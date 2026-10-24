//! Half of X tokens: "When ~ enters, create half X Food tokens, rounded up." The renderers have no wording for a
//! halved count of tokens, so the count is written as X and the ending is added.

use super::super::variants;
use super::bare::with_effects;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::value::Amount;

/// Adds the wordings of an ability whose last step creates half of X tokens.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Some((last, before)) = ability.effects().split_last() else {
        return;
    };
    let Effect::CreateToken {
        who,
        token,
        count: Amount::Half { of, up },
        tapped,
        attacking,
        bind,
    } = last
    else {
        return;
    };
    if **of != Amount::X {
        return;
    }
    let mut steps = before.to_vec();
    steps.push(Effect::CreateToken {
        who: who.clone(),
        token: token.clone(),
        count: Amount::X,
        tapped: *tapped,
        attacking: *attacking,
        bind: bind.clone(),
    });
    let Some(plain) = with_effects(ability, steps) else {
        return;
    };
    let rounding = if *up { "rounded up" } else { "rounded down" };
    for text in variants(&plain) {
        let Some(sentence) = text.strip_suffix('.') else {
            continue;
        };
        if let Some((head, tail)) = sentence.rsplit_once("create X ") {
            texts.push(format!("{head}create half X {tail}, {rounding}."));
        }
    }
}
