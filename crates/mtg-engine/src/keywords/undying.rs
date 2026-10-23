//! Undying (CR 702.93): "When this permanent dies, if it had no +1/+1 counters on it, return it to the battlefield under
//! its owner's control with a +1/+1 counter on it."

use mtg_cards::ability::Ability;
use mtg_cards::effect::{Destination, Effect};
use mtg_cards::keyword::Keyword;
use mtg_cards::refs::{Pred, Ref, Selector, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::value::{Amount, Cmp, Condition, CounterKind};

use super::KeywordRules;

/// The rules of undying.
#[derive(Debug)]
pub(super) struct Undying;

/// The trigger of undying and persist: when the permanent dies without a counter of `kind`, the card that is now in a
/// graveyard returns with one. The condition reads the permanent as it was (last known information) when the trigger
/// is judged and again as it resolves; the card is found again by its identity, so one that has left the graveyard
/// since stays where it is (CR 400.7).
pub(super) fn returns_with(kind: CounterKind) -> Vec<Ability> {
    vec![Ability::Triggered {
        when: TriggerPattern::ZoneChange {
            what: Selector::this(),
            from: Some(ZoneKind::Battlefield),
            to: Some(ZoneKind::Graveyard),
        },
        intervening_if: Some(Condition::Compare {
            left: Amount::Counters {
                on: Ref::This,
                counter: kind.clone(),
            },
            op: Cmp::Eq,
            right: Amount::Fixed(0),
        }),
        limit: None,
        from: ZoneKind::Battlefield,
        targets: Vec::new(),
        effects: vec![Effect::Move {
            what: Ref::Each(Selector(vec![Pred::In(ZoneKind::Graveyard), Pred::This])),
            to: Destination::Battlefield {
                tapped: false,
                under: None,
                transformed: false,
                counters: vec![(kind, 1)],
            },
        }],
    }]
}

impl KeywordRules for Undying {
    fn adds_abilities(&self, _keyword: &Keyword) -> Vec<Ability> {
        returns_with(CounterKind::PlusOnePlusOne)
    }
}
