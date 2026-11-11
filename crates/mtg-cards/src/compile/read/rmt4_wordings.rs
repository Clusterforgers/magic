//! Wordings of the rmt4 card pack: an enters trigger of a Bargain card under "if it was bargained".

use crate::ability::Ability;
use crate::refs::{Pred, ZoneKind};
use crate::trigger::TriggerPattern;
use crate::value::Condition;

/// "When ~ enters, if it was bargained, ...": the optional additional cost of the permanent's own spell
/// (`Condition::AdditionalCostPaid`), which the writer words as the gift.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when:
            TriggerPattern::ZoneChange {
                what,
                from: None,
                to: Some(ZoneKind::Battlefield),
            },
        intervening_if: Some(Condition::AdditionalCostPaid),
        ..
    } = ability
    else {
        return;
    };
    if what.0.as_slice() != [Pred::This] {
        return;
    }
    let found: Vec<String> = texts
        .iter()
        .filter_map(|text| {
            text.strip_prefix("When ~ enters, if the gift was promised, ")
                .map(|rest| format!("When ~ enters, if it was bargained, {rest}"))
        })
        .collect();
    for text in found {
        if !texts.contains(&text) {
            texts.push(text);
        }
    }
}
