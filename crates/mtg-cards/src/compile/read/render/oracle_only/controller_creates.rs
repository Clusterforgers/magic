//! The controller of the creature of an event may create a token: "Whenever a Sliver deals combat damage to a player,
//! its controller may create a 1/1 colorless Sliver creature token." The renderers word the creation of the controller
//! without the choice.

use super::super::variants;
use super::bare::with_effects;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::Ref;

/// The controller as the renderers word it and as the oracle text does, before the creation.
const CREATES: (&str, &str) = (
    ", that permanent's controller creates ",
    ", its controller may create ",
);

/// Adds the wordings of a trigger whose one effect is a creation of tokens the controller of the object of the event may
/// make.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered { effects, .. } = ability else {
        return;
    };
    let controller = Ref::ControllerOf(Box::new(Ref::Triggering));
    let [
        Effect::May {
            who: chooser,
            effects: inner,
        },
    ] = effects.as_slice()
    else {
        return;
    };
    let [Effect::CreateToken { who, .. }] = inner.as_slice() else {
        return;
    };
    if *chooser != controller || *who != controller {
        return;
    }
    let Some(bare) = with_effects(ability, inner.clone()) else {
        return;
    };
    let (said, wanted) = CREATES;
    for text in variants(&bare) {
        if text.contains(said) {
            texts.push(text.replacen(said, wanted, 1));
        }
    }
}
