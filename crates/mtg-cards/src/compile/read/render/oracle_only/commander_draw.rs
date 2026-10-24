//! A draw that grows with the casts of a commander: "Target player draws X cards plus an additional card for each time
//! they've cast a commander from the command zone this game."

use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{Ref, TargetSpec, Targetable};
use crate::value::Amount;

/// Adds the wording of a spell that makes one player draw X cards and one more for each cast of their commander.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Spell { targets, effects } = ability else {
        return;
    };
    let [
        TargetSpec {
            what: Targetable::Player,
            bounds,
            divide: None,
        },
    ] = targets.as_slice()
    else {
        return;
    };
    let [
        Effect::Draw {
            who: Ref::Target(0),
            count: Amount::Sum(parts),
        },
    ] = effects.as_slice()
    else {
        return;
    };
    if bounds.is_one() && *parts == [Amount::X, Amount::CommanderCasts(Ref::Target(0))] {
        texts.push("Target player draws X cards plus an additional card for each time they've cast a commander from the command zone this game.".to_owned());
    }
}
