//! "Rather than pay {2} for each previous time you've cast this spell from the command zone this game, pay 2 life that
//! many times." (`GameRule::TaxAsLife`): a commander cast from the command zone pays its tax as life, not as generic
//! mana (CR 903.8).

use mtg_cards::ability::Ability;
use mtg_cards::modify::GameRule;
use mtg_core::ids::ObjectId;

use crate::state::GameState;
use crate::zones::Zone;

/// The life the card `id`, cast from the command zone, pays in place of its commander tax: the life of its rule for each
/// previous cast from the command zone. `None` for a card without the rule, outside the command zone or whose owner pays
/// no commander tax.
pub(crate) fn r5500_tax_life(state: &GameState, id: ObjectId) -> Option<u32> {
    let card = state.object(id)?;
    if !matches!(card.zone(), Zone::Command(_))
        || crate::rules::pays_no_commander_tax(state, card.owner())
    {
        return None;
    }
    let life =
        crate::ability_reader::printed_rules(card.card()).find_map(|rule| match &**rule {
            Ability::Rule {
                rule: GameRule::TaxAsLife(life),
                ..
            } => Some(*life),
            _ => None,
        })?;
    Some(state.commander_casts(id).saturating_mul(life))
}
