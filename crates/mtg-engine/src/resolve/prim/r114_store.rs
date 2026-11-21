//! `Effect::R114StoreValue`: a number the source stores on itself for as long as it stays (CR 614.12: "As this creature
//! enters, pay any amount of life", read by "equal to the life paid as it entered").

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::events::Event;
use crate::memory::{Held, Remembered};
use crate::resolve::{Cx, ResolveError, Run};

/// The number, read now, replaces what the source stored in the lasting slot.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::R114StoreValue { value, slot } = effect else {
        return Err(wrong_effect("R114StoreValue"));
    };
    let number = cx.scope.amount(value)?;
    Ok(Run::events(vec![Event::Remembered(Box::new(Remembered {
        source: cx.scope.source,
        slot: crate::memory::r112_lasting(slot),
        objects: Vec::new(),
        values: vec![Held::Number(number)],
        replace: true,
    }))]))
}
