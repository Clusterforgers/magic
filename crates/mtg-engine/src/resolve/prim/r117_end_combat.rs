//! `Effect::R117EndCombat`: "end the combat phase" (the Mandate of Peace rulings).

use crate::resolve::{Cx, Run};
use crate::turn::Phase;

/// Exiles every spell and ability on the stack, this one included, removes attackers and blockers from combat and goes
/// on at the phase after combat: the steps of combat left are not begun, so nothing triggers "at end of combat" and
/// effects "until end of combat" end. Nothing happens outside a combat phase.
pub(super) fn run(cx: &Cx<'_>) -> Run {
    let state = cx.state();
    if state.turn().step().phase() != Phase::Combat {
        return Run::events(Vec::new());
    }
    let mut events = super::end_turn::stack_exiled(cx);
    events.extend(crate::steps::r117_leave_combat(state));
    Run::events(events)
}
