//! A pact's upkeep line as the last step of its spell: "At the beginning of your next upkeep, pay {1}{W}{W}. If you don't,
//! you lose the game." written on its own line after the spell's other steps (Intervention Pact, Slaughter Pact).

use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::Ref;
use crate::trigger::{Step, TriggerPattern};

/// The upkeep line of a pact, or `None` for any other step.
fn r408_pact_line(effect: &Effect) -> Option<String> {
    let Effect::DelayedTrigger {
        when:
            TriggerPattern::StepBegins {
                step: Step::Upkeep,
                whose: Ref::You,
            },
        effects,
        once: true,
        this_turn: false,
        until: None,
        targets,
    } = effect
    else {
        return None;
    };
    let [
        Effect::Pay {
            who: Ref::You,
            cost,
            times,
            paid,
            unpaid,
        },
    ] = effects.as_slice()
    else {
        return None;
    };
    let [Effect::LoseGame { who: Ref::You, .. }] = unpaid.as_slice() else {
        return None;
    };
    (targets.is_empty() && times.is_one() && paid.is_empty()).then_some(())?;
    let cost = super::costs::text(cost)?;
    Some(format!(
        "At the beginning of your next upkeep, pay {cost}. If you don't, you lose the game."
    ))
}

/// The wordings of a spell that ends with a pact's upkeep line: the other steps as the writers word them, then that line
/// on a line of its own; `None` for any other ability.
pub(super) fn r408_pact_variants(
    ability: &Ability,
    variants: fn(&Ability) -> Vec<String>,
) -> Option<Vec<String>> {
    let Ability::Spell { targets, effects } = ability else {
        return None;
    };
    let (last, rest) = effects.split_last()?;
    let line = r408_pact_line(last)?;
    if rest.is_empty() {
        return Some(vec![line]);
    }
    let twin = Ability::Spell {
        targets: targets.clone(),
        effects: rest.to_vec(),
    };
    Some(
        variants(&twin)
            .into_iter()
            .map(|text| format!("{text}\n{line}"))
            .collect(),
    )
}
