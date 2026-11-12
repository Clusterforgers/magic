//! The first discards of a kind in a turn ("whenever you discard one or more nonland cards for the first time each
//! turn"): the batch's discards of that kind by a player who discarded no other card of that kind this turn.

use mtg_cards::refs::Selector;
use mtg_cards::value::TallyEvent;
use mtg_core::ids::PlayerId;

use super::Happened;
use crate::events::ZoneCause;
use crate::resolve::Scope;
use crate::zones::Zone;

/// The discards of a batch that are their player's first discards of the kind `what` this turn: each player's discards
/// of that kind in the batch, when the turn's log holds no other discard of that kind by them.
pub(super) fn l293_first_discards_of<'a>(
    scope: &Scope<'_>,
    members: &'a [Happened],
    what: &Selector,
) -> Vec<&'a Happened> {
    let state = scope.state;
    let of_kind = |happened: &Happened| match happened {
        Happened::Moved {
            object,
            from: Zone::Hand(player),
            cause: Some(ZoneCause::Discarded),
            ..
        } => state
            .object(*object)
            .is_some_and(|card| scope.selects(what, card.reference()).unwrap_or(false))
            .then_some(*player),
        _ => None,
    };
    let mut firsts = Vec::new();
    let mut players: Vec<PlayerId> = members.iter().filter_map(of_kind).collect();
    players.sort_unstable();
    players.dedup();
    for player in players {
        let here: Vec<&Happened> = members
            .iter()
            .filter(|member| of_kind(member) == Some(player))
            .collect();
        let turn = scope
            .logged_for(TallyEvent::Discarded, &[player], what)
            .unwrap_or(0);
        if usize::try_from(turn).is_ok_and(|turn| turn == here.len()) {
            firsts.extend(here);
        }
    }
    firsts
}
