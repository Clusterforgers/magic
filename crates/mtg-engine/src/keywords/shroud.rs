//! Shroud (CR 702.18): a permanent or player with shroud can't be the target of any spell or ability, its own
//! controller's included.

use mtg_cards::keyword::Keyword;
use mtg_core::ids::{ObjectId, PlayerId};

use super::KeywordRules;
use crate::state::GameState;
use crate::zones::Zone;

/// The rules of shroud.
#[derive(Debug)]
pub(super) struct Shroud;

impl KeywordRules for Shroud {
    fn restricts_targeting(
        &self,
        state: &GameState,
        _keyword: &Keyword,
        holder: ObjectId,
        _source: ObjectId,
        _controller: PlayerId,
    ) -> bool {
        // A static ability works on the battlefield only (CR 113.6).
        state
            .object(holder)
            .is_some_and(|object| object.zone() == Zone::Battlefield)
    }

    fn restricts_player_targeting(
        &self,
        _keyword: &Keyword,
        _holder: PlayerId,
        _controller: PlayerId,
    ) -> bool {
        true
    }
}
