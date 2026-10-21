//! Foretell (CR 702.143): "During your turn, you may pay {2} and exile this card from your hand face down. Cast it on a
//! later turn for its foretell cost."
//!
//! The keyword adds no ability. The special action is `PriorityAction::Foretell` (`priority.rs` offers it,
//! `announce/foretell.rs` pays for it), the card lies face down in exile (`object::FaceDown::Foretold`), and `ways.rs`
//! offers the cast for the foretell cost from exile once the turn it was foretold in is over.

use mtg_core::mana::{ManaCost, ManaSymbol};

use super::KeywordRules;

/// The rules of foretell.
#[derive(Debug)]
pub(super) struct Foretell;

impl KeywordRules for Foretell {}

/// The mana the special action costs: {2}, whatever the card (CR 702.143a). Cost modifiers do not apply to it.
pub(crate) fn action_cost() -> ManaCost {
    ManaCost::from_symbols([ManaSymbol::Generic(2)])
}

#[cfg(test)]
mod tests {
    //! Goal: the special action of foretell costs {2} and the keyword adds no ability of its own.
    use mtg_cards::ability::Ability;
    use mtg_cards::cost::Cost;
    use mtg_cards::keyword::Keyword;

    use super::*;

    #[test]
    fn the_action_costs_two_generic_mana() {
        assert_eq!(action_cost().to_string(), "{2}");
        assert_eq!(action_cost().mana_value(), 2);
    }

    #[test]
    fn the_keyword_adds_no_ability() {
        let keyword = Keyword::Foretell(Cost::mana("{1}{U}".parse().unwrap()));
        let added: Vec<Ability> = Foretell.adds_abilities(&keyword);
        assert!(added.is_empty());
    }
}
