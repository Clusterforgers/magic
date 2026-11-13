//! "Spend only mana produced by basic lands to cast this spell." (`GameRule::SpendOnlyFrom`): the cast of such a card is
//! offered and paid with the pool's mana whose source the filter holds for and with mana abilities of such sources only.

use mtg_cards::ability::Ability;
use mtg_cards::modify::GameRule;
use mtg_cards::spend::SpendFilter;
use mtg_core::ids::ObjectId;

use crate::mana::{ManaOption, ManaPool};
use crate::state::GameState;

/// The sources the card `id` says its mana must come from, if it says so.
pub(crate) fn r4705_spend_from(state: &GameState, id: ObjectId) -> Option<SpendFilter> {
    let card = state.object(id)?;
    crate::ability_reader::printed_rules(card.card()).find_map(|rule| match &**rule {
        Ability::Rule {
            rule: GameRule::SpendOnlyFrom(filter),
            ..
        } => Some(filter.clone()),
        _ => None,
    })
}

/// The pool a payment for the card `id` may spend: all of it, or only the mana of the sources its rule names.
pub(crate) fn r4705_pool(state: &GameState, id: ObjectId, pool: &ManaPool) -> Option<ManaPool> {
    let filter = r4705_spend_from(state, id)?;
    Some(pool.only_from(|from| filter.holds(from)))
}

/// Keeps only the mana abilities of sources the rule of the card `id` names, if it has one.
pub(crate) fn r4705_sources(state: &GameState, id: ObjectId, sources: &mut Vec<ManaOption>) {
    if let Some(filter) = r4705_spend_from(state, id) {
        sources
            .retain(|option| filter.holds(&crate::spend::source_kind(state, option.source.object)));
    }
}
