//! Miracle (CR 702.94): "You may reveal this card from your hand as you draw it if it's the first card you've drawn
//! this turn. When you reveal it this way, you may cast it by paying [cost] rather than its mana cost."
//!
//! The keyword adds a trigger that works from a hand: it fires when its owner draws this card as the first card they
//! drew this turn, and names the card to every seat while it waits on the stack. The trigger casts the card during its
//! resolution (`Effect::CastFromZone` with the miracle way) if the card is still in the hand it was drawn into.

use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Ref, Selector, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::CastWay;

use super::KeywordRules;

/// The rules of miracle.
#[derive(Debug)]
pub(super) struct Miracle;

impl KeywordRules for Miracle {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        if !matches!(keyword, Keyword::Miracle(_)) {
            return Vec::new();
        }
        vec![Ability::Triggered {
            when: TriggerPattern::Nth {
                event: Box::new(TriggerPattern::Drew {
                    who: Ref::You,
                    what: Selector::this(),
                }),
                nth: 1,
            },
            intervening_if: None,
            limit: None,
            from: ZoneKind::Hand,
            targets: Vec::new(),
            effects: vec![Effect::CastFromZone {
                who: Ref::You,
                what: Ref::This,
                free: false,
                way: Some(CastWay::Miracle),
                otherwise: Vec::new(),
            }],
        }]
    }
}

#[cfg(test)]
mod tests {
    //! Goal: Miracle adds one trigger that works from a hand, fires on the first draw of the turn of this card and casts it for the miracle way, and no other keyword adds it.
    use mtg_cards::cost::Cost;

    use super::*;

    #[test]
    fn miracle_triggers_from_the_hand_on_the_first_draw_of_this_card() {
        let added = Miracle.adds_abilities(&Keyword::Miracle(Cost::mana("{W}".parse().unwrap())));
        let [
            Ability::Triggered {
                when: TriggerPattern::Nth { event, nth: 1 },
                from: ZoneKind::Hand,
                effects,
                ..
            },
        ] = added.as_slice()
        else {
            panic!("one trigger from the hand: {added:?}");
        };
        assert!(matches!(**event, TriggerPattern::Drew { .. }));
        assert!(matches!(
            effects.as_slice(),
            [Effect::CastFromZone {
                way: Some(CastWay::Miracle),
                ..
            }]
        ));
    }

    #[test]
    fn no_other_keyword_adds_it() {
        assert_eq!(Miracle.adds_abilities(&Keyword::Flying), []);
    }
}
