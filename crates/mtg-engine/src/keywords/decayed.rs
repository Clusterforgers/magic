//! Decayed (CR 702.147): "This creature can't block." and "When this creature attacks, sacrifice it at end of combat."
//!
//! The first half is the hook `restricts_blocked`: no attacker can be blocked by a creature with decayed. The second is
//! an attack trigger that sets a delayed trigger for the end of combat step; it names the creature as it is when it
//! attacks, so a creature that left the battlefield and came back is not sacrificed (CR 400.7). Each instance of the
//! keyword is its own trigger, as for any keyword that is a triggered ability.

use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Ref, Selector, ZoneKind};
use mtg_cards::trigger::{Step, TriggerPattern};
use mtg_cards::value::Amount;
use mtg_core::ids::ObjectId;

use super::KeywordRules;
use crate::state::GameState;

/// The rules of decayed.
#[derive(Debug)]
pub(super) struct Decayed;

impl KeywordRules for Decayed {
    fn restricts_blocked(
        &self,
        _state: &GameState,
        _keyword: &Keyword,
        _attacker: ObjectId,
        _blocker: ObjectId,
    ) -> bool {
        true
    }

    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        if !matches!(keyword, Keyword::Decayed) {
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
            effects: vec![Effect::DelayedTrigger {
                when: TriggerPattern::StepBegins {
                    step: Step::EndCombat,
                    whose: Ref::EachPlayer,
                },
                once: true,
                effects: vec![Effect::Sacrifice {
                    who: Ref::ControllerOf(Box::new(Ref::This)),
                    filter: Selector(vec![Pred::Is(Ref::This)]),
                    count: Amount::one(),
                }],
            }],
        }]
    }
}

#[cfg(test)]
mod tests {
    //! Goal: Decayed adds one attack trigger that sacrifices the creature at the end of combat and refuses every block by its holder, and no other keyword adds a trigger.
    use mtg_cards::cost::Cost;
    use rstest::rstest;

    use super::*;
    use crate::fixtures::{creature, state};

    #[test]
    fn decayed_adds_an_attack_trigger_that_sacrifices_the_creature_at_the_end_of_combat() {
        let added = Decayed.adds_abilities(&Keyword::Decayed);
        let [
            Ability::Triggered {
                when: TriggerPattern::Attacks { what },
                effects,
                ..
            },
        ] = added.as_slice()
        else {
            panic!("one attack trigger, got {added:?}");
        };
        assert_eq!(*what, Selector::this());
        let [
            Effect::DelayedTrigger {
                when:
                    TriggerPattern::StepBegins {
                        step: Step::EndCombat,
                        ..
                    },
                once: true,
                effects,
            },
        ] = effects.as_slice()
        else {
            panic!("a delayed trigger of the end of combat, got {effects:?}");
        };
        assert!(matches!(effects.as_slice(), [Effect::Sacrifice { .. }]));
    }

    #[test]
    fn a_creature_with_decayed_may_block_nothing() {
        let game = state(
            vec![vec![], vec![]],
            vec![vec![creature("Bear", 2, 2)], vec![creature("Zombie", 2, 2)]],
        );
        let ids = game.zone(crate::zones::Zone::Battlefield).to_vec();
        assert!(Decayed.restricts_blocked(&game, &Keyword::Decayed, ids[0], ids[1]));
    }

    #[rstest]
    #[case::riot(Keyword::Riot)]
    #[case::haste(Keyword::Haste)]
    #[case::squad(Keyword::Squad(Cost::default()))]
    fn no_other_keyword_adds_it(#[case] keyword: Keyword) {
        assert_eq!(Decayed.adds_abilities(&keyword), []);
    }
}
