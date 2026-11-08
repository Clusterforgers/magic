//! Recover (CR 702.59a): "When a creature is put into your graveyard from the battlefield, you may pay [cost]. If you do,
//! return this card from your graveyard to your hand. Otherwise, exile this card."
//!
//! The keyword adds a trigger that works from the graveyard. "Your graveyard" is the graveyard of the creature's owner,
//! so the trigger watches creatures you own. The card is found again by its identity in the graveyard as the trigger
//! resolves, so a card that has left it since stays where it is (CR 400.7). A recover creature that dies does not see its
//! own death: the trigger looks back to the battlefield, where it did not work (CR 603.10a).

use mtg_cards::ability::Ability;
use mtg_cards::effect::{Destination, Effect};
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Ref, Selector, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::Amount;
use mtg_core::card::CardType;

use super::KeywordRules;

/// The rules of recover.
#[derive(Debug)]
pub(super) struct Recover;

/// This card while it is still in a graveyard.
fn this_card() -> Ref {
    Ref::Each(Selector(vec![Pred::In(ZoneKind::Graveyard), Pred::This]))
}

impl KeywordRules for Recover {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        let Keyword::Recover(cost) = keyword else {
            return Vec::new();
        };
        vec![Ability::Triggered {
            when: TriggerPattern::ZoneChange {
                what: Selector(vec![
                    Pred::Type(CardType::Creature),
                    Pred::OwnedBy(Ref::You),
                    Pred::Other,
                ]),
                from: Some(ZoneKind::Battlefield),
                to: Some(ZoneKind::Graveyard),
            },
            intervening_if: None,
            limit: None,
            from: ZoneKind::Graveyard,
            targets: Vec::new(),
            effects: vec![Effect::Pay {
                who: Ref::You,
                cost: cost.clone(),
                times: Amount::one(),
                paid: vec![Effect::Move {
                    what: this_card(),
                    to: Destination::Hand,
                    bind: None,
                }],
                unpaid: vec![Effect::Exile {
                    what: this_card(),
                    bind: None,
                    duration: None,
                }],
            }],
        }]
    }
}

#[cfg(test)]
mod tests {
    //! Goal: recover adds one graveyard trigger on the death of another creature its owner owns that pays the cost to return the card to the hand and exiles it otherwise; no other keyword adds it.
    use mtg_cards::cost::Cost;

    use super::*;

    #[test]
    fn the_trigger_pays_to_return_the_card_or_exiles_it() {
        let cost = Cost::mana("{R}{R}".parse().unwrap());
        let added = Recover.adds_abilities(&Keyword::Recover(cost.clone()));
        let [
            Ability::Triggered {
                when: TriggerPattern::ZoneChange { what, from, to },
                from: zone,
                effects,
                ..
            },
        ] = added.as_slice()
        else {
            panic!("one triggered ability: {added:?}");
        };
        assert_eq!(*zone, ZoneKind::Graveyard);
        assert_eq!(
            (*from, *to),
            (Some(ZoneKind::Battlefield), Some(ZoneKind::Graveyard))
        );
        assert!(what.0.contains(&Pred::OwnedBy(Ref::You)));
        let [
            Effect::Pay {
                cost: paid_cost,
                paid,
                unpaid,
                ..
            },
        ] = effects.as_slice()
        else {
            panic!("one payment: {effects:?}");
        };
        assert_eq!(*paid_cost, cost);
        assert!(matches!(
            paid.as_slice(),
            [Effect::Move {
                to: Destination::Hand,
                ..
            }]
        ));
        assert!(matches!(unpaid.as_slice(), [Effect::Exile { .. }]));
    }

    #[test]
    fn recover_adds_nothing_for_another_keyword() {
        assert_eq!(Recover.adds_abilities(&Keyword::Rebound), Vec::new());
    }
}
