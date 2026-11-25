//! `Effect::R112DoubleX` and `Effect::R123XBecomes`: the value of X of a spell or ability on the stack doubles or
//! becomes a number (CR 107.3).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::events::Event;
use crate::object::ObjectKind;
use crate::resolve::{Cx, ResolveError, Run};
use crate::zones::Zone;

/// Each spell the effect names that is still the object on the stack it was gets twice its X, logged as `R112XChanged`;
/// a spell cast with no X, or one that has left the stack, is left alone.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::R112DoubleX { what } = effect else {
        return Err(wrong_effect("R112DoubleX"));
    };
    let events = cx
        .scope
        .live_objects(what)?
        .into_iter()
        .filter(|spell| spell.zone() == Zone::Stack && spell.kind() != ObjectKind::Ability)
        .filter_map(|spell| {
            let x = spell.choices().x?;
            Some(Event::R112XChanged {
                object: spell.reference(),
                x: x.saturating_mul(2),
            })
        })
        .collect();
    Ok(Run::events(events))
}

/// `Effect::R123XBecomes`: each spell or ability the effect names that is still the object on the stack it was and was
/// put there with an X gets the new X, logged as `R112XChanged`; anything else is left alone.
pub(super) fn r123_run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::R123XBecomes { what, value } = effect else {
        return Err(wrong_effect("R123XBecomes"));
    };
    let events = cx
        .scope
        .live_objects(what)?
        .into_iter()
        .filter(|object| object.zone() == Zone::Stack && object.choices().x.is_some())
        .map(|object| Event::R112XChanged {
            object: object.reference(),
            x: *value,
        })
        .collect();
    Ok(Run::events(events))
}
