//! Scavenge (CR 702.97): "[Cost], Exile this card from your graveyard: Put a number of +1/+1 counters equal to this card's
//! power on target creature. Activate only as a sorcery."
//!
//! An activated ability that works from a graveyard. The card is exiled as a cost, so the number of counters is the power
//! the card had there (last known information).

use mtg_cards::ability::{Ability, Timing};
use mtg_cards::cost::{Cost, CostItem};
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Bounds, Pred, Ref, Selector, TargetSpec, Targetable, ZoneKind};
use mtg_cards::value::{Amount, CounterKind, StatKind};
use mtg_core::card::CardType;

use super::KeywordRules;

/// The rules of scavenge.
#[derive(Debug)]
pub(super) struct Scavenge;

impl KeywordRules for Scavenge {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        let Keyword::Scavenge(cost) = keyword else {
            return Vec::new();
        };
        let mut paid = cost.0.clone();
        paid.push(CostItem::ExileSelf);
        vec![Ability::Activated {
            cost: Cost(paid),
            timing: Timing::Sorcery,
            condition: None,
            limit: None,
            from: ZoneKind::Graveyard,
            targets: vec![TargetSpec {
                what: Targetable::Object(Selector(vec![Pred::Type(CardType::Creature)])),
                bounds: Bounds::default(),
                divide: None,
            }],
            effects: vec![Effect::PutCounters {
                on: Ref::Target(0),
                counter: CounterKind::PlusOnePlusOne,
                count: Amount::Stat {
                    of: Ref::This,
                    stat: StatKind::Power,
                },
            }],
        }]
    }
}

#[cfg(test)]
mod tests {
    //! Goal: scavenge adds one sorcery-speed ability from a graveyard that costs the keyword cost and the exile of the card, targets a creature and puts as many +1/+1 counters on it as the card's power; no other keyword adds it.
    use rstest::rstest;

    use super::*;

    fn mana(text: &str) -> Cost {
        Cost::mana(text.parse().unwrap())
    }

    #[rstest]
    #[case::paid("{4}{G}{G}")]
    #[case::free("{0}")]
    fn the_ability_exiles_the_card_for_the_cost_and_puts_its_power_in_counters(
        #[case] price: &str,
    ) {
        let added = Scavenge.adds_abilities(&Keyword::Scavenge(mana(price)));
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
            [CostItem::Mana(price.parse().unwrap()), CostItem::ExileSelf]
        );
        assert_eq!((*timing, *from), (Timing::Sorcery, ZoneKind::Graveyard));
        assert_eq!(targets.len(), 1);
        assert_eq!(
            effects,
            &[Effect::PutCounters {
                on: Ref::Target(0),
                counter: CounterKind::PlusOnePlusOne,
                count: Amount::Stat {
                    of: Ref::This,
                    stat: StatKind::Power
                },
            }]
        );
    }

    #[rstest]
    #[case::other_keyword(Keyword::Flying)]
    #[case::embalm(Keyword::Embalm(mana("{1}")))]
    fn scavenge_adds_nothing_for_another_keyword(#[case] keyword: Keyword) {
        assert_eq!(Scavenge.adds_abilities(&keyword), Vec::new());
    }
}
