//! Hexproof (CR 702.11): a permanent or player with hexproof can't be the target of spells or abilities its controller's opponents control.

use mtg_cards::keyword::Keyword;
use mtg_core::ids::{ObjectId, PlayerId};

use super::KeywordRules;
use crate::state::GameState;
use crate::zones::Zone;

/// The rules of hexproof.
#[derive(Debug)]
pub(super) struct Hexproof;

impl KeywordRules for Hexproof {
    fn restricts_targeting(
        &self,
        state: &GameState,
        _keyword: &Keyword,
        holder: ObjectId,
        _source: ObjectId,
        controller: PlayerId,
    ) -> bool {
        // A static ability works on the battlefield only (CR 113.6): a hexproof creature spell can be countered
        // and a hexproof card in a graveyard can be returned.
        state.object(holder).is_some_and(|object| {
            object.zone() == Zone::Battlefield && object.controller() != controller
        })
    }

    fn restricts_player_targeting(
        &self,
        _keyword: &Keyword,
        holder: PlayerId,
        controller: PlayerId,
    ) -> bool {
        holder != controller
    }
}
