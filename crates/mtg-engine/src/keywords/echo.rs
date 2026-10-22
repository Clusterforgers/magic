//! Echo (CR 702.30): "At the beginning of your upkeep, if this permanent came under your control since the beginning of
//! your last turn, sacrifice it unless you pay its echo cost."
//!
//! The fact "came under your control since your last turn began" is recorded per permanent
//! (`PermanentState::fresh`) when each turn begins and read by `Condition::NewlyControlled`.

use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Ref, Selector, ZoneKind};
use mtg_cards::trigger::{Step, TriggerPattern};
use mtg_cards::value::{Amount, Condition};

use super::KeywordRules;

/// The rules of echo.
#[derive(Debug)]
pub(super) struct Echo;

impl KeywordRules for Echo {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        let Keyword::Echo(cost) = keyword else {
            return Vec::new();
        };
        vec![Ability::Triggered {
            when: TriggerPattern::StepBegins {
                step: Step::Upkeep,
                whose: Ref::You,
            },
            intervening_if: Some(Condition::NewlyControlled(Ref::This)),
            limit: None,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: vec![Effect::Pay {
                who: Ref::You,
                cost: cost.clone(),
                times: Amount::one(),
                paid: Vec::new(),
                unpaid: vec![Effect::Sacrifice {
                    who: Ref::You,
                    filter: Selector::this(),
                    count: Amount::one(),
                }],
            }],
        }]
    }
}
