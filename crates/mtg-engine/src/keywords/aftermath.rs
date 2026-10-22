//! Aftermath (CR 702.127): "Cast this half of this split card only from your graveyard. Then exile it."
//!
//! The half with the keyword has no way to be cast from any zone but a graveyard (`ways.rs`), and a spell of it that
//! was cast from a graveyard is exiled instead of going anywhere else from the stack (`replace.rs`, with flashback).

use mtg_cards::keyword::Keyword;
use mtg_core::ids::ObjectId;

use super::{KeywordRules, keywords_of};
use crate::state::GameState;

/// The rules of aftermath.
#[derive(Debug)]
pub(super) struct Aftermath;

impl KeywordRules for Aftermath {}

/// Whether the object `id`, as the half it is in use as, has aftermath.
pub(crate) fn has(state: &GameState, id: ObjectId) -> bool {
    keywords_of(state, id)
        .into_iter()
        .any(|keyword| matches!(keyword, Keyword::Aftermath))
}

#[cfg(test)]
mod tests {
    //! Goal: aftermath is a static rule of the engine and adds no ability of its own.
    use super::*;

    #[test]
    fn it_adds_no_ability() {
        assert_eq!(Aftermath.adds_abilities(&Keyword::Aftermath), []);
    }
}
