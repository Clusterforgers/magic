//! Evoke (CR 702.74): "You may cast this spell for its evoke cost. If you do, it's sacrificed when it enters."
//!
//! The cost is an alternative cost chosen with the cast (`ways.rs`); the keyword adds the trigger that sacrifices the
//! permanent when it was cast for it.

use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Ref, Selector, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::{Amount, CastWay, Condition};

use super::KeywordRules;

/// The rules of evoke.
#[derive(Debug)]
pub(super) struct Evoke;

impl KeywordRules for Evoke {
    fn adds_abilities(&self, _keyword: &Keyword) -> Vec<Ability> {
        vec![Ability::Triggered {
            when: TriggerPattern::ZoneChange {
                what: Selector::this(),
                from: None,
                to: Some(ZoneKind::Battlefield),
            },
            intervening_if: Some(Condition::CastWith(CastWay::Evoke)),
            limit: None,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: vec![Effect::Sacrifice {
                who: Ref::You,
                filter: Selector::this(),
                count: Amount::one(),
            }],
        }]
    }
}
