//! A counter of an ability on the stack: "Counter target activated ability.", "Counter target triggered ability." and
//! "Counter target activated or triggered ability." The ability is worded as the counter of a spell, whose sentence the
//! renderers write for any cost and timing.

use super::super::variants;
use super::bare::with_effects;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{AbilityKind, Pred, Ref, Selector, TargetSpec, Targetable};

/// The words the counter of a spell is written with.
const SPELL: &str = "Counter target spell.";

/// The counter of an activated ability, of a triggered one, and of either.
fn wording(selector: &Selector) -> Option<&'static str> {
    let activated = Pred::Ability(AbilityKind::Activated);
    let triggered = Pred::Ability(AbilityKind::Triggered);
    match selector.0.as_slice() {
        [one] if *one == activated => Some("Counter target activated ability."),
        [one] if *one == triggered => Some("Counter target triggered ability."),
        [Pred::AnyOf(kinds)] if *kinds == [activated, triggered] => {
            Some("Counter target activated or triggered ability.")
        }
        _ => None,
    }
}

/// Adds the wordings of a spell or an activated ability whose one effect counters its one target, an ability.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let (Ability::Spell { targets, effects }
    | Ability::Activated {
        targets, effects, ..
    }) = ability
    else {
        return;
    };
    let [
        TargetSpec {
            what: Targetable::Object(selector),
            bounds,
            divide: None,
        },
    ] = targets.as_slice()
    else {
        return;
    };
    let counter = Effect::Counter {
        what: Ref::Target(0),
    };
    let Some(said) =
        wording(selector).filter(|_| bounds.is_one() && effects.as_slice() == [counter.clone()])
    else {
        return;
    };
    let mut stand_in = ability.clone();
    if let Ability::Spell { targets, .. } | Ability::Activated { targets, .. } = &mut stand_in {
        targets[0].what = Targetable::Object(Selector(vec![Pred::Spell]));
    }
    let Some(stand_in) = with_effects(&stand_in, vec![counter]) else {
        return;
    };
    let found: Vec<String> = variants(&stand_in)
        .iter()
        .filter(|text| text.ends_with(SPELL))
        .map(|text| text.replacen(SPELL, said, 1))
        .collect();
    texts.extend(found);
}
