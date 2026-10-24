//! Two cards returned from a graveyard in one sentence: "Return target creature card and up to one target Cleric,
//! Rogue, Warrior, or Wizard creature card from your graveyard to the battlefield." The renderers write a sentence for
//! each target.

use crate::ability::Ability;
use crate::effect::{Destination, Effect};
use crate::refs::Ref;

/// The words that end the first return and begin the second, as the renderers write them.
const JOINTS: [&str; 2] = [
    " from your graveyard to the battlefield. Return up to one target ",
    " from your graveyard to the battlefield. Then return up to one target ",
];

/// Whether the effect puts the target with this index onto the battlefield.
fn returns(effect: &Effect, index: u8) -> bool {
    matches!(
        effect,
        Effect::Move { what: Ref::Target(found), to: Destination::Battlefield { .. } } if *found == index
    )
}

/// Adds the wordings of two returns to the battlefield from one graveyard that share their ending.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let [first, second] = ability.effects() else {
        return;
    };
    if !returns(first, 0) || !returns(second, 1) {
        return;
    }
    let mut said = Vec::new();
    for text in texts.iter() {
        for joint in JOINTS {
            if text.contains(joint) {
                said.push(text.replacen(joint, " and up to one target ", 1));
            }
        }
    }
    texts.extend(said);
}
