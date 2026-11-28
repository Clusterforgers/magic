//! Space sculptor (Space Beleren): "Space Beleren divides the battlefield into alpha, beta, and gamma sectors. If a
//! creature isn't assigned to a sector, its controller assigns it to one. Opponents assign first."
//!
//! The keyword has no hook of its own: while a permanent with it is on the battlefield, the sectors of `p170a_sectors`
//! ask for the assignments as a state-based action, and they are lost once none is left.

use mtg_cards::keyword::Keyword;
use mtg_core::ids::{ObjectId, PlayerId};

use super::{KeywordRules, keywords_of};
use crate::state::GameState;
use crate::zones::Zone;

/// The rules of space sculptor: none of the hooks (`p170a_sectors` reads the keyword).
#[derive(Debug)]
pub(super) struct SpaceSculptor;

impl KeywordRules for SpaceSculptor {}

/// The players who control a permanent with space sculptor, one entry for each such permanent.
pub(crate) fn p170a_sculptors(state: &GameState) -> Vec<PlayerId> {
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| p170a_has_space_sculptor(state, **id))
        .filter_map(|id| crate::characteristics::controller_of(state, *id))
        .collect()
}

/// Whether the permanent `id` has space sculptor.
fn p170a_has_space_sculptor(state: &GameState, id: ObjectId) -> bool {
    keywords_of(state, id)
        .into_iter()
        .any(|keyword| matches!(keyword, Keyword::SpaceSculptor))
}
