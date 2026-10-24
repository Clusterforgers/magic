//! A return at the next end step that the card says first: "Exile target creature. At the beginning of the next end
//! step, return it to the battlefield tapped under its owner's control." The renderers put the time last.

use crate::ability::Ability;
use crate::effect::Effect;
use crate::trigger::{Step, TriggerPattern};

/// Adds the wordings that open the last sentence with the time of the delayed trigger.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let delays = ability.effects().iter().any(|effect| {
        matches!(
            effect,
            Effect::DelayedTrigger {
                when: TriggerPattern::StepBegins {
                    step: Step::End,
                    ..
                },
                ..
            }
        )
    });
    if !delays {
        return;
    }
    let found: Vec<String> = texts.iter().filter_map(|text| time_first(text)).collect();
    texts.extend(found);
}

/// "A. Return it ... at the beginning of the next end step." as "A. At the beginning of the next end step, return it ...".
fn time_first(text: &str) -> Option<String> {
    let time = " at the beginning of the next end step.";
    let body = text.strip_suffix(time)?;
    let (before, last) = body.rsplit_once(". ")?;
    let (verb, rest) = last.split_once(' ')?;
    Some(format!(
        "{before}. At the beginning of the next end step, {} {rest}.",
        verb.to_lowercase()
    ))
}
