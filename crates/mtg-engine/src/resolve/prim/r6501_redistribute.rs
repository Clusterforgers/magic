//! `Effect::R6501RedistributeLife`: "Redistribute any number of players' life totals" (Reverse the Sands).
//!
//! Each player gets one of the life totals the players have as the effect begins, each total given once (the rulings:
//! a total is never split, and the controller chooses who gets which as the spell resolves). The controller assigns
//! them in turn order from themselves, one `ChooseOption` among the totals still unassigned for each player while two
//! or more different ones are left; a player left out of the redistribution is one given their own total. Each player
//! then gains or loses the difference as life (CR 119.5); one whose life total can't change, or can't gain life when
//! it would go up, keeps theirs, and the others still change.

use mtg_cards::effect::Effect;
use mtg_core::ids::PlayerId;

use super::fe1n_life::move_to;
use super::wrong_effect;
use crate::choose::{Choice, ChoiceOption, OptionOptions};
use crate::decision::{Decision, DecisionKind};
use crate::resolve::{Cx, ResolveError, Run};

/// Asks for each player's new life total in turn, then changes them all at once.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    if !matches!(effect, Effect::R6501RedistributeLife) {
        return Err(wrong_effect("R6501RedistributeLife"));
    }
    let state = cx.state();
    let players: Vec<(PlayerId, i64)> = state
        .turn_order_from(cx.controller())
        .into_iter()
        .filter_map(|player| state.player(player).map(|p| (player, i64::from(p.life()))))
        .collect();
    let mut left: Vec<i64> = players.iter().map(|(_, life)| *life).collect();
    let mut answers = cx.journal().iter();
    let mut given = Vec::with_capacity(players.len());
    for (player, _) in &players {
        let mut offered = left.clone();
        offered.sort_unstable_by(|a, b| b.cmp(a));
        offered.dedup();
        let total = match offered.as_slice() {
            [] => break,
            [only] => *only,
            _ => match answers.next() {
                None => return Ok(Run::Ask(question(cx, *player, &offered))),
                Some((who, Choice::Option(index)))
                    if *who == cx.controller() && *index < offered.len() =>
                {
                    offered[*index]
                }
                Some(other) => {
                    return Err(ResolveError::Invalid(format!(
                        "{other:?} is no answer to the choice of a life total"
                    )));
                }
            },
        };
        if let Some(at) = left.iter().position(|life| *life == total) {
            left.remove(at);
        }
        given.push((*player, total));
    }
    if answers.next().is_some() {
        return Err(ResolveError::Invalid(
            "an answer was given to a choice of a life total that was not asked".into(),
        ));
    }
    let events = players
        .iter()
        .zip(&given)
        .filter_map(|((player, from), (_, to))| move_to(cx, *player, *from, *to))
        .collect();
    Ok(Run::events(events))
}

/// "Which life total does this player get?": the totals still unassigned, highest first.
fn question(cx: &Cx<'_>, player: PlayerId, offered: &[i64]) -> Decision {
    Decision {
        player: cx.controller(),
        kind: DecisionKind::ChooseOption(OptionOptions {
            can_cancel: false,
            source: cx.scope.source.id,
            options: offered
                .iter()
                .map(|life| ChoiceOption::Label(format!("{life} life")))
                .collect(),
            text: format!(
                "Choose the life total player {} gets.",
                player.index() + 1
            ),
        }),
    }
}
