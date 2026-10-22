//! Undaunted (CR 702.125): "This spell costs {1} less to cast for each opponent." A static ability of the spell that the
//! cost modifiers of the announcement read (`announce/modifiers.rs`); an opponent who has left the game is not counted.

use mtg_cards::ability::{Ability, CostChange};
use mtg_cards::keyword::Keyword;
use mtg_cards::mana::ManaCostX;
use mtg_cards::refs::{Ref, Selector};
use mtg_cards::value::Amount;
use mtg_core::mana::{ManaCost, ManaSymbol};

use super::KeywordRules;

/// The rules of undaunted.
#[derive(Debug)]
pub(super) struct Undaunted;

impl KeywordRules for Undaunted {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        if !matches!(keyword, Keyword::Undaunted) {
            return Vec::new();
        }
        vec![Ability::CostModifier {
            applies_to: Selector::this(),
            change: CostChange::Reduce,
            mana: ManaCostX::new(0, ManaCost::from_symbols([ManaSymbol::Generic(1)])),
            per: Amount::Players(Ref::Opponents),
            condition: None,
        }]
    }
}

#[cfg(test)]
mod tests {
    //! Goal: undaunted adds one cost reduction of the spell itself, {1} for each opponent, and no other keyword adds it.
    use rstest::rstest;

    use super::*;

    #[test]
    fn undaunted_reduces_the_cost_by_one_for_each_opponent() {
        let written: Ability = serde_json::from_str(
            r#"{"CostModifier":{"applies_to":["This"],"change":"Reduce","mana":"{1}","per":{"Players":"Opponents"}}}"#,
        )
        .unwrap();
        assert_eq!(Undaunted.adds_abilities(&Keyword::Undaunted), [written]);
    }

    #[rstest]
    #[case::flying(Keyword::Flying)]
    #[case::split_second(Keyword::SplitSecond)]
    fn no_other_keyword_adds_it(#[case] keyword: Keyword) {
        assert_eq!(Undaunted.adds_abilities(&keyword), []);
    }
}
