//! "As an additional cost to cast this spell, choose a number between 0 and 5." (`CostItem::R110ChooseNumber`, CR 601.2b):
//! the number is announced as the X of the spell, from the least to the greatest the card names, whatever way it is cast.

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::cost::CostItem;

/// The least and the greatest number the spell's required additional cost lets its caster choose; `None` when it has none.
pub(crate) fn r110_range(card: &CardDefinition) -> Option<(u32, u32)> {
    crate::ability_reader::printed_rules(card).find_map(|rule| match &**rule {
        Ability::AdditionalCost {
            cost,
            required: true,
            ..
        } => cost.0.iter().find_map(|item| match item {
            CostItem::R110ChooseNumber { least, most } => Some((*least, *most)),
            _ => None,
        }),
        _ => None,
    })
}
