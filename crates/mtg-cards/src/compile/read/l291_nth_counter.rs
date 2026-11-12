//! "When the fourth plan counter is put on this enchantment": the counters of a kind on the permanent reach a number
//! (`TriggerPattern::NthCounterPut`).

use super::counters::{kind, kind_text};
use super::tokens::Reader;
use crate::refs::Pred;
use crate::trigger::TriggerPattern;

/// The ordinals a card counts its counters with, from the first.
const ORDINALS: [&str; 10] = [
    "first", "second", "third", "fourth", "fifth", "sixth", "seventh", "eighth", "ninth", "tenth",
];

/// "the fourth plan counter is put on ~", after "When".
pub(super) fn event(r: &mut Reader<'_>) -> Option<TriggerPattern> {
    r.word("the").then_some(())?;
    let number = ORDINALS.iter().position(|word| r.word(word))?;
    let counter = kind(r)?;
    r.need("counter is put on")?;
    let what = super::triggers::subject(r)?;
    (what.0 == [Pred::This]).then_some(TriggerPattern::NthCounterPut {
        counter,
        number: u32::try_from(number).ok()? + 1,
    })
}

/// The words of [`event`]: "When the fourth plan counter is put on ~".
pub(super) fn text(pattern: &TriggerPattern) -> Option<String> {
    let TriggerPattern::NthCounterPut { counter, number } = pattern else {
        return None;
    };
    let ordinal = ORDINALS.get(usize::try_from(number.checked_sub(1)?).ok()?)?;
    Some(format!(
        "When the {ordinal} {} counter is put on ~",
        kind_text(counter)?
    ))
}
