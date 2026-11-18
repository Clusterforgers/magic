//! Helpers the campaign's tests share: a fight's boss deck resolved from the test corpus.

use std::sync::Arc;

use mtg_campaign::FightPlan;
use mtg_cards::CardDefinition;
use mtg_testkit::corpus;

/// The boss's deck of `plan` as [`FightPlan::boss_decks`] names it (its precon, then its satchel), each card from the
/// test corpus; a card the corpus lacks (a draft of the read tier) stands in as Wastes.
pub fn boss_deck(plan: &FightPlan) -> Vec<Arc<CardDefinition>> {
    let wastes = mtg_testkit::cards::wastes();
    plan.boss_decks()
        .first()
        .unwrap()
        .iter()
        .flat_map(|(name, copies)| {
            let card = corpus::card(name).unwrap_or_else(|_| Arc::clone(&wastes));
            (0..*copies).map(move |_| Arc::clone(&card))
        })
        .collect()
}
