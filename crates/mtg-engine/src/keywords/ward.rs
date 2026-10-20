//! Ward (CR 702.21): "Whenever this permanent becomes the target of a spell or ability an opponent controls, counter
//! that spell or ability unless that player pays the ward cost."

use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Ref, Selector, ZoneKind};
use mtg_cards::trigger::TriggerPattern;

use super::KeywordRules;

/// The rules of ward.
#[derive(Debug)]
pub(super) struct Ward;

impl KeywordRules for Ward {
    /// The triggering objects of `BecomesTarget` are the permanent, the spell or ability that targets it and the
    /// player who controls that: the player pays, and the only one of the objects on the stack is what is countered.
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        let Keyword::Ward(cost) = keyword else {
            return Vec::new();
        };
        vec![Ability::Triggered {
            when: TriggerPattern::BecomesTarget {
                what: Selector::this(),
                by: Selector(vec![Pred::ControlledBy(Ref::Opponents)]),
            },
            intervening_if: None,
            limit: None,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: vec![Effect::Pay {
                who: Ref::Triggering,
                cost: cost.clone(),
                paid: Vec::new(),
                unpaid: vec![Effect::Counter {
                    what: Ref::Triggering,
                }],
            }],
        }]
    }
}
