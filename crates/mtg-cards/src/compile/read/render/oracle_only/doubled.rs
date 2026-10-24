//! The doubled power of a creature: "Double the power of target creature until end of turn." The renderers write the
//! bonus as an amount: "gets +X/+0, where X is that creature's power".

use super::super::spec_text;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::modify::{Duration, Modification};
use crate::refs::{Ref, Targetable};
use crate::value::{Amount, StatKind};

/// Adds the wording of a spell that gives its one target a bonus to power of its own power.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Spell { targets, effects } = ability else {
        return;
    };
    let (
        [spec],
        [
            Effect::Continuous {
                affected: Ref::Target(0),
                mods,
                duration: Duration::UntilEndOfTurn,
            },
        ],
    ) = (targets.as_slice(), effects.as_slice())
    else {
        return;
    };
    let [
        Modification::ModifyPT {
            power:
                Amount::Stat {
                    of: Ref::Target(0),
                    stat: StatKind::Power,
                },
            toughness: Amount::Fixed(0),
        },
    ] = mods.as_slice()
    else {
        return;
    };
    if !matches!(spec.what, Targetable::Object(_)) {
        return;
    }
    if let Some(target) = spec_text(spec) {
        texts.push(format!("Double the power of {target} until end of turn."));
    }
}
