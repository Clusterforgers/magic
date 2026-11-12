//! Facts of the turn's history written back (e16a), one row each: "you've sacrificed an artifact this turn", "you
//! sacrificed three or more Clues this turn", "for each Food you've sacrificed this turn" and "the total life lost by all
//! players this turn", "an opponent was dealt noncombat damage this turn" and a player's first discards of the turn.

use super::facts::quantity_texts;
use super::selectors::{Form, phrase_text};
use super::triggers::article;
use crate::refs::{Ref, Selector};
use crate::trigger::TriggerPattern;
use crate::value::{Amount, Cmp, Condition, TallyEvent, TurnTally};

/// The kind of permanent you sacrificed this turn, when the amount is that count.
fn sacrificed(amount: &Amount) -> Option<&Selector> {
    match amount {
        Amount::Tally {
            event: TallyEvent::Sacrificed,
            who: Ref::You,
            filter,
        } if !filter.is_empty() => Some(filter),
        _ => None,
    }
}

/// The wordings of the amount compared with the number, or nothing when no row has it.
pub(super) fn texts(left: &Amount, op: Cmp, number: u32) -> Vec<String> {
    let Some(filter) = sacrificed(left) else {
        return dealt_texts(left, op, number);
    };
    if op != Cmp::Ge || number == 0 {
        return Vec::new();
    }
    let kinds = if number == 1 {
        let Some(one) = phrase_text(filter, Form::Each) else {
            return Vec::new();
        };
        vec![format!("{} {one}", article(&one))]
    } else {
        let Some(many) = phrase_text(filter, Form::All) else {
            return Vec::new();
        };
        quantity_texts(number)
            .into_iter()
            .map(|n| format!("{n} or more {many}"))
            .collect()
    };
    kinds
        .iter()
        .flat_map(|kind| {
            [
                format!("you've sacrificed {kind} this turn"),
                format!("you sacrificed {kind} this turn"),
            ]
        })
        .collect()
}

/// What "for each" counts: "Food you've sacrificed this turn".
pub(super) fn each(amount: &Amount) -> Option<String> {
    let one = phrase_text(sacrificed(amount)?, Form::Count)?;
    Some(format!("{one} you've sacrificed this turn"))
}

/// The amount after "equal to": "the total life lost by all players this turn".
pub(super) fn said(amount: &Amount) -> Option<String> {
    (*amount
        == Amount::Tallied {
            who: Ref::EachPlayer,
            what: TurnTally::LifeLost,
        })
    .then(|| "the total life lost by all players this turn".to_owned())
}

/// "combat" or "noncombat" for the damage tallies of that kind.
pub(super) const fn damage_kind(what: TurnTally) -> Option<&'static str> {
    match what {
        TurnTally::CombatDamageTaken => Some("combat"),
        TurnTally::NoncombatDamageTaken => Some("noncombat"),
        _ => None,
    }
}

/// "an opponent was dealt noncombat damage this turn" (also "has been dealt"): at least one of the opponents took damage
/// of that kind this turn.
fn dealt_texts(left: &Amount, op: Cmp, number: u32) -> Vec<String> {
    let Amount::Players(Ref::PlayersWhere { who, test }) = left else {
        return Vec::new();
    };
    let Condition::Compare {
        left: Amount::Tallied {
            who: Ref::You,
            what,
        },
        op: Cmp::Ge,
        right: Amount::Fixed(1),
    } = &**test
    else {
        return Vec::new();
    };
    let (Some(kind), Ref::Opponents, Cmp::Ge, 1) = (damage_kind(*what), &**who, op, number) else {
        return Vec::new();
    };
    vec![
        format!("an opponent was dealt {kind} damage this turn"),
        format!("an opponent has been dealt {kind} damage this turn"),
    ]
}

/// "Whenever you discard one or more cards" (or "nonland cards") of the batch of a player's first discards of that kind
/// in a turn.
pub(super) fn first_discards(inner: &TriggerPattern) -> Option<String> {
    let TriggerPattern::Discarded { .. } = inner else {
        return None;
    };
    super::r7e_events::r7e_player_batch(inner)
}
