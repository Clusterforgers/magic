//! Annihilator N (CR 702.86): "Whenever this creature attacks, defending player sacrifices N permanents."

use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Ref, Selector, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::Amount;

use super::KeywordRules;

/// The rules of annihilator.
#[derive(Debug)]
pub(super) struct Annihilator;

impl KeywordRules for Annihilator {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        let Keyword::Annihilator(count) = keyword else {
            return Vec::new();
        };
        vec![Ability::Triggered {
            when: TriggerPattern::Attacks {
                what: Selector::this(),
            },
            intervening_if: None,
            limit: None,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: vec![Effect::Sacrifice {
                who: Ref::Defending,
                filter: Selector::default(),
                count: Amount::Fixed(i32::try_from(*count).unwrap_or(i32::MAX)),
            }],
        }]
    }
}
