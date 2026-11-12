//! "Whenever you activate an ability of a creature": an activation of any ability of a permanent, a mana ability
//! included (`Action::ActivatedAny`, CR 605.3b), as the card words it.

use super::tokens::Reader;
use super::triggers::{player_verb, subject, subject_text};
use crate::did::Action;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::trigger::TriggerPattern;

/// "an ability of an Elemental" after "you activate": the source is a permanent (CR 109.2), so it is on the battlefield.
pub(super) fn event(r: &mut Reader<'_>, by: Ref) -> Option<TriggerPattern> {
    r.need("an ability of")?;
    let mut what = subject(r)?;
    if what.is_empty()
        || what
            .0
            .iter()
            .any(|pred| matches!(pred, Pred::This | Pred::In(_)))
    {
        return None;
    }
    // "that isn't a mana ability" belongs to `Action::Activated`.
    if r.peek_word().is_some_and(|word| word == "that") {
        return None;
    }
    what.0.push(Pred::In(ZoneKind::Battlefield));
    Some(TriggerPattern::Did {
        action: Action::ActivatedAny,
        who: by,
        what,
    })
}

/// The words of [`event`]: "Whenever an opponent activates an ability of an artifact".
pub(super) fn text(who: &Ref, what: &Selector) -> Option<String> {
    let [rest @ .., Pred::In(ZoneKind::Battlefield)] = what.0.as_slice() else {
        return None;
    };
    if rest.is_empty() {
        return None;
    }
    Some(format!(
        "Whenever {} an ability of {}",
        player_verb(who, "activate")?,
        subject_text(&Selector(rest.to_vec()))?
    ))
}
