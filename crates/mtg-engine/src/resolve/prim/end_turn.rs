//! `Effect::EndTurn`: the turn ends (CR 723).

use crate::events::{Event, ResolutionOutcome, ZoneCause};
use crate::object::ObjectKind;
use crate::resolve::{Cx, Run};
use crate::turn::{Phase, Step};
use crate::zones::Zone;

/// Exiles the other spells on the stack and ends the other abilities (CR 723.1a), ends combat, and goes on at the cleanup
/// step: no player gets priority and the steps between are not begun. The resolving spell or ability finishes as usual.
/// A turn already in its cleanup step has nothing more to end.
pub(super) fn run(cx: &Cx<'_>) -> Run {
    let state = cx.state();
    let mut events = Vec::new();
    for &id in state.zone(Zone::Stack) {
        if id == cx.resolving.object() {
            continue;
        }
        let Some(object) = state.object(id) else {
            continue;
        };
        events.push(if object.kind() == ObjectKind::Ability {
            Event::ResolutionEnded {
                object: id,
                outcome: ResolutionOutcome::Countered,
            }
        } else {
            Event::ZoneChanged {
                object: id,
                from: Zone::Stack,
                to: Zone::Exile,
                cause: ZoneCause::Exiled,
            }
        });
    }
    let step = state.turn().step();
    if step != Step::Cleanup {
        if step.phase() == Phase::Combat {
            events.push(Event::CombatEnded);
        }
        events.push(Event::StepBegan {
            step: Step::Cleanup,
        });
    }
    Run::events(events)
}
