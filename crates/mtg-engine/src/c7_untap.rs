//! The end of a rule that a permanent doesn't untap during its controller's next untap step (CR 502.3, 611.2b).

use mtg_cards::modify::Duration;
use mtg_core::ids::PlayerId;

use crate::abilities::Grant;
use crate::characteristics::controller_of;
use crate::effects::Modification;
use crate::events::Event;
use crate::state::GameState;
use crate::zones::Zone;

/// The `EffectExpired` events of the untap step of `active`: each effect that lasts through the next untap step and
/// holds down a permanent `active` controls is used up by this step, whether that permanent is tapped or not. The
/// untap of the step reads the effects before these events remove them.
pub(crate) fn c7_untap_spent(state: &GameState, active: PlayerId) -> Vec<Event> {
    let mut spent: Vec<_> = state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| controller_of(state, **id) == Some(active))
        .filter_map(|id| state.layers().get(*id))
        .flat_map(|held| &held.applied)
        .filter(|applied| matches!(applied.modification, Modification::DoesntUntap))
        .filter_map(|applied| match applied.origin {
            Grant::Effect(id) => Some(id),
            Grant::Static { .. } => None,
        })
        .filter(|id| {
            state
                .effects()
                .iter()
                .any(|e| e.id == *id && e.duration == Duration::NextUntapStep)
        })
        .collect();
    spent.sort();
    spent.dedup();
    spent
        .into_iter()
        .map(|id| Event::EffectExpired { id })
        .collect()
}
