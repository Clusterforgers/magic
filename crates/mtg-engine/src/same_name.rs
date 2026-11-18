//! The limit on creatures with one name (the Delves modifier Mirror Hall, `GameRule::SameNameLimit`): a player who
//! controls more creatures with the same name than the limit sacrifices the newest of them as a state-based action.

use std::collections::BTreeMap;

use mtg_core::ids::{ObjectId, PlayerId};

use crate::characteristics::{characteristics, controller_of_object};
use crate::events::{Event, ZoneCause};
use crate::state::GameState;
use crate::zones::Zone;

/// The sacrifices the limit makes now: for each player under it and each name, the creatures beyond the limit, newest
/// (latest timestamp) first to go; the permanents in `gone` already leave in this pass and are not counted. A creature
/// with no name counts for none.
pub(crate) fn sacrifices(state: &GameState, gone: &[ObjectId]) -> Vec<Event> {
    let mut found: BTreeMap<(PlayerId, String), Vec<(u64, ObjectId)>> = BTreeMap::new();
    for &id in state.zone(Zone::Battlefield) {
        let Some(object) = state.object(id) else {
            continue;
        };
        let controller = controller_of_object(state, object);
        if gone.contains(&id)
            || !state.is_alive(controller)
            || crate::rules::same_name_limit(state, controller).is_none()
            || !characteristics(state, id).is_some_and(|chars| chars.is_creature())
        {
            continue;
        }
        let Some(name) = state.name_of(id).filter(|name| !name.is_empty()) else {
            continue;
        };
        let stamp = object
            .permanent()
            .map_or(0, |permanent| permanent.timestamp);
        found
            .entry((controller, name))
            .or_default()
            .push((stamp, id));
    }
    found
        .into_iter()
        .flat_map(|((player, _), mut creatures)| {
            let most = crate::rules::same_name_limit(state, player).unwrap_or(u32::MAX);
            let most = usize::try_from(most).unwrap_or(usize::MAX);
            creatures.sort_unstable();
            creatures.into_iter().skip(most).map(|(_, id)| id)
        })
        .filter_map(|id| state.object(id))
        .map(|object| Event::ZoneChanged {
            object: object.id(),
            from: Zone::Battlefield,
            to: Zone::Graveyard(object.owner()),
            cause: ZoneCause::Sacrificed,
        })
        .collect()
}
