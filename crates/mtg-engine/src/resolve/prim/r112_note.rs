//! `Effect::R112NoteChosen`: the source notes the word a resolution chose ("Note a creature type that hasn't been noted
//! for this Saga"), kept in its memory until it leaves the battlefield (CR 400.7).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::events::Event;
use crate::memory::{Held, R112_NOTED_SLOT, Remembered};
use crate::object::Chosen;
use crate::resolve::{Cx, ResolveError, Run};
use crate::zones::Zone;

/// Logs the creature type chosen under the name as a word the source noted; nothing when none was chosen or the
/// source has left the battlefield.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::R112NoteChosen(name) = effect else {
        return Err(wrong_effect("R112NoteChosen"));
    };
    let source = cx.scope.source;
    let present = cx.state().object(source.id).is_some_and(|object| {
        object.counter() == source.counter && object.zone() == Zone::Battlefield
    });
    let word = match cx.scope.chosen_value(name) {
        Some(Chosen::CreatureType(subtype)) if present => Some(subtype.0.clone()),
        _ => None,
    };
    Ok(Run::events(
        word.map(|word| {
            Event::Remembered(Box::new(Remembered {
                source,
                slot: R112_NOTED_SLOT.to_owned(),
                objects: Vec::new(),
                values: vec![Held::R112Word(word)],
                replace: false,
            }))
        })
        .into_iter()
        .collect(),
    ))
}
