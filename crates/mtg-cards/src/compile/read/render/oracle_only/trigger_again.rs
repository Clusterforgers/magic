//! A rule that triggers abilities again: "If a permanent entering causes a triggered ability of a permanent you control
//! to trigger, that ability triggers an additional time." The renderers have no wording for the rule.

use crate::ability::Ability;
use crate::modify::GameRule;
use crate::refs::{Pred, Selector, ZoneKind};
use crate::trigger::TriggerPattern;
use mtg_core::card::CardType;

/// The events that cause the trigger, and the words for them.
fn cause(pattern: &TriggerPattern) -> Option<&'static str> {
    let creature = Selector(vec![Pred::Type(CardType::Creature)]);
    match pattern {
        TriggerPattern::ZoneChange {
            what,
            from: None,
            to: Some(ZoneKind::Battlefield),
        } if what.0.is_empty() => Some("a permanent entering"),
        TriggerPattern::ZoneChange {
            what,
            from: None,
            to: Some(ZoneKind::Battlefield),
        } if *what == creature => Some("a creature entering"),
        TriggerPattern::Attacks { what } if *what == creature => Some("a creature attacking"),
        _ => None,
    }
}

/// Adds the wording of a rule that makes the triggered abilities of the controller's permanents trigger again.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Rule {
        rule: GameRule::TriggerAgain { cause: event },
        condition: None,
        ..
    } = ability
    else {
        return;
    };
    let Some(cause) = cause(event) else {
        return;
    };
    texts.push(format!(
        "If {cause} causes a triggered ability of a permanent you control to trigger, that ability triggers an additional time."
    ));
}
