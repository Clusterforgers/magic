//! Cards drawn for a characteristic of a target: "Draw cards equal to the power of target creature you control." The
//! renderers word an amount of a characteristic only with "where X is".

use super::super::super::amounts::stat_name;
use super::super::spec_text;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{Ref, Targetable};
use crate::value::Amount;

/// Adds the wording of a spell that draws as many cards as a characteristic of its one target.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Spell { targets, effects } = ability else {
        return;
    };
    let (
        [spec],
        [
            Effect::Draw {
                who: Ref::You,
                count:
                    Amount::Stat {
                        of: Ref::Target(0),
                        stat,
                    },
            },
        ],
    ) = (targets.as_slice(), effects.as_slice())
    else {
        return;
    };
    if !matches!(spec.what, Targetable::Object(_)) {
        return;
    }
    if let (Some(name), Some(target)) = (stat_name(*stat), spec_text(spec)) {
        texts.push(format!("Draw cards equal to the {name} of {target}."));
    }
}
