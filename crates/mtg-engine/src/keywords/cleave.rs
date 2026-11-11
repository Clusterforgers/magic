//! Cleave (CR 702.148): "You may cast this spell by paying [cost] rather than paying its mana cost. If you do, change its
//! text by removing all text found within square brackets in the spell's rules text."
//!
//! The cost is an alternative cost chosen with the cast (`ways.rs`). The keyword carries the spell ability as it reads
//! without the bracketed words, and a spell cast for its cleave cost announces and resolves that ability in place of the
//! printed one ([`cleaved`]): a text change of the spell, whose targets differ from the printed ones.

use std::sync::Arc;

use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CastWay;
use mtg_core::ids::ObjectId;

use super::KeywordRules;
use crate::state::GameState;

/// The rules of cleave.
#[derive(Debug)]
pub(super) struct Cleave;

impl KeywordRules for Cleave {}

/// The spell ability `id` reads when it is cast in `way`: the cleaved one for a cast for its cleave cost, else none.
pub(crate) fn cleaved(
    state: &GameState,
    id: ObjectId,
    way: Option<CastWay>,
) -> Option<Arc<Ability>> {
    if way != Some(CastWay::Cleave) {
        return None;
    }
    crate::ability_reader::printed_rules(state.object(id)?.card()).find_map(
        |rules| match &**rules {
            Ability::Keyword(Keyword::Cleave { spell, .. }) => Some(Arc::new((**spell).clone())),
            _ => None,
        },
    )
}
