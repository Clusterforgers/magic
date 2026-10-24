//! Tokens as many as the targets: "Return up to three target land cards from your graveyard to the battlefield tapped.
//! Create that many 4/2 green Plant Warrior creature tokens with reach." The renderers have no wording for a count of
//! targets, so the count is written as X and the words are changed.

use super::super::variants;
use super::bare::with_effects;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::value::Amount;

/// Adds the wordings of an ability whose last step creates one token for each target chosen.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Some((last, before)) = ability.effects().split_last() else {
        return;
    };
    let Effect::CreateToken {
        who,
        token,
        count: Amount::TargetCount(0),
        tapped,
        attacking,
        bind,
    } = last
    else {
        return;
    };
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
    for text in variants(&plain) {
        if let Some((head, tail)) = text.rsplit_once("reate X ") {
            texts.push(format!("{head}reate that many {tail}"));
        }
    }
}
