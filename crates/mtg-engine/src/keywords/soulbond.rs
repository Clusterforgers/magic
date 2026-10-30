//! Soulbond (CR 702.95): "When this creature enters, you may pair this creature with another unpaired creature you
//! control" and "Whenever another creature enters under your control, if you control both that creature and this one and
//! neither is paired, you may pair this creature with that creature."
//!
//! The first trigger exists only while an unpaired creature can be the partner, so a creature with nobody to pair with
//! does not ask. The pairing itself is state (`soulbond.rs`): `Effect::Pair` logs `Paired`, and the state-based pass ends
//! it with `Unpaired` when either creature leaves, stops being a creature or changes controller.

use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Bounds, Pred, Ref, Selector, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::{Amount, Cmp, Condition};
use mtg_core::card::CardType;

use super::KeywordRules;

/// The name the creature chosen as the partner is bound to.
const PARTNER: &str = "partner";

/// The rules of soulbond.
#[derive(Debug)]
pub(super) struct Soulbond;

/// A creature its controller controls that is paired with no one.
fn free_creature() -> Vec<Pred> {
    vec![
        Pred::Type(CardType::Creature),
        Pred::ControlledBy(Ref::You),
        Pred::Not(Box::new(Pred::Paired)),
    ]
}

impl KeywordRules for Soulbond {
    /// One enters trigger for the soulbond creature and one for the creatures that enter after it; each instance of the
    /// keyword has its own (CR 702.95c).
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        if !matches!(keyword, Keyword::Soulbond) {
            return Vec::new();
        }
        let mut others = free_creature();
        others.push(Pred::Other);
        let enters = Ability::Triggered {
            when: TriggerPattern::ZoneChange {
                what: Selector::this(),
                from: None,
                to: Some(ZoneKind::Battlefield),
            },
            intervening_if: Some(Condition::Compare {
                left: Amount::Count(Selector(others.clone())),
                op: Cmp::Ge,
                right: Amount::one(),
            }),
            limit: None,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: vec![Effect::May {
                who: Ref::You,
                effects: vec![
                    Effect::Choose {
                        who: Ref::You,
                        from: Ref::Each(Selector(others)),
                        count: Bounds::default(),
                        bind: PARTNER.to_owned(),
                        rest_as: None,
                    },
                    Effect::Pair {
                        what: Ref::This,
                        with: Ref::Bound(PARTNER.to_owned()),
                    },
                ],
            }],
        };
        let joined = Ability::Triggered {
            when: TriggerPattern::ZoneChange {
                what: Selector(vec![
                    Pred::Type(CardType::Creature),
                    Pred::Other,
                    Pred::ControlledBy(Ref::You),
                ]),
                from: None,
                to: Some(ZoneKind::Battlefield),
            },
            intervening_if: Some(Condition::All(
                [Ref::This, Ref::Triggering]
                    .into_iter()
                    .map(|subject| Condition::Matches {
                        subject,
                        selector: Selector(free_creature()),
                    })
                    .collect(),
            )),
            limit: None,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: vec![Effect::May {
                who: Ref::You,
                effects: vec![Effect::Pair {
                    what: Ref::This,
                    with: Ref::Triggering,
                }],
            }],
        };
        vec![enters, joined]
    }
}

#[cfg(test)]
mod tests {
    //! Goal: soulbond adds two enters triggers, one for the creature itself and one for another creature of its controller, each with an intervening "if" and an optional `Pair`, and nothing for another keyword.
    use super::*;

    #[test]
    fn soulbond_adds_two_optional_pairing_triggers() {
        let added = Soulbond.adds_abilities(&Keyword::Soulbond);
        assert_eq!(added.len(), 2);
        for ability in &added {
            let Ability::Triggered {
                when,
                intervening_if,
                effects,
                ..
            } = ability
            else {
                panic!("a trigger expected");
            };
            assert!(matches!(
                when,
                TriggerPattern::ZoneChange {
                    to: Some(ZoneKind::Battlefield),
                    ..
                }
            ));
            assert!(intervening_if.is_some());
            let [Effect::May { effects: inner, .. }] = effects.as_slice() else {
                panic!("one optional step expected");
            };
            assert!(matches!(inner.last(), Some(Effect::Pair { .. })));
        }
    }

    #[test]
    fn another_keyword_adds_nothing() {
        assert_eq!(Soulbond.adds_abilities(&Keyword::Exploit), Vec::new());
    }
}
