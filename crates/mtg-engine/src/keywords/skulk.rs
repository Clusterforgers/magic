//! Skulk (CR 702.118): the creature can't be blocked by creatures with greater power.

use mtg_cards::keyword::Keyword;
use mtg_core::ids::ObjectId;

use super::KeywordRules;
use crate::characteristics::characteristics;
use crate::state::GameState;

/// The rules of skulk.
#[derive(Debug)]
pub(super) struct Skulk;

impl KeywordRules for Skulk {
    fn restricts_blocking(
        &self,
        state: &GameState,
        _keyword: &Keyword,
        attacker: ObjectId,
        blocker: ObjectId,
    ) -> bool {
        let power = |id| characteristics(state, id).and_then(|chars| chars.power);
        match (power(attacker), power(blocker)) {
            (Some(attacking), Some(blocking)) => blocking > attacking,
            _ => false,
        }
    }
}
