//! `Effect::PayAnyTimes`: a player may pay a cost any number of times ("you may pay {1}{G} any number of times"): they
//! choose how many, then pay the cost that many times at once; a `Reflexive` right after it reads the number.

use mtg_cards::effect::Effect;

use super::may::performed_name;
use super::pay::{Settled, can_pay, price_of, repeated, settle, unexpected};
use super::wrong_effect;
use crate::choose::{Choice, NumberOptions, Target};
use crate::decision::{Decision, DecisionKind};
use crate::price::Price;
use crate::resolve::{Cx, LET, Next, ResolveError, Run};

/// The most payments a search tries: more mana than any game has.
const MOST_TIMES: u32 = 1_000;

/// The name the number of payments of the step at `position` is kept under, which a reflexive trigger after it reads.
pub(super) fn times_name(position: &[usize]) -> String {
    let path: Vec<String> = position.iter().map(ToString::to_string).collect();
    format!("?times {}", path.join("."))
}

/// The price of the cost paid `times` times at once.
fn price(cx: &Cx<'_>, cost: &mtg_cards::cost::Cost, times: u32) -> Result<Price, ResolveError> {
    let times = usize::try_from(times).unwrap_or(usize::MAX);
    Ok(Price {
        granter: cx.scope.granter,
        ..price_of(&repeated(cost, times))?
    })
}

/// The most times the player can pay the cost: paying is monotone in the count, so a doubling and then a halving
/// search finds it.
fn most(
    cx: &Cx<'_>,
    player: mtg_core::ids::PlayerId,
    cost: &mtg_cards::cost::Cost,
) -> Result<u32, ResolveError> {
    let affords = |times: u32| -> Result<bool, ResolveError> {
        Ok(can_pay(cx, player, &price(cx, cost, times)?))
    };
    if !affords(1)? {
        return Ok(0);
    }
    let mut high = 2;
    while high < MOST_TIMES && affords(high)? {
        high = high.saturating_mul(2).min(MOST_TIMES);
    }
    let mut low = high / 2;
    if affords(high)? {
        return Ok(high);
    }
    while low + 1 < high {
        let mid = low + (high - low) / 2;
        if affords(mid)? {
            low = mid;
        } else {
            high = mid;
        }
    }
    Ok(low)
}

/// The first player `who` names who is in the game chooses how many times to pay (`ChooseNumber`, from 0 to the most they
/// can pay), then pays the cost that many times at once as a payment an effect asks for. The step binds the number for a
/// following reflexive trigger and reports whether anything was paid for "When you pay this cost one or more times".
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::PayAnyTimes { who, cost } = effect else {
        return Err(wrong_effect("PayAnyTimes"));
    };
    let state = cx.state();
    let Some(player) = cx
        .scope
        .players(who)?
        .into_iter()
        .find(|player| state.is_alive(*player))
    else {
        return Ok(done(cx, 0, Vec::new()));
    };
    let mut journal = cx
        .journal()
        .iter()
        .filter(|entry| entry.0 == player)
        .peekable();
    let times = match journal.next() {
        None => {
            return Ok(Run::Ask(Decision {
                player,
                kind: DecisionKind::ChooseNumber(NumberOptions {
                    source: cx.scope.source.id,
                    min: 0,
                    max: Some(most(cx, player, cost)?),
                    except: Vec::new(),
                    text: format!("{}: how many times?", cost_words(cx, cost)?),
                    can_cancel: false,
                    cost: None,
                    per_x: 0,
                    secret: false,
                }),
            }));
        }
        Some((_, Choice::Number(times))) => *times,
        Some((_, other)) => return Err(unexpected(other, player)),
    };
    if times == 0 {
        return Ok(done(cx, 0, Vec::new()));
    }
    let price = price(cx, cost, times)?;
    match settle(cx, player, &price, &mut journal)? {
        Settled::Ask(decision) => Ok(Run::Ask(decision)),
        Settled::Paid(events, _) => {
            if journal.next().is_some() {
                return Err(ResolveError::Invalid(
                    "an answer was given to a question that was not asked".into(),
                ));
            }
            Ok(done(cx, times, events))
        }
    }
}

/// The words of one payment: "{1}{G}", "2 life".
fn cost_words(cx: &Cx<'_>, cost: &mtg_cards::cost::Cost) -> Result<String, ResolveError> {
    Ok(super::pay::said(&price(cx, cost, 1)?).join(", "))
}

/// The finished step: the events of the payment, the number of payments kept for a reflexive trigger, and the
/// controller bound as having done it when they paid at least once.
fn done(cx: &Cx<'_>, times: u32, events: Vec<crate::events::Event>) -> Run {
    let position = cx.resolving.position();
    let who = if times > 0 {
        vec![Target::Player(cx.scope.controller)]
    } else {
        Vec::new()
    };
    Run::Done {
        events,
        bound: vec![
            (format!("{LET}{times}:{}", times_name(position)), Vec::new()),
            (performed_name(position), who),
        ],
        snapshots: Vec::new(),
        next: Next::Sibling,
    }
}
