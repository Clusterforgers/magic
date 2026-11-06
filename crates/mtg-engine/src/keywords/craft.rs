//! Craft (CR 702.167): "[Cost], Exile this permanent, Exile [materials] from among permanents you control and/or cards in
//! your graveyard: Return this card to the battlefield transformed under its owner's control. Activate only as a sorcery."
//!
//! An activated ability that works from the battlefield. The materials are a pick of the cost (`parts/exile.rs`); the
//! card is exiled as a cost and returned from exile as its back face.

use mtg_cards::ability::{Ability, Timing};
use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::effect::{Destination, Effect};
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Ref, ZoneKind};

use super::KeywordRules;

/// The rules of craft.
#[derive(Debug)]
pub(super) struct Craft;

impl KeywordRules for Craft {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        let Keyword::Craft(cost) = keyword else {
            return Vec::new();
        };
        let mut paid = cost.0.clone();
        paid.insert(0, CostItem::ExileSelf);
        // The mana comes first, as for any cost.
        paid.sort_by_key(|item| !matches!(item, CostItem::Mana(_)));
        vec![Ability::Activated {
            cost: Cost(paid),
            timing: Timing::Sorcery,
            condition: None,
            limit: None,
            by: mtg_cards::ability::Activators::Controller,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: vec![Effect::Move {
                what: Ref::Arrived {
                    of: Box::new(Ref::This),
                    zone: ZoneKind::Exile,
                },
                to: Destination::Battlefield {
                    tapped: false,
                    under: None,
                    transformed: true,
                    counters: Vec::new(),
                },
                bind: None,
            }],
        }]
    }
}

#[cfg(test)]
mod tests {
    //! Goal: Craft adds one sorcery-speed activated ability from the battlefield whose cost is the mana, the exile of the permanent and the materials, and which returns the card transformed; no other keyword adds one.
    use super::*;

    fn materials() -> CostItem {
        CostItem::Craft {
            filter: mtg_cards::refs::Selector::default(),
            count: mtg_cards::value::Amount::X,
            least: None,
        }
    }

    #[test]
    fn craft_adds_the_activated_ability() {
        let cost = Cost(vec![CostItem::Mana("{2}{B}".parse().unwrap()), materials()]);
        let added = Craft.adds_abilities(&Keyword::Craft(cost));
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
            panic!("one activated ability, got {added:?}");
        };
        assert_eq!(*timing, Timing::Sorcery);
        assert_eq!(*from, ZoneKind::Battlefield);
        assert!(matches!(cost.0[0], CostItem::Mana(_)));
        assert!(cost.0.contains(&CostItem::ExileSelf));
        assert!(cost.0.contains(&materials()));
        assert!(matches!(
            effects.as_slice(),
            [Effect::Move {
                to: Destination::Battlefield {
                    transformed: true,
                    ..
                },
                ..
            }]
        ));
    }

    #[test]
    fn other_keywords_add_nothing() {
        assert_eq!(Craft.adds_abilities(&Keyword::Flying), []);
    }
}
