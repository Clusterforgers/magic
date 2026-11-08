//! Epic (CR 702.50a): "For the rest of the game, you can't cast spells" and "For the rest of the game, at the beginning of
//! each of your upkeeps, copy this spell except for its epic ability. If the spell has any targets, you may choose new
//! targets for the copy."
//!
//! The two are spell abilities, so the spell ability of a card with epic gains them as its last steps (`epic`): a rule
//! that lasts for the game, and a delayed trigger that never lapses. The trigger keeps the spell as it resolved
//! (`copies::KeptSpell`): its copy is made from that once the spell has left the stack, without the keyword, so the
//! copies set nothing up. A copy is not cast, so the rule does not stop it.

use std::sync::Arc;

use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::modify::{Duration, GameRule, Modification};
use mtg_cards::refs::Ref;
use mtg_cards::trigger::{Step, TriggerPattern};
use mtg_cards::value::Amount;

use super::KeywordRules;
use crate::ability_reader;
use crate::object::GameObject;

/// The rules of epic: it adds no ability of its own (`epic` adds its steps to the spell ability).
#[derive(Debug)]
pub(super) struct Epic;

impl KeywordRules for Epic {}

/// The two spell abilities of epic, as steps.
fn steps() -> [Effect; 2] {
    [
        Effect::Rule {
            rule: GameRule::CantCastSpells {
                who: Ref::You,
                your_turn: false,
            },
            duration: Duration::Forever,
        },
        Effect::DelayedTrigger {
            when: TriggerPattern::StepBegins {
                step: Step::Upkeep,
                whose: Ref::You,
            },
            once: false,
            this_turn: false,
            until: Some(Duration::Forever),
            effects: vec![Effect::CopySpell {
                what: Ref::This,
                who: Ref::You,
                count: Amount::one(),
                new_targets: true,
                except: vec![Modification::RemoveKeyword(Keyword::Epic)],
                each_target: None,
            }],
        },
    ]
}

/// The rules of the spell `object` runs, with the steps of epic last when the spell has the keyword; other rules come
/// back as they are.
pub(crate) fn with_epic(object: &GameObject, rules: Arc<Ability>) -> Arc<Ability> {
    if !ability_reader::printed_rules(object.card())
        .any(|rule| matches!(**rule, Ability::Keyword(Keyword::Epic)))
    {
        return rules;
    }
    let Ability::Spell { targets, effects } = &*rules else {
        return rules;
    };
    let mut effects = effects.clone();
    effects.extend(steps());
    Arc::new(Ability::Spell {
        targets: targets.clone(),
        effects,
    })
}

#[cfg(test)]
mod tests {
    //! Goal: a spell with epic ends its spell ability with a rule against casting spells for the game and an upkeep trigger, lasting for the game, that copies the spell without its epic; a spell without the keyword keeps its rules.
    use mtg_cards::CardDefinition;
    use mtg_core::card::CardType;
    use mtg_core::ids::{ObjectId, PlayerId};
    use rstest::rstest;

    use super::*;
    use crate::zones::Zone;

    fn spell(keyword: bool) -> (GameObject, Arc<Ability>) {
        let rules = Arc::new(Ability::Spell {
            targets: Vec::new(),
            effects: vec![Effect::Draw {
                bind: None,
                who: Ref::You,
                count: Amount::one(),
            }],
        });
        let mut card = CardDefinition::new("Epic Spell")
            .with_type(CardType::Sorcery)
            .with_rules((*rules).clone());
        if keyword {
            card = card.with_rules(Ability::Keyword(Keyword::Epic));
        }
        let object = GameObject::new(ObjectId(1), PlayerId(0), Arc::new(card), Zone::Stack);
        (object, rules)
    }

    #[rstest]
    #[case::with_epic(true, 3)]
    #[case::without(false, 1)]
    fn the_spell_ability_gains_the_steps_of_epic(#[case] keyword: bool, #[case] steps: usize) {
        let (object, rules) = spell(keyword);
        let Ability::Spell { effects, .. } = &*with_epic(&object, rules) else {
            panic!("a spell ability");
        };
        assert_eq!(effects.len(), steps);
        if keyword {
            assert!(matches!(
                &effects[1],
                Effect::Rule {
                    rule: GameRule::CantCastSpells {
                        who: Ref::You,
                        your_turn: false
                    },
                    duration: Duration::Forever,
                }
            ));
            let Effect::DelayedTrigger {
                once,
                until,
                effects: copied,
                ..
            } = &effects[2]
            else {
                panic!("an upkeep trigger: {:?}", effects[2]);
            };
            assert!(!once);
            assert_eq!(*until, Some(Duration::Forever));
            assert!(matches!(
                copied.as_slice(),
                [Effect::CopySpell { what: Ref::This, new_targets: true, except, .. }]
                    if *except == [Modification::RemoveKeyword(Keyword::Epic)]
            ));
        }
    }
}
