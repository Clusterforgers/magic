//! r47-04 wording: "Whenever ~ deals [combat ]damage to a creature, if that creature was dealt excess damage this turn,
//! you get X {E}, where X is that excess damage." (the excess counted from the turn's log of that creature).

use mtg_core::card::CardType;

use super::ClauseKind;
use crate::ability::Ability;
use crate::effect::Effect;
use crate::refs::{Pred, Ref, Selector, ZoneKind};
use crate::trigger::TriggerPattern;
use crate::value::{Amount, Condition, CounterKind, TallyEvent};

/// The head of a trigger on a creature this one dealt damage to that was dealt excess damage this turn.
const R4704_EXCESS_HEAD: &str = ", if that creature was dealt excess damage this turn, you get X {E}, where X is that excess damage.";

/// The excess damage dealt this turn to the creature the trigger is about.
fn r4704_that_excess() -> Amount {
    Amount::Tally {
        event: TallyEvent::DealtExcess,
        who: Ref::EachPlayer,
        filter: Selector(vec![Pred::Is(Ref::Triggering)]),
    }
}

/// "Whenever ~ deals [combat ]damage to a creature, if that creature was dealt excess damage this turn, you get X {E},
/// where X is that excess damage."
fn r4704_excess_energy(combat: Option<bool>) -> Ability {
    Ability::Triggered {
        when: TriggerPattern::DealsDamage {
            source: Selector::this(),
            to: Some(Ref::Each(Selector(vec![Pred::Type(CardType::Creature)]))),
            combat,
        },
        intervening_if: Some(Condition::Matches {
            subject: Ref::Triggering,
            selector: Selector(vec![Pred::DidThisTurn {
                event: TallyEvent::DealtExcess,
                by: None,
                to: None,
                combat: false,
            }]),
        }),
        limit: None,
        from: ZoneKind::Battlefield,
        targets: Vec::new(),
        effects: vec![Effect::PutCounters {
            on: Ref::You,
            counter: CounterKind::Other("energy".to_owned()),
            count: r4704_that_excess(),
        }],
    }
}

/// The trigger heads `r4704_excess_energy` is written with.
fn r4704_excess_heads() -> [(Option<bool>, &'static str); 2] {
    [
        (Some(true), "Whenever ~ deals combat damage to a creature"),
        (None, "Whenever ~ deals damage to a creature"),
    ]
}

/// The ability of a wording of this pack.
pub(super) fn r4704_read(text: &str, kind: ClauseKind) -> Option<Ability> {
    if kind != ClauseKind::Triggered {
        return None;
    }
    for (combat, head) in r4704_excess_heads() {
        if text.strip_prefix(head) == Some(R4704_EXCESS_HEAD) {
            return Some(r4704_excess_energy(combat));
        }
    }
    None
}

/// The wordings of the abilities of this pack.
pub(super) fn r4704_wordings(ability: &Ability, texts: &mut Vec<String>) {
    for (combat, head) in r4704_excess_heads() {
        if *ability == r4704_excess_energy(combat) {
            texts.push(format!("{head}{R4704_EXCESS_HEAD}"));
        }
    }
}
