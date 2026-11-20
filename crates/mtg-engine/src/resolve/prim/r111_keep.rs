//! `Effect::R111KeepChosen`: the source keeps a value the resolution chose as its own public choice, which its later
//! abilities read as "the chosen name" (`Pred::ChosenName`, `Ref::Chosen`).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::zones::Zone;

/// Logs the value bound under the name as the source's choice; nothing when none was chosen or the source has left the
/// battlefield (a new object, CR 400.7).
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::R111KeepChosen(name) = effect else {
        return Err(wrong_effect("R111KeepChosen"));
    };
    let source = cx.scope.source;
    let present = cx.state().object(source.id).is_some_and(|object| {
        object.counter() == source.counter && object.zone() == Zone::Battlefield
    });
    let chosen = cx.scope.chosen_value(name).cloned().filter(|_| present);
    Ok(Run::events(
        chosen
            .map(|chosen| Event::R111ChoiceKept {
                object: source.id,
                chosen,
            })
            .into_iter()
            .collect(),
    ))
}
