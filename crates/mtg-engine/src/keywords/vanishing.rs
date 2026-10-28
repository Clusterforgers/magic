//! Vanishing (CR 702.63): "This permanent enters with N time counters on it", "At the beginning of your upkeep, remove
//! a time counter from it" and "When the last is removed, sacrifice it."

use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Ref, Selector, ZoneKind};
use mtg_cards::trigger::{ReplaceBy, ReplaceWhen, Step, TriggerPattern};
use mtg_cards::value::{Amount, Cmp, Condition, CounterKind};

use super::KeywordRules;
use super::suspend::time_counter;

/// The rules of vanishing.
#[derive(Debug)]
pub(super) struct Vanishing;

/// The replacement that makes a permanent enter with `count` counters of `kind` (vanishing and fading); none when the
/// count is 0.
pub(super) fn enters_with(kind: CounterKind, count: u32) -> Option<Ability> {
    let count = i32::try_from(count).ok().filter(|count| *count > 0)?;
    Some(Ability::Replacement {
        when: ReplaceWhen::Enters {
            what: Selector::this(),
        },
        by: ReplaceBy::WithCounters {
            counter: kind,
            count: Amount::Fixed(count),
            tapped: false,
        },
        condition: None,
        from: ZoneKind::Battlefield,
    })
}

/// The beginning of the controller's upkeep.
pub(super) fn upkeep() -> TriggerPattern {
    TriggerPattern::StepBegins {
        step: Step::Upkeep,
        whose: Ref::You,
    }
}

/// The number of counters of `kind` on the permanent compares to `to` this way.
pub(super) fn counters_are(kind: &CounterKind, op: Cmp, to: i32) -> Condition {
    Condition::Compare {
        left: Amount::Counters {
            on: Ref::This,
            counter: kind.clone(),
        },
        op,
        right: Amount::Fixed(to),
    }
}

impl KeywordRules for Vanishing {
    /// The upkeep trigger asks for a time counter on the permanent when it is put on the stack and as it resolves; the
    /// sacrifice follows the removal of the last, whatever removes it, and asks for none left as it resolves.
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        let Keyword::Vanishing(count) = keyword else {
            return Vec::new();
        };
        let time = time_counter();
        let mut abilities: Vec<Ability> = enters_with(time.clone(), *count).into_iter().collect();
        abilities.push(Ability::Triggered {
            when: upkeep(),
            intervening_if: Some(counters_are(&time, Cmp::Ge, 1)),
            limit: None,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: vec![Effect::RemoveCounters {
                from: Ref::This,
                counter: Some(time.clone()),
                count: Amount::one(),
                up_to: false,
            }],
        });
        abilities.push(Ability::Triggered {
            when: TriggerPattern::CountersRemoved {
                what: Selector::this(),
                counter: Some(time.clone()),
            },
            intervening_if: Some(counters_are(&time, Cmp::Eq, 0)),
            limit: None,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: vec![Effect::Sacrifice {
                who: Ref::You,
                filter: Selector::this(),
                count: Amount::one(),
            }],
        });
        abilities
    }
}
