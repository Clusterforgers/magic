//! Eternalize (CR 702.129): "[Cost], Exile this card from your graveyard: Create a token that's a copy of it, except it's a
//! 4/4 black Zombie in addition to its other types and it has no mana cost. Activate only as a sorcery."
//!
//! The same ability as embalm (`embalm.rs`) with a black 4/4 copy.

use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::modify::Modification;
use mtg_cards::value::Amount;
use mtg_core::color::Color;

use super::KeywordRules;
use super::embalm::zombie_copy;

/// The rules of eternalize.
#[derive(Debug)]
pub(super) struct Eternalize;

impl KeywordRules for Eternalize {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        let Keyword::Eternalize(cost) = keyword else {
            return Vec::new();
        };
        let four_by_four = Modification::SetPT {
            power: Amount::Fixed(4),
            toughness: Amount::Fixed(4),
        };
        vec![zombie_copy(cost, vec![four_by_four], Color::Black)]
    }
}
