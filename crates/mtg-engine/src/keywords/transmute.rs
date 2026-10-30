//! Transmute (CR 702.53): "[Cost], Discard this card: Search your library for a card with the same mana value as the
//! discarded card, reveal it, put it into your hand, then shuffle. Activate only as a sorcery."
//!
//! An activated ability that works from a hand, like typecycling at sorcery speed. The discarded card is read by last
//! known information, so the search compares with the mana value it had in the hand.

use mtg_cards::ability::{Ability, Timing};
use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::effect::{Destination, Effect};
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Bounds, Pred, Ref, Selector, ZoneKind};
use mtg_cards::value::{Amount, Cmp, StatKind};

use super::KeywordRules;

/// The rules of transmute.
#[derive(Debug)]
pub(super) struct Transmute;

/// The name the card found is bound to.
const FOUND: &str = "found";

impl KeywordRules for Transmute {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        let Keyword::Transmute(cost) = keyword else {
            return Vec::new();
        };
        let mut paid = cost.0.clone();
        paid.push(CostItem::DiscardSelf);
        vec![Ability::Activated {
            cost: Cost(paid),
            timing: Timing::Sorcery,
            condition: None,
            limit: None,
            from: ZoneKind::Hand,
            targets: Vec::new(),
            effects: vec![
                Effect::Search {
                    who: Ref::You,
                    zone: ZoneKind::Library,
                    also: Vec::new(),
                    filter: Selector(vec![Pred::Compare {
                        stat: StatKind::ManaValue,
                        op: Cmp::Eq,
                        to: Amount::Stat {
                            of: Ref::This,
                            stat: StatKind::ManaValue,
                        },
                    }]),
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
    //! Goal: transmute adds one sorcery-speed ability from a hand that costs the keyword cost and the discard of the card, searches the library for a card with the mana value of the discarded one, reveals it, puts it into the hand and shuffles; no other keyword adds it.
    use rstest::rstest;

    use super::*;

    fn mana(text: &str) -> Cost {
        Cost::mana(text.parse().unwrap())
    }

    #[test]
    fn the_ability_discards_the_card_for_the_cost_and_searches_for_its_mana_value() {
        let added = Transmute.adds_abilities(&Keyword::Transmute(mana("{1}{U}{U}")));
        let [
            Ability::Activated {
                cost,
                timing,
                from,
                targets,
                effects,
                ..
            },
        ] = added.as_slice()
        else {
            panic!("one activated ability: {added:?}");
        };
        assert_eq!(
            cost.0,
            [
                CostItem::Mana("{1}{U}{U}".parse().unwrap()),
                CostItem::DiscardSelf
            ]
        );
        assert_eq!((*timing, *from), (Timing::Sorcery, ZoneKind::Hand));
        assert_eq!(*targets, Vec::new());
        let [
            Effect::Search { filter, reveal, .. },
            Effect::Move { to, .. },
            Effect::Shuffle { .. },
        ] = effects.as_slice()
        else {
            panic!("search, move, shuffle: {effects:?}");
        };
        assert!(*reveal);
        assert_eq!(*to, Destination::Hand);
        assert_eq!(filter.0.len(), 1);
    }

    #[rstest]
    #[case::other_keyword(Keyword::Flying)]
    #[case::cycling(Keyword::Cycling(mana("{1}")))]
    fn transmute_adds_nothing_for_another_keyword(#[case] keyword: Keyword) {
        assert_eq!(Transmute.adds_abilities(&keyword), Vec::new());
    }
}
