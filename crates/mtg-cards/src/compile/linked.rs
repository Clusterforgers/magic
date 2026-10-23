//! The two-trigger exile of Oblivion Ring, Journey to Nowhere and Fiend Hunter: "When ~ enters, exile target ... When ~
//! leaves the battlefield, return the exiled card to the battlefield under its owner's control." is the exile that lasts
//! until the source leaves the battlefield, so the second trigger is read into the first.

use super::Pending;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::modify::Duration;
use crate::refs::{Pred, ZoneKind};
use crate::trigger::TriggerPattern;

/// What the trigger that returns the exiled card says after it names the card.
const RETURN_TAIL: &str =
    " leaves the battlefield, return the exiled card to the battlefield under its owner's control.";

/// Whether the clause is "When ~ leaves the battlefield, return the exiled card ...", the card named by its name or as
/// "this enchantment".
fn is_return_trigger(text: &str) -> bool {
    text.strip_prefix("When ")
        .and_then(|rest| rest.strip_suffix(RETURN_TAIL))
        .is_some_and(|subject| subject == "~" || subject.starts_with("this "))
}

/// Whether the ability triggers when the card itself enters the battlefield.
fn enters_itself(ability: &Ability) -> bool {
    matches!(
        ability,
        Ability::Triggered {
            when: TriggerPattern::ZoneChange { what, from: None, to: Some(ZoneKind::Battlefield) },
            intervening_if: None,
            ..
        } if what.0 == [Pred::This]
    )
}

/// Makes the first plain exile among the effects last until the source leaves the battlefield; whether there was one.
fn lasts_until_it_leaves(effects: &mut [Effect]) -> bool {
    for effect in effects {
        match effect {
            Effect::Exile {
                bind: None,
                duration: duration @ None,
                ..
            } => {
                *duration = Some(Duration::WhileSourceOnBattlefield);
                return true;
            }
            Effect::May { effects, .. } => {
                if lasts_until_it_leaves(effects) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

/// Reads the trigger that returns the exiled card into the trigger of the same card that exiled it: the exile then lasts
/// until the card leaves the battlefield and the returning trigger is part of it.
pub(super) fn exile_pair(pending: &mut [Pending]) {
    let Some(second) = pending.iter().position(|clause| {
        clause.ability.is_none() && !clause.read && is_return_trigger(&clause.text)
    }) else {
        return;
    };
    let linked = pending[..second]
        .iter_mut()
        .filter_map(|clause| clause.ability.as_mut())
        .filter(|ability| enters_itself(ability))
        .find_map(|ability| match ability {
            Ability::Triggered { effects, .. } => lasts_until_it_leaves(effects).then_some(()),
            _ => None,
        });
    if linked.is_some() {
        pending[second].read = true;
    }
}
