//! Dethrone (CR 702.105): "Whenever this creature attacks the player with the most life or tied for most life, put a
//! +1/+1 counter on this creature."

use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Ref, Selector, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::{Amount, CounterKind};

use super::KeywordRules;

/// The rules of dethrone.
#[derive(Debug)]
pub(super) struct Dethrone;

impl KeywordRules for Dethrone {
    fn adds_abilities(&self, _keyword: &Keyword) -> Vec<Ability> {
        vec![Ability::Triggered {
            when: TriggerPattern::AttacksPlayer {
                what: Selector::this(),
                at: Ref::MostLife,
            },
            intervening_if: None,
            limit: None,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: vec![Effect::PutCounters {
                on: Ref::This,
                counter: CounterKind::PlusOnePlusOne,
                count: Amount::one(),
            }],
        }]
    }
}
