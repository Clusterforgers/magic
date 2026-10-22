//! Mentor (CR 702.134): "Whenever this creature attacks, put a +1/+1 counter on target attacking creature with lesser
//! power."

use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Bounds, Pred, Ref, Selector, TargetSpec, Targetable, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::{Amount, Cmp, CounterKind, StatKind};
use mtg_core::card::CardType;

use super::KeywordRules;

/// The rules of mentor.
#[derive(Debug)]
pub(super) struct Mentor;

impl KeywordRules for Mentor {
    fn adds_abilities(&self, _keyword: &Keyword) -> Vec<Ability> {
        vec![Ability::Triggered {
            when: TriggerPattern::Attacks {
                what: Selector::this(),
            },
            intervening_if: None,
            limit: None,
            from: ZoneKind::Battlefield,
            targets: vec![TargetSpec {
                what: Targetable::Object(Selector(vec![
                    Pred::Type(CardType::Creature),
                    Pred::Attacking,
                    Pred::Compare {
                        stat: StatKind::Power,
                        op: Cmp::Lt,
                        to: Amount::Stat {
                            of: Ref::This,
                            stat: StatKind::Power,
                        },
                    },
                ])),
                bounds: Bounds::default(),
                divide: None,
            }],
            effects: vec![Effect::PutCounters {
                on: Ref::Target(0),
                counter: CounterKind::PlusOnePlusOne,
                count: Amount::one(),
            }],
        }]
    }
}
