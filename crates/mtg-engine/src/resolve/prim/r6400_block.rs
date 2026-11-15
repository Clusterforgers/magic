//! `Effect::R6400Block`: a creature blocks an attacking creature the effect's controller chooses (CR 509.1h): "then it
//! blocks an attacking creature of your choice" (Balduvian Warlord), "You may have it block an attacking creature of your
//! choice." (False Orders).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::choose::{Picked, Selection, pick_selections};
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::zones::Zone;

/// For each creature the effect names that is still on the battlefield, the effect's controller chooses one of the
/// attackers it could block (`combat::r6400_could_block`), or none when the effect is optional, and it blocks that
/// attacker. A creature with no attacker to block blocks nothing.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::R6400Block { what, optional } = effect else {
        return Err(wrong_effect("R6400Block"));
    };
    let state = cx.state();
    let blockers: Vec<_> = cx
        .scope
        .live_objects(what)?
        .into_iter()
        .filter(|object| object.zone() == Zone::Battlefield)
        .map(|object| object.id())
        .collect();
    let selections: Vec<Selection> = blockers
        .iter()
        .map(|blocker| Selection {
            one_per_name: false,
            groups: Vec::new(),
            player: cx.controller(),
            from: crate::combat::r6400_could_block(state, *blocker),
            min: usize::from(!*optional),
            max: 1,
            text: "Choose an attacking creature for it to block.".to_owned(),
        })
        .collect();
    let taken = match pick_selections(cx, &selections)? {
        Picked::Ask(decision) => return Ok(Run::Ask(decision)),
        Picked::Done(taken) => taken,
    };
    let events = blockers
        .iter()
        .zip(taken)
        .filter_map(|(blocker, (_, attackers))| {
            attackers.first().map(|attacker| Event::R6400Blocks {
                blocker: *blocker,
                attacker: *attacker,
            })
        })
        .collect();
    Ok(Run::events(events))
}
