//! Aura swap (CR 702.65): "[Cost]: You may exchange this permanent with an Aura card in your hand." The exchange is
//! `Effect::AuraSwap`.

use mtg_cards::ability::{Ability, Activators, Timing};
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::ZoneKind;

use super::KeywordRules;

/// The rules of aura swap.
#[derive(Debug)]
pub(super) struct AuraSwap;

impl KeywordRules for AuraSwap {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        let Keyword::AuraSwap(cost) = keyword else {
            return Vec::new();
        };
        vec![Ability::Activated {
            cost: cost.clone(),
            timing: Timing::Instant,
            condition: None,
            limit: None,
            by: Activators::Controller,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: vec![Effect::AuraSwap],
        }]
    }
}

#[cfg(test)]
mod tests {
    //! Goal: Aura swap adds one instant-speed activated ability that costs its aura swap cost and exchanges its source, whatever the cost, and no other keyword adds it.
    use mtg_cards::ability::Ability;
    use mtg_cards::cost::Cost;
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::two_blue("{2}{U}")]
    #[case::one("{1}")]
    #[case::white("{3}{W}")]
    fn aura_swap_exchanges_for_its_cost(#[case] cost: &str) {
        let written: Ability = serde_json::from_str(&format!(
            r#"{{"Activated":{{"cost":"{cost}","effects":["AuraSwap"]}}}}"#
        ))
        .unwrap();
        let keyword = Keyword::AuraSwap(Cost::mana(cost.parse().unwrap()));
        assert_eq!(AuraSwap.adds_abilities(&keyword), [written]);
    }

    #[rstest]
    #[case::flying(Keyword::Flying)]
    #[case::outlast(Keyword::Outlast(Cost::mana("{1}".parse().unwrap())))]
    fn no_other_keyword_adds_it(#[case] keyword: Keyword) {
        assert_eq!(AuraSwap.adds_abilities(&keyword), []);
    }
}
