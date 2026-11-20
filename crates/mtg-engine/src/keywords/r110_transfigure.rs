//! Transfigure (CR 702.71): "[Cost], Sacrifice this permanent: Search your library for a creature card with the same mana
//! value as this permanent, put it onto the battlefield, then shuffle. Transfigure only as a sorcery."
//!
//! An activated ability of the permanent, like transmute's of a card in a hand. The sacrificed permanent is read by last
//! known information, so the search compares with the mana value it had on the battlefield.

use mtg_cards::ability::{Ability, Timing};
use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::effect::{Destination, Effect};
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Bounds, Pred, Ref, Selector, ZoneKind};
use mtg_cards::value::{Amount, Cmp, StatKind};
use mtg_core::card::CardType;

use super::KeywordRules;

/// The rules of transfigure.
#[derive(Debug)]
pub(super) struct R110Transfigure;

/// The name the card found is bound to.
const FOUND: &str = "found";

impl KeywordRules for R110Transfigure {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        let Keyword::R110Transfigure(cost) = keyword else {
            return Vec::new();
        };
        let mut paid = cost.0.clone();
        paid.push(CostItem::Sacrifice {
            filter: Selector(vec![Pred::This]),
            count: Amount::one(),
        });
        vec![Ability::Activated {
            cost: Cost(paid),
            timing: Timing::Sorcery,
            condition: None,
            limit: None,
            by: mtg_cards::ability::Activators::Controller,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: vec![
                Effect::Search {
                    who: Ref::You,
                    of: None,
                    zone: ZoneKind::Library,
                    also: Vec::new(),
                    filter: Selector(vec![
                        Pred::Type(CardType::Creature),
                        Pred::Compare {
                            stat: StatKind::ManaValue,
                            op: Cmp::Eq,
                            to: Amount::Stat {
                                of: Ref::This,
                                stat: StatKind::ManaValue,
                            },
                        },
                    ]),
                    count: Bounds::default(),
                    reveal: false,
                    distinct: false,
                    bind: FOUND.into(),
                },
                Effect::Move {
                    what: Ref::Bound(FOUND.into()),
                    to: Destination::Battlefield {
                        tapped: false,
                        under: None,
                        transformed: false,
                        counters: Vec::new(),
                    },
                    bind: None,
                },
                Effect::Shuffle { who: Ref::You },
            ],
        }]
    }
}

#[cfg(test)]
mod tests {
    //! Goal: transfigure adds one sorcery-speed ability of the permanent that costs the keyword cost and its sacrifice and
    //! searches the library for a creature card with its mana value, put onto the battlefield, then shuffles; no other
    //! keyword adds it.
    use super::*;

    #[test]
    fn r110_the_ability_sacrifices_the_permanent_and_finds_a_creature_of_its_mana_value() {
        let cost = Cost::mana("{1}{B}{B}".parse().unwrap());
        let added = R110Transfigure.adds_abilities(&Keyword::R110Transfigure(cost));
        let [
            Ability::Activated {
                cost,
                timing,
                from,
                effects,
                ..
            },
        ] = added.as_slice()
        else {
            panic!("one activated ability: {added:?}");
        };
        assert_eq!(*timing, Timing::Sorcery);
        assert_eq!(*from, ZoneKind::Battlefield);
        assert!(
            matches!(cost.0.last(), Some(CostItem::Sacrifice { filter, .. }) if filter.0 == [Pred::This])
        );
        assert!(matches!(
            effects.as_slice(),
            [
                Effect::Search { filter, reveal: false, .. },
                Effect::Move { to: Destination::Battlefield { .. }, .. },
                Effect::Shuffle { .. }
            ] if filter.0.contains(&Pred::Type(CardType::Creature))
        ));
    }

    #[test]
    fn r110_another_keyword_adds_nothing() {
        assert_eq!(
            R110Transfigure.adds_abilities(&Keyword::Flying),
            Vec::<Ability>::new()
        );
    }
}
