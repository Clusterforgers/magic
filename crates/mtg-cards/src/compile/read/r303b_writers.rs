//! r303b writers: "You may reveal this card from your opening hand. If you do, at the beginning of the first upkeep, ...".

use crate::ability::Ability;
use crate::refs::Ref;
use crate::trigger::{Step, TriggerPattern};

/// The wordings of an opening-hand reveal whose delayed trigger waits for the first upkeep: the effects as a spell
/// says them, after the reveal.
pub(super) fn r303b_whole(ability: &Ability) -> Option<Vec<String>> {
    let Ability::OpeningReveal { when, effects } = ability else {
        return None;
    };
    let TriggerPattern::StepBegins {
        step: Step::Upkeep,
        whose: Ref::EachPlayer,
    } = when
    else {
        return None;
    };
    let spell = Ability::Spell {
        targets: Vec::new(),
        effects: effects.clone(),
    };
    let texts: Vec<String> = super::render::variants(&spell)
        .into_iter()
        .filter_map(|text| {
            let mut chars = text.chars();
            let first = chars.next()?;
            Some(format!(
                "You may reveal this card from your opening hand. If you do, at the beginning of the first upkeep, {}{}",
                first.to_lowercase(),
                chars.as_str()
            ))
        })
        .collect();
    (!texts.is_empty()).then_some(texts)
}
