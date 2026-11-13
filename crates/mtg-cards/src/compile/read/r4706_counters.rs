//! Counters a player puts: "Whenever you put one or more counters on another creature" and "Whenever an opponent puts
//! one or more +1/+1 counters on a creature", a batch of counters caused by that player (`Caused`), each permanent an
//! event of its own; and "put the same number and kind of counters on ~" (`PutCountersOf` with `put`).

use mtg_core::card::Subtype;

use crate::effect::Effect;
use crate::refs::{Pred, Ref, Selector};
use crate::trigger::TriggerPattern;
use crate::value::Condition;

use super::counters;
use super::tokens::Reader;
use super::triggers::subject;

/// The trigger after the player and the verb "put" or "puts": "one or more counters on another creature".
pub(super) fn r4706_put_event(r: &mut Reader<'_>, by: Ref) -> Option<TriggerPattern> {
    if !matches!(by, Ref::You | Ref::Opponents) {
        return None;
    }
    r.need("one or more")?;
    let counter = if r.word("counters") {
        None
    } else {
        let counter = counters::kind(r)?;
        r.need("counters")?;
        Some(counter)
    };
    r.need("on")?;
    let what = subject(r)?;
    Some(TriggerPattern::Caused {
        inner: Box::new(TriggerPattern::Batch(Box::new(
            TriggerPattern::CountersPut {
                what,
                counter,
                each: true,
            },
        ))),
        by: Some(by),
        source: Selector::default(),
    })
}

/// The words of a trigger `r4706_put_event` reads.
pub(super) fn r4706_put_text(pattern: &TriggerPattern) -> Option<String> {
    let TriggerPattern::Caused {
        inner,
        by: Some(by),
        source,
    } = pattern
    else {
        return None;
    };
    let TriggerPattern::Batch(inner) = inner.as_ref() else {
        return None;
    };
    let TriggerPattern::CountersPut {
        what,
        counter,
        each: true,
    } = inner.as_ref()
    else {
        return None;
    };
    if !source.is_empty() || what.is_empty() {
        return None;
    }
    let who = match by {
        Ref::You => "you put",
        Ref::Opponents => "an opponent puts",
        _ => return None,
    };
    let kind = match counter {
        Some(counter) => format!("{} ", counters::kind_text(counter)?),
        None => String::new(),
    };
    Some(format!(
        "Whenever {who} one or more {kind}counters on {}",
        super::triggers::subject_text(what)?
    ))
}

/// "it's not a Kree" after "if": the object of the event lacks the subtype.
pub(super) fn r4706_not_subtype(r: &mut Reader<'_>) -> Option<Condition> {
    r.attempt(|r| {
        r.need("it's not")?;
        (r.word("a") || r.word("an")).then_some(())?;
        let kind = super::selectors::subtype(r)?;
        Some(Condition::Not(Box::new(Condition::Matches {
            subject: Ref::Triggering,
            selector: Selector(vec![Pred::Subtype(Subtype(kind))]),
        })))
    })
}

/// The words of a condition `r4706_not_subtype` reads.
pub(super) fn r4706_not_subtype_text(condition: &Condition) -> Option<String> {
    let Condition::Not(inner) = condition else {
        return None;
    };
    let Condition::Matches {
        subject: Ref::Triggering,
        selector,
    } = inner.as_ref()
    else {
        return None;
    };
    let [Pred::Subtype(Subtype(kind))] = selector.0.as_slice() else {
        return None;
    };
    Some(format!(
        "it's not {} {kind}",
        super::triggers::article(kind)
    ))
}

/// "put the same number and kind of counters on ~": the counters the triggering event put.
pub(super) fn r4706_same_counters(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    r.need("put the same number and kind of counters on")?;
    let on = counters::place(r)?;
    Some(vec![Effect::PutCountersOf {
        on,
        of: Ref::Triggering,
        each: None,
        one_kind: false,
        per_kind: None,
        put: true,
    }])
}
