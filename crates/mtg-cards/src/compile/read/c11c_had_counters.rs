//! "if it had counters on it": an intervening condition on the counters the object of a leave or dies event held as it
//! left, read from its last known information (`Amount::AnyCounters` or `Amount::Counters` of `Triggering`).

use super::counters;
use super::tokens::Reader;
use crate::refs::Ref;
use crate::value::{Amount, Cmp, Condition, CounterKind};

/// "it had counters on it", "it had one or more counters on it", "it had one or more +1/+1 counters on it".
pub fn c11c_had(r: &mut Reader<'_>) -> Option<Condition> {
    r.need("it had")?;
    let more = r.words("one or more")
        || r.attempt(|r| (r.number()? == 1 && r.words("or more")).then_some(()))
            .is_some();
    let counter = if more {
        r.attempt(counters::kind)
    } else {
        None
    };
    r.need("counters")?;
    r.need("on it")?;
    let left = match counter {
        Some(counter) => Amount::Counters {
            on: Ref::Triggering,
            counter,
        },
        None => Amount::AnyCounters(Ref::Triggering),
    };
    Some(Condition::Compare {
        left,
        op: Cmp::Ge,
        right: Amount::Fixed(1),
    })
}

/// The kind the condition counts, `None` for counters of any kind; nothing for another condition.
fn c11c_kind(condition: &Condition) -> Option<Option<&CounterKind>> {
    let Condition::Compare {
        left,
        op: Cmp::Ge,
        right: Amount::Fixed(1),
    } = condition
    else {
        return None;
    };
    match left {
        Amount::AnyCounters(Ref::Triggering) => Some(None),
        Amount::Counters {
            on: Ref::Triggering,
            counter,
        } => Some(Some(counter)),
        _ => None,
    }
}

/// The longer wording "it had one or more counters on it" of each of `texts` that says "it had counters on it".
pub fn c11c_had_wordings(texts: &[String]) -> Vec<String> {
    texts
        .iter()
        .filter(|text| text.contains("it had counters on it"))
        .map(|text| text.replace("it had counters on it", "it had one or more counters on it"))
        .collect()
}

/// The wording of the condition: "it had counters on it" for any kind, "it had one or more +1/+1 counters on it".
pub fn c11c_had_texts(condition: &Condition) -> Vec<String> {
    match c11c_kind(condition) {
        Some(None) => vec!["it had counters on it".to_owned()],
        Some(Some(counter)) => counters::kind_text(counter)
            .map(|kind| format!("it had one or more {kind} counters on it"))
            .into_iter()
            .collect(),
        None => Vec::new(),
    }
}
