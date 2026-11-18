//! The coin flips of the Delves modifier rules No Rest (`GameRule::FlipOrExiled`), Stone Sleep
//! (`GameRule::FlipOrStayTapped`) and No Mercy (`GameRule::FlipOrNoLifeGain`). Nobody calls the coin: heads wins. The
//! side comes from the game's random stream and the flip is logged as `CoinFlipped`, whose fold moves the stream on,
//! so a game replays from its seed.

use std::collections::BTreeSet;

use mtg_core::ids::{ObjectId, PlayerId};

use crate::choose::CoinSide;
use crate::events::Event;
use crate::rng::GameRng;
use crate::state::GameState;

/// The flip of `player` drawn `offset` substreams after the game's current one (the batch's earlier flips come first).
pub(crate) fn flip(state: &GameState, player: PlayerId, offset: u64) -> Event {
    let index = state.rng_index().saturating_add(offset);
    let side = if GameRng::substream(state.seed(), index).below(2) == 0 {
        CoinSide::Heads
    } else {
        CoinSide::Tails
    };
    Event::CoinFlipped {
        player,
        won: side == CoinSide::Heads,
        call: CoinSide::Heads,
        side,
        coins: Vec::new(),
    }
}

/// Whether a replacement that is no ability may apply in this game: a reusable Satchel card, or a rule of No Rest or
/// No Mercy in force.
pub(crate) fn r_moves_possible(state: &GameState) -> bool {
    state.satchels.any_reusable()
        || crate::rules::flips_or_exiled_any(state)
        || crate::rules::flips_or_no_life_gain_any(state)
}

/// Whether the flip event was lost.
pub(crate) fn lost(event: &Event) -> bool {
    matches!(event, Event::CoinFlipped { won: false, .. })
}

/// The flips of the untap step of `actives` under Stone Sleep, one for each active player with the rule, and the
/// permanents a lost flip holds down: those of the rule its player controls.
pub(crate) fn stone_sleep(
    state: &GameState,
    actives: &[PlayerId],
) -> (Vec<Event>, BTreeSet<ObjectId>) {
    let mut flips = Vec::new();
    let mut held = BTreeSet::new();
    for player in actives {
        let Some(asleep) = crate::rules::flip_stay_tapped(state, *player) else {
            continue;
        };
        let event = flip(
            state,
            *player,
            u64::try_from(flips.len()).unwrap_or(u64::MAX),
        );
        if lost(&event) {
            held.extend(
                asleep.iter().copied().filter(|id| {
                    crate::characteristics::controller_of(state, *id) == Some(*player)
                }),
            );
        }
        flips.push(event);
    }
    (flips, held)
}
