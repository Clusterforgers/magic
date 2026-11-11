//! A counter that brings a permanent to a count as a trigger event: "When the third plan counter is put on this
//! enchantment" triggers once, when counters of the kind put on it take it from below the number to the number or more.

use super::counters;
use super::tokens::Reader;
use super::triggers::{subject, subject_text};
use crate::trigger::TriggerPattern;

/// The ordinals the reader takes, from "first".
const R289_ORDINALS: [&str; 10] = [
    "first", "second", "third", "fourth", "fifth", "sixth", "seventh", "eighth", "ninth", "tenth",
];

/// "the third plan counter is put on ~".
pub(super) fn r289_nth_counter(r: &mut Reader<'_>) -> Option<TriggerPattern> {
    r.need("the")?;
    let word = r.take_word()?;
    let at = R289_ORDINALS.iter().position(|ordinal| *ordinal == word)?;
    let counter = counters::trigger_kind(r)?;
    r.need("is put on")?;
    let what = subject(r)?;
    Some(TriggerPattern::CountersPut {
        what,
        counter: Some(counter),
        each: false,
        reaching: Some(u32::try_from(at).ok()? + 1),
    })
}

/// The head `r289_nth_counter` reads, written back.
pub(super) fn r289_nth_counter_text(pattern: &TriggerPattern) -> Option<String> {
    let TriggerPattern::CountersPut {
        what,
        counter: Some(counter),
        each: false,
        reaching: Some(number),
    } = pattern
    else {
        return None;
    };
    let ordinal = R289_ORDINALS.get(usize::try_from(*number).ok()?.checked_sub(1)?)?;
    Some(format!(
        "When the {ordinal} {} counter is put on {}",
        counters::kind_text(counter)?,
        subject_text(what)?
    ))
}
