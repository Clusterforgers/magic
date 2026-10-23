//! Myriad (CR 702.116): "Whenever this creature attacks, for each opponent other than defending player, you may create
//! a token that's a copy of this creature that's tapped and attacking that player or a planeswalker they control. Exile
//! the tokens at end of combat."
//!
//! The keyword adds an attack trigger that makes the copies (`Effect::MyriadCopies`) and sets a delayed trigger that
//! exiles the tokens it bound at the end of combat. With one opponent there is no one else to attack, so nothing is
//! asked; otherwise one choice covers all the copies.

use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Ref, Selector, ZoneKind};
use mtg_cards::trigger::{Step, TriggerPattern};
use mtg_cards::value::{Amount, Cmp, Condition};

use super::KeywordRules;

/// The name the tokens of one myriad trigger are bound to.
const COPIES: &str = "myriad";

/// The rules of myriad.
#[derive(Debug)]
pub(super) struct Myriad;

impl KeywordRules for Myriad {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        if !matches!(keyword, Keyword::Myriad) {
            return Vec::new();
        }
        vec![Ability::Triggered {
            when: TriggerPattern::Attacks {
                what: Selector::this(),
            },
            intervening_if: None,
            limit: None,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: vec![Effect::If {
                condition: Condition::Compare {
                    left: Amount::Players(Ref::Opponents),
                    op: Cmp::Gt,
                    right: Amount::Fixed(1),
                },
                then: vec![Effect::May {
                    who: Ref::You,
                    effects: vec![
                        Effect::MyriadCopies {
                            of: Ref::This,
                            bind: Some(COPIES.to_owned()),
                        },
                        Effect::DelayedTrigger {
                            when: TriggerPattern::StepBegins {
                                step: Step::EndCombat,
                                whose: Ref::EachPlayer,
                            },
                            once: true,
                            effects: vec![Effect::Exile {
                                what: Ref::Bound(COPIES.to_owned()),
                                bind: None,
                                duration: None,
                            }],
                        },
                    ],
                }],
                otherwise: Vec::new(),
            }],
        }]
    }
}

#[cfg(test)]
mod tests {
    //! Goal: Myriad adds one attack trigger that asks once, copies the attacker for the other opponents and exiles the tokens at the end of combat, and no other keyword adds it.
    use mtg_cards::cost::Cost;
    use rstest::rstest;

    use super::*;

    #[test]
    fn myriad_adds_an_attack_trigger_that_copies_and_exiles_at_the_end_of_combat() {
        let added = Myriad.adds_abilities(&Keyword::Myriad);
        let [
            Ability::Triggered {
                when: TriggerPattern::Attacks { .. },
                effects,
                ..
            },
        ] = added.as_slice()
        else {
            panic!("one attack trigger, got {added:?}");
        };
        let [Effect::If { then, .. }] = effects.as_slice() else {
            panic!("a test of the opponents, got {effects:?}");
        };
        let [Effect::May { effects, .. }] = then.as_slice() else {
            panic!("one question, got {then:?}");
        };
        assert!(matches!(
            effects.as_slice(),
            [
                Effect::MyriadCopies { .. },
                Effect::DelayedTrigger {
                    when: TriggerPattern::StepBegins {
                        step: Step::EndCombat,
                        ..
                    },
                    once: true,
                    ..
                }
            ]
        ));
    }

    #[rstest]
    #[case::squad(Keyword::Squad(Cost::default()))]
    #[case::haste(Keyword::Haste)]
    #[case::persist(Keyword::Persist)]
    fn no_other_keyword_adds_it(#[case] keyword: Keyword) {
        assert_eq!(Myriad.adds_abilities(&keyword), []);
    }
}
