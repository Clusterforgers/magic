//! `Effect::R122NoteCounters` and `Destination::R122BattlefieldNoted`: a resolution notes the number and kind of
//! counters on an object ("Note the number and kind of counters that were on that creature"), and a later step, also of
//! a delayed trigger it sets up (CR 603.7c), puts a card onto the battlefield tapped under its owner's control with them
//! (CR 122.6). The note travels as bound names, one per kind: `?r122noted:<name>:<index>:<count>:<kind as JSON>`.

use mtg_cards::effect::Effect;
use mtg_cards::value::CounterKind;

use super::wrong_effect;
use crate::resolve::{Bindings, Cx, Next, ResolveError, Run};

/// The prefix of a bound name that carries a noted kind of counters.
pub(crate) const R122_NOTED: &str = "?r122noted:";

/// Notes the counters on the first object the effect names that is still where it was named; none noted when it is
/// gone or has none.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::R122NoteCounters { what, bind } = effect else {
        return Err(wrong_effect("R122NoteCounters"));
    };
    let counters: Vec<(CounterKind, u32)> = cx
        .scope
        .live_objects(what)?
        .into_iter()
        .find_map(|object| object.permanent())
        .map(|permanent| {
            permanent
                .counters()
                .filter(|(_, count)| *count > 0)
                .map(|(kind, count)| (kind.clone(), count))
                .collect()
        })
        .unwrap_or_default();
    let bound = counters
        .iter()
        .enumerate()
        .filter_map(|(index, (kind, count))| {
            let kind = serde_json::to_string(kind).ok()?;
            Some((
                format!("{R122_NOTED}{bind}:{index}:{count}:{kind}"),
                Vec::new(),
            ))
        })
        .collect();
    Ok(Run::Done {
        events: Vec::new(),
        bound,
        snapshots: Vec::new(),
        next: Next::Sibling,
    })
}

/// The counters noted under `name`, each kind with its number, in the order they were noted.
pub(crate) fn r122_noted(bindings: &Bindings, name: &str) -> Vec<(CounterKind, u32)> {
    let prefix = format!("{R122_NOTED}{name}:");
    let mut noted: Vec<(usize, CounterKind, u32)> = bindings
        .keys()
        .filter_map(|key| {
            let rest = key.strip_prefix(&prefix)?;
            let (index, rest) = rest.split_once(':')?;
            let (count, kind) = rest.split_once(':')?;
            Some((
                index.parse().ok()?,
                serde_json::from_str(kind).ok()?,
                count.parse().ok()?,
            ))
        })
        .collect();
    noted.sort_by_key(|(index, _, _)| *index);
    noted
        .into_iter()
        .map(|(_, kind, count)| (kind, count))
        .collect()
}

#[cfg(test)]
mod tests {
    //! Goal: a note written as bound names reads back as the same kinds and numbers in order, and other names are not
    //! read.
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[rstest::rstest]
    #[case::none(vec![])]
    #[case::one(vec![(CounterKind::PlusOnePlusOne, 2)])]
    #[case::several(vec![
        (CounterKind::PlusOnePlusOne, 3),
        (CounterKind::Other("-0/-1".into()), 1),
        (CounterKind::Other("time".into()), 12),
    ])]
    fn r122_a_note_reads_back(#[case] counters: Vec<(CounterKind, u32)>) {
        let mut bindings = Bindings::new();
        for (index, (kind, count)) in counters.iter().enumerate() {
            let kind = serde_json::to_string(kind).unwrap();
            bindings.insert(
                format!("{R122_NOTED}noted:{index}:{count}:{kind}"),
                Vec::new(),
            );
            bindings.insert(format!("{R122_NOTED}other:{index}:9:{kind}"), Vec::new());
        }
        bindings.insert("noted".into(), Vec::new());
        assert_eq!(r122_noted(&bindings, "noted"), counters);
        assert_eq!(r122_noted(&bindings, "else"), Vec::new());
    }
}
