//! Affinity (CR 702.41): "This spell costs {1} less to cast for each [permanent] you control." A static ability of the
//! spell that the cost modifiers of the announcement read (`announce/modifiers.rs`).

use mtg_cards::ability::{Ability, CostChange};
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Ref, Selector};
use mtg_cards::value::Amount;

use super::KeywordRules;

/// The rules of affinity.
#[derive(Debug)]
pub(super) struct Affinity;

impl KeywordRules for Affinity {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        let Keyword::Affinity(kind) = keyword else {
            return Vec::new();
        };
        let mut counted = kind.0.clone();
        counted.push(Pred::ControlledBy(Ref::You));
        vec![Ability::CostModifier {
            applies_to: Selector::this(),
            change: CostChange::Reduce,
            mana: mtg_cards::mana::ManaCostX::new(
                0,
                mtg_core::mana::ManaCost::from_symbols([mtg_core::mana::ManaSymbol::Generic(1)]),
            ),
            per: Amount::Count(Selector(counted)),
            condition: None,
        }]
    }
}

#[cfg(test)]
mod tests {
    //! Goal: Affinity adds one cost reduction of the spell itself, {1} for each permanent the selector names that its controller controls, and no other keyword adds it.
    use rstest::rstest;

    use super::*;

    fn written(per: &str) -> Ability {
        serde_json::from_str(&format!(
            r#"{{"CostModifier":{{"applies_to":["This"],"change":"Reduce","mana":"{{1}}","per":{per}}}}}"#
        ))
        .unwrap()
    }

    #[rstest]
    #[case::artifacts(
        Selector(vec![Pred::Type(mtg_core::card::CardType::Artifact)]),
        r#"{"Count":[{"Type":"Artifact"},{"ControlledBy":"You"}]}"#
    )]
    #[case::a_subtype(
        Selector(vec![Pred::Subtype(mtg_core::card::Subtype("Island".into()))]),
        r#"{"Count":[{"Subtype":"Island"},{"ControlledBy":"You"}]}"#
    )]
    #[case::tokens(Selector(vec![Pred::Token]), r#"{"Count":["Token",{"ControlledBy":"You"}]}"#)]
    fn affinity_reduces_the_cost_by_one_for_each_permanent_counted(
        #[case] kind: Selector,
        #[case] per: &str,
    ) {
        assert_eq!(Affinity.adds_abilities(&Keyword::Affinity(kind)), [written(per)]);
    }

    #[rstest]
    #[case::flying(Keyword::Flying)]
    #[case::convoke(Keyword::Convoke)]
    fn no_other_keyword_adds_it(#[case] keyword: Keyword) {
        assert_eq!(Affinity.adds_abilities(&keyword), []);
    }
}
