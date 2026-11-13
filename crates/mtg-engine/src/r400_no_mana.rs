//! "You can't spend mana to cast this spell." (`GameRule::CantSpendMana`): the cast of such a card is offered and paid
//! with no mana of the pool and no mana ability, so only convoke and delve pay its cost.

use mtg_cards::ability::Ability;
use mtg_cards::modify::GameRule;
use mtg_core::ids::ObjectId;

use crate::state::GameState;

/// Whether the card `id` says no mana may be spent to cast it.
pub(crate) fn r400_no_mana(state: &GameState, id: ObjectId) -> bool {
    state.object(id).is_some_and(|card| {
        crate::ability_reader::printed_rules(card.card()).any(|rule| {
            matches!(
                &**rule,
                Ability::Rule {
                    rule: GameRule::CantSpendMana,
                    ..
                }
            )
        })
    })
}
