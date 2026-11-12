//! Cumulative upkeep whose cost is an action (CR 702.24): "At the beginning of your upkeep, put an age counter on this
//! permanent, then sacrifice it unless you [do the actions] for each age counter on it."
//!
//! The controller chooses between doing the actions once for each age counter and sacrificing the permanent; the actions
//! are offered only while the keyword's `payable` condition holds (CR 118.3), and a permanent that left has no counter, so
//! the actions are done no time at all.

use mtg_cards::ability::Ability;
use mtg_cards::effect::{Effect, MOST_PASSES};
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Ref, Selector, ZoneKind};
use mtg_cards::trigger::{Step, TriggerPattern};
use mtg_cards::value::{Amount, Cmp, Condition, CounterKind};

use super::KeywordRules;

/// The rules of cumulative upkeep with an action for its cost.
#[derive(Debug)]
pub(super) struct CumulativeUpkeepActs;

/// The kind of counter cumulative upkeep counts.
fn age() -> CounterKind {
    CounterKind::Other("age".to_owned())
}

/// The sacrifice of the permanent, the other choice.
fn sacrifice() -> Vec<Effect> {
    vec![Effect::Sacrifice {
        who: Ref::You,
        filter: Selector::this(),
        count: Amount::one(),
        bind: None,
    }]
}

impl KeywordRules for CumulativeUpkeepActs {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        let Keyword::CumulativeUpkeepActs { acts, payable } = keyword else {
            return Vec::new();
        };
        let counters = Amount::Counters {
            on: Ref::This,
            counter: age(),
        };
        // Done once for each age counter: a pass binds its own names, so "an opponent" is chosen anew each time.
        let pay = vec![Effect::RepeatWhile {
            condition: Condition::Compare {
                left: counters.clone(),
                op: Cmp::Ge,
                right: Amount::one(),
            },
            effects: acts.clone(),
            most: MOST_PASSES,
            times: Some(counters),
            at_least_once: false,
            ask: None,
        }];
        let choice = Effect::OneOf {
            who: Ref::You,
            options: vec![pay, sacrifice()],
        };
        let decide = match payable {
            None => choice,
            Some(condition) => Effect::If {
                condition: condition.clone(),
                then: vec![choice],
                otherwise: sacrifice(),
            },
        };
        vec![Ability::Triggered {
            when: TriggerPattern::StepBegins {
                step: Step::Upkeep,
                whose: Ref::You,
            },
            intervening_if: None,
            limit: None,
            from: ZoneKind::Battlefield,
            targets: Vec::new(),
            effects: vec![
                Effect::PutCounters {
                    on: Ref::This,
                    counter: age(),
                    count: Amount::one(),
                },
                decide,
            ],
        }]
    }
}
