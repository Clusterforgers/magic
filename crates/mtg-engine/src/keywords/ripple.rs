//! Ripple N (CR 702.60a): "When you cast this spell, you may reveal the top N cards of your library. If you do, you may
//! cast spells with the same name as this spell from among cards revealed this way without paying their mana costs. Then
//! put the rest on the bottom of your library."
//!
//! The keyword adds a trigger that works from the stack, like cascade's. The revealed cards with the spell's name are
//! taken once, as the reveal happens, and each is offered in turn (`Effect::CastFromZone`); the cards not cast go to the
//! bottom in the order their owner chooses. A card cast this way is cast, so its own ripple triggers in its turn
//! (CR 702.60b).

use mtg_cards::ability::Ability;
use mtg_cards::effect::{Destination, Effect, Order, Place};
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Ref, Selector, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::Amount;

use super::KeywordRules;

/// The rules of ripple.
#[derive(Debug)]
pub(super) struct Ripple;

/// The name the revealed cards are bound to.
const REVEALED: &str = "rippled";
/// The name of the card a pass offers.
const OFFERED: &str = "ripple";

/// The revealed cards still in the library.
fn revealed_left() -> Ref {
    Ref::Each(Selector(vec![
        Pred::Is(Ref::Bound(REVEALED.to_owned())),
        Pred::In(ZoneKind::Library),
    ]))
}

/// The steps of ripple `count`.
fn ripple(count: u32) -> Vec<Effect> {
    vec![Effect::May {
        who: Ref::You,
        effects: vec![
            Effect::LookAtTop {
                who: Ref::You,
                count: Amount::Fixed(i32::try_from(count).unwrap_or(i32::MAX)),
                bind: REVEALED.to_owned(),
            },
            Effect::Reveal {
                what: Ref::Bound(REVEALED.to_owned()),
            },
            Effect::RepeatEach {
                over: Ref::Each(Selector(vec![
                    Pred::Is(Ref::Bound(REVEALED.to_owned())),
                    Pred::In(ZoneKind::Library),
                    Pred::SharesName(Ref::This),
                ])),
                bind: OFFERED.to_owned(),
                effects: vec![Effect::CastFromZone {
                    rather: None,
                    spend_as: Default::default(),
                    who: Ref::You,
                    what: Ref::Bound(OFFERED.to_owned()),
                    free: true,
                    play: false,
                    way: None,
                    transformed: false,
                    bind: None,
                    otherwise: Vec::new(),
                }],
            },
            Effect::Move {
                what: revealed_left(),
                to: Destination::Library {
                    place: Place::Bottom,
                    order: Some(Order::Chosen),
                },
                bind: None,
            },
        ],
    }]
}

impl KeywordRules for Ripple {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        let Keyword::Ripple(count) = keyword else {
            return Vec::new();
        };
        vec![Ability::Triggered {
            when: TriggerPattern::SpellCast {
                what: Selector::this(),
                by: Ref::You,
            },
            intervening_if: None,
            limit: None,
            from: ZoneKind::Stack,
            targets: Vec::new(),
            effects: ripple(*count),
        }]
    }
}

#[cfg(test)]
mod tests {
    //! Goal: ripple adds one cast trigger from the stack that may reveal the top N cards, offers each with the spell's name for free and puts the rest on the bottom; no other keyword adds it.
    use super::*;

    #[test]
    fn the_trigger_reveals_offers_and_puts_the_rest_on_the_bottom() {
        let added = Ripple.adds_abilities(&Keyword::Ripple(4));
        let [
            Ability::Triggered {
                when: TriggerPattern::SpellCast { .. },
                from: ZoneKind::Stack,
                effects,
                ..
            },
        ] = added.as_slice()
        else {
            panic!("one cast trigger: {added:?}");
        };
        let [Effect::May { effects: steps, .. }] = effects.as_slice() else {
            panic!("a may: {effects:?}");
        };
        assert!(matches!(
            steps.as_slice(),
            [
                Effect::LookAtTop {
                    count: Amount::Fixed(4),
                    ..
                },
                Effect::Reveal { .. },
                Effect::RepeatEach { .. },
                Effect::Move {
                    to: Destination::Library {
                        place: Place::Bottom,
                        ..
                    },
                    ..
                }
            ]
        ));
    }

    #[test]
    fn ripple_adds_nothing_for_another_keyword() {
        assert_eq!(Ripple.adds_abilities(&Keyword::Storm), Vec::new());
    }
}
