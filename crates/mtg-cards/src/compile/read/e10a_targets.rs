//! Targets that depend on the targets chosen before them, written back: "another target" that may be a player (CR
//! 115.3) and "up to three target creature cards with total mana value 3 or less" (a total over the chosen set).

use super::selectors::{Form, phrase_text, spec_text};
use crate::refs::{Bounds, Selector, TargetSpec, Targetable};
use crate::value::Amount;

/// "Another target", "another target player", "another target creature": none of the earlier targets.
pub(super) fn another_text(what: &Targetable) -> Option<String> {
    Some(match what {
        Targetable::AnyTarget => "another target".to_owned(),
        Targetable::Player => "another target player".to_owned(),
        Targetable::Opponent => "another target opponent".to_owned(),
        Targetable::Object(selector) => {
            format!("another target {}", phrase_text(selector, Form::Target)?)
        }
        _ => return None,
    })
}

/// The targets of `what` within the bounds of `spec`, then "with total mana value 3 (or X) or less".
pub(super) fn total_text(spec: &TargetSpec, what: &Targetable, most: &Amount) -> Option<String> {
    let most = match most {
        Amount::Fixed(n) => n.to_string(),
        Amount::X => "X".to_owned(),
        _ => return None,
    };
    let Targetable::Object(selector) = what else {
        return None;
    };
    // Up to some number, or any number: a total that is reached by one target alone is not worded as a set.
    if spec.bounds.is_one() || spec.bounds.fixed_min() != Some(0) {
        return None;
    }
    // Up to as many as there are of the targets themselves is any number of them.
    let bounds = match &spec.bounds.max {
        Some(Amount::Count(counted)) if same_set(counted, selector) => Bounds::up_to(None),
        _ => spec.bounds.clone(),
    };
    let plain = TargetSpec {
        what: what.clone(),
        bounds,
        ..spec.clone()
    };
    let text = spec_text(&plain)?;
    let total = format!(" with total mana value {most} or less");
    // The total comes before the zone: "creature cards with total mana value 3 or less from your graveyard".
    Some(match text.rfind(" from ") {
        Some(at) => format!("{}{total}{}", &text[..at], &text[at..]),
        None => format!("{text}{total}"),
    })
}

/// Whether two selectors hold the same predicates, in any order.
fn same_set(counted: &Selector, target: &Selector) -> bool {
    counted.0.len() == target.0.len() && counted.0.iter().all(|pred| target.0.contains(pred))
}
