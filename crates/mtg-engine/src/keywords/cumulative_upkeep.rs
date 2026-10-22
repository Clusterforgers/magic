//! Cumulative upkeep (CR 702.24): "At the beginning of your upkeep, put an age counter on this permanent, then sacrifice
//! it unless you pay its upkeep cost for each age counter on it."

use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Ref, Selector, ZoneKind};
use mtg_cards::trigger::{Step, TriggerPattern};
use mtg_cards::value::{Amount, CounterKind};

use super::KeywordRules;

/// The rules of cumulative upkeep.
#[derive(Debug)]
pub(super) struct CumulativeUpkeep;

/// The kind of counter cumulative upkeep counts.
fn age() -> CounterKind {
    CounterKind::Other("age".to_owned())
}

impl KeywordRules for CumulativeUpkeep {
    /// The cost is paid once for each age counter, counted after the new one is on: a permanent that has left has none,
    /// so nothing is asked.
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        let Keyword::CumulativeUpkeep(cost) = keyword else {
            return Vec::new();
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
                Effect::Pay {
                    who: Ref::You,
                    cost: cost.clone(),
                    times: Amount::Counters {
                        on: Ref::This,
                        counter: age(),
                    },
                    paid: Vec::new(),
                    unpaid: vec![Effect::Sacrifice {
                        who: Ref::You,
                        filter: Selector::this(),
                        count: Amount::one(),
                    }],
                },
            ],
        }]
    }
}
