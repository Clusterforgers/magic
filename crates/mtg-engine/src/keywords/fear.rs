//! Fear (CR 702.36): the creature can't be blocked except by artifact creatures and/or black creatures.

use mtg_cards::keyword::Keyword;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;

use super::KeywordRules;
use crate::characteristics::characteristics;
use crate::state::GameState;

/// The rules of fear.
#[derive(Debug)]
pub(super) struct Fear;

impl KeywordRules for Fear {
    fn restricts_blocking(
        &self,
        state: &GameState,
        _keyword: &Keyword,
        _attacker: ObjectId,
        blocker: ObjectId,
    ) -> bool {
        characteristics(state, blocker).is_some_and(|chars| {
            !(chars.types.contains(CardType::Artifact) || chars.colors.contains(Color::Black))
        })
    }
}
