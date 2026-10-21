//! Cycling (CR 702.29): "[Cost], Discard this card: Draw a card." An activated ability that works from a hand and may
//! be activated whenever its owner has priority.

use mtg_cards::ability::{Ability, Timing};
use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::ZoneKind;

use super::KeywordRules;

/// The rules of cycling.
#[derive(Debug)]
pub(super) struct Cycling;

impl KeywordRules for Cycling {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        let Keyword::Cycling(cost) = keyword else {
            return Vec::new();
        };
        let mut parts = cost.0.clone();
        parts.push(CostItem::DiscardSelf);
        vec![Ability::Activated {
            cost: Cost(parts),
            timing: Timing::Instant,
            condition: None,
            limit: None,
            from: ZoneKind::Hand,
            targets: Vec::new(),
            effects: vec![Effect::Draw {
                who: mtg_cards::refs::Ref::You,
                count: mtg_cards::value::Amount::one(),
            }],
        }]
    }
}

#[cfg(test)]
mod tests {
    //! Goal: Cycling adds one activated ability that works from a hand at instant speed, costs the cycling cost and the discard of its source, draws a card, and no other keyword adds it.
    use mtg_cards::ability::Ability;
    use rstest::rstest;

    use super::*;

    fn written(cost: &str) -> Ability {
        serde_json::from_str(&format!(
            r#"{{"Activated":{{"cost":{cost},"from":"Hand","effects":[{{"Draw":{{}}}}]}}}}"#
        ))
        .unwrap()
    }

    #[rstest]
    #[case::mana(
        Keyword::Cycling(Cost::mana("{2}".parse().unwrap())),
        r#"[{"Mana":"{2}"},"DiscardSelf"]"#
    )]
    #[case::life(
        Keyword::Cycling(Cost(vec![CostItem::PayLife(mtg_cards::value::Amount::Fixed(2))])),
        r#"[{"PayLife":2},"DiscardSelf"]"#
    )]
    fn cycling_discards_the_source_for_its_cost_and_draws(
        #[case] keyword: Keyword,
        #[case] cost: &str,
    ) {
        assert_eq!(Cycling.adds_abilities(&keyword), [written(cost)]);
    }

    #[rstest]
    #[case::flying(Keyword::Flying)]
    #[case::flash(Keyword::Flash)]
    fn no_other_keyword_adds_it(#[case] keyword: Keyword) {
        assert_eq!(Cycling.adds_abilities(&keyword), []);
    }
}
