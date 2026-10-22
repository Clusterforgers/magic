//! Evolve (CR 702.100): "Whenever a creature enters under your control, if that creature has greater power or toughness
//! than this creature, put a +1/+1 counter on this creature."

use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Ref, Selector, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::{Amount, Cmp, Condition, CounterKind, StatKind};
use mtg_core::card::CardType;

use super::KeywordRules;

/// The rules of evolve.
#[derive(Debug)]
pub(super) struct Evolve;

/// The entering creature has more of `stat` than the creature with evolve.
fn greater(stat: StatKind) -> Condition {
    Condition::Compare {
        left: Amount::Stat {
            of: Ref::Triggering,
            stat,
        },
        op: Cmp::Gt,
        right: Amount::Stat {
            of: Ref::This,
            stat,
        },
    }
}

impl KeywordRules for Evolve {
    fn adds_abilities(&self, _keyword: &Keyword) -> Vec<Ability> {
        vec![Ability::Triggered {
            when: TriggerPattern::ZoneChange {
                what: Selector(vec![
                    Pred::Type(CardType::Creature),
                    Pred::ControlledBy(Ref::You),
                ]),
                from: None,
                to: Some(ZoneKind::Battlefield),
            },
            intervening_if: Some(Condition::Any(vec![
                greater(StatKind::Power),
                greater(StatKind::Toughness),
            ])),
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
