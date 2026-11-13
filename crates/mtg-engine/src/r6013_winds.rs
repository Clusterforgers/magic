//! Counters that hold a permanent down in its controller's untap step: "If a permanent with a wind counter on it would
//! untap during its controller's untap step, remove all wind counters from it instead." (`Rule::UntapRemovesCounters`).

use mtg_core::ids::PlayerId;

use crate::characteristics::controller_of;
use crate::counters::CountersChanged;
use crate::events::Event;
use crate::object::GameObject;
use crate::state::GameState;

/// The events by which `object` would untap in the untap step of `active`: when its controller is `active` and it has
/// counters of a kind a rule names, all of them are removed instead and it stays tapped; otherwise it untaps as
/// stun counters allow.
pub(crate) fn r6013_untap_events(
    state: &GameState,
    object: &GameObject,
    active: Option<PlayerId>,
) -> Vec<Event> {
    let Some(permanent) = object.permanent() else {
        return Vec::new();
    };
    let held: Vec<Event> = if active.is_some() && controller_of(state, object.id()) == active {
        state
            .rules()
            .untap_removes
            .iter()
            .filter_map(|kind| {
                let count = permanent.counters_of(kind);
                (count > 0).then(|| {
                    Event::CountersChanged(CountersChanged {
                        on: crate::choose::Target::Object(object.reference()),
                        counter: kind.clone(),
                        delta: -i32::try_from(count).unwrap_or(i32::MAX),
                    })
                })
            })
            .collect()
    } else {
        Vec::new()
    };
    if held.is_empty() {
        vec![crate::stun::untap_event(object)]
    } else {
        held
    }
}
