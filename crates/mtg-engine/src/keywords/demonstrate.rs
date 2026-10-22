//! Demonstrate (CR 702.144): "When you cast this spell, you may copy it. If you do, choose an opponent to also copy it.
//! Players may choose new targets for their copies."
//!
//! The keyword adds a trigger that works from the stack. Its controller's copy goes on the stack first and the
//! opponent's copy above it, so the opponent's resolves first; each copy is controlled by its own player, who chooses
//! its new targets. A copy is not cast, so it has no demonstrate trigger of its own.

use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Bounds, Ref, Selector, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::Amount;

use super::KeywordRules;

/// The name the chosen opponent is bound to.
const OPPONENT: &str = "demonstrated";

/// The rules of demonstrate.
#[derive(Debug)]
pub(super) struct Demonstrate;

fn copy_by(who: Ref) -> Effect {
    Effect::CopySpell {
        who,
        what: Ref::This,
        count: Amount::one(),
        new_targets: true,
        except: Vec::new(),
    }
}

impl KeywordRules for Demonstrate {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        if !matches!(keyword, Keyword::Demonstrate) {
            return Vec::new();
        }
        vec![Ability::Triggered {
            when: TriggerPattern::SpellCast {
                what: Selector::this(),
                by: Ref::You,
            },
            intervening_if: None,
            limit: None,
            from: ZoneKind::Stack,
            targets: Vec::new(),
            effects: vec![Effect::May {
                who: Ref::You,
                effects: vec![
                    copy_by(Ref::You),
                    Effect::Choose {
                        who: Ref::You,
                        from: Ref::Opponents,
                        count: Bounds::default(),
                        bind: OPPONENT.to_owned(),
                        rest_as: None,
                    },
                    copy_by(Ref::Bound(OPPONENT.to_owned())),
                ],
            }],
        }]
    }
}

#[cfg(test)]
mod tests {
    //! Goal: Demonstrate adds one cast trigger that may copy the spell for its controller, chooses an opponent and copies it for them, and no other keyword does.
    use rstest::rstest;

    use super::*;

    #[test]
    fn demonstrate_adds_a_cast_trigger_with_a_copy_for_each_of_two_players() {
        let added = Demonstrate.adds_abilities(&Keyword::Demonstrate);
        let [
            Ability::Triggered {
                when: TriggerPattern::SpellCast { .. },
                from,
                effects,
                ..
            },
        ] = added.as_slice()
        else {
            panic!("one cast trigger, got {added:?}");
        };
        assert_eq!(*from, ZoneKind::Stack);
        let [Effect::May { effects, .. }] = effects.as_slice() else {
            panic!("one may, got {effects:?}");
        };
        assert!(matches!(
            effects.as_slice(),
            [
                Effect::CopySpell { who: Ref::You, .. },
                Effect::Choose {
                    from: Ref::Opponents,
                    ..
                },
                Effect::CopySpell {
                    who: Ref::Bound(_),
                    ..
                },
            ]
        ));
    }

    #[rstest]
    #[case::storm(Keyword::Storm)]
    #[case::casualty(Keyword::Casualty(2))]
    #[case::hideaway(Keyword::Hideaway(4))]
    fn no_other_keyword_does(#[case] keyword: Keyword) {
        assert_eq!(Demonstrate.adds_abilities(&keyword), []);
    }
}
