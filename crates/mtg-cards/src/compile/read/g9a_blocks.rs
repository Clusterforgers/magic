//! Block triggers about the other creature, written back: "Whenever ~ blocks a creature with flying" and "Whenever ~
//! becomes blocked by an artifact creature" (one trigger for each pair), and "Whenever ~ blocks or becomes blocked by
//! one or more black creatures" (once, the other creature checked as it triggers).

use super::selectors::{Form, phrase_text};
use super::triggers::{article, subject_text};
use crate::refs::{Pred, Ref, Selector};
use crate::trigger::TriggerPattern;
use crate::value::{Amount, Cmp, Condition};

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
    let TriggerPattern::BecomesBlocked { what, by } = pattern else {
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

/// The head of a block trigger whose condition, checked only as it triggers, is that one or more creatures of a kind
/// block or are blocked by the subject: "Whenever ~ blocks or becomes blocked by one or more black creatures,".
pub(super) fn g9a_one_or_more_head(
    when: &TriggerPattern,
    condition: Option<&Condition>,
) -> Option<String> {
    let Some(Condition::AtTrigger(inner)) = condition else {
        return None;
    };
    let Condition::Compare {
        left: Amount::Count(found),
        op: Cmp::Ge,
        right: Amount::Fixed(1),
    } = inner.as_ref()
    else {
        return None;
    };
    let (subject, verb) = match when {
        TriggerPattern::Blocks { what } => (what, "blocks"),
        TriggerPattern::BecomesBlocked { what, by } if by.0.is_empty() => {
            (what, "becomes blocked by")
        }
        TriggerPattern::AnyOf(list) => match list.as_slice() {
            [
                TriggerPattern::Blocks { what },
                TriggerPattern::BecomesBlocked { what: second, by },
            ]
            | [
                TriggerPattern::BecomesBlocked { what: second, by },
                TriggerPattern::Blocks { what },
            ] if what == second && by.0.is_empty() => (what, "blocks or becomes blocked by"),
            _ => return None,
        },
        _ => return None,
    };
    let who = g9a_object(subject)?;
    let paired = Pred::AnyOf(vec![Pred::BlockedBy(who.clone()), Pred::Blocks(who)]);
    let at = found.0.iter().position(|pred| *pred == paired)?;
    let mut rest = found.clone();
    rest.0.remove(at);
    let others = phrase_text(&rest, Form::All)?;
    Some(format!(
        "Whenever {} {verb} one or more {others},",
        subject_text(subject)?
    ))
}
