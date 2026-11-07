//! Block triggers about the other creature, written back: "Whenever ~ blocks a creature with flying" and "Whenever ~
//! becomes blocked by an artifact creature" (one trigger for each pair); "one or more" is `g9c_blocks`.

use super::selectors::{Form, phrase_text};
use super::triggers::{article, subject_text};
use crate::refs::{Pred, Ref, Selector};
use crate::trigger::TriggerPattern;

/// The one object of a subject selector: the card or what it is attached to.
fn g9a_object(what: &Selector) -> Option<Ref> {
    match what.0.as_slice() {
        [Pred::This] => Some(Ref::This),
        [Pred::Is(Ref::Attached)] => Some(Ref::Attached),
        _ => None,
    }
}

/// "Whenever ~ blocks a creature with flying", "Whenever ~ becomes blocked by an artifact creature": a block pattern
/// with a blocker filter, one trigger for each pair.
pub(super) fn g9a_pair_text(pattern: &TriggerPattern) -> Option<String> {
    let TriggerPattern::BecomesBlocked {
        what,
        by,
        once: false,
    } = pattern
    else {
        return None;
    };
    if by.0.is_empty() {
        return None;
    }
    if by.0 == [Pred::This] {
        let attacker = phrase_text(what, Form::Subject)?;
        return Some(format!(
            "Whenever ~ blocks {} {attacker}",
            article(&attacker)
        ));
    }
    g9a_object(what)?;
    // The noun "creature" comes last: "artifact creature".
    let mut ordered = by.clone();
    ordered
        .0
        .sort_by_key(|pred| *pred == Pred::Type(mtg_core::card::CardType::Creature));
    let blocker =
        phrase_text(by, Form::Subject).or_else(|| phrase_text(&ordered, Form::Subject))?;
    Some(format!(
        "Whenever {} becomes blocked by {} {blocker}",
        subject_text(what)?,
        article(&blocker)
    ))
}
