//! Compleated (CR 702.150): "If life was paid for this permanent's Phyrexian mana symbols as it was cast, it enters with
//! two fewer loyalty counters for each symbol paid that way." The spell keeps the life its cast paid for Phyrexian symbols
//! (`CostTaken::phyrexian_life`, 2 for each), and the loyalty the card enters with is reduced by that much
//! (`loyalty::entering`).

use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;

use super::KeywordRules;
use crate::ability_reader;
use crate::object::GameObject;

/// The rules of compleated: a hook of the loyalty a planeswalker enters with.
#[derive(Debug)]
pub(super) struct Compleated;

impl KeywordRules for Compleated {}

/// How many fewer loyalty counters the card `object` enters with: the life paid for its Phyrexian symbols when it has
/// compleated, 0 otherwise (a card that was not cast paid none).
pub(crate) fn fewer(object: &GameObject) -> u32 {
    let compleated = ability_reader::printed_rules(object.card())
        .any(|rule| matches!(&**rule, Ability::Keyword(Keyword::Compleated)));
    if !compleated {
        return 0;
    }
    object
        .choices
        .cost_taken
        .as_ref()
        .map_or(0, |taken| taken.phyrexian_life)
}
