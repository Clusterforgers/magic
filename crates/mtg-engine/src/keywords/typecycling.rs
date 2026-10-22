//! Typecycling (CR 702.29): "[Cost], Discard this card: Search your library for a [type] card, reveal it, put it into your
//! hand, then shuffle." Cycling that searches: an activated ability that works from a hand whenever its owner has priority.

use mtg_cards::ability::{Ability, Timing};
use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::effect::{Destination, Effect};
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Bounds, Ref, ZoneKind};

use super::KeywordRules;

/// The rules of typecycling.
#[derive(Debug)]
pub(super) struct Typecycling;

/// The name the card found is bound to.
const FOUND: &str = "found";

impl KeywordRules for Typecycling {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        let Keyword::Typecycling { find, cost } = keyword else {
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
            effects: vec![
                Effect::Search {
                    who: Ref::You,
                    zone: ZoneKind::Library,
                    filter: find.clone(),
                    count: Bounds::default(),
                    reveal: true,
                    bind: FOUND.into(),
                },
                Effect::Move {
                    what: Ref::Bound(FOUND.into()),
                    to: Destination::Hand,
                },
                Effect::Shuffle { who: Ref::You },
            ],
        }]
    }
}

#[cfg(test)]
mod tests {
    //! Goal: Typecycling adds one activated ability that works from a hand at instant speed, costs the cost and the discard of its source, searches the library for a card the selector names, reveals it, puts it into the hand and shuffles, and no other keyword adds it.
    use mtg_cards::ability::Ability;
    use mtg_cards::refs::Selector;
    use rstest::rstest;

    use super::*;

    fn written(cost: &str, find: &str) -> Ability {
        serde_json::from_str(&format!(
            r#"{{"Activated":{{"cost":{cost},"from":"Hand","effects":[{{"Search":{{"filter":{find},"reveal":true,"as":"found"}}}},{{"Move":{{"what":{{"Bound":"found"}},"to":"Hand"}}}},{{"Shuffle":{{}}}}]}}}}"#
        ))
        .unwrap()
    }

    #[rstest]
    #[case::plainscycling(
        r#"[{"Subtype":"Plains"}]"#,
        "{2}",
        r#"[{"Mana":"{2}"},"DiscardSelf"]"#
    )]
    #[case::basic_landcycling(
        r#"[{"Type":"Land"},{"Supertype":"Basic"}]"#,
        "{1}{G}",
        r#"[{"Mana":"{1}{G}"},"DiscardSelf"]"#
    )]
    fn typecycling_discards_the_source_for_its_cost_and_searches(
        #[case] find: &str,
        #[case] mana: &str,
        #[case] cost: &str,
    ) {
        let keyword = Keyword::Typecycling {
            find: serde_json::from_str::<Selector>(find).unwrap(),
            cost: Cost::mana(mana.parse().unwrap()),
        };
        assert_eq!(Typecycling.adds_abilities(&keyword), [written(cost, find)]);
    }

    #[rstest]
    #[case::cycling(Keyword::Cycling(Cost::default()))]
    #[case::flying(Keyword::Flying)]
    fn no_other_keyword_adds_it(#[case] keyword: Keyword) {
        assert_eq!(Typecycling.adds_abilities(&keyword), []);
    }
}
