//! `Effect::SetLife` and `Effect::ExchangeLife`: a life total becomes a number (CR 119.5), or two totals are exchanged
//! (CR 119.7). Both go through `LifeGained` and `LifeLost`, so triggers and replacements see the change.

use mtg_cards::effect::Effect;
use mtg_core::ids::PlayerId;

use super::wrong_effect;
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::rules::{cant_gain_life, life_total_cant_change};

/// Whether a change from `from` to `to` is barred: the total can't change, or the change is a gain that can't happen.
fn blocked(cx: &Cx<'_>, player: PlayerId, from: i64, to: i64) -> bool {
    let state = cx.state();
    life_total_cant_change(state, player) || (to > from && cant_gain_life(state, player))
}

/// The life event that takes `player` from `from` to `to`, none when they are equal or the change is barred.
fn move_to(cx: &Cx<'_>, player: PlayerId, from: i64, to: i64) -> Option<Event> {
    let amount = u32::try_from(from.abs_diff(to)).unwrap_or(u32::MAX);
    if amount == 0 || blocked(cx, player, from, to) {
        return None;
    }
    Some(if to > from {
        Event::LifeGained { player, amount }
    } else {
        Event::LifeLost { player, amount }
    })
}

/// The life total of a player in the game.
fn life_of(cx: &Cx<'_>, player: PlayerId) -> Option<i64> {
    cx.state()
        .player(player)
        .filter(|_| cx.state().is_alive(player))
        .map(|p| i64::from(p.life()))
}

/// Each player the effect names who is in the game ends on the new life total: the difference is gained or lost.
pub(super) fn set(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::SetLife { who, amount } = effect else {
        return Err(wrong_effect("SetLife"));
    };
    let target = cx.scope.amount(amount)?;
    let mut events = Vec::new();
    for player in cx.scope.players(who)? {
        if let Some(life) = life_of(cx, player) {
            events.extend(move_to(cx, player, life, target));
        }
    }
    Ok(Run::events(events))
}

/// The first player each reference names who is in the game swap totals; if either swap is barred nothing is exchanged
/// (CR 119.7). The same player on both sides exchanges nothing.
pub(super) fn exchange(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::ExchangeLife { who, with } = effect else {
        return Err(wrong_effect("ExchangeLife"));
    };
    let pick = |reference| -> Result<Option<(PlayerId, i64)>, ResolveError> {
        Ok(cx
            .scope
            .players(reference)?
            .into_iter()
            .find_map(|p| life_of(cx, p).map(|life| (p, life))))
    };
    let (Some((a, life_a)), Some((b, life_b))) = (pick(who)?, pick(with)?) else {
        return Ok(Run::events(Vec::new()));
    };
    if a == b || blocked(cx, a, life_a, life_b) || blocked(cx, b, life_b, life_a) {
        return Ok(Run::events(Vec::new()));
    }
    let events = [
        move_to(cx, a, life_a, life_b),
        move_to(cx, b, life_b, life_a),
    ]
    .into_iter()
    .flatten()
    .collect();
    Ok(Run::events(events))
}
