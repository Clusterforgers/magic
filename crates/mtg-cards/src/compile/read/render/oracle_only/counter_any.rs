//! A counter of anything on the stack: "Counter target spell, activated ability, or triggered ability."

use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{AbilityKind, Pred, Ref, Selector, TargetSpec, Targetable};

/// Adds the wording of a spell that counters its one target, a spell or an activated or triggered ability.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Spell { targets, effects } = ability else {
        return;
    };
    let [
        TargetSpec {
            what,
            bounds,
            divide: None,
        },
    ] = targets.as_slice()
    else {
        return;
    };
    let anything = Targetable::Object(Selector(vec![Pred::AnyOf(vec![
        Pred::Spell,
        Pred::Ability(AbilityKind::Activated),
        Pred::Ability(AbilityKind::Triggered),
    ])]));
    if *what == anything
        && bounds.is_one()
        && matches!(
            effects.as_slice(),
            [Effect::Counter {
                what: Ref::Target(0)
            }]
        )
    {
        texts.push("Counter target spell, activated ability, or triggered ability.".to_owned());
    }
}
