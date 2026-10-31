//! Suspend (CR 702.62): "If you could begin to cast this card by putting it onto the stack from your hand, you may pay
//! [cost] and exile it with [N] time counters on it. This action doesn't use the stack", "At the beginning of your
//! upkeep, if this card is suspended, remove a time counter from it" and "When the last time counter is removed from
//! this card, if it's exiled, you may cast it without paying its mana cost. If you cast a creature spell this way, it
//! gains haste until you lose control of the spell or the permanent it becomes."
//!
//! The special action is not an ability: `priority.rs` offers it (`PriorityAction::Suspend`) and `announce/suspend.rs`
//! pays for it. The keyword adds the two triggers, which work from exile, and a static ability that gives a creature
//! cast this way haste while its owner controls it.

use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::keyword::Keyword;
use mtg_cards::modify::Modification;
use mtg_cards::refs::{Pred, Ref, Selector, ZoneKind};
use mtg_cards::trigger::{Step, TriggerPattern};
use mtg_cards::value::{Amount, CastWay, Cmp, Condition, CounterKind};

use super::KeywordRules;

/// The kind of counter a suspended card holds.
pub(crate) fn time_counter() -> CounterKind {
    CounterKind::Other("time".to_owned())
}

/// The rules of suspend.
#[derive(Debug)]
pub(super) struct Suspend;

/// The time counters on the card compare to `to` this way.
fn time_counters(op: Cmp, to: i32) -> Condition {
    Condition::Compare {
        left: Amount::Counters {
            on: Ref::This,
            counter: time_counter(),
        },
        op,
        right: Amount::Fixed(to),
    }
}

impl KeywordRules for Suspend {
    fn adds_abilities(&self, keyword: &Keyword) -> Vec<Ability> {
        if !matches!(keyword, Keyword::Suspend { .. }) {
            return Vec::new();
        }
        vec![
            Ability::Triggered {
                when: TriggerPattern::StepBegins {
                    step: Step::Upkeep,
                    whose: Ref::You,
                },
                intervening_if: Some(time_counters(Cmp::Ge, 1)),
                limit: None,
                from: ZoneKind::Exile,
                targets: Vec::new(),
                effects: vec![Effect::RemoveCounters {
                    from: Ref::This,
                    counter: Some(time_counter()),
                    count: Amount::one(),
                    up_to: false,
                }],
            },
            Ability::Triggered {
                when: TriggerPattern::CountersRemoved {
                    what: Selector::this(),
                    counter: Some(time_counter()),
                },
                intervening_if: Some(time_counters(Cmp::Eq, 0)),
                limit: None,
                from: ZoneKind::Exile,
                targets: Vec::new(),
                effects: vec![Effect::CastFromZone {
                    who: Ref::You,
                    what: Ref::This,
                    free: true,
                    play: false,
                    way: Some(CastWay::Suspend),
                    transformed: false,
                    bind: None,
                    otherwise: Vec::new(),
                }],
            },
            Ability::Static {
                affected: Ref::This,
                mods: vec![Modification::AddKeyword(Keyword::Haste)],
                condition: Some(Condition::All(vec![
                    Condition::CastWith(CastWay::Suspend),
                    Condition::Matches {
                        subject: Ref::This,
                        selector: Selector(vec![
                            Pred::ControlledBy(Ref::You),
                            Pred::OwnedBy(Ref::You),
                        ]),
                    },
                ])),
                defining: false,
                from: ZoneKind::Battlefield,
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    //! Goal: Suspend adds the upkeep trigger and the trigger of the last counter that work from exile, and the haste of a creature cast this way that works on the battlefield, and no other keyword adds them.
    use mtg_cards::cost::Cost;

    use super::*;

    #[test]
    fn suspend_adds_two_triggers_from_exile_and_a_static_of_haste() {
        let keyword = Keyword::Suspend {
            time: 3,
            cost: Cost::mana("{R}".parse().unwrap()),
        };
        let added = Suspend.adds_abilities(&keyword);
        assert!(matches!(
            added.as_slice(),
            [
                Ability::Triggered {
                    from: ZoneKind::Exile,
                    ..
                },
                Ability::Triggered {
                    from: ZoneKind::Exile,
                    ..
                },
                Ability::Static {
                    from: ZoneKind::Battlefield,
                    ..
                }
            ]
        ));
    }

    #[test]
    fn no_other_keyword_adds_it() {
        assert_eq!(Suspend.adds_abilities(&Keyword::Haste), []);
    }
}
