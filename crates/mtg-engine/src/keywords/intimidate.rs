//! Intimidate (CR 702.13): the creature can't be blocked except by artifact creatures and/or creatures that share a
//! color with it; a colorless creature shares none.

use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;

use super::KeywordRules;
use crate::characteristics::characteristics;
use crate::state::GameState;

/// The rules of intimidate.
#[derive(Debug)]
pub(super) struct Intimidate;

impl KeywordRules for Intimidate {
    fn restricts_blocking(
        &self,
        state: &GameState,
        _keyword: &Keyword,
        attacker: ObjectId,
        blocker: ObjectId,
    ) -> bool {
        let (Some(attacker), Some(blocker)) = (
            characteristics(state, attacker),
            characteristics(state, blocker),
        ) else {
            return false;
        };
        !(blocker.types.contains(CardType::Artifact)
            || blocker
                .colors
                .iter()
                .any(|color| attacker.colors.contains(color)))
    }
}
